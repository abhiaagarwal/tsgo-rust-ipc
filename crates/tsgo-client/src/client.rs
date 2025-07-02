//! High-level tsgo client that spawns a tsgo process in a background thread and
//! provides a synchronous interface for sending requests and receiving responses.

use std::sync::Arc;

use crossbeam_channel::{Receiver, Sender, bounded, unbounded};
use serde::Serialize;
use serde_json::{Value, json};
use tsgo_transport::{TransportError, TsgoTransport};
use tsgo_vfs::VirtualFileSystem;

use crate::{
    errors::{ClientError, Result},
    proto::{ConfigResponse, ProjectResponse},
    registry::{ObjectRegistry, Project},
};

/// Options for [`Client::new`].  Mirrors the TypeScript `APIOptions` interface.
pub struct ClientOptions {
    /// Path to the `tsgo` binary.  Defaults to `"tsgo"` (must be in `$PATH`).
    pub tsgo_path: String,
    /// Working directory for the spawned process (`--cwd`).
    pub cwd: Option<String>,
    /// Optional server-side log file path.
    pub log_file: Option<String>,
    /// Optional virtual file system whose callbacks will be exposed to the
    /// tsgo server.
    pub fs: Option<Arc<dyn VirtualFileSystem + Send + Sync + 'static>>,
}

impl Default for ClientOptions {
    fn default() -> Self {
        Self {
            tsgo_path: "tsgo".into(),
            cwd: None,
            log_file: None,
            fs: None,
        }
    }
}

/// Internal commands sent to the background worker thread.
enum Command {
    Json {
        method: String,
        payload: Value,
        resp_tx: Sender<Result<Value>>,
    },
    Binary {
        method: String,
        data: Vec<u8>,
        resp_tx: Sender<Result<Vec<u8>>>,
    },
    Shutdown,
}

/// Cloneable handle that can be shared freely across threads/tasks.
pub(crate) struct TransportClient {
    tx: Sender<Command>,
    // hold join handle so that thread is joined when Client is dropped
    _worker: std::thread::JoinHandle<()>,
}

