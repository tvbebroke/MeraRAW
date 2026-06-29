//! Adobe DNG Camera Profile (.dcp) parser — IIRC container + IFD tags.

use crate::color::{bradford_adapt, mat_inverse, mat_mul, CameraCalibration, Mat3, XYZ_TO_REC2020};
use crate::curve::ProfileToneCurve;
use crate::error::CoreError;
use crate::profile::hue_sat_map::{apply_hue_sat_maps, apply_look_table, HueSatMap};
use crate::profile::tone_curve::{parse_baseline_exposure_offset, parse_tone_curve};
use std::collections::HashMap;
use std::path::Path;

const TAG_UNIQUE_CAMERA_MODEL: u16 = 50708;
const TAG_PROFILE_NAME: u16 = 50936;
const TAG_CALIBRATION_ILLUMINANT1: u16 = 50778;
const TAG_CALIBRATION_ILLUMINANT2: u16 = 50779;
const TAG_COLOR_MATRIX1: u16 = 50721;
const TAG_COLOR_MATRIX2: u16 = 50722;
const TAG_FORWARD_MATRIX1: u16 = 50964;
const TAG_FORWARD_MATRIX2: u16 = 50965;
const TAG_HUE_SAT_MAP_DIMS: u16 = 50937;
const TAG_HUE_SAT_MAP_DATA1: u16 = 50938;
const TAG_HUE_SAT_MAP_DATA2: u16 = 50939;
const TAG_LOOK_TABLE_DIMS: u16 = 50981;
const TAG_LOOK_TABLE_DATA: u16 = 50982;
const TAG_PROFILE_TONE_CURVE: u16 = 50940;
const TAG_BASELINE_EXPOSURE_OFFSET: u16 = 51109;

const TIFF_SRATIONAL: u16 = 10;
const TIFF_SHORT: u16 = 3;
const TIFF_LONG: u16 = 4;
const TIFF_FLOAT: u16 = 11;

const D50_XYZ: [f32; 3] = [0.96422, 1.0, 0.82521];
const D65_XYZ: [f32; 3] = [0.95047, 1.0, 1.08883];

fn dcp_err(e: impl std::fmt::Display) -> CoreError {
    CoreError::Decode(format!("dcp: {e}"))
}

fn read_u16(data: &[u8], off: usize) -> Result<u16, CoreError> {
    data.get(off..off + 2)
        .ok_or_else(|| dcp_err("unexpected EOF"))?
        .try_into()
        .map(u16::from_le_bytes)
        .map_err(|_| dcp_err("bad u16"))
}

fn read_u32(data: &[u8], off: usize) -> Result<u32, CoreError> {
    data.get(off..off + 4)
        .ok_or_else(|| dcp_err("unexpected EOF"))?
        .try_into()
        .map(u32::from_le_bytes)
        .map_err(|_| dcp_err("bad u32"))
}

fn read_i32(data: &[u8], off: usize) -> Result<i32, CoreError> {
    data.get(off..off + 4)
        .ok_or_else(|| dcp_err("unexpected EOF"))?
        .try_into()
        .map(i32::from_le_bytes)
        .map_err(|_| dcp_err("bad i32"))
}

fn read_ascii(data: &[u8], _typ: u16, count: u32, val: u32) -> Result<String, CoreError> {
    let count = count as usize;
    if count == 0 {
        return Ok(String::new());
    }
    let owned;
    let bytes: &[u8] = if count <= 4 {
        owned = val.to_le_bytes();
        &owned[..count]
    } else {
        let off = val as usize;
        data.get(off..off + count)
            .ok_or_else(|| dcp_err("ascii out of range"))?
    };
    let s = if bytes.last() == Some(&0) {
        &bytes[..bytes.len() - 1]
    } else {
        bytes
    };
    Ok(String::from_utf8_lossy(s).trim().to_string())
}

