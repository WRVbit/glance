//! Custom error types for Linux Optimizer
//! Uses thiserror for ergonomic error definitions

use thiserror::Error;

/// Application-level errors
#[derive(Error, Debug)]
pub enum AppError {
    #[error("System error: {0}")]
    System(String),

    #[error("IO error: {0}")]
    Io(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Command failed: {0}")]
    CommandFailed(String),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Not supported on this distribution")]
    UnsupportedDistro,

    #[error("User cancelled operation")]
    UserCancelled,
}

// Manual From implementation for std::io::Error
impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::Io(err.to_string())
    }
}

/// Result type alias for convenience
pub type Result<T> = std::result::Result<T, AppError>;

/// Convert AppError to a serializable format for Tauri
impl serde::Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

