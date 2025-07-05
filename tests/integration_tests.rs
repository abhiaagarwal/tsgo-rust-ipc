#![allow(dead_code)]
use std::{collections::HashMap, env, path::Path, sync::Arc};

use assert_unordered::assert_eq_unordered;
use rstest::rstest;
use serde_json::json;
use tsgo_client::{
    client::{Client, ClientOptions},
    errors::Result,
};
use tsgo_vfs::{MemoryFileSystem, VirtualFileSystem};

/// Common test utilities for integration tests
pub mod common {
    use std::{fs, path::PathBuf};

    use super::*;

    pub fn get_tsgo_binary_path() -> Option<String> {
        if let Ok(path) = env::var("TSGO_PATH") {
            if Path::new(&path).exists() {
                return Some(path);
            }
        }

        let possible_paths = [
            "./tsgo/built/local/tsgo",
            "tsgo/built/local/tsgo",
            "../tsgo/built/local/tsgo",
            "tsgo",
            "./tsgo",
        ];

        for path in &possible_paths {
            if Path::new(path).exists() {
                return Some(path.to_string());
            }
        }

        None
    }

    /// Verify that tsgo binary is working by checking version
    pub fn verify_tsgo_binary(tsgo_path: &str) -> bool {
        std::process::Command::new(tsgo_path)
            .arg("--version")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    /// Parse a tsgo test case file and extract individual files and metadata
    pub fn parse_tsgo_test_case(content: &str) -> TestCaseData {
        let mut files = HashMap::new();
        let mut current_file = None;
        let mut current_content = String::new();
        let mut current_directory = None;
        let mut symlinks = HashMap::new();

        for line in content.lines() {
            let line = line.trim();

            if line.starts_with("// @filename:") {
                if let Some(filename) = current_file.take() {
                    files.insert(filename, current_content.clone());
                    current_content.clear();
                }

                let filename = line.strip_prefix("// @filename:").unwrap().trim();
                current_file = Some(filename.to_string());
            } else if line.starts_with("// @currentDirectory:") {
                current_directory = Some(
                    line.strip_prefix("// @currentDirectory:")
                        .unwrap()
                        .trim()
                        .to_string(),
                );
            } else if line.starts_with("// @link:") {
                let link_spec = line.strip_prefix("// @link:").unwrap().trim();
                if let Some((source, target)) = link_spec.split_once(" -> ") {
                    symlinks.insert(source.trim().to_string(), target.trim().to_string());
                }
            } else if (!line.starts_with("//") || line.starts_with("///")) && current_file.is_some()
            {
                current_content.push_str(line);
                current_content.push('\n');
            }
        }

        if let Some(filename) = current_file {
            files.insert(filename, current_content);
        }

        if files.is_empty() {
            files.insert("/main.ts".to_string(), content.to_string());
        }

        TestCaseData {
            files,
            current_directory,
            symlinks,
        }
    }

    /// Extract test name from file path
    pub fn extract_test_name(test_path: &Path) -> String {
        test_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string()
    }

    /// Find all tsgo test case files
    pub fn find_tsgo_test_cases() -> Result<Vec<PathBuf>> {
        let test_dirs = [
            "tsgo/testdata/tests/cases/compiler",
            "tsgo/testdata/tests/cases/conformance",
            "../tsgo/testdata/tests/cases/compiler",
            "../tsgo/testdata/tests/cases/conformance",
        ];

        let mut test_files = Vec::new();

        for test_dir in &test_dirs {
            if let Ok(entries) = fs::read_dir(test_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() && path.extension().is_some_and(|ext| ext == "ts") {
                        test_files.push(path);
                    }
                }
            }
        }

        Ok(test_files)
    }

    #[derive(Debug)]
    pub struct TestCaseData {
        pub files: HashMap<String, String>,
        pub current_directory: Option<String>,
        pub symlinks: HashMap<String, String>,
    }
}

/// Test helper to create a client with default test files
fn create_test_client() -> Result<Client> {
    let tsgo_path = common::get_tsgo_binary_path().expect("tsgo binary not found");

    let default_files = create_default_test_files();
    let vfs: Arc<dyn VirtualFileSystem + Send + Sync> =
        Arc::new(MemoryFileSystem::from_files(default_files));

    Client::new(ClientOptions {
        tsgo_path,
        cwd: Some(".".into()),
        log_file: None,
        fs: Some(vfs),
    })
}