fn read_srational9(data: &[u8], typ: u16, count: u32, val: u32) -> Result<Option<Mat3>, CoreError> {
    if typ != TIFF_SRATIONAL || count < 9 {
        return Ok(None);
    }
    let off = val as usize;
    let mut vals = [0.0f32; 9];
    for (i, slot) in vals.iter_mut().enumerate() {
        let n = read_i32(data, off + i * 8)?;
        let d = read_i32(data, off + i * 8 + 4)?;
        *slot = if d == 0 { 0.0 } else { n as f32 / d as f32 };
    }
    Ok(Some([
        [vals[0], vals[1], vals[2]],
        [vals[3], vals[4], vals[5]],
        [vals[6], vals[7], vals[8]],
    ]))
}

fn read_u16_scalar(_data: &[u8], typ: u16, count: u32, val: u32) -> Result<Option<u16>, CoreError> {
    match typ {
        TIFF_SHORT if count >= 1 => Ok(Some((val & 0xffff) as u16)),
        TIFF_LONG if count >= 1 => Ok(Some(val as u16)),
        _ => Ok(None),
    }
}

fn read_u32_array(data: &[u8], typ: u16, count: u32, val: u32) -> Result<Vec<u32>, CoreError> {
    if typ != TIFF_LONG || count == 0 {
        return Ok(Vec::new());
    }
    let count = count as usize;
    if count <= 1 {
        return Ok(vec![val]);
    }
    let off = val as usize;
    (0..count)
        .map(|i| read_u32(data, off + i * 4))
        .collect()
}

fn read_float_array(data: &[u8], typ: u16, count: u32, val: u32) -> Result<Vec<f32>, CoreError> {
    if typ != TIFF_FLOAT || count == 0 {
        return Ok(Vec::new());
    }
    let count = count as usize;
    let off = val as usize;
    (0..count)
        .map(|i| read_u32(data, off + i * 4).map(f32::from_bits))
        .collect()
}

fn parse_hue_sat_map(
    data: &[u8],
    tags: &HashMap<u16, (u16, u32, u32)>,
    dims_tag: u16,
    data_tag: u16,
) -> Result<Option<HueSatMap>, CoreError> {
    let Some(&(typ, count, val)) = tags.get(&dims_tag) else {
        return Ok(None);
    };
    let dims = read_u32_array(data, typ, count, val)?;
    if dims.len() < 3 {
        return Ok(None);
    }
    let (hue_div, sat_div, val_div) = (dims[0], dims[1], dims[2]);
    let Some(&(dtyp, dcount, dval)) = tags.get(&data_tag) else {
        return Ok(None);
    };
    let floats = read_float_array(data, dtyp, dcount, dval)?;
    let expected = (hue_div * sat_div * val_div * 3) as usize;
    if floats.len() < expected {
        return Ok(None);
    }
    let deltas: Vec<[f32; 3]> = floats[..expected]
        .chunks(3)
        .map(|c| [c[0], c[1], c[2]])
        .collect();
    let map = HueSatMap {
        hue_div,
        sat_div,
        val_div,
        deltas,
    };
    Ok(map.is_valid().then_some(map))
}

/// Parse Adobe DCP IFD at byte offset 8 (after the IIRC/MMCR header).
fn parse_ifd(data: &[u8], ifd_off: usize) -> Result<HashMap<u16, (u16, u32, u32)>, CoreError> {
    let n = read_u16(data, ifd_off)? as usize;
    let mut tags = HashMap::new();
    for i in 0..n {
        let e = ifd_off + 2 + i * 12;
        let tag = read_u16(data, e)?;
        let typ = read_u16(data, e + 2)?;
        let count = read_u32(data, e + 4)?;
        let val = read_u32(data, e + 8)?;
        tags.insert(tag, (typ, count, val));
    }
    Ok(tags)
}

fn illuminant_code_cct(code: u16) -> f32 {
    match code {
        17 => 2856.0,
        21 => 6504.0,
        20 => 5003.0,
        23 => 5500.0,
        22 => 7504.0,
        13 => 5500.0,
        14 => 6504.0,
        _ => 5500.0,
    }
}

