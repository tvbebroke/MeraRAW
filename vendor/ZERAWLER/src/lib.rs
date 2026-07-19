//! ZERAWLER — a sidecar RAW engine.
//!
//! Philosophy (opposite of merawler): don't reimplement demosaic algorithms —
//! **call the canonical implementations** as external executables across a
//! process boundary, the way a Tauri sidecar does. Bugs, optimizations and
//! camera quirks stay upstream; the process boundary keeps GPL code out of
//! the host application.
//!
//! Backends:
//! * [`backend::rt`] — RawTherapee CLI (GPL-3.0): **RCD, LMMSE, AMaZE**
//! * [`backend::libraw`] — LibRaw `dcraw_emu` (LGPL-2.1/CDDL-1.0): **DHT**,
//!   AHD, DCB, AAHD
//!
//! Modes:
//! * [`Mode::Native`] — linear, camera-native colour, unity white balance:
//!   the contract a colour pipeline wants (WB + camera matrix applied later).
//! * [`Mode::Preview`] — the engine's own camera WB + display encoding:
//!   what a human wants to eyeball demosaic quality with.

pub(crate) mod backend;
pub(crate) mod process;
pub mod tiff;

use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Interleaved linear-ish RGB float image (one `[r,g,b]` per pixel, row-major).
#[derive(Clone)]
pub struct RgbImage {
    pub width: usize,
    pub height: usize,
    pub data: Vec<[f32; 3]>,
}

/// Which external binary carries an algorithm.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backend {
    RawTherapee,
    LibRaw,
}

impl Backend {
    pub fn name(self) -> &'static str {
        match self {
            Backend::RawTherapee => "rawtherapee-cli (GPL-3.0)",
            Backend::LibRaw => "dcraw_emu / LibRaw (LGPL-2.1 | CDDL-1.0)",
        }
    }
}

/// Demosaic algorithms zerawler can delegate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Algorithm {
    /// Ratio Corrected Demosaicing (L. Sanz Rodríguez) — RawTherapee.
    Rcd,
    /// Zhang–Wu directional LMMSE — RawTherapee.
    Lmmse,
    /// AMaZE (E. Martinec) — RawTherapee.
    Amaze,
    /// DHT (A. Petrusevich) — LibRaw.
    Dht,
    /// AHD (Hirakawa–Parks) — LibRaw.
    Ahd,
    /// DCB — LibRaw.
    Dcb,
    /// AAHD (modified AHD) — LibRaw.
    Aahd,
}

impl Algorithm {
    pub fn name(self) -> &'static str {
        match self {
            Algorithm::Rcd => "rcd",
            Algorithm::Lmmse => "lmmse",
            Algorithm::Amaze => "amaze",
            Algorithm::Dht => "dht",
            Algorithm::Ahd => "ahd",
            Algorithm::Dcb => "dcb",
            Algorithm::Aahd => "aahd",
        }
    }

    pub fn from_name(s: &str) -> Option<Self> {
        Some(match s.to_ascii_lowercase().as_str() {
            "rcd" => Algorithm::Rcd,
            "lmmse" => Algorithm::Lmmse,
            "amaze" => Algorithm::Amaze,
            "dht" => Algorithm::Dht,
            "ahd" => Algorithm::Ahd,
            "dcb" => Algorithm::Dcb,
            "aahd" => Algorithm::Aahd,
            _ => return None,
        })
    }

    pub fn backend(self) -> Backend {
        match self {
            Algorithm::Rcd | Algorithm::Lmmse | Algorithm::Amaze => Backend::RawTherapee,
            _ => Backend::LibRaw,
        }
    }

    pub fn all() -> &'static [Algorithm] {
        &[
            Algorithm::Rcd,
            Algorithm::Lmmse,
            Algorithm::Amaze,
            Algorithm::Dht,
            Algorithm::Ahd,
            Algorithm::Dcb,
            Algorithm::Aahd,
        ]
    }
}

/// Output contract for a decode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Linear, camera-native colour, unity WB — for a colour pipeline.
    Native,
    /// Engine's own camera WB + display encoding — for human eyeballs.
    Preview,
}