/// Creates default test files similar to TypeScript API tests
fn create_default_test_files() -> HashMap<String, String> {
    let mut files = HashMap::new();
    files.insert("/tsconfig.json".to_string(), "{}".to_string());
    files.insert(
        "/src/index.ts".to_string(),
        "import { foo } from './foo';".to_string(),
    );
    files.insert(
        "/src/foo.ts".to_string(),
        "export const foo = 42;".to_string(),
    );
    files
}

#[cfg(test)]
mod api_tests {
    use super::*;

    #[test]
    fn test_parse_config_file() -> Result<()> {
        let client = create_test_client()?;
        let config = client.parse_config_file("/tsconfig.json")?;

        assert_eq_unordered!(
            config.file_names.clone(),
            vec!["/src/foo.ts".to_string(), "/src/index.ts".to_string()]
        );
        assert!(config.options.is_object());
        assert_eq!(
            config.options.get("configFilePath"),
            Some(&json!("/tsconfig.json"))
        );

        client.close()
    }

    #[test]
    fn test_load_project() -> Result<()> {
        let client = create_test_client()?;
        let project = client.load_project("/tsconfig.json")?;

        assert_eq!(project.config_file_name, "/tsconfig.json");
        assert!(project.compiler_options.is_object());
        assert_eq_unordered!(
            project.root_files.clone(),
            vec!["/src/foo.ts".to_string(), "/src/index.ts".to_string()]
        );

        client.close()
    }
}

#[cfg(test)]
mod project_tests {
    use super::*;

    #[test]
    fn test_get_symbol_at_position() -> Result<()> {
        let client = create_test_client()?;
        let project = client.load_project("/tsconfig.json")?;

        // TODO: Implement Project::get_symbol_at_position method
        // This should get the symbol at position 9 in "/src/index.ts" (the "foo" import)
        // let symbol = project.get_symbol_at_position("/src/index.ts", 9)?;
        // assert!(symbol.is_some());
        // let symbol = symbol.unwrap();
        // assert_eq!(symbol.name, "foo");
        // assert!(symbol.flags & 0x200000 != 0); // SymbolFlags::ALIAS

        println!("TODO: Implement Project::get_symbol_at_position method");
        client.close()
    }

    #[test]
    fn test_get_symbol_at_location() -> Result<()> {
        let client = create_test_client()?;
        let project = client.load_project("/tsconfig.json")?;

        // TODO: Implement Project::get_symbol_at_location method
        // This would require:
        // 1. Getting the source file AST
        // 2. Navigating to the import declaration
        // 3. Getting the symbol at that AST location
        // let source_file = project.get_source_file("/src/index.ts")?;
        // let import_decl = source_file.statements[0]; // ImportDeclaration
        // let named_binding = import_decl.import_clause.named_bindings; // NamedImports
        // let element = named_binding.elements[0]; // ImportSpecifier
        // let node = element.name; // Identifier
        // let symbol = project.get_symbol_at_location(&node)?;
        // assert!(symbol.is_some());
        // let symbol = symbol.unwrap();
        // assert_eq!(symbol.name, "foo");
        // assert!(symbol.flags & 0x200000 != 0); // SymbolFlags::ALIAS

        println!("TODO: Implement Project::get_symbol_at_location method");
        client.close()
    }

    #[test]
    fn test_get_type_of_symbol() -> Result<()> {
        let client = create_test_client()?;
        let project = client.load_project("/tsconfig.json")?;

        // TODO: Implement Project::get_type_of_symbol method
        // let symbol = project.get_symbol_at_position("/src/index.ts", 9)?;
        // assert!(symbol.is_some());
        // let symbol = symbol.unwrap();
        // let type_info = project.get_type_of_symbol(&symbol)?;
        // assert!(type_info.is_some());
        // let type_info = type_info.unwrap();
        // assert!(type_info.flags & TypeFlags::NUMBER_LITERAL.bits() != 0);

        println!("TODO: Implement Project::get_type_of_symbol method");
        client.close()
    }

    #[test]
    fn test_get_source_file() -> Result<()> {
        let client = create_test_client()?;
        let project = client.load_project("/tsconfig.json")?;

        // TODO: Uncomment and implement Project::get_source_file method
        // The method is currently commented out in the code
        // let source_file = project.get_source_file("/src/index.ts")?;
        // assert!(source_file.is_some());
        // let source_file = source_file.unwrap();
        // // TODO: Implement SourceFile wrapper with text, fileName properties
        // assert_eq!(source_file.text, "import { foo } from './foo';");
        // assert_eq!(source_file.file_name, "/src/index.ts");

        println!("TODO: Implement Project::get_source_file method");
        client.close()
    }
}

