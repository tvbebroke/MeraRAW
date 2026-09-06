//! Fill `ImageMeta` shooting fields + GPS from an on-disk EXIF blob.

use crate::raw::ImageMeta;
use exif::{In, Reader, Tag, Value};
use rawler::Orientation;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

/// EXIF Orientation tag, or `Normal` when missing / unreadable.
pub fn file_orientation(path: &Path) -> Orientation {
    let Ok(file) = File::open(path) else {
        return Orientation::Normal;
    };
    let mut buf = BufReader::new(file);
    let Ok(exif) = Reader::new().read_from_container(&mut buf) else {
        return Orientation::Normal;
    };
    uint_field(&exif, Tag::Orientation)
        .map(|n| Orientation::from_u16(n as u16))
        .filter(|o| !matches!(o, Orientation::Unknown))
        .unwrap_or(Orientation::Normal)
}

/// Best-effort: open `path`, parse EXIF, fill empty camera / exposure / GPS
/// fields on `meta`. Failures are silent (no EXIF is common for PNG/BMP).
pub fn enrich_from_file(path: &Path, meta: &mut ImageMeta) {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return,
    };
    let mut buf = BufReader::new(file);
    let exif = match Reader::new().read_from_container(&mut buf) {
        Ok(e) => e,
        Err(_) => return,
    };

    if meta.camera_make.is_empty() {
        if let Some(s) = ascii_field(&exif, Tag::Make) {
            meta.camera_make = s;
        }
    }
    if meta.camera_model.is_empty() {
        if let Some(s) = ascii_field(&exif, Tag::Model) {
            meta.camera_model = s;
        }
    }
    if meta.lens.is_none() {
        meta.lens = ascii_field(&exif, Tag::LensModel)
            .or_else(|| ascii_field(&exif, Tag::LensMake))
            .filter(|s| !s.is_empty());
    }
    if meta.iso.is_none() {
        meta.iso = uint_field(&exif, Tag::PhotographicSensitivity)
            .or_else(|| uint_field(&exif, Tag::ISOSpeed));
    }
    if meta.aperture.is_none() {
        meta.aperture = rational_f32(&exif, Tag::FNumber);
    }
    if meta.focal_mm.is_none() {
        meta.focal_mm = rational_f32(&exif, Tag::FocalLength);
    }
    if meta.shutter.is_none() {
        meta.shutter = exposure_string(&exif);
    }
    if meta.captured_at.is_none() {
        meta.captured_at =
            ascii_field(&exif, Tag::DateTimeOriginal).or_else(|| ascii_field(&exif, Tag::DateTime));
    }
    if let Some(o) = orientation_name(&exif) {
        // Prefer file EXIF for rendered sources; RAW already has bake-aware string.
        if meta.kind == crate::raw::ImageKind::Rendered || meta.orientation == "Normal" {
            meta.orientation = o;
        }
    }

    if meta.gps_lat.is_none() || meta.gps_lon.is_none() {
        if let Some((lat, lon)) = read_gps(&exif) {
            meta.gps_lat = Some(lat);
            meta.gps_lon = Some(lon);
        }
    }
}

fn ascii_field(exif: &exif::Exif, tag: Tag) -> Option<String> {
    let f = exif.get_field(tag, In::PRIMARY)?;
    match &f.value {
        Value::Ascii(chunks) => {
            let s = chunks
                .iter()
                .flat_map(|c| c.iter().copied())
                .take_while(|&b| b != 0)
                .collect::<Vec<_>>();
            let t = String::from_utf8_lossy(&s).trim().to_string();
            if t.is_empty() {
                None
            } else {
                Some(t)
            }
        }
        _ => {
            let t = f.display_value().to_string();
            let t = t.trim().trim_matches('"').to_string();
            if t.is_empty() {
                None
            } else {
                Some(t)
            }
        }
    }
}

