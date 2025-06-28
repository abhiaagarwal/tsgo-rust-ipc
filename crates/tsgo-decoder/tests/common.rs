use std::path::Path;

pub fn extract_test_name(binary_path: &Path) -> String {
    binary_path
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.strip_suffix(".bin").unwrap_or(s))
        .unwrap_or("unknown")
        .to_string()
}

pub fn find_go_dump_path(binary_path: &Path) -> std::path::PathBuf {
    let test_name = extract_test_name(binary_path);
    let test_path_string = format!("tests/fixtures/dumps/go/{}.txt", test_name);
    let test_path = Path::new(&test_path_string);

    if test_path.exists() {
        test_path.to_path_buf()
    } else {
        panic!("No reference dump found for {}", test_name);
    }
}
