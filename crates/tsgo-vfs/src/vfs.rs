use serde::{Deserialize, Serialize};

use crate::Result;

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
pub trait VirtualFileSystem {
    /// Read the contents of a file
    /// Returns None if the file doesn't exist, Some(content) if it does
    fn read_file(&self, path: &str) -> Result<Option<String>>;

    /// Write content to a file (optional operation)
    /// Some VFS implementations may be read-only
    fn write_file(&self, path: &str, content: &str) -> Result<()>;

    /// Check if a file exists
    fn file_exists(&self, path: &str) -> bool;

    /// Check if a directory exists
    fn directory_exists(&self, path: &str) -> bool;

    /// Get the real path (canonical path) for a given path
    /// This should resolve symlinks and relative paths
    fn realpath(&self, path: &str) -> String;

    /// Get accessible entries (files and directories) in a directory
    /// Returns None if the directory doesn't exist or can't be read
    fn get_accessible_entries(&self, path: &str) -> Result<Option<FileSystemEntries>>;
}
