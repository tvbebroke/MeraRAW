//! Probe IIRC DCP → TIFF decoder offset (dev helper).

use meratech_core::profile::dcp::DcpProfile;
use std::path::PathBuf;

fn main() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../meraraw-derivatives/Sony ILCE-7M4 MeraRAW Standard.dcp");
    match DcpProfile::load(&path) {
        Ok(d) => println!(
            "OK model={} name={}",
            d.unique_camera_model, d.profile_name
        ),
        Err(e) => println!("ERR {e}"),
    }
}