#[cfg(test)]
mod source_file_tests {
    use super::*;

    #[test]
    fn test_source_file_properties() -> Result<()> {
        let client = create_test_client()?;
        let project = client.load_project("/tsconfig.json")?;

        // TODO: Implement SourceFile wrapper with properties
        // let source_file = project.get_source_file("/src/index.ts")?;
        // assert!(source_file.is_some());
        // let source_file = source_file.unwrap();
        // assert_eq!(source_file.text, "import { foo } from './foo';");
        // assert_eq!(source_file.file_name, "/src/index.ts");

        println!("TODO: Implement SourceFile wrapper with text and fileName properties");
        client.close()
    }

    #[test]
    fn test_ast_traversal() -> Result<()> {
        let client = create_test_client()?;
        let project = client.load_project("/tsconfig.json")?;

        // TODO: Implement AST traversal methods
        // let source_file = project.get_source_file("/src/index.ts")?;
        // assert!(source_file.is_some());
        // let source_file = source_file.unwrap();
        //
        // let mut node_count = 1;
        // source_file.for_each_child(|node| {
        //     // TODO: Implement type guards for AST nodes
        //     // if is_template_head(node) {
        //     //     assert_eq!(node.text, "head ");
        //     //     assert_eq!(node.raw_text, "head ");
        //     //     assert_eq!(node.template_flags, 0);
        //     // } else if is_template_middle(node) {
        //     //     assert_eq!(node.text, "middle");
        //     //     assert_eq!(node.raw_text, "middle");
        //     //     assert_eq!(node.template_flags, 0);
        //     // } else if is_template_tail(node) {
        //     //     assert_eq!(node.text, " tail");
        //     //     assert_eq!(node.raw_text, " tail");
        //     //     assert_eq!(node.template_flags, 0);
        //     // }
        //     node_count += 1;
        //     node.for_each_child(|child| {
        //         // Recursive traversal
        //     });
        // });
        // assert_eq!(node_count, 8);

        println!("TODO: Implement AST traversal methods and type guards");
        client.close()
    }
}

#[cfg(test)]
mod object_lifecycle_tests {
    use super::*;

    #[test]
    fn test_object_equality() -> Result<()> {
        let client = create_test_client()?;
        let project1 = client.load_project("/tsconfig.json")?;
        let project2 = client.load_project("/tsconfig.json")?;

        // Test that the same project loaded twice returns the same Arc
        assert!(Arc::ptr_eq(&project1, &project2));

        // TODO: Test symbol equality
        // let symbol1 = project1.get_symbol_at_position("/src/index.ts", 9)?;
        // let symbol2 = project1.get_symbol_at_position("/src/index.ts", 10)?;
        // assert!(symbol1.is_some() && symbol2.is_some());
        // assert!(Arc::ptr_eq(&symbol1.unwrap(), &symbol2.unwrap()));

        println!("TODO: Test symbol equality once get_symbol_at_position is implemented");
        client.close()
    }

    #[test]
    fn test_symbol_disposal() -> Result<()> {
        let client = create_test_client()?;
        let project = client.load_project("/tsconfig.json")?;

        // TODO: Test symbol disposal
        // let symbol = project.get_symbol_at_position("/src/index.ts", 9)?;
        // assert!(symbol.is_some());
        // let symbol = symbol.unwrap();
        //
        // // Symbol should not be disposed initially
        // assert!(!symbol.is_disposed());
        //
        // // Dispose the symbol
        // symbol.dispose();
        // assert!(symbol.is_disposed());
        //
        // // Using disposed symbol should error
        // let result = project.get_type_of_symbol(&symbol);
        // assert!(result.is_err());
        // assert!(result.unwrap_err().to_string().contains("Symbol is disposed"));

        println!("TODO: Test symbol disposal once symbols are fully implemented");
        client.close()
    }

    #[test]
    fn test_automatic_cleanup() -> Result<()> {
        let client = create_test_client()?;
        let project = client.load_project("/tsconfig.json")?;

        // TODO: Test automatic cleanup when Arc is dropped
        // let symbol = project.get_symbol_at_position("/src/index.ts", 9)?;
        // assert!(symbol.is_some());
        // let symbol = symbol.unwrap();
        // let symbol_id = symbol.id().to_string();
        //
        // // Drop the symbol
        // drop(symbol);
        //
        // // Creating a new symbol at the same position should work
        // let symbol2 = project.get_symbol_at_position("/src/index.ts", 9)?;
        // assert!(symbol2.is_some());
        // let symbol2 = symbol2.unwrap();
        // assert_ne!(symbol2.id(), symbol_id); // Should be a new symbol

        println!("TODO: Test automatic cleanup once symbols are fully implemented");
        client.close()
    }
}

