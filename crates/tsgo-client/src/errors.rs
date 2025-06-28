use thiserror::Error;

pub type Result<T> = std::result::Result<T, ClientError>;

#[derive(Error, Debug)]
pub enum ClientError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Transport error: {0}")]
    Transport(#[from] tsgo_transport::TransportError),

    #[error("Virtual file system error: {0}")]
    Vfs(#[from] tsgo_vfs::VfsError),
}
