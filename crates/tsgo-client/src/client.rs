//! High-level Tsgo Client

use std::sync::Arc;

use serde::Serialize;
use serde_json::{Value, json};
use tsgo_transport::{TransportError, TsgoTransport};
use tsgo_vfs::VirtualFileSystem;

use crate::errors::{ClientError, Result};

/// Options for [`Client::new`].  Mirrors the TypeScript `APIOptions` interface.
pub struct ClientOptions<'t> {
    /// Path to the `tsgo` binary.  Defaults to `"tsgo"` (must be in `$PATH`).
    pub tsgo_path: String,
    /// Working directory for the spawned process (`--cwd`).
    pub cwd: Option<String>,
    /// Optional server-side log file path.
    pub log_file: Option<String>,
    /// Optional virtual file system whose callbacks will be exposed to the
    /// tsgo server.
    pub fs: Option<Arc<dyn VirtualFileSystem + Send + Sync + 't>>,
}

impl<'t> Default for ClientOptions<'t> {
    fn default() -> Self {
        Self {
            tsgo_path: "tsgo".into(),
            cwd: None,
            log_file: None,
            fs: None,
        }
    }
}

/// High-level asynchronous client that speaks the tsgo IPC protocol.
pub struct Client<'t> {
    transport: TsgoTransport<'t, ClientError>,
    fs: Option<Arc<dyn VirtualFileSystem + Send + Sync + 't>>,
}

impl<'t> Client<'t> {
    /// Spawn a new `tsgo` process and establish the transport channel.
    pub async fn new(options: ClientOptions<'t>) -> Result<Self> {
        let transport = TsgoTransport::new(&options.tsgo_path, options.cwd.as_deref()).await?;

        let mut client = Self {
            transport,
            fs: options.fs,
        };

        let callback_names = if let Some(ref fs_arc) = client.fs {
            Self::register_fs_callbacks(&mut client.transport, Arc::clone(fs_arc));
            vec![
                "readFile".to_string(),
                "fileExists".to_string(),
                "directoryExists".to_string(),
                "realpath".to_string(),
                "getAccessibleEntries".to_string(),
            ]
        } else {
            vec![]
        };

        client
            .transport
            .configure(options.log_file.as_deref(), &callback_names)
            .await?;

        Ok(client)
    }

    fn register_fs_callbacks(
        transport: &mut TsgoTransport<'t, ClientError>,
        fs: Arc<dyn VirtualFileSystem + Send + Sync + 't>,
    ) {
        let fs_read = Arc::clone(&fs);
        transport.register_async_callback("readFile".into(), move |args| {
            let fs_inner = Arc::clone(&fs_read);
            let path_res: std::result::Result<String, _> = args
                .as_str()
                .ok_or_else(|| {
                    ClientError::Transport(TransportError::CallbackExecutionFailed {
                        method: "readFile".into(),
                        reason: "Expected string argument for path".into(),
                    })
                })
                .map(|s| s.to_owned());

            Box::pin(async move {
                let path = path_res?;
                let result = fs_inner.read_file(&path).await?;
                Ok(match result {
                    Some(content) => Value::String(content),
                    None => Value::Null,
                })
            })
        });

        let fs_clone = Arc::clone(&fs);
        transport.register_callback("fileExists".into(), move |args| {
            let path = args
                .as_str()
                .ok_or_else(|| TransportError::CallbackExecutionFailed {
                    method: "fileExists".into(),
                    reason: "Expected string argument for path".into(),
                })?;
            Ok(Value::Bool(fs_clone.file_exists(path)))
        });

        let fs_clone = Arc::clone(&fs);
        transport.register_callback("directoryExists".into(), move |args| {
            let path = args
                .as_str()
                .ok_or_else(|| TransportError::CallbackExecutionFailed {
                    method: "directoryExists".into(),
                    reason: "Expected string argument for path".into(),
                })?;
            Ok(Value::Bool(fs_clone.directory_exists(path)))
        });

        let fs_clone = Arc::clone(&fs);
        transport.register_callback("realpath".into(), move |args| {
            let path = args
                .as_str()
                .ok_or_else(|| TransportError::CallbackExecutionFailed {
                    method: "realpath".into(),
                    reason: "Expected string argument for path".into(),
                })?;
            let real = fs_clone.realpath(path);
            Ok(Value::String(real))
        });

        let fs_entries = Arc::clone(&fs);
        transport.register_async_callback("getAccessibleEntries".into(), move |args| {
            let fs_inner = Arc::clone(&fs_entries);
            let path_res: std::result::Result<String, _> = args
                .as_str()
                .ok_or_else(|| TransportError::CallbackExecutionFailed {
                    method: "getAccessibleEntries".into(),
                    reason: "Expected string argument for path".into(),
                })
                .map(|s| s.to_owned());

            Box::pin(async move {
                let path = path_res?;
                let result = fs_inner.get_accessible_entries(&path).await?;
                Ok(match result {
                    Some(entries) => serde_json::to_value(entries)?,
                    None => Value::Null,
                })
            })
        });
    }

    /// Send an arbitrary JSON request and deserialize the JSON response.
    pub async fn request<P, R>(&mut self, method: &str, payload: P) -> Result<R>
    where
        P: Serialize,
        R: serde::de::DeserializeOwned,
    {
        let value = serde_json::to_value(payload)?;
        let response_value = self.transport.request(method, value).await?;
        let response: R = serde_json::from_value(response_value)?;
        Ok(response)
    }

    /// Convenience helper returning untyped JSON [`Value`].
    pub async fn request_value<P>(&mut self, method: &str, payload: P) -> Result<Value>
    where
        P: Serialize,
    {
        let value = serde_json::to_value(payload)?;
        let response = self.transport.request(method, value).await?;
        Ok(response)
    }

    /// Simple text round-trip (useful for latency measurements / smoke tests).
    pub async fn echo(&mut self, message: &str) -> Result<String> {
        let val = json!(message);
        let response = self.transport.request("echo", val).await?;
        let response: String = serde_json::from_value(response)?;
        Ok(response)
    }

    /// Binary echo variant.  The server treats the request payload and response
    /// as opaque byte vectors.
    pub async fn echo_binary(&mut self, data: Vec<u8>) -> Result<Vec<u8>> {
        let response = self.transport.request_binary("echo", data).await?;
        Ok(response)
    }

    /// Gracefully terminate the underlying process.
    pub async fn close(self) -> Result<()> {
        self.transport.close().await?;
        Ok(())
    }
}
