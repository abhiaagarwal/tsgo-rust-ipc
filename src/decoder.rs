use crate::syntax::SyntaxKind;

pub mod constants {
    pub const NODE_OFFSET_KIND: usize = 0;
    pub const NODE_OFFSET_POS: usize = 4;
    pub const NODE_OFFSET_END: usize = 8;
    pub const NODE_OFFSET_NEXT: usize = 12;
    pub const NODE_OFFSET_PARENT: usize = 16;
    pub const NODE_OFFSET_DATA: usize = 20;
    pub const NODE_SIZE: usize = 24;

    pub const NODE_DATA_TYPE_CHILDREN: u32 = 0x00_00_00_00;
    pub const NODE_DATA_TYPE_STRING: u32 = 0x40_00_00_00;
    pub const NODE_DATA_TYPE_EXTENDED_DATA: u32 = 0x80_00_00_00;

    pub const NODE_DATA_TYPE_MASK: u32 = 0xc0_00_00_00;
    pub const NODE_DATA_CHILD_MASK: u32 = 0x00_00_00_ff;
    pub const NODE_DATA_STRING_INDEX_MASK: u32 = 0x00_ff_ff_ff;

    pub const SYNTAX_KIND_NODE_LIST: u32 = 0xff_ff_ff_ff;

    pub const HEADER_OFFSET_METADATA: usize = 0;
    pub const HEADER_OFFSET_STRING_OFFSETS: usize = 4;
    pub const HEADER_OFFSET_STRING_DATA: usize = 8;
    pub const HEADER_OFFSET_EXTENDED_DATA: usize = 12;
    pub const HEADER_OFFSET_NODES: usize = 16;
    pub const HEADER_SIZE: usize = 20;

    pub const PROTOCOL_VERSION: u8 = 1;
}

/// Represents a decoded AST node
#[derive(Debug, Clone)]
pub struct Node {
    pub kind: SyntaxKind,
    pub pos: u32,
    pub end: u32,
    pub next_sibling: u32,
    pub parent: u32,
    pub data: u32,
    pub text: Option<String>,
}

/// Header information from the binary format
#[derive(Debug)]
pub struct Header {
    pub protocol_version: u8,
    pub string_offsets_offset: u32,
    pub string_data_offset: u32,
    pub extended_data_offset: u32,
    pub nodes_offset: u32,
}

/// Main decoder for the tsgo binary format
pub struct TsgoDecoder {
    data: Vec<u8>,
    header: Header,
    string_table: Vec<String>,
    nodes: Vec<Node>,
}

impl TsgoDecoder {
    /// Create a new decoder from binary data
    pub fn new(data: Vec<u8>) -> Result<Self, Box<dyn std::error::Error>> {
        let header = Self::decode_header(&data)?;
        let string_table = Self::decode_string_table(&data, &header)?;
        let nodes = Self::decode_nodes(&data, &header, &string_table)?;

        Ok(TsgoDecoder {
            data,
            header,
            string_table,
            nodes,
        })
    }

    /// Decode the header from binary data
    fn decode_header(data: &[u8]) -> Result<Header, Box<dyn std::error::Error>> {
        if data.len() < constants::HEADER_SIZE {
            return Err("Data too short for header".into());
        }

        let metadata = u32::from_le_bytes([
            data[constants::HEADER_OFFSET_METADATA],
            data[constants::HEADER_OFFSET_METADATA + 1],
            data[constants::HEADER_OFFSET_METADATA + 2],
            data[constants::HEADER_OFFSET_METADATA + 3],
        ]);

        let protocol_version = (metadata >> 24) as u8;
        if protocol_version != constants::PROTOCOL_VERSION {
            return Err(format!("Unsupported protocol version: {}", protocol_version).into());
        }

        let string_offsets_offset = u32::from_le_bytes([
            data[constants::HEADER_OFFSET_STRING_OFFSETS],
            data[constants::HEADER_OFFSET_STRING_OFFSETS + 1],
            data[constants::HEADER_OFFSET_STRING_OFFSETS + 2],
            data[constants::HEADER_OFFSET_STRING_OFFSETS + 3],
        ]);

        let string_data_offset = u32::from_le_bytes([
            data[constants::HEADER_OFFSET_STRING_DATA],
            data[constants::HEADER_OFFSET_STRING_DATA + 1],
            data[constants::HEADER_OFFSET_STRING_DATA + 2],
            data[constants::HEADER_OFFSET_STRING_DATA + 3],
        ]);

        let extended_data_offset = u32::from_le_bytes([
            data[constants::HEADER_OFFSET_EXTENDED_DATA],
            data[constants::HEADER_OFFSET_EXTENDED_DATA + 1],
            data[constants::HEADER_OFFSET_EXTENDED_DATA + 2],
            data[constants::HEADER_OFFSET_EXTENDED_DATA + 3],
        ]);

        let nodes_offset = u32::from_le_bytes([
            data[constants::HEADER_OFFSET_NODES],
            data[constants::HEADER_OFFSET_NODES + 1],
            data[constants::HEADER_OFFSET_NODES + 2],
            data[constants::HEADER_OFFSET_NODES + 3],
        ]);

        Ok(Header {
            protocol_version,
            string_offsets_offset,
            string_data_offset,
            extended_data_offset,
            nodes_offset,
        })
    }

