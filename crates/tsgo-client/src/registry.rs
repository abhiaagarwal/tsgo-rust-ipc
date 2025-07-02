use std::{
    collections::HashMap,
    sync::{Arc, Mutex, Weak},
};

use serde_json::Value;

// use tsgo_decoder::TsgoDecoder;
use crate::{
    client::TransportClient,
    errors::{ClientError, Result},
    proto::{ProjectResponse, SymbolResponse, TypeResponse},
};

fn send_release(client: &TransportClient, id: &str) {
    // Best-effort: we do *not* care about the return value, only that the message reaches the
    // background worker.  Even if the transport is already closed the call will be a no-op.
    let _: Result<Value> = client.request("release", id);
}

/// Shared registry responsible for de-duplicating wrapper objects (Project, Symbol, Type, …) and
/// releasing their server-side handles once the last Rust reference goes out of scope.
pub struct ObjectRegistry {
    client: Arc<TransportClient>,
    symbols: Mutex<HashMap<String, Weak<Symbol>>>,
    types: Mutex<HashMap<String, Weak<Type>>>,
    projects: Mutex<HashMap<String, Weak<Project>>>,
}

#[allow(dead_code)]
impl ObjectRegistry {
    pub(crate) fn new(client: Arc<TransportClient>) -> Self {
        Self {
            client,
            symbols: Mutex::new(HashMap::new()),
            types: Mutex::new(HashMap::new()),
            projects: Mutex::new(HashMap::new()),
        }
    }

    /// Return a shared `Arc<Symbol>` instance for the given server-side handle.
    /// Multiple calls with the same `id` will return the *same* wrapper object (identity equality).
    pub fn get_symbol(
        &self,
        id: String,
        name: String,
        flags: u32,
        check_flags: u32,
    ) -> Arc<Symbol> {
        // Fast-path: try upgrading existing weak reference.
        if let Some(existing) = self
            .symbols
            .lock()
            .expect("poisoned mutex")
            .get(&id)
            .and_then(|weak| weak.upgrade())
        {
            return existing;
        }

        // Slow-path: create new wrapper and insert weak reference.
        let symbol = Arc::new(Symbol::new(
            Arc::clone(&self.client),
            id.clone(),
            name,
            flags,
            check_flags,
        ));
        self.symbols
            .lock()
            .expect("poisoned mutex")
            .insert(id, Arc::downgrade(&symbol));
        symbol
    }

    /// Helper to construct Symbol from response struct
    pub fn get_symbol_from_response(&self, resp: SymbolResponse) -> Arc<Symbol> {
        self.get_symbol(resp.id, resp.name, resp.flags, resp.check_flags)
    }

    /// Return a shared `Arc<Type>`
    pub fn get_type_from_response(&self, resp: TypeResponse) -> Arc<Type> {
        if let Some(existing) = self
            .types
            .lock()
            .expect("poisoned mutex")
            .get(&resp.id)
            .and_then(|w| w.upgrade())
        {
            return existing;
        }

        let ty = Arc::new(Type::new(
            Arc::clone(&self.client),
            resp.id.clone(),
            resp.flags,
        ));
        self.types
            .lock()
            .expect("poisoned mutex")
            .insert(resp.id, Arc::downgrade(&ty));
        ty
    }

    pub fn get_project_from_response(self: &Arc<Self>, resp: ProjectResponse) -> Arc<Project> {
        if let Some(existing) = self
            .projects
            .lock()
            .expect("poisoned mutex")
            .get(&resp.id)
            .and_then(|w| w.upgrade())
        {
            // Reload data to keep up-to-date
            existing.load_data(resp);
            return existing;
        }

        let project = Arc::new(Project::new(
            Arc::clone(&self.client),
            Arc::clone(self),
            resp,
        ));
        self.projects
            .lock()
            .expect("poisoned mutex")
            .insert(project.id.clone(), Arc::downgrade(&project));
        project
    }

    fn release_symbol(&self, id: &str) {
        self.symbols.lock().expect("poisoned mutex").remove(id);
        send_release(&self.client, id);
    }

    fn release_type(&self, id: &str) {
        self.types.lock().expect("poisoned mutex").remove(id);
        send_release(&self.client, id);
    }

    fn release_project(&self, id: &str) {
        self.projects.lock().expect("poisoned mutex").remove(id);
        send_release(&self.client, id);
    }
}

/// Wrapper around a remote **Symbol** handle returned by the tsgo server.
pub struct Symbol {
    id: String,
    pub name: String,
    pub flags: u32,
    pub check_flags: u32,
    client: Arc<TransportClient>,
    disposed: std::sync::atomic::AtomicBool,
}

