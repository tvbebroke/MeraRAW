//! Image signal processing: demosaic, white balance, highlight recovery, denoise.
//! Live impls fold into `rawler` decode + `core/src/graph/*.wgsl`.
pub mod demosaic;
pub mod highlight_recovery;
pub mod noise;
pub mod white_balance;