    /// Decode the string table from binary data
    fn decode_string_table(
        data: &[u8],
        header: &Header,
    ) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let string_offsets_start = header.string_offsets_offset as usize;
        let string_data_start = header.string_data_offset as usize;
        let _extended_data_start = header.extended_data_offset as usize;

        let mut strings = Vec::new();
        let mut offset = string_offsets_start;

        while offset + 8 <= string_data_start {
            let start = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]) as usize;

            let end = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]) as usize;

            let string_start = string_data_start + start;
            let string_end = string_data_start + end;

            if string_end <= data.len() {
                let string_bytes = &data[string_start..string_end];
                let string = String::from_utf8(string_bytes.to_vec())?;
                strings.push(string);
            }

            offset += 8;
        }

        Ok(strings)
    }

    /// Decode all nodes from binary data
    fn decode_nodes(
        data: &[u8],
        header: &Header,
        string_table: &[String],
    ) -> Result<Vec<Node>, Box<dyn std::error::Error>> {
        let nodes_start = header.nodes_offset as usize;
        let mut nodes = Vec::new();
        let mut offset = nodes_start;

        while offset + constants::NODE_SIZE <= data.len() {
            let kind_raw = u32::from_le_bytes([
                data[offset + constants::NODE_OFFSET_KIND],
                data[offset + constants::NODE_OFFSET_KIND + 1],
                data[offset + constants::NODE_OFFSET_KIND + 2],
                data[offset + constants::NODE_OFFSET_KIND + 3],
            ]);

            let pos = u32::from_le_bytes([
                data[offset + constants::NODE_OFFSET_POS],
                data[offset + constants::NODE_OFFSET_POS + 1],
                data[offset + constants::NODE_OFFSET_POS + 2],
                data[offset + constants::NODE_OFFSET_POS + 3],
            ]);

            let end = u32::from_le_bytes([
                data[offset + constants::NODE_OFFSET_END],
                data[offset + constants::NODE_OFFSET_END + 1],
                data[offset + constants::NODE_OFFSET_END + 2],
                data[offset + constants::NODE_OFFSET_END + 3],
            ]);

            let next_sibling = u32::from_le_bytes([
                data[offset + constants::NODE_OFFSET_NEXT],
                data[offset + constants::NODE_OFFSET_NEXT + 1],
                data[offset + constants::NODE_OFFSET_NEXT + 2],
                data[offset + constants::NODE_OFFSET_NEXT + 3],
            ]);

            let parent = u32::from_le_bytes([
                data[offset + constants::NODE_OFFSET_PARENT],
                data[offset + constants::NODE_OFFSET_PARENT + 1],
                data[offset + constants::NODE_OFFSET_PARENT + 2],
                data[offset + constants::NODE_OFFSET_PARENT + 3],
            ]);

            let data_raw = u32::from_le_bytes([
                data[offset + constants::NODE_OFFSET_DATA],
                data[offset + constants::NODE_OFFSET_DATA + 1],
                data[offset + constants::NODE_OFFSET_DATA + 2],
                data[offset + constants::NODE_OFFSET_DATA + 3],
            ]);

            let kind = SyntaxKind::from_repr(kind_raw).unwrap_or(SyntaxKind::Unknown);

            let text = if (data_raw & constants::NODE_DATA_TYPE_MASK)
                == constants::NODE_DATA_TYPE_STRING
            {
                let string_index =
                    ((data_raw & constants::NODE_DATA_STRING_INDEX_MASK) as usize) / 2;
                if string_index < string_table.len() {
                    Some(string_table[string_index].clone())
                } else {
                    None
                }
            } else {
                None
            };

            nodes.push(Node {
                kind,
                pos,
                end,
                next_sibling,
                parent,
                data: data_raw,
                text,
            });

            offset += constants::NODE_SIZE;
        }

        Ok(nodes)
    }

    /// Get all nodes
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    /// Get string table
    pub fn string_table(&self) -> &[String] {
        &self.string_table
    }

    /// Get header
    pub fn header(&self) -> &Header {
        &self.header
    }

    /// Get raw data (for debugging)
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Format the decoded source file for debugging (similar to Go test)
    pub fn format_encoded_source_file(&self) -> String {
        let mut result = String::new();

        fn get_indent(parent_index: u32, nodes: &[Node]) -> String {
            if parent_index == 0 {
                return String::new();
            }

            let parent_node = &nodes[parent_index as usize];
            format!("  {}", get_indent(parent_node.parent, nodes))
        }

        for (i, node) in self.nodes.iter().enumerate().skip(1) {
            // Skip the nil node at index 0
            let indent = get_indent(node.parent, &self.nodes);

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

            result.push_str(&format!(
                " [{}, {}), i={}, next={}",
                node.pos, node.end, i, node.next_sibling
            ));
            result.push('\n');
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    const TEST_CASES: &[(&str, &str)] = &[
        ("test", "Simple import and function"),
        ("simple", "Simple import and function"),
        ("interface_class", "Interface and class with inheritance"),
        ("generics_conditionals", "Generics and conditional types"),
        ("jsx_example", "JSX components and props"),
        ("decorators_accessors", "Decorators, getters, and setters"),
    ];

    fn load_fixture(name: &str) -> (Vec<u8>, String) {
        let binary_path = format!("test_data/encoded/encoded_{}.bin", name);
        let expected_path = format!("test_data/encoded/formatted_{}.txt", name);
        let binary_data = fs::read(&binary_path)
            .unwrap_or_else(|e| panic!("Failed to read binary test data for {}: {}", name, e));
        let expected_output = fs::read_to_string(&expected_path)
            .unwrap_or_else(|e| panic!("Failed to read expected output for {}: {}", name, e));
        (binary_data, expected_output)
    }

    #[test]
    fn test_decode_all_cases() {
        for &(test_name, description) in TEST_CASES {
            println!("\nTesting case: {} ({})", test_name, description);

            let (binary_data, expected_output) = load_fixture(test_name);

            let decoder = TsgoDecoder::new(binary_data).unwrap_or_else(|e| {
                panic!("Failed to decode binary data for {}: {}", test_name, e)
            });
            let formatted_output = decoder.format_encoded_source_file();

            assert_eq!(
                formatted_output, expected_output,
                "Formatted AST does not match reference output for test case: {}",
                test_name
            )
        }
    }

    #[test]
    fn test_syntax_kind_coverage() {
        let mut found_syntax_kinds = std::collections::HashSet::new();

        for &(test_name, _) in TEST_CASES {
            let (binary_data, _) = load_fixture(test_name);
            let decoder = TsgoDecoder::new(binary_data).expect("Failed to decode");

            for node in decoder.nodes() {
                found_syntax_kinds.insert(node.kind);
            }
        }

        println!(
            "Found {} unique syntax kinds across all test cases:",
            found_syntax_kinds.len()
        );
        let mut kinds: Vec<_> = found_syntax_kinds.iter().copied().collect();
        kinds.sort_by_key(|k| *k as u32);

        for kind in kinds {
            println!("  {:?} ({})", kind, kind as u32);
        }

        assert!(
            found_syntax_kinds.len() >= 30,
            "Expected at least 30 different syntax kinds, found: {}",
            found_syntax_kinds.len()
        );
    }

    #[test]
    fn test_string_table_integrity() {
        for &(test_name, _) in TEST_CASES {
            let (binary_data, _) = load_fixture(test_name);
            let decoder = TsgoDecoder::new(binary_data).expect("Failed to decode");

            for (i, node) in decoder.nodes().iter().enumerate() {
                if let Some(text) = &node.text {
                    let string_index =
                        (node.data & constants::NODE_DATA_STRING_INDEX_MASK) as usize / 2;
                    assert!(
                        string_index < decoder.string_table().len(),
                        "Node {} in {} has invalid string index: {} >= {}",
                        i,
                        test_name,
                        string_index,
                        decoder.string_table().len()
                    );

                    assert_eq!(
                        text,
                        &decoder.string_table()[string_index],
                        "Node {} in {} has mismatched string data",
                        i,
                        test_name
                    );
                }
            }
        }
    }

    #[test]
    fn test_node_tree_structure() {
        for &(test_name, _) in TEST_CASES {
            let (binary_data, _) = load_fixture(test_name);
            let decoder = TsgoDecoder::new(binary_data).expect("Failed to decode");

            for (i, node) in decoder.nodes().iter().enumerate() {
                if node.parent != 0 && (node.parent as usize) < decoder.nodes().len() {
                    let parent = &decoder.nodes()[node.parent as usize];
                    assert!(
                        parent.pos <= node.pos && node.end <= parent.end,
                        "Node {} in {} has invalid position relative to parent {}: [{}, {}) not within [{}, {})",
                        i,
                        test_name,
                        node.parent,
                        node.pos,
                        node.end,
                        parent.pos,
                        parent.end
                    );
                }

                assert!(
                    node.pos <= node.end,
                    "Node {} in {} has invalid position range: {} > {}",
                    i,
                    test_name,
                    node.pos,
                    node.end
                );
            }
        }
    }
}