#[derive(Debug, Clone)]
pub struct DcpProfile {
    pub unique_camera_model: String,
    pub profile_name: String,
    illuminants: Vec<(f32, Mat3)>,
    ill1_cct: f32,
    ill2_cct: f32,
    hue_sat_map1: Option<HueSatMap>,
    hue_sat_map2: Option<HueSatMap>,
    look_table: Option<HueSatMap>,
    tone_curve: ProfileToneCurve,
    baseline_exposure_offset: f32,
}

/// GPU-upload-ready view of a profile's look (see `DcpProfile::look_data`).
/// Each map is `(hue_div, sat_div, val_div, deltas)` with deltas packed RGBA.
pub struct DcpLookData {
    pub map1: Option<(u32, u32, u32, Vec<[f32; 4]>)>,
    pub map2: Option<(u32, u32, u32, Vec<[f32; 4]>)>,
    pub look: Option<(u32, u32, u32, Vec<[f32; 4]>)>,
    /// tone curve evaluated at LUT_SIZE points over [0,1]
    pub tone_lut: Vec<f32>,
    pub baseline_gain: f32,
    pub ill1_cct: f32,
    pub ill2_cct: f32,
}

impl DcpProfile {
    pub fn load(path: &Path) -> Result<Self, CoreError> {
        let data = std::fs::read(path)?;
        Self::parse(&data, path)
    }

    pub fn read_header(path: &Path) -> Result<(String, String), CoreError> {
        let data = std::fs::read(path)?;
        let tags = parse_ifd(&data, 8)?;
        let model = tags
            .get(&TAG_UNIQUE_CAMERA_MODEL)
            .map(|&(typ, count, val)| read_ascii(&data, typ, count, val))
            .transpose()?
            .filter(|s| !s.is_empty())
            .ok_or_else(|| dcp_err(format!("missing UniqueCameraModel in {}", path.display())))?;
        let name = tags
            .get(&TAG_PROFILE_NAME)
            .map(|&(typ, count, val)| read_ascii(&data, typ, count, val))
            .transpose()?
            .unwrap_or_default();
        Ok((model, name))
    }

