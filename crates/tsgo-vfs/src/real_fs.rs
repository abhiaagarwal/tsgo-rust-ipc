use std::path::{Path, PathBuf};

use async_trait::async_trait;
use tokio::fs;

use crate::{FileSystemEntries, Result, VfsError, VirtualFileSystem};

/// Real file system implementation that delegates to the actual OS file system
#[derive(Debug)]
pub struct RealFileSystem {
    /// Base directory for relative path resolution
    base_dir: PathBuf,
}

impl RealFileSystem {
    /// Create a new real file system with the given base directory
    pub fn new<P: AsRef<Path>>(base_dir: P) -> Self {
        Self {
            base_dir: base_dir.as_ref().to_path_buf(),
        }
    }

    /// Create a new real file system using the current working directory
    pub fn with_cwd() -> Result<Self> {
        let cwd = std::env::current_dir().map_err(|e| VfsError::Operation {
            operation: "get_current_dir".to_string(),
            path: ".".to_string(),
            error_message: e.to_string(),
        })?;
        Ok(Self::new(cwd))
    }

    /// Resolve a path relative to the base directory
    fn resolve_path(&self, path: &str) -> PathBuf {
        if Path::new(path).is_absolute() {
            PathBuf::from(path)
        } else {
            self.base_dir.join(path)
        }
    }
}

#[async_trait]
impl VirtualFileSystem for RealFileSystem {
    async fn read_file(&self, path: &str) -> Result<Option<String>> {
        let resolved_path = self.resolve_path(path);

        match fs::read_to_string(&resolved_path).await {
            Ok(content) => Ok(Some(content)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(VfsError::Operation {
                operation: "read_file".to_string(),
                path: path.to_string(),
                error_message: e.to_string(),
            }),
        }
    }

    async fn write_file(&self, path: &str, content: &str) -> Result<()> {
        let resolved_path = self.resolve_path(path);

        // Create parent directories if they don't exist
        if let Some(parent) = resolved_path.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| VfsError::Operation {
                    operation: "create_dir_all".to_string(),
                    path: parent.to_string_lossy().to_string(),
                    error_message: e.to_string(),
                })?;
        }

        fs::write(&resolved_path, content)
            .await
            .map_err(|e| VfsError::Operation {
                operation: "write_file".to_string(),
                path: path.to_string(),
                error_message: e.to_string(),
            })
    }

    fn file_exists(&self, path: &str) -> bool {
        let resolved_path = self.resolve_path(path);
        resolved_path.is_file()
    }

    fn directory_exists(&self, path: &str) -> bool {
        let resolved_path = self.resolve_path(path);
        resolved_path.is_dir()
    }

    fn realpath(&self, path: &str) -> String {
        let resolved_path = self.resolve_path(path);

        // Try to canonicalize the path, fall back to the resolved path if it fails
        match resolved_path.canonicalize() {
            Ok(canonical) => canonical.to_string_lossy().to_string(),
            Err(_) => resolved_path.to_string_lossy().to_string(),
        }
    }

    async fn get_accessible_entries(&self, path: &str) -> Result<Option<FileSystemEntries>> {
        let resolved_path = self.resolve_path(path);

        let mut entries = match fs::read_dir(&resolved_path).await {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => {
                return Err(VfsError::Operation {
                    operation: "read_dir".to_string(),
                    path: path.to_string(),
                    error_message: e.to_string(),
                });
            }
        };

        let mut files = Vec::new();
        let mut directories = Vec::new();

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| VfsError::Operation {
                operation: "read_dir_entry".to_string(),
                path: path.to_string(),
                error_message: e.to_string(),
            })?
        {
            let file_name = entry.file_name().to_string_lossy().to_string();
            let file_type = entry.file_type().await.map_err(|e| VfsError::Operation {
                operation: "get_file_type".to_string(),
                path: entry.path().to_string_lossy().to_string(),
                error_message: e.to_string(),
            })?;

            if file_type.is_file() {
                files.push(file_name);
            } else if file_type.is_dir() {
                directories.push(file_name);
            }
        }

        Ok(Some(FileSystemEntries { files, directories }))
    }
}
