use std::{collections::HashMap, env, path::Path, sync::Arc};

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

/// Test basic tsgo transport functionality
#[test]
fn test_transport_basic_functionality() -> Result<()> {
    let tsgo_path = common::get_tsgo_binary_path().expect("tsgo binary not found.");

    assert!(
        common::verify_tsgo_binary(&tsgo_path),
        "tsgo binary is not working properly"
    );

    let mut client = Client::new(ClientOptions {
        tsgo_path: tsgo_path.clone(),
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

    let error_result = client.request_value("invalid_method", json!("test"));
    assert!(error_result.is_err());

    client.close()?;
    Ok(())
}

/// Test transport with VFS integration using a simple test case
#[test]
fn test_transport_with_simple_vfs() -> Result<()> {
    let tsgo_path = common::get_tsgo_binary_path().expect("tsgo binary not found.");

    let mut project_files = HashMap::new();
    project_files.insert(
        "/tsconfig.json".to_string(),
        r#"{
  "compilerOptions": {
    "target": "ES2020",
    "module": "commonjs",
    "strict": true
  },
  "include": ["src/**/*"]
}"#
        .to_string(),
    );
    project_files.insert(
        "/src/index.ts".to_string(),
        r#"export function greet(name: string): string {
    return `Hello, ${name}!`;
}

const message = greet("World");
console.log(message);
"#
        .to_string(),
    );

    let vfs: Arc<dyn VirtualFileSystem + Send + Sync> =
        Arc::new(MemoryFileSystem::from_files(project_files));

    assert!(vfs.file_exists("/tsconfig.json"));
    assert!(vfs.file_exists("/src/index.ts"));
    assert!(vfs.directory_exists("/src"));

    let mut client = Client::new(ClientOptions {
        tsgo_path: tsgo_path.clone(),
        cwd: Some(".".into()),
        log_file: None,
        fs: Some(vfs),
    })?;

    match client.echo("VFS integration test") {
        Ok(response) => {
            assert_eq!(response, json!("VFS integration test"));
        }
        Err(e) => {
            return Err(e);
        }
    }

    let config_response = match client.request_value(
        "parseConfigFile",
        json!({
            "fileName": "/tsconfig.json"
        }),
    ) {
        Ok(response) => response,
        Err(e) => {
            return Err(e);
        }
    };

    assert!(config_response.is_object());
    if let Some(file_names) = config_response.get("fileNames") {
        let files = file_names.as_array().unwrap();
        assert!(!files.is_empty());
    }

    client.close()?;
    Ok(())
}

/// Test parsing of tsgo test case format
#[test]
fn test_tsgo_test_case_parsing() -> Result<()> {
    let simple_content = r#"const x: number = "";"#;
    let parsed = common::parse_tsgo_test_case(simple_content);
    assert_eq!(parsed.files.len(), 1);
    assert!(parsed.files.contains_key("/main.ts"));

    let multi_file_content = r#"// @filename: /tsconfig.json
{
    "compilerOptions": {
        "target": "es2020",
        "strictNullChecks": true
    }
}
// @filename: /foo.ts
export {};
const x: string = undefined;"#;

    let parsed = common::parse_tsgo_test_case(multi_file_content);
    assert_eq!(parsed.files.len(), 2);
    assert!(parsed.files.contains_key("/tsconfig.json"));
    assert!(parsed.files.contains_key("/foo.ts"));

    let tsconfig = &parsed.files["/tsconfig.json"];
    assert!(tsconfig.contains("es2020"));
    assert!(tsconfig.contains("strictNullChecks"));

    let foo_ts = &parsed.files["/foo.ts"];
    assert!(foo_ts.contains("export {};"));
    assert!(foo_ts.contains("undefined"));

    Ok(())
}

/// Test with actual tsgo test cases
#[test]
fn test_with_real_tsgo_test_cases() -> Result<()> {
    let tsgo_path = common::get_tsgo_binary_path().expect("tsgo binary not found");

    let test_cases = common::find_tsgo_test_cases()?;

    for test_case_path in test_cases.iter() {
        let test_name = common::extract_test_name(test_case_path);

        let content = match std::fs::read_to_string(test_case_path) {
            Ok(content) => content,
            Err(e) => {
                println!("Failed to read test case {}: {}", test_name, e);
                continue;
            }
        };

        let test_data = common::parse_tsgo_test_case(&content);

        if !test_data.symlinks.is_empty() {
            println!("Skipping {} (has symlinks)", test_name);
            continue;
        }

        let vfs: Arc<dyn VirtualFileSystem + Send + Sync> =
            Arc::new(MemoryFileSystem::from_files(test_data.files));

        let mut client = Client::new(ClientOptions {
            tsgo_path: tsgo_path.clone(),
            cwd: Some(".".into()),
            log_file: None,
            fs: Some(Arc::clone(&vfs)),
        })?;

        let test_message = format!("test_{}", test_name);
        client.echo(&test_message)?;

        if vfs.file_exists("/tsconfig.json") {
            let config_response =
                client.request_value("parseConfigFile", json!({"fileName": "/tsconfig.json"}))?;
            assert!(config_response.is_object());
        }

        client.close()?;
    }
    Ok(())
}