fn uint_field(exif: &exif::Exif, tag: Tag) -> Option<u32> {
    let f = exif.get_field(tag, In::PRIMARY)?;
    f.value.get_uint(0)
}

fn rational_f32(exif: &exif::Exif, tag: Tag) -> Option<f32> {
    let f = exif.get_field(tag, In::PRIMARY)?;
    match &f.value {
        Value::Rational(v) if !v.is_empty() => {
            let r = &v[0];
            if r.denom == 0 {
                None
            } else {
                Some(r.num as f32 / r.denom as f32)
            }
        }
        _ => None,
    }
}

fn exposure_string(exif: &exif::Exif) -> Option<String> {
    let f = exif.get_field(Tag::ExposureTime, In::PRIMARY)?;
    match &f.value {
        Value::Rational(v) if !v.is_empty() => {
            let r = &v[0];
            if r.num == 0 || r.denom == 0 {
                return None;
            }
            if r.num < r.denom {
                let den = (r.denom as f32 / r.num as f32).round() as u32;
                Some(format!("1/{den}"))
            } else {
                Some(format!("{:.1}s", r.num as f32 / r.denom as f32))
            }
        }
        _ => None,
    }
}

fn orientation_name(exif: &exif::Exif) -> Option<String> {
    let n = uint_field(exif, Tag::Orientation)?;
    Some(
        match n {
            1 => "Normal",
            2 => "HorizontalFlip",
            3 => "Rotate180",
            4 => "VerticalFlip",
            5 => "Transpose",
            6 => "Rotate90",
            7 => "Transverse",
            8 => "Rotate270",
            _ => return None,
        }
        .into(),
    )
}

fn read_gps(exif: &exif::Exif) -> Option<(f64, f64)> {
    let lat = decimal_coord(exif, Tag::GPSLatitude, Tag::GPSLatitudeRef, b'N', b'S')?;
    let lon = decimal_coord(exif, Tag::GPSLongitude, Tag::GPSLongitudeRef, b'E', b'W')?;
    Some((lat, lon))
}

fn decimal_coord(exif: &exif::Exif, tag: Tag, ref_tag: Tag, pos: u8, neg: u8) -> Option<f64> {
    let f = exif.get_field(tag, In::PRIMARY)?;
    let Value::Rational(coords) = &f.value else {
        return None;
    };
    if coords.len() < 3 {
        return None;
    }
    let deg = coords[0].to_f64();
    let min = coords[1].to_f64();
    let sec = coords[2].to_f64();
    let mut dec = deg + min / 60.0 + sec / 3600.0;
    if let Some(r) = exif.get_field(ref_tag, In::PRIMARY) {
        if let Value::Ascii(v) = &r.value {
            let b = v.first().and_then(|s| s.first()).copied().unwrap_or(pos);
            if b == neg || b == neg.to_ascii_lowercase() {
                dec = -dec;
            }
        }
    }
    if dec.is_finite() {
        Some(dec)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raw::ImageKind;

    #[test]
    fn enrich_noop_on_missing_file() {
        let mut meta = ImageMeta {
            path: "/no/such.jpg".into(),
            kind: ImageKind::Rendered,
            format: "JPEG".into(),
            bit_depth: 8,
            camera_make: String::new(),
            camera_model: String::new(),
            lens: None,
            iso: None,
            shutter: None,
            aperture: None,
            focal_mm: None,
            captured_at: None,
            width: 1,
            height: 1,
            orientation: "Normal".into(),
            as_shot_wb: [1.0, 1.0, 1.0],
            estimated_cct: None,
            camera_profile: None,
            available_profiles: Vec::new(),
            available_profile_files: Vec::new(),
            demosaic: String::new(),
            available_demosaic: Vec::new(),
            gps_lat: None,
            gps_lon: None,
            input_color_space: None,
            video: None,
        };
        enrich_from_file(Path::new("/no/such.jpg"), &mut meta);
        assert!(meta.camera_make.is_empty());
        assert!(meta.gps_lat.is_none());
    }
}
