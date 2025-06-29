use std::{
    collections::HashMap,
    error::Error,
    io::{BufReader, Read as _, Write as _},
    marker::PhantomData,
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    sync::Arc,
};

use rmp::{
    decode::{read_array_len, read_bin_len, read_u8},
    encode::{write_array_len, write_bin, write_u8},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, from_str, to_string};
use strum::FromRepr;

use crate::{Result, TransportError};

/// Message types for the tsgo protocol  
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, FromRepr)]
#[repr(u8)]
pub enum MessageType {
    Request = 1,
    CallResponse = 2,
    CallError = 3,
    Response = 4,
    Error = 5,
    Call = 6,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProtocolMessage(pub MessageType, pub String, pub Value);

impl ProtocolMessage {
    pub fn new(message_type: MessageType, method: impl Into<String>, payload: Value) -> Self {
        Self(message_type, method.into(), payload)
    }

    /// Encode a protocol message to binary MessagePack format
    pub fn encode(&self) -> Result<Vec<u8>> {
        let mut encoded = Vec::new();

        // Write 3-element array
        write_array_len(&mut encoded, 3)?;

        // Write MessageType as explicit uint8 (this forces 0xcc encoding)
        write_u8(&mut encoded, self.0 as u8)?;

        // Write method name as binary data
        write_bin(&mut encoded, self.1.as_bytes())?;

        // Encode payload as JSON string, then as binary data
        let payload_json = to_string(&self.2)?;
        write_bin(&mut encoded, payload_json.as_bytes())?;

        Ok(encoded)
    }

    /// Decode a binary MessagePack protocol message
    pub fn decode(buffer: &[u8]) -> Result<Self> {
        let mut cursor = std::io::Cursor::new(buffer);

        // Read array length (should be 3)
        let array_len = read_array_len(&mut cursor)?;
        if array_len != 3 {
            return Err(TransportError::InvalidProtocolArrayLength { actual: array_len });
        }

        // Read MessageType as uint8
        let msg_type_value = read_u8(&mut cursor)?;
        let msg_type =
            MessageType::from_repr(msg_type_value).ok_or(TransportError::InvalidMessageType {
                message_type: msg_type_value,
            })?;

        // Read method name as binary data
        let method_len = read_bin_len(&mut cursor)? as usize;
        let mut method_bytes = vec![0u8; method_len];
        std::io::Read::read_exact(&mut cursor, &mut method_bytes)?;
        let method =
            String::from_utf8(method_bytes).map_err(|e| TransportError::InvalidProtocolUtf8 {
                field: "method name".to_string(),
                error_message: e.to_string(),
            })?;

        // Read payload as binary data
        let payload_len = read_bin_len(&mut cursor)? as usize;
        let mut payload_bytes = vec![0u8; payload_len];
        std::io::Read::read_exact(&mut cursor, &mut payload_bytes)?;
        let payload_str =
            String::from_utf8(payload_bytes).map_err(|e| TransportError::InvalidProtocolUtf8 {
                field: "payload".to_string(),
                error_message: e.to_string(),
            })?;

        // Parse payload as JSON
        let payload: Value = if payload_str.is_empty() {
            Value::Null
        } else {
            from_str(&payload_str)?
        };

        Ok(ProtocolMessage(msg_type, method, payload))
    }
}

pub type CallbackFunction<'t, CallbackError> =
    Arc<dyn Fn(Value) -> std::result::Result<Value, CallbackError> + Send + Sync + 't>;

pub enum Callback<'t, CallbackError> {
    Sync(CallbackFunction<'t, CallbackError>),
}

/// Transport layer for communicating with tsgo server
pub struct TsgoTransport<'t, CallbackError: Error> {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    callbacks: HashMap<String, Callback<'t, CallbackError>>,
    _phantom: PhantomData<CallbackError>,
}