    fn parse(data: &[u8], path: &Path) -> Result<Self, CoreError> {
        if data.len() < 12 {
            return Err(dcp_err("file too small"));
        }
        if &data[0..4] != b"IIRC" && &data[0..4] != b"MMCR" {
            return Err(dcp_err("not an Adobe DCP (expected IIRC/MMCR)"));
        }
        let tags = parse_ifd(data, 8)?;
        let unique_camera_model = tags
            .get(&TAG_UNIQUE_CAMERA_MODEL)
            .map(|&(typ, count, val)| read_ascii(data, typ, count, val))
            .transpose()?
            .filter(|s| !s.is_empty())
            .ok_or_else(|| dcp_err(format!("missing UniqueCameraModel in {}", path.display())))?;
        let profile_name = tags
            .get(&TAG_PROFILE_NAME)
            .map(|&(typ, count, val)| read_ascii(data, typ, count, val))
            .transpose()?
            .unwrap_or_default();

        let ill1 = tags
            .get(&TAG_CALIBRATION_ILLUMINANT1)
            .and_then(|&(typ, count, val)| read_u16_scalar(data, typ, count, val).ok().flatten())
            .unwrap_or(21);
        let ill2 = tags
            .get(&TAG_CALIBRATION_ILLUMINANT2)
            .and_then(|&(typ, count, val)| read_u16_scalar(data, typ, count, val).ok().flatten())
            .unwrap_or(17);

        let fwd1 = tags
            .get(&TAG_FORWARD_MATRIX1)
            .and_then(|&(typ, count, val)| read_srational9(data, typ, count, val).ok().flatten());
        let fwd2 = tags
            .get(&TAG_FORWARD_MATRIX2)
            .and_then(|&(typ, count, val)| read_srational9(data, typ, count, val).ok().flatten());
        let cm1 = tags
            .get(&TAG_COLOR_MATRIX1)
            .and_then(|&(typ, count, val)| read_srational9(data, typ, count, val).ok().flatten());
        let cm2 = tags
            .get(&TAG_COLOR_MATRIX2)
            .and_then(|&(typ, count, val)| read_srational9(data, typ, count, val).ok().flatten());

        let mut illuminants: Vec<(f32, Mat3)> = Vec::new();
        if let Some(m) = fwd1.or_else(|| cm1.and_then(|cm| mat_inverse(&cm))) {
            illuminants.push((illuminant_code_cct(ill1), m));
        }
        if let Some(m) = fwd2.or_else(|| cm2.and_then(|cm| mat_inverse(&cm))) {
            illuminants.push((illuminant_code_cct(ill2), m));
        }
        illuminants.sort_by(|a, b| a.0.total_cmp(&b.0));
        illuminants.dedup_by(|a, b| (a.0 - b.0).abs() < 1.0);

        if illuminants.is_empty() {
            return Err(dcp_err(format!(
                "no usable matrices in {}",
                path.display()
            )));
        }

        let ill1_cct = illuminant_code_cct(ill1);
        let ill2_cct = illuminant_code_cct(ill2);
        let hue_sat_map1 =
            parse_hue_sat_map(data, &tags, TAG_HUE_SAT_MAP_DIMS, TAG_HUE_SAT_MAP_DATA1)?;
        let hue_sat_map2 =
            parse_hue_sat_map(data, &tags, TAG_HUE_SAT_MAP_DIMS, TAG_HUE_SAT_MAP_DATA2)?;
        let look_table = parse_hue_sat_map(data, &tags, TAG_LOOK_TABLE_DIMS, TAG_LOOK_TABLE_DATA)?;

        let tone_curve = tags
            .get(&TAG_PROFILE_TONE_CURVE)
            .and_then(|&(typ, count, val)| parse_tone_curve(data, typ, count, val).ok().flatten())
            .unwrap_or_else(ProfileToneCurve::adobe_default);
        let baseline_exposure_offset = tags
            .get(&TAG_BASELINE_EXPOSURE_OFFSET)
            .map(|&(typ, count, val)| parse_baseline_exposure_offset(data, typ, count, val))
            .transpose()?
            .unwrap_or(0.0);

        Ok(Self {
            unique_camera_model,
            profile_name,
            illuminants,
            ill1_cct,
            ill2_cct,
            hue_sat_map1,
            hue_sat_map2,
            look_table,
            tone_curve,
            baseline_exposure_offset,
        })
    }

    pub fn matches_camera(&self, make: &str, model: &str) -> bool {
        crate::profile::normalize_key(&self.unique_camera_model)
            == crate::profile::camera_model_key(make, model)
            || crate::profile::normalize_key(&self.unique_camera_model)
                == crate::profile::normalize_key(model)
    }

    fn cam_to_xyz_d50_at(&self, cct: f32) -> Option<Mat3> {
        match self.illuminants.len() {
            0 => None,
            1 => Some(self.illuminants[0].1),
            _ => {
                let (t1, m1) = &self.illuminants[0];
                let (t2, m2) = self.illuminants.last().unwrap();
                let cct = cct.clamp(*t1, *t2);
                let w = (1.0 / cct - 1.0 / t2) / (1.0 / t1 - 1.0 / t2);
                let mut out = [[0.0f32; 3]; 3];
                for i in 0..3 {
                    for j in 0..3 {
                        out[i][j] = w * m1[i][j] + (1.0 - w) * m2[i][j];
                    }
                }
                Some(out)
            }
        }
    }

    /// Apply DCP look on linear Rec.2020 RGB: HueSatMap → tone curve → baseline EV → LookTable.
    pub fn apply_look(&self, rec2020: [f32; 3], cct: f32) -> [f32; 3] {
        let mut rgb = apply_hue_sat_maps(
            rec2020,
            self.hue_sat_map1.as_ref(),
            self.hue_sat_map2.as_ref(),
            cct,
            self.ill1_cct,
            self.ill2_cct,
        );
        rgb = self.tone_curve.apply_rgb(rgb);
        if self.baseline_exposure_offset.abs() > 1e-6 {
            let gain = 2f32.powf(self.baseline_exposure_offset);
            rgb = [rgb[0] * gain, rgb[1] * gain, rgb[2] * gain];
        }
        if let Some(lt) = self.look_table.as_ref() {
            rgb = apply_look_table(rgb, lt);
        }
        rgb
    }

