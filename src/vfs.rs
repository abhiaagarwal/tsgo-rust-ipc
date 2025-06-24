use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use tokio::fs;

use crate::errors::{Result, TsgoError};

/// Represents the files and directories in a directory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSystemEntries {
    pub files: Vec<String>,
    pub directories: Vec<String>,
}

/// Trait for virtual file system implementations
///
/// This trait provides the interface that tsgo expects for file system operations.
/// All methods that tsgo may call back into are represented here.
#[trait_variant::make(SendVirtualFileSystem: Send + Sync)]
#[dynosaur::dynosaur(pub DynVirtualFileSystem = dyn VirtualFileSystem)]
#[dynosaur::dynosaur(pub DynSendVirtualFileSystem = dyn SendVirtualFileSystem)]
pub trait VirtualFileSystem {
    /// Read the contents of a file
    /// Returns None if the file doesn't exist, Some(content) if it does
    async fn read_file(&self, path: &str) -> Result<Option<String>>;

    /// Write content to a file (optional operation)
    /// Some VFS implementations may be read-only
    async fn write_file(&self, path: &str, content: &str) -> Result<()>;

    /// Check if a file exists
    fn file_exists(&self, path: &str) -> bool;

    /// Check if a directory exists
    fn directory_exists(&self, path: &str) -> bool;

    /// Get the real path (canonical path) for a given path
    /// This should resolve symlinks and relative paths
    fn realpath(&self, path: &str) -> String;