#[cfg(test)]
mod transport_tests {
    use super::*;

    #[test]
    fn test_basic_transport() -> Result<()> {
        let tsgo_path = common::get_tsgo_binary_path().expect("tsgo binary not found");
        assert!(
            common::verify_tsgo_binary(&tsgo_path),
            "tsgo binary is not working properly"
        );

        let client = Client::new(ClientOptions {
            tsgo_path,
            cwd: Some(".".into()),
            log_file: None,
            fs: None,
        })?;

        let echo_response = client.echo("Hello from integration test!")?;
        assert_eq!(echo_response, "Hello from integration test!");

        for i in 1..=3 {
            let test_message = format!("Test message #{}", i);
            let response = client.echo(&test_message)?;
            assert_eq!(response, test_message);
        }

        client.close()
    }

    #[test]
    fn test_vfs_integration() -> Result<()> {
        let client = create_test_client()?;

        // Test echo with VFS
        let response = client.echo("VFS integration test")?;
        assert_eq!(response, "VFS integration test");

        // Test config parsing with VFS
        let config_response = client.parse_config_file("/tsconfig.json")?;
        assert!(config_response.options.is_object());
        assert_eq!(config_response.file_names.len(), 2);
        assert_eq_unordered!(
            config_response.file_names,
            vec!["/src/foo.ts".to_string(), "/src/index.ts".to_string()]
        );

        client.close()
    }
}

#[cfg(test)]
mod benchmarks {
    use super::*;

    #[test]
    fn test_basic_benchmarks() -> Result<()> {
        // TODO: Implement benchmarks similar to TypeScript API tests
        // This would measure performance of various operations:
        // - Symbol resolution
        // - Type checking
        // - AST traversal
        // - Project loading

        println!("TODO: Implement performance benchmarks");
        Ok(())
    }
}

// Parametrized tests for different scenarios
#[cfg(test)]
mod parametrized_tests {
    use super::*;

    #[rstest]
    #[case(
        "simple_import",
        "import { foo } from './foo';",
        "export const foo = 42;"
    )]
    #[case(
        "class_import",
        "import { MyClass } from './class';",
        "export class MyClass { prop = 'test'; }"
    )]
    #[case(
        "interface_import",
        "import { MyInterface } from './interface';",
        "export interface MyInterface { prop: string; }"
    )]
    fn test_different_import_scenarios(
        #[case] scenario: &str,
        #[case] main_content: &str,
        #[case] imported_content: &str,
    ) -> Result<()> {
        let tsgo_path = common::get_tsgo_binary_path().expect("tsgo binary not found");

        let mut files = HashMap::new();
        files.insert("/tsconfig.json".to_string(), "{}".to_string());
        files.insert("/src/index.ts".to_string(), main_content.to_string());
        files.insert("/src/foo.ts".to_string(), imported_content.to_string());

        let vfs: Arc<dyn VirtualFileSystem + Send + Sync> =
            Arc::new(MemoryFileSystem::from_files(files));

        let client = Client::new(ClientOptions {
            tsgo_path,
            cwd: Some(".".into()),
            log_file: None,
            fs: Some(vfs),
        })?;

        // Test that the project loads successfully
        let project = client.load_project("/tsconfig.json")?;
        assert_eq!(project.config_file_name, "/tsconfig.json");
        assert_eq_unordered!(
            project.root_files.clone(),
            vec!["/src/foo.ts".to_string(), "/src/index.ts".to_string()]
        );

        // Test echo to verify transport works
        let echo_response = client.echo(&format!("test_{}", scenario))?;
        assert_eq!(echo_response, format!("test_{}", scenario));

        // TODO: Add symbol resolution tests once implemented
        // let symbol = project.get_symbol_at_position("/src/index.ts", find_import_position(main_content))?;
        // assert!(symbol.is_some());
        // let symbol = symbol.unwrap();
        // // Test based on scenario expectations

        client.close()
    }
}

// Helper function for finding import positions (stub for now)
fn find_import_position(_content: &str) -> usize {
    // TODO: Implement proper position finding
    9
}
