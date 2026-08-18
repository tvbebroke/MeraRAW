//! Build `models/profile_index.json` from `meraraw-derivatives/*.dcp`.
//!
//! Usage:
//!   MERARAW_PROFILES_DIR=/path/to/meraraw-derivatives \
//!     cargo run --example index_profiles -p meratech-core

use meratech_core::profile::dcp::DcpProfile;
use meratech_core::profile::{profiles_dir, ProfileRef};
use std::collections::BTreeMap;
use std::path::Path;

fn main() {
    let dir = profiles_dir();
    eprintln!("indexing profiles in {}", dir.display());
    if !dir.is_dir() {
        eprintln!("profiles dir not found: {}", dir.display());
        std::process::exit(1);
    }

    let mut cameras: BTreeMap<String, Vec<ProfileRef>> = BTreeMap::new();
    let mut ok = 0u64;
    let mut fail = 0u64;

    for entry in std::fs::read_dir(&dir).expect("read profiles dir") {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("dcp") {
            continue;
        }
        match DcpProfile::read_header(&path) {
            Ok((camera, name)) => {
                let file = path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string();
                cameras
                    .entry(camera)
                    .or_default()
                    .push(ProfileRef { name, file });
                ok += 1;
            }
            Err(e) => {
                eprintln!("skip {}: {e}", path.display());
                fail += 1;
            }
        }
    }

    for profiles in cameras.values_mut() {
        profiles.sort_by(|a, b| a.file.cmp(&b.file));
    }

    let mut slim: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (camera, refs) in cameras {
        slim.insert(camera, refs.into_iter().map(|r| r.file).collect());
    }

    let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("models/profile_index.json");
    let json = serde_json::to_string_pretty(&serde_json::json!({ "cameras": slim }))
        .expect("serialize index");
    std::fs::write(&out, json).expect("write index");
    eprintln!(
        "wrote {} ({} profiles indexed, {} skipped)",
        out.display(),
        ok,
        fail
    );
}