impl TransportClient {
    /// Spawn the `tsgo` process in a background thread and return a handle.
    pub fn new(options: ClientOptions) -> Result<Self> {
        let mut transport = TsgoTransport::new(&options.tsgo_path, options.cwd.as_deref())?;

        if let Some(ref fs) = options.fs {
            Self::register_fs_callbacks(&mut transport, Arc::clone(fs));
        }

        let callback_names = if options.fs.is_some() {
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

        transport.configure(options.log_file.as_deref(), &callback_names)?;

        let (tx, rx): (Sender<Command>, Receiver<Command>) = unbounded();

        let worker = std::thread::spawn(move || worker_loop(transport, rx));

        Ok(Self {
            tx,
            _worker: worker,
        })
    }

    /// Send JSON request and deserialize response.
    pub fn request<P, R>(&self, method: &str, payload: P) -> Result<R>
    where
        P: Serialize,
        R: serde::de::DeserializeOwned,
    {
        let value = serde_json::to_value(payload)?;
        let (resp_tx, resp_rx) = bounded(1);
        self.tx
            .send(Command::Json {
                method: method.to_string(),
                payload: value,
                resp_tx,
            })
            .map_err(|_| TransportError::TransportConnectionClosed)?;

        let inner = resp_rx
            .recv()
            .map_err(|_| ClientError::Transport(TransportError::TransportConnectionClosed))?;
        let raw = inner?;
        let deserialized = serde_json::from_value(raw)?;
        Ok(deserialized)
    }

    /// Send a request that expects binary data back.
    #[allow(dead_code)]
    pub fn request_binary<P>(&self, method: &str, payload: P) -> Result<Vec<u8>>
    where
        P: Serialize,
    {
        let value = serde_json::to_string(&payload)?;
        let (resp_tx, resp_rx) = bounded(1);
        self.tx
            .send(Command::Binary {
                method: method.to_string(),
                data: value.into_bytes(),
                resp_tx,
            })
            .map_err(|_| TransportError::TransportConnectionClosed)?;

        resp_rx
            .recv()
            .map_err(|_| ClientError::Transport(TransportError::TransportConnectionClosed))?
    }

    /// Text echo helper.
    pub fn echo(&self, message: &str) -> Result<String> {
        let val = json!(message);
        let raw: Value = self.request("echo", val)?;
        Ok(serde_json::from_value(raw)?)
    }

    /// Binary echo helper.
    pub fn echo_binary(&self, data: Vec<u8>) -> Result<Vec<u8>> {
        let (resp_tx, resp_rx) = bounded(1);
        self.tx
            .send(Command::Binary {
                method: "echo".into(),
                data,
                resp_tx,
            })
            .map_err(|_| TransportError::TransportConnectionClosed)?;

        resp_rx
            .recv()
            .map_err(|_| ClientError::Transport(TransportError::TransportConnectionClosed))?
    }

    /// Clean shutdown, blocks until the worker thread exits.
    pub fn close(self) -> Result<()> {
        // ignore send error, worker may already have exited
        let _ = self.tx.send(Command::Shutdown);
        Ok(())
    }

    fn register_fs_callbacks(
        transport: &mut TsgoTransport<'static, ClientError>,
        fs: Arc<dyn VirtualFileSystem + Send + Sync + 'static>,
    ) {
        let fs_clone = Arc::clone(&fs);
        transport.register_callback("readFile".into(), move |args| {
            let path = args
                .as_str()
                .ok_or_else(|| TransportError::CallbackExecutionFailed {
                    method: "readFile".into(),
                    reason: "Expected string argument for path".into(),
                })?;
            let result = fs_clone.read_file(path)?;
            Ok(match result {
                Some(c) => Value::String(c),
                None => Value::Null,
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
            Ok(Value::String(fs_clone.realpath(path)))
        });

        let fs_entries = Arc::clone(&fs);
        transport.register_callback("getAccessibleEntries".into(), move |args| {
            let path = args
                .as_str()
                .ok_or_else(|| TransportError::CallbackExecutionFailed {
                    method: "getAccessibleEntries".into(),
                    reason: "Expected string argument for path".into(),
                })?;
            let result = fs_entries.get_accessible_entries(path)?;
            Ok(match result {
                Some(entries) => serde_json::to_value(entries)?,
                None => Value::Null,
            })
        });
    }
}

fn worker_loop(mut transport: TsgoTransport<'static, ClientError>, rx: Receiver<Command>) {
    for cmd in rx.iter() {
        match cmd {
            Command::Json {
                method,
                payload,
                resp_tx,
            } => {
                let result = transport
                    .request(&method, payload)
                    .map_err(ClientError::from);
                // ignore send failure, caller may have been dropped
                let _ = resp_tx.send(result);
            }
            Command::Binary {
                method,
                data,
                resp_tx,
            } => {
                let result = transport
                    .request_binary(&method, data)
                    .map_err(ClientError::from);
                let _ = resp_tx.send(result);
            }
            Command::Shutdown => {
                let _ = transport.close();
                break;
            }
        }
    }
}

/// Public entry-point mirroring the TypeScript `API` class.
pub struct Client {
    inner: Arc<TransportClient>,
    registry: Arc<ObjectRegistry>,
}

impl Client {
    /// Spawn a `tsgo` child process and return a ready-to-use API client.
    pub fn new(options: ClientOptions) -> Result<Self> {
        let inner = Arc::new(TransportClient::new(options)?);
        let registry = Arc::new(ObjectRegistry::new(Arc::clone(&inner)));
        Ok(Self { inner, registry })
    }

    /// Parse a `tsconfig.json` (or similar) and return compiler options + root files.
    pub fn parse_config_file(&self, file_name: &str) -> Result<ConfigResponse> {
        let payload = json!({ "fileName": file_name });
        self.inner.request("parseConfigFile", payload)
    }

    /// Load a project
    pub fn load_project(&self, config_file_name: &str) -> Result<Arc<Project>> {
        let payload = json!({ "configFileName": config_file_name });
        let resp: ProjectResponse = self.inner.request("loadProject", payload)?;
        Ok(self.registry.get_project_from_response(resp))
    }

    /// String echo (debug / round-trip latency measurement).
    pub fn echo(&self, msg: &str) -> Result<String> {
        self.inner.echo(msg)
    }

    /// Binary echo (round-trip for raw bytes).
    pub fn echo_binary(&self, data: Vec<u8>) -> Result<Vec<u8>> {
        self.inner.echo_binary(data)
    }

    /// Graceful shutdown.
    pub fn close(self) -> Result<()> {
        match Arc::try_unwrap(self.inner) {
            Ok(inner) => inner.close(),
            Err(_) => Ok(()),
        }
    }
}
