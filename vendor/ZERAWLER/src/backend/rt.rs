//! RawTherapee backend via `rawtherapee-cli` (GPL-3.0).
//!
//! Carries **RCD, LMMSE, AMaZE** — the reference implementations, maintained
//! upstream. Invoked at arm's length as a separate process; a generated .pp3
//! processing profile selects the demosaic method and neutralizes everything
//! else.
//!
//! Invocation shape:
//!   rawtherapee-cli -o <out.tif> -p <profile.pp3> -t -b16 -Y -c <input>
//!
//! Modes:
//! * Preview — camera WB, RT default input→working→sRGB colour management.
//! * Native  — **linear Rec.2020, camera WB** (validated contract): RT is
//!   asked for camera-profile → Rec2020 working → RTv4_Rec2020 output, and
//!   the sRGB TRC that RT bakes into that output (even for float) is decoded
//!   here before returning. Highlights clamp at 1.0 (RT limitation). True
//!   camera-native is not reachable via supported pp3 keys — differential
//!   tests show a camera→working matrix persists regardless.

use crate::{tiff, Algorithm, DecodeOpts, Error, Mode, RgbImage};
use std::io::Write;
use std::path::Path;
use std::process::Command;

fn method(algo: Algorithm) -> &'static str {
    match algo {
        Algorithm::Rcd => "rcd",
        Algorithm::Lmmse => "lmmse",
        Algorithm::Amaze => "amaze",
        _ => unreachable!("{algo:?} is not a RawTherapee algorithm"),
    }
}

fn pp3(algo: Algorithm, mode: Mode) -> String {
    let mut p = String::from("[Version]\nAppVersion=5.12\nVersion=350\n\n");
    p += &format!("[RAW Bayer]\nMethod={}\n\n", method(algo));
    match mode {
        Mode::Preview => {
            p += "[White Balance]\nEnabled=true\nSetting=Camera\n";
        }
        Mode::Native => {
            // Contract: **linear Rec.2020, camera WB, RT colour management.**
            //
            // True camera-native (unity WB, no matrix) is NOT reachable via
            // supported pp3 keys — validated 2026-07-05: RT keeps a
            // camera→working matrix in its raw path even with
            // InputProfile=(none) + no-ICM output (differential tests show
            // both knobs apply, yet a ~47%-off-diagonal matrix remains vs
            // ground truth). So instead of an ill-defined "almost native"
            // state, ask RT for a fully colour-managed, exactly-known one:
            // camera WB + camera input profile + Rec2020 working + Rec2020
            // output. Consumers get linear Rec.2020 directly (TRC handling
            // per the validator's findings).
            p += "[White Balance]\nEnabled=true\nSetting=Camera\n\n";
            p += "[Color Management]\nInputProfile=(camera)\nWorkingProfile=Rec2020\nOutputProfile=RTv4_Rec2020\n\n";
            // Neutralize incidental processing so output is demosaic + colour
            // only (defaults are mostly off, but be explicit).
            p += "[PostDemosaicSharpening]\nEnabled=false\n\n";
            p += "[Sharpening]\nEnabled=false\n\n";
            p += "[Exposure]\nAuto=false\n\n";
            p += "[RAW]\nCA=false\nHotPixelFilter=false\nDeadPixelFilter=false\n";
        }
    }
    p
}

pub fn decode(
    bin: &Path,
    raw: &Path,
    algo: Algorithm,
    mode: Mode,
    opts: DecodeOpts,
) -> Result<(RgbImage, &'static str), Error> {
    let dir = tempfile::tempdir().map_err(|e| Error::Exec(e.to_string()))?;
    let prof = dir.path().join("zerawler.pp3");
    let mut f = std::fs::File::create(&prof).map_err(|e| Error::Exec(e.to_string()))?;
    f.write_all(pp3(algo, mode).as_bytes())
        .map_err(|e| Error::Exec(e.to_string()))?;
    drop(f);
    let out_tif = dir.path().join("out.tif");

    // Native = 32-bit float TIFF (linear, no TRC ambiguity, no quantization);
    // preview = 16-bit display-encoded.
    let depth = match mode {
        Mode::Native => "-b32",
        Mode::Preview => "-b16",
    };
    let mut cmd = Command::new(bin);
    cmd.arg("-o")
        .arg(&out_tif)
        .arg("-p")
        .arg(&prof)
        .args(["-t", depth, "-Y", "-q", "-c"])
        .arg(raw);
    let timeout = opts.timeout.unwrap_or(crate::process::DEFAULT_TIMEOUT);
    let out = crate::process::run_with_timeout(&mut cmd, timeout)?;
    if !out.status.success() {
        return Err(Error::Exec(format!(
            "rawtherapee-cli exited {}: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        )));
    }
    if !out_tif.is_file() {
        return Err(Error::Output(format!(
            "rawtherapee-cli produced no output; stdout: {}",
            String::from_utf8_lossy(&out.stdout)
        )));
    }

    let mut image = tiff::read_rgb(&out_tif)?;
    let space = match mode {
        Mode::Native => {
            // RTv4_Rec2020 output carries the sRGB transfer curve even at
            // 32-bit float (validated). Decode it HERE so the Native contract
            // is genuinely linear and no consumer needs to know RT's quirk.
            for px in &mut image.data {
                for c in px.iter_mut() {
                    *c = srgb_decode(*c);
                }
            }
            "linear Rec.2020, camera WB (RT colour management; highlights clamp at 1.0)"
        }
        Mode::Preview => "camera WB, RT sRGB output (display-referred)",
    };
    Ok((image, space))
}

/// Inverse sRGB OETF.
#[inline]
fn srgb_decode(v: f32) -> f32 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}
