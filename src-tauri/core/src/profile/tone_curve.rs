//! ProfileToneCurve parsing from DCP tag 50940.

use crate::curve::ProfileToneCurve;
use crate::error::CoreError;

const TIFF_FLOAT: u16 = 11;
const TIFF_SRATIONAL: u16 = 10;

const MAX_TONE_PAIRS: u32 = 4096;

pub fn parse_tone_curve(
    data: &[u8],
    typ: u16,
    count: u32,
    val: u32,
) -> Result<Option<ProfileToneCurve>, CoreError> {
    if typ != TIFF_FLOAT || count < 4 || !count.is_multiple_of(2) {
        return Ok(None);
    }
    let pairs = count / 2;
    if pairs > MAX_TONE_PAIRS {
        return Ok(None);
    }
    let off = val as usize;
    let need = off.saturating_add((count as usize).saturating_mul(4));
    if need > data.len() {
        return Ok(None);
    }
    let pairs = pairs as usize;
    let mut pts = Vec::with_capacity(pairs);
    for i in 0..pairs {
        let x = read_f32(data, off + i * 8)?;
        let y = read_f32(data, off + i * 8 + 4)?;
        pts.push([x, y]);
    }
    if pts.first().map(|p| p[0] > 1e-4 || p[1] > 1e-4).unwrap_or(true)
        || pts.last().map(|p| (p[0] - 1.0).abs() > 1e-3 || (p[1] - 1.0).abs() > 1e-3).unwrap_or(true)
    {
        return Ok(None);
    }
    Ok(Some(ProfileToneCurve::from_points(pts)))
}

pub fn parse_baseline_exposure_offset(
    data: &[u8],
    typ: u16,
    count: u32,
    val: u32,
) -> Result<f32, CoreError> {
    if typ != TIFF_SRATIONAL || count < 1 {
        return Ok(0.0);
    }
    let off = val as usize;
    let n = read_i32(data, off)?;
    let d = read_i32(data, off + 4)?;
    Ok(if d == 0 { 0.0 } else { n as f32 / d as f32 })
}

fn read_f32(data: &[u8], off: usize) -> Result<f32, CoreError> {
    Ok(f32::from_bits(read_u32(data, off)?))
}

fn read_u32(data: &[u8], off: usize) -> Result<u32, CoreError> {
    data.get(off..off + 4)
        .ok_or_else(|| CoreError::Decode("dcp: tone curve EOF".into()))?
        .try_into()
        .map(u32::from_le_bytes)
        .map_err(|_| CoreError::Decode("dcp: bad u32".into()))
}

fn read_i32(data: &[u8], off: usize) -> Result<i32, CoreError> {
    data.get(off..off + 4)
        .ok_or_else(|| CoreError::Decode("dcp: baseline exposure EOF".into()))?
        .try_into()
        .map(i32::from_le_bytes)
        .map_err(|_| CoreError::Decode("dcp: bad i32".into()))
}

#[cfg(test)]
mod tests {
    use crate::profile::DcpProfile;
    use std::path::PathBuf;

    fn profiles_dir() -> PathBuf {
        if let Ok(d) = std::env::var("MERARAW_PROFILES_DIR") {
            return PathBuf::from(d);
        }
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../meraraw-derivatives")
    }

    #[test]
    fn meraraw_standard_uses_adobe_default_curve() {
        let path = profiles_dir().join("Sony ILCE-7M3 MeraRAW Standard.dcp");
        if !path.exists() {
            return;
        }
        let dcp = DcpProfile::load(&path).unwrap();
        assert!(
            !dcp.tone_curve_embedded(),
            "MeraRAW Standard has no embedded curve"
        );
        let lifted = dcp.tone_curve().apply_rgb([0.18, 0.18, 0.18]);
        assert!(lifted[0] > 0.25, "default curve should lift midtones: {}", lifted[0]);
    }
}