/// Optional per-decode controls.
#[derive(Default, Clone, Copy, Debug)]
pub struct DecodeOpts {
    /// Force the black level (LibRaw `-k`). Native mode only.
    pub black: Option<u32>,
    /// Force the saturation/white level (LibRaw `-S`). Native mode only.
    /// Passing the host pipeline's own levels makes dcraw's normalization
    /// match it exactly (validated: removes the residual uniform scale).
    pub white: Option<u32>,
    /// Kill the worker if it exceeds this (default 120 s). Workers can hang
    /// pre-`main` (observed: rawtherapee-cli's App Sandbox initializer).
    pub timeout: Option<std::time::Duration>,
}

impl DecodeOpts {
    /// Level-forcing opts from per-CFA-position black levels + a white level.
    ///
    /// dcraw takes a single darkness value, so the mean black is used —
    /// exact for uniform-black sensors (the common case), a small documented
    /// approximation for per-channel-black cameras.
    pub fn from_levels(blacks: &[f32], white: Option<u32>) -> Self {
        let black = if blacks.is_empty() {
            None
        } else {
            Some((blacks.iter().sum::<f32>() / blacks.len() as f32).round() as u32)
        };
        DecodeOpts {
            black,
            white,
            ..Default::default()
        }
    }
}

/// A completed decode plus provenance.
pub struct Decoded {
    pub image: RgbImage,
    pub backend: Backend,
    pub algorithm: Algorithm,
    pub mode: Mode,
    pub elapsed: Duration,
    /// Human-readable note about the colour state of `image`.
    pub space: &'static str,
}

#[derive(Debug)]
pub enum Error {
    /// The backend binary for this algorithm was not found.
    BackendMissing(Backend),
    /// Spawning or running the external binary failed.
    Exec(String),
    /// The binary ran but produced no/unreadable output.
    Output(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::BackendMissing(b) => write!(f, "backend not found: {}", b.name()),
            Error::Exec(e) => write!(f, "external process failed: {e}"),
            Error::Output(e) => write!(f, "could not read output: {e}"),
        }
    }
}

impl std::error::Error for Error {}

/// Locations of the external worker binaries.
///
/// Resolution order per backend: explicit env override → well-known install
/// locations → `$PATH`. In a Tauri app you would instead resolve the bundled
/// sidecar path and construct this directly.
pub struct Engine {
    pub rt_cli: Option<PathBuf>,
    pub dcraw_emu: Option<PathBuf>,
}

