//! `merawler` — CLI to decode a RAW file, demosaic it with a chosen algorithm,
//! and write a viewable PNG. This is a demosaic *preview* tool: it applies
//! as-shot white balance and an sRGB transfer curve so you can eyeball
//! interpolation quality (zippering, false color, mazing). It is not a full
//! color-managed pipeline — that lives in the MeraRAW editor.
//!
//! Usage:
//!   merawler <input.raw> [--algo <name>] [--out <file.png>] [--no-wb] [--list]

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use merawler::rawler_adapter::cfa_from_path;
use merawler::Algorithm;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.iter().any(|a| a == "--list") {
        print_algorithms();
        return ExitCode::SUCCESS;
    }
    if args.is_empty() || args.iter().any(|a| a == "-h" || a == "--help") {
        print_usage();
        return if args.is_empty() {
            ExitCode::FAILURE
        } else {
            ExitCode::SUCCESS
        };
    }

    let mut input: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut algo = Algorithm::Malvar;
    let mut apply_wb = true;

    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--algo" | "-a" => match it.next().and_then(|s| Algorithm::from_name(s)) {
                Some(a) => algo = a,
                None => {
                    eprintln!("error: unknown --algo (try --list)");
                    return ExitCode::FAILURE;
                }
            },
            "--out" | "-o" => out = it.next().map(PathBuf::from),
            "--no-wb" => apply_wb = false,
            other if !other.starts_with('-') => input = Some(PathBuf::from(other)),
            other => {
                eprintln!("error: unknown argument {other:?}");
                return ExitCode::FAILURE;
            }
        }
    }

    let Some(input) = input else {
        eprintln!("error: no input file given");
        print_usage();
        return ExitCode::FAILURE;
    };

    if !algo.is_implemented() {
        eprintln!(
            "error: '{}' is not implemented yet (planned). Implemented: bilinear, malvar.",
            algo.name()
        );
        return ExitCode::FAILURE;
    }

    match run(&input, out.as_deref(), algo, apply_wb) {
        Ok(path) => {
            println!("wrote {}", path.display());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(
    input: &Path,
    out: Option<&Path>,
    algo: Algorithm,
    apply_wb: bool,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let cfa = cfa_from_path(input)?;
    println!(
        "decoded {}x{} {} mosaic (wb r={:.3} b={:.3})",
        cfa.width,
        cfa.height,
        cfa.pattern.name(),
        cfa.wb[0],
        cfa.wb[2]
    );

    let t = std::time::Instant::now();
    let rgb = merawler::demosaic(&cfa, algo).ok_or("algorithm not implemented")?;
    println!("demosaic ({}) took {:?}", algo.name(), t.elapsed());

    let wb = if apply_wb { cfa.wb } else { [1.0, 1.0, 1.0] };
    let mut buf = ::image::RgbImage::new(rgb.width as u32, rgb.height as u32);
    for (i, px) in rgb.data.iter().enumerate() {
        let r = srgb_encode((px[0] * wb[0]).clamp(0.0, 1.0));
        let g = srgb_encode((px[1] * wb[1]).clamp(0.0, 1.0));
        let b = srgb_encode((px[2] * wb[2]).clamp(0.0, 1.0));
        let (x, y) = ((i % rgb.width) as u32, (i / rgb.width) as u32);
        buf.put_pixel(x, y, ::image::Rgb([to_u8(r), to_u8(g), to_u8(b)]));
    }

    let out = out.map(PathBuf::from).unwrap_or_else(|| {
        input.with_file_name(format!(
            "{}_{}.png",
            input.file_stem().and_then(|s| s.to_str()).unwrap_or("out"),
            algo.name()
        ))
    });
    buf.save(&out)?;
    Ok(out)
}

/// Linear -> sRGB gamma encode.
#[inline]
fn srgb_encode(c: f32) -> f32 {
    if c <= 0.003_130_8 {
        12.92 * c
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

#[inline]
fn to_u8(c: f32) -> u8 {
    (c.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

fn print_usage() {
    eprintln!(
        "merawler — RAW demosaic preview\n\
         \n\
         USAGE:\n  \
           merawler <input.raw> [--algo <name>] [--out <file.png>] [--no-wb]\n\
         \n\
         OPTIONS:\n  \
           -a, --algo <name>   demosaic algorithm (default: malvar; see --list)\n  \
           -o, --out <file>    output PNG (default: <input>_<algo>.png)\n  \
               --no-wb         skip as-shot white balance\n  \
               --list          list algorithms and their status\n"
    );
}

fn print_algorithms() {
    println!("algorithms:");
    for &a in Algorithm::all() {
        let status = if a.is_implemented() {
            "ready"
        } else {
            "planned"
        };
        println!("  {:<9} [{}]", a.name(), status);
    }
}
