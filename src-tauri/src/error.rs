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
        // Log detail natively; don't ship absolute paths to the renderer.
        tracing::warn!(error = %e, "io error");
        match e.kind() {
            std::io::ErrorKind::NotFound => AppError::NotFound("not found".into()),
            std::io::ErrorKind::PermissionDenied => AppError::Io("permission denied".into()),
            _ => AppError::Io("io error".into()),
        }
    }
}

impl From<EngineError> for AppError {
    fn from(e: EngineError) -> Self {
        tracing::warn!(error = %e, "engine error");
        AppError::Engine("engine error".into())
    }
}

impl From<meratech_core::error::CoreError> for AppError {
    fn from(e: meratech_core::error::CoreError) -> Self {
        use meratech_core::error::CoreError as CE;
        tracing::warn!(error = %e, "core error");
        match e {
            CE::Io(_) => AppError::Io("io error".into()),
            CE::Decode(_) => AppError::Decode("decode failed".into()),
            CE::Gpu(_) => AppError::Gpu("gpu error".into()),
            CE::NoImage => AppError::NotFound("no image open".into()),
            CE::Engine(_) => AppError::Engine("engine error".into()),
            // InvalidOp messages are intentional user-facing validation text.
            CE::InvalidOp(m) => AppError::InvalidOp(m),
        }
    }
}