impl<'t, CallbackError: Error> TsgoTransport<'t, CallbackError> {
    /// Create a new transport by spawning the tsgo process
    pub fn new(tsgo_path: &str, cwd: Option<&str>) -> Result<Self> {
        let mut cmd = Command::new(tsgo_path);
        cmd.args(["--api", "-cwd", cwd.unwrap_or(".")])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd
            .spawn()
            .map_err(|e| TransportError::TransportProcessStartFailed {
                reason: format!("Failed to spawn tsgo process '{}': {}", tsgo_path, e),
            })?;

        let stdin = child.stdin.take().ok_or_else(|| {
            TransportError::TransportProcessHandleUnavailable {
                handle_type: "stdin".to_string(),
            }
        })?;

        let stdout = child.stdout.take().ok_or_else(|| {
            TransportError::TransportProcessHandleUnavailable {
                handle_type: "stdout".to_string(),
            }
        })?;

        let stdout = BufReader::new(stdout);

        Ok(TsgoTransport {
            child,
            stdin,
            stdout,
            callbacks: HashMap::new(),
            _phantom: PhantomData,
        })
    }

    /// Configure the transport with optional settings
    pub fn configure(&mut self, log_file: Option<&str>, callback_names: &[String]) -> Result<()> {
        let config = serde_json::json!({
            "logFile": log_file,
            "callbacks": callback_names
        });

        self.request("configure", config)?;
        Ok(())
    }

    /// Register a synchronous callback function
    pub fn register_callback<F>(&mut self, name: String, callback: F)
    where
        F: Fn(Value) -> std::result::Result<Value, CallbackError> + Send + Sync + 't,
    {
        self.callbacks
            .insert(name, Callback::Sync(Arc::new(callback)));
    }

    /// Send a request and return the response
    pub fn request(&mut self, method: &str, payload: Value) -> Result<Value> {
        let request = ProtocolMessage(MessageType::Request, method.to_string(), payload);
        self.send_message(&request)?;

        loop {
            let response = self.read_message()?;
            match response.0 {
                MessageType::Response => {
                    return Ok(response.2);
                }
                MessageType::Error => {
                    return Err(TransportError::ServerError {
                        server_message: response.2.to_string(),
                    });
                }
                MessageType::Call => {
                    self.handle_callback(&response.1, response.2)?;
                }
                _ => {
                    return Err(TransportError::InvalidResponse {
                        expected: "Response or Error".to_string(),
                        actual: format!("{:?}", response.0),
                    });
                }
            }
        }
    }

    /// Send a binary request and return binary response
    pub fn request_binary(&mut self, method: &str, payload: Vec<u8>) -> Result<Vec<u8>> {
        let payload_json: Value = from_str(&String::from_utf8(payload).map_err(|e| {
            TransportError::InvalidBinaryPayload {
                reason: format!("Invalid UTF-8 in binary payload: {}", e),
            }
        })?)?;

        let request = ProtocolMessage(MessageType::Request, method.to_string(), payload_json);
        self.send_message(&request)?;

        loop {
            let response = self.read_message()?;
            match response.0 {
                MessageType::Response => {
                    let response_str = to_string(&response.2)?;
                    return Ok(response_str.into_bytes());
                }
                MessageType::Error => {
                    return Err(TransportError::ServerError {
                        server_message: response.2.to_string(),
                    });
                }
                MessageType::Call => {
                    self.handle_callback(&response.1, response.2)?;
                }
                _ => {
                    return Err(TransportError::InvalidResponse {
                        expected: "Response or Error".to_string(),
                        actual: format!("{:?}", response.0),
                    });
                }
            }
        }
    }

    /// Handle incoming callback
    fn handle_callback(&mut self, method: &str, args: Value) -> Result<()> {
        if let Some(callback) = self.callbacks.get(method) {
            let result = match callback {
                Callback::Sync(sync_callback) => {
                    sync_callback(args).map_err(|e| TransportError::CallbackExecutionFailed {
                        method: method.to_string(),
                        reason: e.to_string(),
                    })?
                }
            };
            let response = ProtocolMessage(MessageType::CallResponse, method.to_string(), result);
            self.send_message(&response)?;
        } else {
            let error = ProtocolMessage(
                MessageType::CallError,
                method.to_string(),
                Value::String(format!("Unknown callback method: {}", method)),
            );
            self.send_message(&error)?;
            return Err(TransportError::UnknownCallback {
                method: method.to_string(),
            });
        }
        Ok(())
    }

    /// Send a protocol message
    fn send_message(&mut self, message: &ProtocolMessage) -> Result<()> {
        let encoded = message.encode()?;
        self.stdin.write_all(&encoded)?;
        self.stdin.flush()?;
        Ok(())
    }

