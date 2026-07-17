//! `zerawler` — CLI for the sidecar RAW engine.
//!
//! Usage:
//!   zerawler <input.raw> [--algo <name>] [--native] [-o <out.png|out.tif>]
//!   zerawler --list          list algorithms + carrying backend
//!   zerawler --engines       show detected worker binaries

use std::path::PathBuf;
use std::process::ExitCode;

use zerawler::{Algorithm, Engine, Mode};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let engine = Engine::detect();

    if args.iter().any(|a| a == "--engines") {
        println!(
            "rawtherapee-cli: {}",
            engine
                .rt_cli
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "NOT FOUND".into())
        );
        println!(
            "dcraw_emu:       {}",
            engine
                .dcraw_emu
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "NOT FOUND".into())
        );
        return ExitCode::SUCCESS;
    }
    if args.iter().any(|a| a == "--list") {
        println!("algorithms (name — backend):");
        for &a in Algorithm::all() {
            println!("  {:<6} — {}", a.name(), a.backend().name());
        }
        return ExitCode::SUCCESS;
    }
    if args.is_empty() || args.iter().any(|a| a == "-h" || a == "--help") {
        eprintln!(
            "zerawler — sidecar RAW engine\n\
             \n\
             USAGE:\n  \
               zerawler <input.raw> [--algo <name>] [--native] [-o <file>]\n  \
               zerawler --list | --engines\n\
             \n\
             OPTIONS:\n  \
               -a, --algo <name>  demosaic algorithm (default: dht; see --list)\n  \
                   --native       linear camera-native output contract\n  \
               -o, --out <file>   output image (.png or .tif; default <input>_<algo>.png)\n"
        );
        return if args.is_empty() { ExitCode::FAILURE } else { ExitCode::SUCCESS };
    }

    let mut input: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut algo = Algorithm::Dht;
    let mut mode = Mode::Preview;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--algo" | "-a" => match it.next().and_then(|s| Algorithm::from_name(s)) {
                Some(x) => algo = x,
                None => {
                    eprintln!("error: unknown --algo (try --list)");
                    return ExitCode::FAILURE;
                }
            },
            "--out" | "-o" => out = it.next().map(PathBuf::from),
            "--native" => mode = Mode::Native,
            other if !other.starts_with('-') => {
                if input.is_some() {
                    eprintln!("error: multiple input files given ({other:?}); one at a time");
                    return ExitCode::FAILURE;
                }
                input = Some(PathBuf::from(other));
            }
            other => {
                eprintln!("error: unknown argument {other:?}");
                return ExitCode::FAILURE;
            }
        }
    }
    let Some(input) = input else {
        eprintln!("error: no input file");
        return ExitCode::FAILURE;
    };

    match engine.decode(&input, algo, mode) {
        Ok(d) => {
            println!(
                "{} via {} — {}x{} in {:.2?}\n  colour state: {}",
                d.algorithm.name(),
                d.backend.name(),
                d.image.width,
                d.image.height,
                d.elapsed,
                d.space
            );
            let out = out.unwrap_or_else(|| {
                input.with_file_name(format!(
                    "{}_{}_z.png",
                    input.file_stem().and_then(|s| s.to_str()).unwrap_or("out"),
                    algo.name()
                ))
            });
            if let Err(e) = save(&d.image, &out, mode) {
                eprintln!("error saving: {e}");
                return ExitCode::FAILURE;
            }
            println!("wrote {}", out.display());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Save output. Preview data is already display-encoded → straight to 8-bit.
/// Native data is linear → apply sRGB OETF for .png so it is viewable (still
/// unity-WB camera colour; expect a green cast), or keep 16-bit linear .tif.
fn save(img: &zerawler::RgbImage, path: &PathBuf, mode: Mode) -> Result<(), String> {
    let is_tif = path
        .extension()
        .map(|e| e.eq_ignore_ascii_case("tif") || e.eq_ignore_ascii_case("tiff"))
        .unwrap_or(false);
    if is_tif {
        let mut buf = image::ImageBuffer::<image::Rgb<u16>, Vec<u16>>::new(
            img.width as u32,
            img.height as u32,
        );
        for (i, px) in img.data.iter().enumerate() {
            let (x, y) = ((i % img.width) as u32, (i / img.width) as u32);
            buf.put_pixel(
                x,
                y,
                image::Rgb([to_u16(px[0]), to_u16(px[1]), to_u16(px[2])]),
            );
        }
        buf.save(path).map_err(|e| e.to_string())
    } else {
        let mut buf = image::RgbImage::new(img.width as u32, img.height as u32);
        for (i, px) in img.data.iter().enumerate() {
            let (x, y) = ((i % img.width) as u32, (i / img.width) as u32);
            let enc = |v: f32| -> u8 {
                let v = match mode {
                    Mode::Preview => v, // already display-encoded by the worker
                    Mode::Native => srgb_encode(v),
                };
                (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
            };
            buf.put_pixel(x, y, image::Rgb([enc(px[0]), enc(px[1]), enc(px[2])]));
        }
        buf.save(path).map_err(|e| e.to_string())
    }
}

fn to_u16(v: f32) -> u16 {
    (v.clamp(0.0, 1.0) * 65535.0 + 0.5) as u16
}

fn srgb_encode(c: f32) -> f32 {
    if c <= 0.003_130_8 {
        12.92 * c
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}
