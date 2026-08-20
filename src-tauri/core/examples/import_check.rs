//! Quick catalog import smoke test:
//!   cargo run -p meratech-core --example import_check -- /path/to/folder

use meratech_core::catalog::{self, Catalog};
use std::path::Path;

fn main() {
    let root = std::env::args()
        .nth(1)
        .expect("usage: import_check <folder>");
    let root = Path::new(&root);
    if !root.is_dir() {
        eprintln!("not a directory: {}", root.display());
        std::process::exit(1);
    }

    let files = catalog::scan_folder(root);
    eprintln!("scan: {} raw files in {}", files.len(), root.display());
    if files.is_empty() {
        std::process::exit(2);
    }

    match catalog::import_one(&files[0]) {
        Ok(f) => eprintln!(
            "import_one ok: {} ({}x{})",
            f.path, f.meta.width, f.meta.height
        ),
        Err(e) => {
            eprintln!("import_one failed: {e}");
            std::process::exit(3);
        }
    }

    let mut cat = Catalog::open_default().expect("open catalog");
    cat.remember_folder(&root.to_string_lossy())
        .expect("remember_folder");
    eprintln!("catalog remember_folder ok");
}