    /// Read a protocol message
    fn read_message(&mut self) -> Result<ProtocolMessage> {
        let mut buffer = Vec::new();
        let mut temp_buf = [0u8; 1024];

        loop {
            let bytes_read = self.stdout.read(&mut temp_buf)?;
            if bytes_read == 0 {
                return Err(TransportError::TransportConnectionClosed);
            }

            buffer.extend_from_slice(&temp_buf[..bytes_read]);
            match ProtocolMessage::decode(&buffer) {
                Ok(message) => return Ok(message),
                Err(TransportError::InvalidProtocolArrayLength { .. })
                | Err(TransportError::InvalidMessageType { .. })
                | Err(TransportError::InvalidProtocolUtf8 { .. }) => {
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Close the transport and terminate the process
    pub fn close(self) -> Result<()> {
        let TsgoTransport {
            stdin, mut child, ..
        } = self;

        drop(stdin);

        let _ = child.wait();

        Ok(())
    }
}

/// High-level client configuration
#[derive(Debug, Clone)]
pub struct ClientOptions {
    pub tsgo_path: String,
    pub cwd: Option<String>,
    pub log_file: Option<String>,
}

impl Default for ClientOptions {
    fn default() -> Self {
        Self {
            tsgo_path: "tsgo".to_string(),
            cwd: None,
            log_file: None,
        }
    }
}

/// Protocol response types matching the TypeScript definitions
#[derive(Debug, Deserialize)]
pub struct ConfigResponse {
    pub options: HashMap<String, Value>,
    pub file_names: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct ProjectResponse {
    pub id: String,
    pub config_file_name: String,
    pub compiler_options: HashMap<String, Value>,
    pub root_files: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct SymbolResponse {
    pub id: String,
    pub name: String,
    pub flags: u32,
    pub check_flags: u32,
}

#[derive(Debug, Deserialize)]
pub struct TypeResponse {
    pub id: String,
    pub flags: u32,
}

#[cfg(test)]
mod tests {
    use std::fs;

    use rmp_serde::{from_slice, to_vec};
    use rstest::rstest;
    use serde_json::{Value as JsonValue, from_str};

    use super::*;

    fn load_fixture(name: &str) -> serde_json::Result<JsonValue> {
        let path = format!("test_data/transports/{}.json", name);
        let content = fs::read_to_string(path).expect("Failed to read fixture file");
        from_str(&content)
    }

    /// Test protocol message encoding with real fixture data
    #[rstest]
    #[case::echo_string("echo_string", "Echo string")]
    #[case::echo_number("echo_number", "Echo number")]
    #[case::echo_boolean("echo_boolean", "Echo boolean")]
    #[case::echo_array("echo_array", "Echo array")]
    #[case::echo_object("echo_object", "Echo object")]
    #[case::echo_null("echo_null", "Echo null")]
    fn test_encode_protocol_message_fixtures(
        #[case] fixture_name: &str,
        #[case] _description: &str,
    ) {
        let fixture = load_fixture(fixture_name).unwrap();

        let expected_bytes: Vec<u8> = fixture["request"]["bytes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap() as u8)
            .collect();

        let payload = fixture["request"]["message"]["payload"].clone();

        let message = ProtocolMessage(MessageType::Request, "echo".to_string(), payload);

        let encoded = message.encode().unwrap();

        assert_eq!(
            encoded, expected_bytes,
            "Encoding mismatch for fixture: {}",
            fixture_name
        );
    }

    /// Test protocol message decoding with real fixture data
    #[rstest]
    #[case::echo_string("echo_string", "Echo string")]
    #[case::echo_number("echo_number", "Echo number")]
    #[case::echo_boolean("echo_boolean", "Echo boolean")]
    #[case::echo_array("echo_array", "Echo array")]
    #[case::echo_object("echo_object", "Echo object")]
    #[case::echo_null("echo_null", "Echo null")]
    fn test_decode_protocol_message_fixtures(
        #[case] fixture_name: &str,
        #[case] _description: &str,
    ) {
        let fixture = load_fixture(fixture_name).unwrap();

        let response_bytes: Vec<u8> = fixture["response"]["bytes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap() as u8)
            .collect();

        let expected_payload = fixture["response"]["message"]["payload"].clone();

        let decoded = ProtocolMessage::decode(&response_bytes).unwrap();

        assert_eq!(decoded.0, MessageType::Response);
        assert_eq!(decoded.1, "echo");
        assert_eq!(
            decoded.2, expected_payload,
            "Payload mismatch for fixture: {}",
            fixture_name
        );
    }

    /// Test round-trip encoding/decoding
    #[test]
    fn test_encode_decode_roundtrip() {
        let test_cases = vec![
            ("string", serde_json::json!("test")),
            ("number", serde_json::json!(42)),
            ("boolean", serde_json::json!(true)),
            ("null", serde_json::json!(null)),
            ("array", serde_json::json!([1, 2, 3, "test"])),
            (
                "object",
                serde_json::json!({"key": "value", "nested": {"deep": true}}),
            ),
        ];

        for (name, payload) in test_cases {
            let original =
                ProtocolMessage(MessageType::Request, "test_method".to_string(), payload);

            let encoded = original.encode().unwrap();
            let decoded = ProtocolMessage::decode(&encoded).unwrap();

            assert_eq!(original.0, decoded.0, "MessageType mismatch for {}", name);
            assert_eq!(original.1, decoded.1, "Method mismatch for {}", name);
            assert_eq!(original.2, decoded.2, "Payload mismatch for {}", name);
        }
    }

    /// Test all message types encoding
    #[test]
    fn test_all_message_types() {
        let message_types = vec![
            MessageType::Request,
            MessageType::CallResponse,
            MessageType::CallError,
            MessageType::Response,
            MessageType::Error,
            MessageType::Call,
        ];

        for msg_type in message_types {
            let message = ProtocolMessage(msg_type, "test".to_string(), serde_json::json!("data"));

            let encoded = message.encode().unwrap();
            let decoded = ProtocolMessage::decode(&encoded).unwrap();

            assert_eq!(message.0, decoded.0);
            assert_eq!(message.1, decoded.1);
            assert_eq!(message.2, decoded.2);
        }
    }

    /// Test error handling for malformed data
    #[test]
    fn test_decode_malformed_data() {
        // Test empty data
        assert!(ProtocolMessage::decode(&[]).is_err());

        // Test incomplete array
        assert!(ProtocolMessage::decode(&[0x93]).is_err());

        // Test wrong array length
        let wrong_length = &[0x92, 0xcc, 0x01, 0xc4, 0x04, b'e', b'c', b'h', b'o'];
        match ProtocolMessage::decode(wrong_length) {
            Err(TransportError::InvalidProtocolArrayLength { actual }) => {
                assert_eq!(actual, 2);
            }
            other => panic!("Expected InvalidProtocolArrayLength, got: {:?}", other),
        }

        // Test invalid message type
        let invalid_type = &[
            0x93, 0xcc, 0x99, 0xc4, 0x04, b'e', b'c', b'h', b'o', 0xc4, 0x04, b't', b'e', b's',
            b't',
        ];
        match ProtocolMessage::decode(invalid_type) {
            Err(TransportError::InvalidMessageType { message_type }) => {
                assert_eq!(message_type, 0x99);
            }
            other => panic!("Expected InvalidMessageType, got: {:?}", other),
        }
    }

    /// Test specific binary protocol format requirements
    #[test]
    fn test_binary_format_requirements() {
        let message = ProtocolMessage(
            MessageType::Request,
            "echo".to_string(),
            serde_json::json!("test"),
        );

        let encoded = message.encode().unwrap();

        assert_eq!(encoded[0], 0x93, "Should start with 3-element array marker");
        assert_eq!(encoded[1], 0xcc, "MessageType should use uint8 format");
        assert_eq!(encoded[2], 0x01, "Request MessageType should be 1");
        assert_eq!(encoded[3], 0xc4, "Method name should use binary format");
        assert_eq!(encoded[4], 0x04, "Method name length should be 4");
        assert_eq!(&encoded[5..9], b"echo");
        assert_eq!(encoded[9], 0xc4, "Payload should use binary format");
        assert_eq!(encoded[10], 0x06, "Payload length should be 6 for \"test\"");
        assert_eq!(&encoded[11..17], b"\"test\"");
    }

    #[test]
    fn test_client_options_default() {
        let options = ClientOptions::default();
        assert_eq!(options.tsgo_path, "tsgo");
        assert!(options.cwd.is_none());
        assert!(options.log_file.is_none());
    }

    #[test]
    fn test_message_type_serialization() {
        let msg_type = MessageType::Request;
        let serialized = to_vec(&msg_type).unwrap();
        let deserialized: MessageType = from_slice(&serialized).unwrap();
        assert_eq!(msg_type, deserialized);
    }
}