    pub fn tone_curve(&self) -> &ProfileToneCurve {
        &self.tone_curve
    }

    pub fn tone_curve_embedded(&self) -> bool {
        self.tone_curve.embedded
    }

    pub fn has_look(&self) -> bool {
        true
    }

    /// Everything the GPU look pass needs, pulled out of the private fields:
    /// the (valid) HSV delta tables as RGBA, the tone curve baked to a 1D LUT,
    /// the baseline-EV gain, and the two illuminant CCTs for the map blend.
    pub fn look_data(&self) -> DcpLookData {
        let conv = |m: &HueSatMap| {
            (
                m.hue_div,
                m.sat_div,
                m.val_div,
                m.deltas.iter().map(|d| [d[0], d[1], d[2], 0.0]).collect::<Vec<[f32; 4]>>(),
            )
        };
        let valid = |m: &Option<HueSatMap>| m.as_ref().filter(|x| x.is_valid()).map(conv);
        let n = crate::curve::LUT_SIZE;
        let tone_lut = (0..n)
            .map(|i| self.tone_curve.eval(i as f32 / (n - 1) as f32))
            .collect();
        DcpLookData {
            map1: valid(&self.hue_sat_map1),
            map2: valid(&self.hue_sat_map2),
            look: valid(&self.look_table),
            tone_lut,
            baseline_gain: 2f32.powf(self.baseline_exposure_offset),
            ill1_cct: self.ill1_cct,
            ill2_cct: self.ill2_cct,
        }
    }

    /// White-balanced camera RGB → linear Rec.2020 (D65).
    pub fn cam_to_rec2020(&self, wb: &[f32; 4], cal: &CameraCalibration) -> Option<Mat3> {
        let cct = cal.estimate_cct(wb);
        let cam_to_xyz_d50 = self.cam_to_xyz_d50_at(cct)?;
        let adapt = bradford_adapt(D50_XYZ, D65_XYZ);
        Some(mat_mul(
            &XYZ_TO_REC2020,
            &mat_mul(&adapt, &cam_to_xyz_d50),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::mat_vec;
    use std::path::PathBuf;

    fn profiles_dir() -> PathBuf {
        if let Ok(d) = std::env::var("MERARAW_PROFILES_DIR") {
            return PathBuf::from(d);
        }
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../meraraw-derivatives")
    }

    #[test]
    fn loads_sony_ilce7m4_meraraw_standard() {
        let path = profiles_dir().join("Sony ILCE-7M4 MeraRAW Standard.dcp");
        if !path.exists() {
            return;
        }
        let dcp = DcpProfile::load(&path).unwrap();
        assert_eq!(dcp.unique_camera_model, "Sony ILCE-7M4");
        assert!(dcp.profile_name.contains("MeraRAW"));
        assert!(dcp.matches_camera("SONY", "ILCE-7M4"));
        assert!(dcp.has_look());
        assert!(
            !dcp.tone_curve_embedded(),
            "MeraRAW Standard should use Adobe default tone curve"
        );
        let mid = dcp.tone_curve().apply_rgb([0.18, 0.18, 0.18]);
        assert!(mid[0] > 0.25, "tone curve should lift midtones");
        let cal = CameraCalibration {
            calibrations: vec![(
                2856.0,
                [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            )],
        };
        let m = dcp.cam_to_rec2020(&[1.0, 1.0, 1.0, 1.0], &cal).unwrap();
        let out = mat_vec(&m, [0.5, 0.5, 0.5]);
        assert!(out.iter().all(|v| v.is_finite() && *v >= 0.0));
    }
}
