use thiserror::Error;

pub type Result<T> = std::result::Result<T, VfsError>;

#[derive(Error, Debug)]
pub enum VfsError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Virtual file system error during {operation} on '{path}': {error_message}")]
    Operation {
        operation: String,
        path: String,
        error_message: String,
    },
} 