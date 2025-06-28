mod common;

use std::fs;

use rstest::rstest;
use similar_asserts::assert_eq;
use tsgo_decoder::{
    Node, TsgoDecoder,
    constants::{NODE_OFFSET_NEXT, NODE_SIZE},
};
use tsgo_syntax::SyntaxKind;

fn format_encoded_source_file(decoder: &TsgoDecoder) -> String {
    let mut result = String::new();

    fn get_indent(parent_index: u32, nodes: &[Node]) -> String {
        if parent_index == 0 {
            return String::new();
        }

        let parent_node = &nodes[parent_index as usize];
        format!("  {}", get_indent(parent_node.parent, nodes))
    }

    let mut next_sibling_map = std::collections::HashMap::new();
    for (i, node) in decoder.nodes().iter().enumerate().skip(1) {
        next_sibling_map.insert(i, node.next_sibling);
    }

    for (i, node) in decoder.nodes().iter().enumerate().skip(1) {
        // Skip the nil node at index 0
        let indent = get_indent(node.parent, decoder.nodes());

        result.push_str(&indent);

        if node.kind == SyntaxKind::NodeList {
            result.push_str("NodeList");
        } else {
            result.push_str(&format!("Kind{}", node.kind));
        }

        if matches!(
            node.kind,
            SyntaxKind::Identifier | SyntaxKind::StringLiteral
        ) {
            if let Some(text) = &node.text {
                result.push_str(&format!(" \"{}\"", text));
            }
        }

        let next_value = decoder.data()
            [decoder.header().nodes_offset as usize + i * NODE_SIZE + NODE_OFFSET_NEXT];

        result.push_str(&format!(
            " [{}, {}), i={}, next={}",
            node.pos, node.end, i, next_value
        ));
        result.push('\n');
    }

    result
}

#[rstest]
fn test_decode_and_format(
    #[files("tests/fixtures/encoded/*.bin")] binary_path: std::path::PathBuf,
) {
    let test_name = common::extract_test_name(&binary_path);

    let binary_data = fs::read(&binary_path).unwrap_or_else(|e| {
        panic!(
            "Failed to read binary test data from {}: {}",
            binary_path.display(),
            e
        )
    });

    let decoder = TsgoDecoder::new(binary_data)
        .unwrap_or_else(|e| panic!("Failed to decode binary data for {}: {}", test_name, e));

    let formatted_output = format_encoded_source_file(&decoder);

    let dump_path = common::find_go_dump_path(&binary_path);
    let expected_output = {
        let bytes = fs::read(&dump_path).unwrap_or_else(|e| {
            panic!(
                "Failed to read expected output from {}: {}",
                dump_path.display(),
                e
            )
        });
        String::from_utf8_lossy(&bytes).to_string()
    };

    assert_eq!(
        formatted_output, expected_output,
        "Formatted AST does not match Go reference output for test case: {}",
        test_name
    );
}
