use std::io::Cursor;

use byteorder::{LittleEndian, ReadBytesExt};
use tsgo_syntax::SyntaxKind;

use crate::{DecoderError, Result};

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
    pub const NODE_EXTENDED_DATA_MASK: u32 = 0x00_ff_ff_ff;

    pub const SYNTAX_KIND_NODE_LIST: u32 = 0xff_ff_ff_ff;

    pub const HEADER_OFFSET_METADATA: usize = 0;
    pub const HEADER_OFFSET_STRING_OFFSETS: usize = 4;
    pub const HEADER_OFFSET_STRING_DATA: usize = 8;
    pub const HEADER_OFFSET_EXTENDED_DATA: usize = 12;
    pub const HEADER_OFFSET_NODES: usize = 16;
    pub const HEADER_SIZE: usize = 20;

    pub const PROTOCOL_VERSION: u8 = 1;
}

use constants::*;

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
    pub flags: Option<u32>,
    pub token: Option<SyntaxKind>,
    pub template_flags: Option<u32>,
    pub file_name: Option<String>,
    pub raw_text: Option<String>,
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
    pub fn new(data: Vec<u8>) -> Result<Self> {
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
    fn decode_header(data: &[u8]) -> Result<Header> {
        if data.len() < HEADER_SIZE {
            return Err(DecoderError::BufferTooSmall {
                needed: HEADER_SIZE,
                available: data.len(),
            });
        }

        let mut cursor = Cursor::new(data);
        let metadata = cursor.read_u32::<LittleEndian>()?;
        let protocol_version = (metadata >> 24) as u8;
        if protocol_version != PROTOCOL_VERSION {
            return Err(DecoderError::UnsupportedProtocolVersion {
                expected: PROTOCOL_VERSION,
                actual: protocol_version,
            });
        }

        cursor.set_position(HEADER_OFFSET_STRING_OFFSETS as u64);
        let string_offsets_offset = cursor.read_u32::<LittleEndian>()?;
        let string_data_offset = cursor.read_u32::<LittleEndian>()?;
        let extended_data_offset = cursor.read_u32::<LittleEndian>()?;
        let nodes_offset = cursor.read_u32::<LittleEndian>()?;

        Ok(Header {
            protocol_version,
            string_offsets_offset,
            string_data_offset,
            extended_data_offset,
            nodes_offset,
        })
    }

    /// Decode the string table from binary data
    fn decode_string_table(data: &[u8], header: &Header) -> Result<Vec<String>> {
        let string_offsets_start = header.string_offsets_offset as usize;
        let string_data_start = header.string_data_offset as usize;
        let _extended_data_start = header.extended_data_offset as usize;

        if string_offsets_start >= data.len() {
            return Err(DecoderError::InvalidDataOffset {
                offset: string_offsets_start,
                buffer_size: data.len(),
            });
        }
        if string_data_start >= data.len() {
            return Err(DecoderError::InvalidDataOffset {
                offset: string_data_start,
                buffer_size: data.len(),
            });
        }

        let mut cursor = Cursor::new(&data[string_offsets_start..string_data_start]);
        let num_strings = (string_data_start - string_offsets_start) / 8;
        let strings: Vec<String> = (0..num_strings)
            .map(|i| {
                let start_offset = cursor.read_u32::<LittleEndian>()? as usize;
                let end_offset = cursor.read_u32::<LittleEndian>()? as usize;

                let string_start = string_data_start + start_offset;
                let string_end = string_data_start + end_offset;

                if string_end > data.len() {
                    return Err(DecoderError::StringBoundsOutOfRange {
                        string_index: i,
                        start: string_start,
                        end: string_end,
                        data_size: data.len(),
                    });
                }

                if string_start > string_end {
                    return Err(DecoderError::StringBoundsInvalid {
                        string_index: i,
                        start: string_start,
                        end: string_end,
                    });
                }

                let string_bytes = &data[string_start..string_end];
                let string = String::from_utf8_lossy(string_bytes).to_string();
                Ok(string)
            })
            .collect::<Result<Vec<String>>>()?;

        Ok(strings)
    }

    /// Decode all nodes from binary data
    fn decode_nodes(data: &[u8], header: &Header, string_table: &[String]) -> Result<Vec<Node>> {
        let nodes_start = header.nodes_offset as usize;
        if nodes_start >= data.len() {
            return Err(DecoderError::InvalidDataOffset {
                offset: nodes_start,
                buffer_size: data.len(),
            });
        }

        let nodes_data = &data[nodes_start..];
        let mut cursor = Cursor::new(nodes_data);
        let num_nodes = nodes_data.len() / NODE_SIZE;

        let nodes: Vec<Node> = (0..num_nodes)
            .map(|_i| {
                let kind_raw = cursor.read_u32::<LittleEndian>()?;
                let kind = SyntaxKind::from_repr(kind_raw).unwrap_or(SyntaxKind::Unknown);

                let pos = cursor.read_u32::<LittleEndian>()?;
                let end = cursor.read_u32::<LittleEndian>()?;
                let next_sibling = cursor.read_u32::<LittleEndian>()?;
                let parent = cursor.read_u32::<LittleEndian>()?;
                let node_data = cursor.read_u32::<LittleEndian>()?;

                let text = Self::decode_node_text(&kind, node_data, string_table, data, header)?;

                let (flags, token, template_flags, file_name, raw_text) =
                    Self::decode_extended_data(&kind, node_data, string_table, data, header)?;

                Ok(Node {
                    kind,
                    pos,
                    end,
                    next_sibling,
                    parent,
                    data: node_data,
                    text,
                    flags,
                    token,
                    template_flags,
                    file_name,
                    raw_text,
                })
            })
            .collect::<Result<Vec<Node>>>()?;

        Ok(nodes)
    }

    fn decode_node_text(
        kind: &SyntaxKind,
        node_data: u32,
        string_table: &[String],
        data: &[u8],
        header: &Header,
    ) -> Result<Option<String>> {
        let data_type = node_data & NODE_DATA_TYPE_MASK;

        match data_type {
            NODE_DATA_TYPE_STRING => {
                let string_index = (node_data & NODE_DATA_STRING_INDEX_MASK) as usize / 2;
                if string_index < string_table.len() {
                    Ok(Some(string_table[string_index].clone()))
                } else {
                    Ok(None)
                }
            }
            NODE_DATA_TYPE_EXTENDED_DATA => match kind {
                SyntaxKind::SourceFile
                | SyntaxKind::TemplateHead
                | SyntaxKind::TemplateMiddle
                | SyntaxKind::TemplateTail => {
                    let extended_data_offset = header.extended_data_offset as usize
                        + (node_data & NODE_EXTENDED_DATA_MASK) as usize;

                    if extended_data_offset + 4 <= data.len() {
                        let mut cursor = Cursor::new(&data[extended_data_offset..]);
                        let string_index = cursor.read_u32::<LittleEndian>()? as usize / 2;

                        if string_index < string_table.len() {
                            Ok(Some(string_table[string_index].clone()))
                        } else {
                            Ok(None)
                        }
                    } else {
                        Ok(None)
                    }
                }
                _ => Ok(None),
            },
            _ => Ok(None),
        }
    }

    fn decode_extended_data(
        kind: &SyntaxKind,
        node_data: u32,
        string_table: &[String],
        data: &[u8],
        header: &Header,
    ) -> Result<(
        Option<u32>,
        Option<SyntaxKind>,
        Option<u32>,
        Option<String>,
        Option<String>,
    )> {
        let data_type = node_data & NODE_DATA_TYPE_MASK;

        // TODO(aagarwal): Make this more efficient
        let mut flags = None;
        let mut token = None;
        let mut template_flags = None;
        let mut file_name = None;
        let mut raw_text = None;

        if kind == &SyntaxKind::VariableDeclarationList {
            flags = Some((node_data & (1 << 24 | 1 << 25)) >> 24);
        }

        if kind == &SyntaxKind::ImportAttributes {
            if (node_data & (1 << 25)) != 0 {
                token = Some(SyntaxKind::AssertKeyword);
            } else {
                token = Some(SyntaxKind::WithKeyword);
            }
        }

        if data_type == NODE_DATA_TYPE_EXTENDED_DATA {
            let extended_data_offset = header.extended_data_offset as usize
                + (node_data & NODE_EXTENDED_DATA_MASK) as usize;

            match kind {
                SyntaxKind::TemplateHead
                | SyntaxKind::TemplateMiddle
                | SyntaxKind::TemplateTail => {
                    if extended_data_offset + 12 <= data.len() {
                        let mut cursor = Cursor::new(&data[extended_data_offset..]);
                        cursor.set_position(4); // raw_text is at offset 4
                        let raw_text_index = cursor.read_u32::<LittleEndian>()? as usize / 2;

                        if raw_text_index < string_table.len() {
                            raw_text = Some(string_table[raw_text_index].clone());
                        }

                        template_flags = Some(cursor.read_u32::<LittleEndian>()?);
                    }
                }
                SyntaxKind::SourceFile => {
                    if extended_data_offset + 8 <= data.len() {
                        let mut cursor = Cursor::new(&data[extended_data_offset..]);
                        cursor.set_position(4); // file_name is at offset 4
                        let file_name_index = cursor.read_u32::<LittleEndian>()? as usize / 2;

                        if file_name_index < string_table.len() {
                            file_name = Some(string_table[file_name_index].clone());
                        }
                    }
                }
                _ => {}
            }
        }

        Ok((flags, token, template_flags, file_name, raw_text))
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
}
