//! External-binary backends. Each module knows how to invoke one worker and
//! interpret its output; nothing here links foreign code.

pub mod libraw;
pub mod rt;
