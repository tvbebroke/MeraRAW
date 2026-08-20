//! Compact `models/profile_index.json` — file paths only (names derived at runtime).
//!
//!   cargo run -p meratech-core --example compact_profile_index

use meratech_core::profile::ProfileIndex;
use std::collections::BTreeMap;
use std::path::Path;

fn main() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("models/profile_index.json");
    let text = std::fs::read_to_string(&path).expect("read profile_index.json");
    let index: ProfileIndex = serde_json::from_str(&text).expect("parse index");

    let mut slim: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (camera, refs) in index.cameras {
        slim.insert(camera, refs.into_iter().map(|r| r.file).collect());
    }

    let out =
        serde_json::to_string_pretty(&serde_json::json!({ "cameras": slim })).expect("serialize");
    std::fs::write(&path, out).expect("write");
    eprintln!("compacted {}", path.display());
}
