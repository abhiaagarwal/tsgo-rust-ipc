use std::{collections::HashMap, sync::Arc};

use dashmap::DashMap;

use crate::{FileSystemEntries, Result, VirtualFileSystem};
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

        if components.len() == 1 {
            // File at root level
            self.root
                .insert(file_name.clone(), VfsNode::File { content });
        } else {
            // File in nested directory - store with full path for now
            // This is a simplified approach that stores files at root level with full path as key
            let full_path = components.join("/");
            self.root.insert(full_path, VfsNode::File { content });
        }
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
    fn read_file(&self, path: &str) -> Result<Option<String>> {
        match self.get_node(path) {
            Some(VfsNode::File { content }) => Ok(Some(content)),
            Some(VfsNode::Directory { .. }) => Ok(None), // Path exists but is a directory
            None => Ok(None),                            // File doesn't exist
        }
    }

    fn write_file(&self, path: &str, content: &str) -> Result<()> {
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

    fn get_accessible_entries(&self, path: &str) -> Result<Option<FileSystemEntries>> {
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

        // For non-root directories, we need to find files that start with this path
        let search_prefix = normalized.trim_start_matches('/').to_string() + "/";
        let mut files = Vec::new();
        let mut directories = std::collections::HashSet::new();

        for entry in self.root.iter() {
            let (key, node) = (entry.key(), entry.value());

            if key.starts_with(&search_prefix) {
                let remaining_path = &key[search_prefix.len()..];

                // If there are no more slashes, it's a direct child file
                if !remaining_path.contains('/') {
                    if let VfsNode::File { .. } = node {
                        files.push(remaining_path.to_string());
                    }
                } else {
                    // If there are slashes, the first part is a subdirectory
                    let subdir_name = remaining_path.split('/').next().unwrap();
                    directories.insert(subdir_name.to_string());
                }
            }
        }

        // Also check if there are explicit directory entries
        for entry in self.root.iter() {
            let (key, node) = (entry.key(), entry.value());

            if key.starts_with(&search_prefix) && !key[search_prefix.len()..].contains('/') {
                if let VfsNode::Directory { .. } = node {
                    directories.insert(key[search_prefix.len()..].to_string());
                }
            }
        }

        let directories: Vec<String> = directories.into_iter().collect();

        if files.is_empty() && directories.is_empty() {
            // Check if the directory itself exists
            if self.directory_exists(path) {
                Ok(Some(FileSystemEntries { files, directories }))
            } else {
                Ok(None)
            }
        } else {
            Ok(Some(FileSystemEntries { files, directories }))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn test_memory_fs_basic_operations() {
        let fs = MemoryFileSystem::new();

        // Test file doesn't exist initially
        assert!(!fs.file_exists("test.txt"));
        assert_eq!(fs.read_file("test.txt").unwrap(), None);

        // Add a file
        fs.add_file("test.txt", "Hello, world!");

        // Test file exists and can be read
        assert!(fs.file_exists("test.txt"));
        assert_eq!(
            fs.read_file("test.txt").unwrap(),
            Some("Hello, world!".to_string())
        );

        // Test write_file
        fs.write_file("test2.txt", "Another file").unwrap();
        assert!(fs.file_exists("test2.txt"));
        assert_eq!(
            fs.read_file("test2.txt").unwrap(),
            Some("Another file".to_string())
        );
    }

    #[test]
    fn test_memory_fs_from_files() {
        let mut files = HashMap::new();
        files.insert("file1.txt", "Content 1");
        files.insert("dir/file2.txt", "Content 2");
        files.insert("dir/subdir/file3.txt", "Content 3");

        let fs = MemoryFileSystem::from_files(files);

        assert!(fs.file_exists("file1.txt"));
        assert!(fs.file_exists("dir/file2.txt"));
        assert!(fs.file_exists("dir/subdir/file3.txt"));

        assert_eq!(
            fs.read_file("file1.txt").unwrap(),
            Some("Content 1".to_string())
        );
        assert_eq!(
            fs.read_file("dir/file2.txt").unwrap(),
            Some("Content 2".to_string())
        );
        assert_eq!(
            fs.read_file("dir/subdir/file3.txt").unwrap(),
            Some("Content 3".to_string())
        );
    }

    #[test]
    fn test_memory_fs_directories() {
        let fs = MemoryFileSystem::new();

        fs.add_directory("mydir");
        assert!(fs.directory_exists("mydir"));
        assert!(!fs.file_exists("mydir"));

        fs.add_file("mydir/file.txt", "content");
        assert!(fs.file_exists("mydir/file.txt"));
    }

    #[test]
    fn test_memory_fs_realpath() {
        let fs = MemoryFileSystem::new();

        assert_eq!(fs.realpath("/"), "/");
        assert_eq!(fs.realpath("file.txt"), "/file.txt");
        assert_eq!(fs.realpath("/dir/../file.txt"), "/file.txt");
        assert_eq!(fs.realpath("./dir/./file.txt"), "/dir/file.txt");
    }
}