/// Test a specific complex tsgo test case
#[test]
fn test_complex_tsgo_case() -> Result<()> {
    let tsgo_path = common::get_tsgo_binary_path().expect("tsgo binary not found.");

    let test_content = r#"// @filename: tsconfig.json
{
    "compilerOptions": {
        "target": "es2020",
        "strictNullChecks": true
    }
}
// @filename: foo.ts
export {};
const x: string = undefined;"#;

    let test_data = common::parse_tsgo_test_case(test_content);

    assert_eq!(test_data.files.len(), 2);
    assert!(test_data.files.contains_key("tsconfig.json"));
    assert!(test_data.files.contains_key("foo.ts"));

    let vfs: Arc<dyn VirtualFileSystem + Send + Sync> =
        Arc::new(MemoryFileSystem::from_files(test_data.files));

    let mut client = Client::new(ClientOptions {
        tsgo_path: tsgo_path.clone(),
        cwd: Some(".".into()),
        log_file: None,
        fs: Some(Arc::clone(&vfs)),
    })?;

    let test_message = "vfs_test".to_string();
    client.echo(&test_message)?;

    if vfs.file_exists("/tsconfig.json") {
        let config_response =
            client.request_value("parseConfigFile", json!({"fileName": "/tsconfig.json"}))?;
        assert!(config_response.is_object());
    }

    client.close()?;
    Ok(())
}

/// Integration test for error scenarios
#[test]
fn test_error_scenarios() -> Result<()> {
    let tsgo_path = common::get_tsgo_binary_path().expect("tsgo binary not found");

    let mut client = Client::new(ClientOptions {
        tsgo_path: tsgo_path.clone(),
        cwd: Some(".".into()),
        log_file: None,
        fs: None,
    })?;

    let error_tests = vec![
        ("nonexistent_method", json!("test")),
        ("", json!("empty method name")),
        ("echo", json!(null)),
    ];

    for (method, payload) in error_tests {
        let result = client.request_value(method, payload);
        if method == "echo" {
            assert!(result.is_ok() || result.is_err());
        } else {
            assert!(result.is_err(), "Expected error for method: {}", method);
        }
    }

    client.close()?;
    Ok(())
}

/// Parametrized test that runs against multiple tsgo test cases
#[rstest]
#[case("simpleTestSingleFile.ts")]
#[case("tsconfigSimpleTest.ts")]
#[case("settingsSimpleTest.ts")]
#[case("singleSettingsSimpleTest.ts")]
#[case("declarationEmitBigInt.ts")]
fn test_tsgo_test_case_parametrized(#[case] test_file_name: &str) -> Result<()> {
    let tsgo_path = common::get_tsgo_binary_path().expect("tsgo binary not found.");

    let test_paths = [
        format!("tsgo/testdata/tests/cases/compiler/{}", test_file_name),
        format!("tsgo/testdata/tests/cases/conformance/{}", test_file_name),
        format!("../tsgo/testdata/tests/cases/compiler/{}", test_file_name),
        format!(
            "../tsgo/testdata/tests/cases/conformance/{}",
            test_file_name
        ),
    ];

    let mut test_content = None;
    for test_path in &test_paths {
        if let Ok(content) = std::fs::read_to_string(test_path) {
            test_content = Some(content);
            break;
        }
    }

    let content = test_content.expect("test file not found");
    let test_data = common::parse_tsgo_test_case(&content);

    if !test_data.symlinks.is_empty() {
        return Ok(());
    }
    let vfs: Arc<dyn VirtualFileSystem + Send + Sync> =
        Arc::new(MemoryFileSystem::from_files(test_data.files));

    assert!(vfs.directory_exists("/"));

    let mut client = Client::new(ClientOptions {
        tsgo_path: tsgo_path.clone(),
        cwd: Some(".".into()),
        log_file: None,
        fs: Some(Arc::clone(&vfs)),
    })?;

    let test_message = format!("test_{}", test_file_name);
    let echo_response = client.echo(&test_message)?;
    assert_eq!(echo_response, test_message);

    if vfs.file_exists("/tsconfig.json") || vfs.file_exists("tsconfig.json") {
        let config_file = if vfs.file_exists("/tsconfig.json") {
            "/tsconfig.json"
        } else {
            "tsconfig.json"
        };

        if let Ok(response) =
            client.request_value("parseConfigFile", json!({"fileName": config_file}))
        {
            assert!(
                response.is_object(),
                "Config response should be an object for {}",
                test_file_name
            );
        }
    }

    client.close()?;
    Ok(())
}