    /// Get accessible entries (files and directories) in a directory
    /// Returns None if the directory doesn't exist or can't be read
    async fn get_accessible_entries(&self, path: &str) -> Result<Option<FileSystemEntries>>;
}

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
        let cwd = std::env::current_dir().map_err(|e| TsgoError::VfsError {
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

impl VirtualFileSystem for RealFileSystem {
    async fn read_file(&self, path: &str) -> Result<Option<String>> {
        let resolved_path = self.resolve_path(path);

        match fs::read_to_string(&resolved_path).await {
            Ok(content) => Ok(Some(content)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(TsgoError::VfsError {
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
                .map_err(|e| TsgoError::VfsError {
                    operation: "create_dir_all".to_string(),
                    path: parent.to_string_lossy().to_string(),
                    error_message: e.to_string(),
                })?;
        }

        fs::write(&resolved_path, content)
            .await
            .map_err(|e| TsgoError::VfsError {
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
                return Err(TsgoError::VfsError {
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
            .map_err(|e| TsgoError::VfsError {
                operation: "read_dir_entry".to_string(),
                path: path.to_string(),
                error_message: e.to_string(),
            })?
        {
            let file_name = entry.file_name().to_string_lossy().to_string();
            let file_type = entry.file_type().await.map_err(|e| TsgoError::VfsError {
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

/// Virtual node in the memory file system
#[derive(Debug, Clone)]
enum VfsNode {
    File { content: String },
    Directory { children: HashMap<String, VfsNode> },
}

/// In-memory file system implementation
///
/// This implementation stores all files and directories in memory,
/// making it ideal for testing and scenarios where you want to provide
/// a controlled file system environment.
#[derive(Debug)]
pub struct MemoryFileSystem {
    root: Arc<DashMap<String, VfsNode>>,
}

impl MemoryFileSystem {
    /// Create a new empty memory file system
    pub fn new() -> Self {
        Self {
            root: Arc::new(DashMap::new()),
        }
    }

    /// Create a memory file system from a map of file paths to contents
    pub fn from_files<S: AsRef<str>>(files: HashMap<S, S>) -> Self {
        let fs = Self::new();

        for (path, content) in files {
            let path = path.as_ref();
            let content = content.as_ref();

            // Create directory structure if needed
            let normalized_path = fs.normalize_path(path);
            fs.ensure_directory_structure(&normalized_path);

            // Add the file
            let components = fs.path_components(&normalized_path);
            fs.create_file_at_path(&components, content.to_string());
        }

        fs
    }

    /// Normalize a path (remove leading/trailing slashes, resolve . and ..)
    fn normalize_path(&self, path: &str) -> String {
        let mut components = Vec::new();

        for component in path.split('/') {
            match component {
                "" | "." => continue,
                ".." => {
                    components.pop();
                }
                c => components.push(c),
            }
        }

        if components.is_empty() {
            "/".to_string()
        } else {
            format!("/{}", components.join("/"))
        }
    }

    /// Split a path into components
    fn path_components(&self, path: &str) -> Vec<String> {
        if path == "/" {
            return vec![];
        }

        path.trim_start_matches('/')
            .split('/')
            .map(|s| s.to_string())
            .collect()
    }

    /// Ensure all parent directories exist for a given path
    fn ensure_directory_structure(&self, path: &str) {
        let components = self.path_components(path);
        if components.is_empty() {
            return;
        }

        // Remove the last component (the file name) to get the directory path
        let dir_components = &components[..components.len().saturating_sub(1)];
        self.ensure_directory_at_path(dir_components);
    }

    /// Ensure a directory exists at the given path components
    fn ensure_directory_at_path(&self, components: &[String]) {
        if components.is_empty() {
            return;
        }

        let current_map = &self.root;

        for component in components {
            if !current_map.contains_key(component) {
                current_map.insert(
                    component.clone(),
                    VfsNode::Directory {
                        children: HashMap::new(),
                    },
                );
            }
        }
    }

    /// Create a file at the given path components
    fn create_file_at_path(&self, components: &[String], content: String) {
        if components.is_empty() {
            return;
        }

        let file_name = &components[components.len() - 1];

        // For simplicity, we'll store files at the root level with their full path as key
        let full_path = if components.len() == 1 {
            components[0].clone()
        } else {
            components.join("/")
        };

        self.root.insert(full_path, VfsNode::File { content });
    }

    /// Get a node at the given path
    fn get_node(&self, path: &str) -> Option<VfsNode> {
        let normalized = self.normalize_path(path);
        let key = if normalized == "/" {
            return Some(VfsNode::Directory {
                children: HashMap::new(),
            });
        } else {
            normalized.trim_start_matches('/').to_string()
        };

        self.root.get(&key).map(|entry| entry.value().clone())
    }

    /// Add a file to the memory file system
    pub fn add_file<P: AsRef<str>, C: AsRef<str>>(&self, path: P, content: C) {
        let path = path.as_ref();
        let content = content.as_ref();

        let normalized_path = self.normalize_path(path);
        self.ensure_directory_structure(&normalized_path);

        let components = self.path_components(&normalized_path);
        self.create_file_at_path(&components, content.to_string());
    }

    /// Add a directory to the memory file system
    pub fn add_directory<P: AsRef<str>>(&self, path: P) {
        let path = path.as_ref();
        let normalized_path = self.normalize_path(path);
        let components = self.path_components(&normalized_path);
        self.ensure_directory_at_path(&components);
    }
}

impl Default for MemoryFileSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl VirtualFileSystem for MemoryFileSystem {
    async fn read_file(&self, path: &str) -> Result<Option<String>> {
        match self.get_node(path) {
            Some(VfsNode::File { content }) => Ok(Some(content)),
            Some(VfsNode::Directory { .. }) => Ok(None), // Path exists but is a directory
            None => Ok(None),                            // File doesn't exist
        }
    }

    async fn write_file(&self, path: &str, content: &str) -> Result<()> {
        self.add_file(path, content);
        Ok(())
    }

    fn file_exists(&self, path: &str) -> bool {
        matches!(self.get_node(path), Some(VfsNode::File { .. }))
    }

    fn directory_exists(&self, path: &str) -> bool {
        if path == "/" || path.is_empty() {
            return true;
        }
        matches!(self.get_node(path), Some(VfsNode::Directory { .. }))
    }

    fn realpath(&self, path: &str) -> String {
        self.normalize_path(path)
    }

    async fn get_accessible_entries(&self, path: &str) -> Result<Option<FileSystemEntries>> {
        let normalized = self.normalize_path(path);

        if normalized == "/" {
            // Root directory - list all top-level entries
            let mut files = Vec::new();
            let mut directories = Vec::new();

            for entry in self.root.iter() {
                let (key, node) = (entry.key(), entry.value());

                // Only include top-level entries (no slashes in the key)
                if !key.contains('/') {
                    match node {
                        VfsNode::File { .. } => files.push(key.clone()),
                        VfsNode::Directory { .. } => directories.push(key.clone()),
                    }
                }
            }

            return Ok(Some(FileSystemEntries { files, directories }));
        }

        match self.get_node(path) {
            Some(VfsNode::Directory { children }) => {
                let mut files = Vec::new();
                let mut directories = Vec::new();

                for (name, node) in children {
                    match node {
                        VfsNode::File { .. } => files.push(name),
                        VfsNode::Directory { .. } => directories.push(name),
                    }
                }

                Ok(Some(FileSystemEntries { files, directories }))
            }
            Some(VfsNode::File { .. }) => Ok(None), // Path exists but is a file
            None => Ok(None),                       // Directory doesn't exist
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[tokio::test]
    async fn test_memory_fs_basic_operations() {
        let fs = MemoryFileSystem::new();

        // Test file doesn't exist initially
        assert!(!fs.file_exists("test.txt"));
        assert_eq!(fs.read_file("test.txt").await.unwrap(), None);

        // Add a file
        fs.add_file("test.txt", "Hello, world!");

        // Test file exists and can be read
        assert!(fs.file_exists("test.txt"));
        assert_eq!(
            fs.read_file("test.txt").await.unwrap(),
            Some("Hello, world!".to_string())
        );

        // Test write_file
        fs.write_file("test2.txt", "Another file").await.unwrap();
        assert!(fs.file_exists("test2.txt"));
        assert_eq!(
            fs.read_file("test2.txt").await.unwrap(),
            Some("Another file".to_string())
        );
    }

    #[tokio::test]
    async fn test_memory_fs_from_files() {
        let mut files = HashMap::new();
        files.insert("file1.txt", "Content 1");
        files.insert("dir/file2.txt", "Content 2");
        files.insert("dir/subdir/file3.txt", "Content 3");

        let fs = MemoryFileSystem::from_files(files);

        assert!(fs.file_exists("file1.txt"));
        assert!(fs.file_exists("dir/file2.txt"));
        assert!(fs.file_exists("dir/subdir/file3.txt"));

        assert_eq!(
            fs.read_file("file1.txt").await.unwrap(),
            Some("Content 1".to_string())
        );
        assert_eq!(
            fs.read_file("dir/file2.txt").await.unwrap(),
            Some("Content 2".to_string())
        );
        assert_eq!(
            fs.read_file("dir/subdir/file3.txt").await.unwrap(),
            Some("Content 3".to_string())
        );
    }

    #[tokio::test]
    async fn test_memory_fs_directories() {
        let fs = MemoryFileSystem::new();

        fs.add_directory("mydir");
        assert!(fs.directory_exists("mydir"));
        assert!(!fs.file_exists("mydir"));

        fs.add_file("mydir/file.txt", "content");
        assert!(fs.file_exists("mydir/file.txt"));
    }

    #[tokio::test]
    async fn test_memory_fs_realpath() {
        let fs = MemoryFileSystem::new();

        assert_eq!(fs.realpath("/"), "/");
        assert_eq!(fs.realpath("file.txt"), "/file.txt");
        assert_eq!(fs.realpath("/dir/../file.txt"), "/file.txt");
        assert_eq!(fs.realpath("./dir/./file.txt"), "/dir/file.txt");
    }

    #[test]
    fn test_real_fs_creation() {
        let fs = RealFileSystem::new("/tmp");
        assert_eq!(fs.base_dir, PathBuf::from("/tmp"));

        let fs_cwd = RealFileSystem::with_cwd().unwrap();
        assert_eq!(fs_cwd.base_dir, std::env::current_dir().unwrap());
    }

    #[test]
    fn test_real_fs_path_resolution() {
        let fs = RealFileSystem::new("/home/user");

        // Absolute paths should not be modified
        assert_eq!(
            fs.resolve_path("/absolute/path"),
            PathBuf::from("/absolute/path")
        );

        // Relative paths should be resolved relative to base_dir
        assert_eq!(
            fs.resolve_path("relative/path"),
            PathBuf::from("/home/user/relative/path")
        );
    }
}