impl Engine {
    /// Resolution order per worker: per-binary env override
    /// (`ZERAWLER_RT_CLI` / `ZERAWLER_DCRAW_EMU`) → `ZERAWLER_WORKERS_DIR` →
    /// known install locations → `$PATH`.
    ///
    /// A prepared workers dir is preferred over the RawTherapee bundle
    /// because the bundled binary carries the App Sandbox entitlement, which
    /// deadlocks in libsecinit when exec'd headlessly; a copy re-signed
    /// without it (see README) is exactly what a Tauri sidecar would bundle.
    /// Embedding hosts should skip detection and fill the fields directly
    /// with their resolved sidecar paths.
    pub fn detect() -> Self {
        let mut workers_dirs: Vec<PathBuf> = Vec::new();
        if let Ok(d) = std::env::var("ZERAWLER_WORKERS_DIR") {
            workers_dirs.push(PathBuf::from(d));
        }
        // Dev checkouts (macOS / Unix home layouts).
        for home_key in ["HOME", "USERPROFILE"] {
            if let Ok(h) = std::env::var(home_key) {
                workers_dirs.push(PathBuf::from(&h).join("Desktop/ZERAWLER/workers"));
                workers_dirs.push(PathBuf::from(&h).join("ZERAWLER/workers"));
            }
        }

        let mut rt_known: Vec<PathBuf> = Vec::new();
        let mut dc_known: Vec<PathBuf> = Vec::new();
        for d in &workers_dirs {
            rt_known.push(d.join(exe_name("rawtherapee-cli")));
            dc_known.push(d.join(exe_name("dcraw_emu")));
        }
        // Bundled next to the host app (Tauri sidecar / release layout).
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                for sub in ["", "workers", "bin", "resources", "resources/workers"] {
                    let base = if sub.is_empty() {
                        dir.to_path_buf()
                    } else {
                        dir.join(sub)
                    };
                    rt_known.push(base.join(exe_name("rawtherapee-cli")));
                    dc_known.push(base.join(exe_name("dcraw_emu")));
                }
            }
        }
        rt_known.push(PathBuf::from(
            "/Applications/RawTherapee.app/Contents/MacOS/rawtherapee-cli",
        ));
        dc_known.push(PathBuf::from("/opt/homebrew/bin/dcraw_emu"));
        dc_known.push(PathBuf::from("/usr/local/bin/dcraw_emu"));
        dc_known.push(PathBuf::from("/usr/bin/dcraw_emu"));
        #[cfg(windows)]
        {
            rt_known.push(PathBuf::from(r"C:\Program Files\RawTherapee\rawtherapee-cli.exe"));
            rt_known.push(PathBuf::from(
                r"C:\Program Files\RawTherapee\rawtherapee-cli",
            ));
            rt_known.push(PathBuf::from(
                r"C:\Program Files (x86)\RawTherapee\rawtherapee-cli.exe",
            ));
            if let Ok(local) = std::env::var("LOCALAPPDATA") {
                rt_known.push(
                    PathBuf::from(local)
                        .join("Programs")
                        .join("RawTherapee")
                        .join("rawtherapee-cli.exe"),
                );
            }
        }
        #[cfg(target_os = "linux")]
        {
            rt_known.push(PathBuf::from("/usr/bin/rawtherapee-cli"));
            rt_known.push(PathBuf::from("/usr/local/bin/rawtherapee-cli"));
        }

        Engine {
            rt_cli: find_bin("ZERAWLER_RT_CLI", &rt_known, "rawtherapee-cli"),
            dcraw_emu: find_bin("ZERAWLER_DCRAW_EMU", &dc_known, "dcraw_emu"),
        }
    }

    /// Decode `raw` with `algo`, delegating to the algorithm's backend.
    pub fn decode(&self, raw: &Path, algo: Algorithm, mode: Mode) -> Result<Decoded, Error> {
        self.decode_opts(raw, algo, mode, DecodeOpts::default())
    }

    /// Decode with per-decode controls (forced levels, etc.).
    pub fn decode_opts(
        &self,
        raw: &Path,
        algo: Algorithm,
        mode: Mode,
        opts: DecodeOpts,
    ) -> Result<Decoded, Error> {
        let started = std::time::Instant::now();
        let (image, space) = match algo.backend() {
            Backend::LibRaw => {
                let bin = self
                    .dcraw_emu
                    .as_deref()
                    .ok_or(Error::BackendMissing(Backend::LibRaw))?;
                backend::libraw::decode(bin, raw, algo, mode, opts)?
            }
            Backend::RawTherapee => {
                let bin = self
                    .rt_cli
                    .as_deref()
                    .ok_or(Error::BackendMissing(Backend::RawTherapee))?;
                backend::rt::decode(bin, raw, algo, mode, opts)?
            }
        };
        Ok(Decoded {
            image,
            backend: algo.backend(),
            algorithm: algo,
            mode,
            elapsed: started.elapsed(),
            space,
        })
    }
}

fn exe_name(base: &str) -> String {
    if cfg!(windows) && !base.ends_with(".exe") {
        format!("{base}.exe")
    } else {
        base.to_string()
    }
}

fn find_bin(env_key: &str, known: &[PathBuf], path_name: &str) -> Option<PathBuf> {
    if let Ok(p) = std::env::var(env_key) {
        let p = PathBuf::from(p);
        if p.is_file() {
            return Some(p);
        }
    }
    for p in known {
        if p.is_file() {
            return Some(p.clone());
        }
    }
    for name in [path_name, &exe_name(path_name)] {
        if let Ok(path) = std::env::var("PATH") {
            for dir in std::env::split_paths(&path) {
                let p = dir.join(name);
                if p.is_file() {
                    return Some(p);
                }
            }
        }
    }
    None
}
