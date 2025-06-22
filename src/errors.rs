use thiserror::Error;

pub type Result<T> = std::result::Result<T, TsgoError>;

#[derive(Error, Debug)]
pub enum TsgoError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] rmp_serde::encode::Error),

    #[error("Deserialization error: {0}")]
    Deserialization(#[from] rmp_serde::decode::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Protocol error: {message}")]
    Protocol { message: String },

    #[error("Process spawn error: {0}")]
    ProcessSpawn(String),

    #[error("Invalid response: expected {expected}, got {actual}")]
    InvalidResponse { expected: String, actual: String },

    #[error("MessagePack encoding error: {0}")]
    MessagePackWrite(#[from] rmp::encode::ValueWriteError),

    #[error("MessagePack decode error: {0}")]
    MessagePackRead(#[from] rmp::decode::ValueReadError),

    // Decoder-specific errors
    #[error("Invalid header: {reason}")]
    InvalidHeader { reason: String },

    #[error("Unsupported protocol version: expected {expected}, got {actual}")]
    UnsupportedProtocolVersion { expected: u8, actual: u8 },

    #[error(
        "String bounds out of range: string {string_index} has bounds [{start}, {end}) but data size is {data_size}"
    )]
    StringBoundsOutOfRange {
        string_index: usize,
        start: usize,
        end: usize,
        data_size: usize,
    },

    #[error("String bounds invalid: start {start} > end {end} for string {string_index}")]
    StringBoundsInvalid {
        string_index: usize,
        start: usize,
        end: usize,
    },

    #[error("String index out of bounds: index {index} >= table size {table_size}")]
    StringIndexOutOfBounds { index: usize, table_size: usize },

    #[error(
        "Node data buffer too small: node {node_index} needs {needed} bytes, only {available} available"
    )]
    NodeDataBufferTooSmall {
        node_index: usize,
        needed: usize,
        available: usize,
    },

    #[error("Invalid extended data: {reason}")]
    InvalidExtendedData { reason: String },

    #[error("Unknown syntax kind: {kind}")]
    UnknownSyntaxKind { kind: u32 },

    #[error("Buffer too small: need {needed} bytes, got {available}")]
    BufferTooSmall { needed: usize, available: usize },

    #[error("Invalid data offset: offset {offset} exceeds buffer size {buffer_size}")]
    InvalidDataOffset { offset: usize, buffer_size: usize },

    #[error("Invalid UTF-8 string: {context}")]
    InvalidUtf8 { context: String },

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
