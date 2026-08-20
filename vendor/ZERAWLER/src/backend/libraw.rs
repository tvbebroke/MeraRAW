//! LibRaw backend via the `dcraw_emu` sample binary (LGPL-2.1 | CDDL-1.0).
//!
//! Carries **DHT** (Anton Petrusevich's demosaicer, `-q 11`) plus AHD (3),
//! DCB (4) and AAHD (12).
//!
//! `dcraw_emu` writes its output next to the *input path*, so the raw file is
//! symlinked into a scratch dir first and the produced TIFF collected there.
//!
//! Flag contract:
//! * Native  — `-q N -o 0 -r 1 1 1 1 -4 -T`
//!   raw (camera) colour space, unity WB, linear 16-bit TIFF, no auto-bright.
//! * Preview — `-q N -w -6 -T`
//!   camera WB, default gamma/brightening, 16-bit TIFF (display-referred).

use crate::{tiff, Algorithm, DecodeOpts, Error, Mode, RgbImage};
use std::path::Path;
use std::process::Command;

fn quality(algo: Algorithm) -> &'static str {
    match algo {
        Algorithm::Ahd => "3",
        Algorithm::Dcb => "4",
        Algorithm::Dht => "11",
        Algorithm::Aahd => "12",
        // RT-carried algorithms never route here
        _ => unreachable!("{algo:?} is not a LibRaw algorithm"),
    }
}

pub fn decode(
    bin: &Path,
    raw: &Path,
    algo: Algorithm,
    mode: Mode,
    opts: DecodeOpts,
) -> Result<(RgbImage, &'static str), Error> {
    let dir = tempfile::tempdir().map_err(|e| Error::Exec(e.to_string()))?;
    let link = dir.path().join(
        raw.file_name()
            .ok_or_else(|| Error::Exec("input has no file name".into()))?,
    );
    #[cfg(unix)]
    std::os::unix::fs::symlink(raw, &link).map_err(|e| Error::Exec(e.to_string()))?;
    #[cfg(windows)]
    std::fs::copy(raw, &link).map_err(|e| Error::Exec(e.to_string()))?;

    let mut cmd = Command::new(bin);
    match mode {
        Mode::Native => {
            cmd.args([
                "-q",
                quality(algo),
                "-o",
                "0",
                "-r",
                "1",
                "1",
                "1",
                "1",
                "-c",
                "0",
                "-4",
                "-T",
            ]);
            // Forcing the host pipeline's own levels makes dcraw's
            // normalization match it exactly (kills the residual uniform
            // scale that comes from differing white-level tables).
            if let Some(k) = opts.black {
                cmd.args(["-k", &k.to_string()]);
            }
            if let Some(s) = opts.white {
                cmd.args(["-S", &s.to_string()]);
            }
        }
        Mode::Preview => {
            cmd.args(["-q", quality(algo), "-w", "-6", "-T"]);
        }
    };
    cmd.arg(&link);

    let timeout = opts.timeout.unwrap_or(crate::process::DEFAULT_TIMEOUT);
    let out = crate::process::run_with_timeout(&mut cmd, timeout)?;
    if !out.status.success() {
        return Err(Error::Exec(format!(
            "dcraw_emu exited {}: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        )));
    }

    // collect the produced tiff (dcraw_emu appends .tiff to the input name)
    let produced = std::fs::read_dir(dir.path())
        .map_err(|e| Error::Output(e.to_string()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .find(|p| {
            p.extension()
                .map(|e| e.eq_ignore_ascii_case("tiff") || e.eq_ignore_ascii_case("tif"))
                .unwrap_or(false)
        })
        .ok_or_else(|| Error::Output("dcraw_emu produced no TIFF".into()))?;

    let image = tiff::read_rgb(&produced)?;
    let space = match mode {
        Mode::Native => "linear camera-native RGB, unity WB (16-bit)",
        Mode::Preview => "camera WB, dcraw default curve (display-referred)",
    };
    Ok((image, space))
}
