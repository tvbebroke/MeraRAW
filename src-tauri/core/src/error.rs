//! Core error type. Tauri layer maps this onto AppError.

#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("io: {0}")]
    Io(String),
    #[error("decode: {0}")]
    Decode(String),
    #[error("gpu: {0}")]
    Gpu(String),
    #[error("no image open")]
    NoImage,
    #[error("engine: {0}")]
    Engine(String),
    #[error("invalid op: {0}")]
    InvalidOp(String),
}

impl From<std::io::Error> for CoreError {
    fn from(e: std::io::Error) -> Self {
        CoreError::Io(e.to_string())
    }
}
