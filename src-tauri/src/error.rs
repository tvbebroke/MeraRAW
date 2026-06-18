//! One serializable error enum across all commands. Contract C4.
//! Frontend sees { kind, message }.

use meratech_core::engine::EngineError;

#[derive(Debug, thiserror::Error, serde::Serialize)]
#[serde(tag = "kind", content = "message")]
#[allow(dead_code)] // variants land across phases (Decode P1, InvalidOp P2)
pub enum AppError {
    #[error("io error: {0}")]
    Io(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("engine error: {0}")]
    Engine(String),
    #[error("gpu error: {0}")]
    Gpu(String),
    #[error("decode error: {0}")]
    Decode(String),
    #[error("invalid op: {0}")]
    InvalidOp(String),
    #[error("internal error: {0}")]
    Internal(String),
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        match e.kind() {
            std::io::ErrorKind::NotFound => AppError::NotFound(e.to_string()),
            _ => AppError::Io(e.to_string()),
        }
    }
}

impl From<EngineError> for AppError {
    fn from(e: EngineError) -> Self {
        AppError::Engine(e.to_string())
    }
}

impl From<meratech_core::error::CoreError> for AppError {
    fn from(e: meratech_core::error::CoreError) -> Self {
        use meratech_core::error::CoreError as CE;
        match e {
            CE::Io(m) => AppError::Io(m),
            CE::Decode(m) => AppError::Decode(m),
            CE::Gpu(m) => AppError::Gpu(m),
            CE::NoImage => AppError::NotFound("no image open".into()),
            CE::Engine(m) => AppError::Engine(m),
            CE::InvalidOp(m) => AppError::InvalidOp(m),
        }
    }
}
