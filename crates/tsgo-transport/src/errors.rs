use thiserror::Error;

pub type Result<T> = std::result::Result<T, TransportError>;

#[derive(Error, Debug)]
pub enum TransportError {
    #[error("VFS error: {0}")]
    Vfs(#[from] tsgo_vfs::VfsError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] rmp_serde::encode::Error),

    #[error("Deserialization error: {0}")]
    Deserialization(#[from] rmp_serde::decode::Error),

    #[error("MessagePack encoding error: {0}")]
    MessagePackWrite(#[from] rmp::encode::ValueWriteError),

    #[error("MessagePack decode error: {0}")]
    MessagePackRead(#[from] rmp::decode::ValueReadError),

    #[error("Process spawn error: {0}")]
    ProcessSpawn(String),

    #[error("Invalid response: expected {expected}, got {actual}")]
    InvalidResponse { expected: String, actual: String },

    #[error("Invalid protocol message array length: expected 3, got {actual}")]
    InvalidProtocolArrayLength { actual: u32 },

    #[error("Invalid message type: {message_type} is not a valid MessageType")]
    InvalidMessageType { message_type: u8 },

    #[error("Invalid UTF-8 in protocol message {field}: {error_message}")]
    InvalidProtocolUtf8 {
        field: String,
        error_message: String,
    },

    #[error("Transport process failed to start: {reason}")]
    TransportProcessStartFailed { reason: String },

    #[error("Transport process handle unavailable: {handle_type}")]
    TransportProcessHandleUnavailable { handle_type: String },

    #[error("Transport connection closed unexpectedly")]
    TransportConnectionClosed,

    #[error("Server returned error response: {server_message}")]
    ServerError { server_message: String },

    #[error("Unknown callback method: {method}")]
    UnknownCallback { method: String },

    #[error("Callback execution failed for method '{method}': {reason}")]
    CallbackExecutionFailed { method: String, reason: String },

    #[error("Invalid binary payload: {reason}")]
    InvalidBinaryPayload { reason: String },

    #[error("Message decode incomplete: expected more data")]
    IncompleteMessage,
}
