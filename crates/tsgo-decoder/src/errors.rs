use thiserror::Error;

pub type Result<T> = std::result::Result<T, DecoderError>;

#[derive(Error, Debug)]
pub enum DecoderError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

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
}