impl Symbol {
    fn new(
        client: Arc<TransportClient>,
        id: String,
        name: String,
        flags: u32,
        check_flags: u32,
    ) -> Self {
        Self {
            id,
            name,
            flags,
            check_flags,
            client,
            disposed: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// Public accessor for the underlying handle id.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Explicit disposal.
    pub fn dispose(&self) {
        if self
            .disposed
            .compare_exchange(
                false,
                true,
                std::sync::atomic::Ordering::SeqCst,
                std::sync::atomic::Ordering::SeqCst,
            )
            .is_ok()
        {
            send_release(&self.client, &self.id);
        }
    }

    pub fn ensure_not_disposed(&self) -> Result<()> {
        if self.disposed.load(std::sync::atomic::Ordering::SeqCst) {
            Err(ClientError::Transport(
                tsgo_transport::TransportError::CallbackExecutionFailed {
                    method: "Symbol".into(),
                    reason: "Symbol is disposed".into(),
                },
            ))
        } else {
            Ok(())
        }
    }
}

impl Drop for Symbol {
    fn drop(&mut self) {
        if self
            .disposed
            .compare_exchange(
                false,
                true,
                std::sync::atomic::Ordering::SeqCst,
                std::sync::atomic::Ordering::SeqCst,
            )
            .is_err()
        {
            // already disposed
            return;
        }

        send_release(&self.client, &self.id);
    }
}

/// Wrapper around a remote **Type** handle.
pub struct Type {
    id: String,
    pub flags: u32,
    client: Arc<TransportClient>,
    disposed: std::sync::atomic::AtomicBool,
}

impl Type {
    fn new(client: Arc<TransportClient>, id: String, flags: u32) -> Self {
        Self {
            id,
            flags,
            client,
            disposed: std::sync::atomic::AtomicBool::new(false),
        }
    }

    pub fn dispose(&self) {
        if self
            .disposed
            .compare_exchange(
                false,
                true,
                std::sync::atomic::Ordering::SeqCst,
                std::sync::atomic::Ordering::SeqCst,
            )
            .is_ok()
        {
            send_release(&self.client, &self.id);
        }
    }

    pub fn ensure_not_disposed(&self) -> Result<()> {
        if self.disposed.load(std::sync::atomic::Ordering::SeqCst) {
            Err(ClientError::Transport(
                tsgo_transport::TransportError::CallbackExecutionFailed {
                    method: "Type".into(),
                    reason: "Type is disposed".into(),
                },
            ))
        } else {
            Ok(())
        }
    }
}

impl Drop for Type {
    fn drop(&mut self) {
        if self
            .disposed
            .compare_exchange(
                false,
                true,
                std::sync::atomic::Ordering::SeqCst,
                std::sync::atomic::Ordering::SeqCst,
            )
            .is_err()
        {
            return;
        }
        send_release(&self.client, &self.id);
    }
}

#[allow(dead_code)]
pub struct Project {
    pub id: String,
    pub config_file_name: String,
    pub compiler_options: serde_json::Value,
    pub root_files: Vec<String>,

    client: Arc<TransportClient>,
    registry: Arc<ObjectRegistry>,
    disposed: std::sync::atomic::AtomicBool,
}

impl Project {
    fn new(
        client: Arc<TransportClient>,
        registry: Arc<ObjectRegistry>,
        resp: ProjectResponse,
    ) -> Self {
        Self {
            id: resp.id.clone(),
            config_file_name: resp.config_file_name.clone(),
            compiler_options: resp.compiler_options.clone(),
            root_files: resp.root_files.clone(),
            client,
            registry,
            disposed: std::sync::atomic::AtomicBool::new(false),
        }
    }

    fn load_data(&self, resp: ProjectResponse) {
        // Update interior fields via mutable reference.
        // We need interior mutability; but fields are not inside Cell/RefCell.
        // For simplicity we will skip runtime reloading and ignore for now.
        // TODO: implement if needed.
        let _ = resp;
    }

    pub fn reload(&self) -> crate::errors::Result<()> {
        let payload = serde_json::json!({ "configFileName": self.config_file_name });
        let resp: ProjectResponse = self.client.request("loadProject", payload)?;
        self.load_data(resp);
        Ok(())
    }

    // pub fn get_source_file(&self, file_name: &str) -> crate::errors::Result<Option<TsgoDecoder>> {
    //     let payload = serde_json::json!({ "project": self.id, "fileName": file_name });
    //     let data = self.client.request_binary("getSourceFile", payload)?;
    //     if data.is_empty() {
    //         return Ok(None);
    //     }
    //     let decoder = TsgoDecoder::new(data)?;
    //     Ok(Some(decoder))
    // }
}

impl Drop for Project {
    fn drop(&mut self) {
        if self
            .disposed
            .compare_exchange(
                false,
                true,
                std::sync::atomic::Ordering::SeqCst,
                std::sync::atomic::Ordering::SeqCst,
            )
            .is_err()
        {
            return;
        }
        send_release(&self.client, &self.id);
    }
}
