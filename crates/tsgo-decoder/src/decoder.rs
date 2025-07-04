use std::{borrow::Cow, io::Cursor};

use byteorder::{LittleEndian, ReadBytesExt};
use typescript_ast_definitions::{SyntaxKind, TokenFlags};

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

#[derive(Debug, Clone, PartialEq)]
pub enum NodeKind {
    SyntaxKind(SyntaxKind),
    NodeList,
}

/// Represents a decoded AST node
#[derive(Debug, Clone)]
pub struct Node<'a> {
    pub kind: NodeKind,
    pub pos: u32,
    pub end: u32,
    pub next_sibling: u32,
    pub parent: u32,
    pub data: u32,
    pub text: Option<Cow<'a, str>>,
    pub flags: Option<u32>,
    pub token: Option<SyntaxKind>,
    pub template_flags: Option<TokenFlags>,
    pub file_name: Option<Cow<'a, str>>,
    pub raw_text: Option<Cow<'a, str>>,
}

pub struct StringTable<'a> {
    entries: Vec<(usize, usize)>,
    bytes: &'a [u8],
}

impl<'a> StringTable<'a> {
    /// Create a new string table from the (start, end) offsets that are **relative** to the
    /// beginning of the `bytes` slice that contains all string data. The offsets are given as
    /// `u32` in the binary format; they are converted to `usize` here for easier slicing.
    pub fn new(raw_entries: Vec<(u32, u32)>, bytes: &'a [u8]) -> Result<Self> {
        let entries = raw_entries
            .into_iter()
            .enumerate()
            .map(|(i, (s, e))| {
                let s = s as usize;
                let e = e as usize;

                if e > bytes.len() {
                    return Err(DecoderError::StringBoundsOutOfRange {
                        string_index: i,
                        start: s,
                        end: e,
                        data_size: bytes.len(),
                    });
                }

                if s > e {
                    return Err(DecoderError::StringBoundsInvalid {
                        string_index: i,
                        start: s,
                        end: e,
                    });
                }

                Ok((s, e))
            })
            .collect::<Result<Vec<(usize, usize)>>>()?;

        Ok(Self { entries, bytes })
    }

    /// Lazily fetch the string at `index`. This returns a `Cow<str>` so that, in the common case
    /// where the underlying bytes are valid UTF-8, no allocation is performed.
    pub fn get(&self, index: usize) -> Option<Cow<'a, str>> {
        self.entries.get(index).map(|&(start, end)| {
            // SAFETY: bounds were already validated during construction of the table.
            unsafe { String::from_utf8_lossy(self.bytes.get_unchecked(start..end)) }
        })
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
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
pub struct TsgoDecoder<'a> {
    data: &'a [u8],
    header: Header,
    string_table: StringTable<'a>,
    nodes: Vec<Node<'a>>,
}

impl<'a> TsgoDecoder<'a> {
    /// Create a new decoder from binary data
    pub fn new(data: &'a [u8]) -> Result<Self> {
        let header = Self::decode_header(data)?;
        let string_table = Self::decode_string_table(data, &header)?;
        let nodes = Self::decode_nodes(data, &header, &string_table)?;

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
    fn decode_string_table(data: &'a [u8], header: &Header) -> Result<StringTable<'a>> {
        let string_offsets_start = header.string_offsets_offset as usize;
        let string_data_start = header.string_data_offset as usize;

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
        let strings: Vec<(u32, u32)> = (0..num_strings)
            .map(|_i| {
                let start_offset = cursor.read_u32::<LittleEndian>()?;
                let end_offset = cursor.read_u32::<LittleEndian>()?;

                Ok((start_offset, end_offset))
            })
            .collect::<Result<Vec<(u32, u32)>>>()?;

        StringTable::new(strings, &data[string_data_start..])
    }

    /// Decode all nodes from binary data
    fn decode_nodes(
        data: &[u8],
        header: &Header,
        string_table: &StringTable<'a>,
    ) -> Result<Vec<Node<'a>>> {
        let nodes_start = header.nodes_offset as usize;
        let buffer = data
            .get(nodes_start..)
            .ok_or(DecoderError::InvalidDataOffset {
                offset: nodes_start,
                buffer_size: data.len(),
            })?;

        let mut cursor = Cursor::new(buffer);
        let num_nodes = buffer.len() / NODE_SIZE;

        let nodes: Vec<Node> = (0..num_nodes)
            .map(|_i| {
                let kind_raw = cursor.read_u32::<LittleEndian>()?;
                let kind = match kind_raw {
                    SYNTAX_KIND_NODE_LIST => NodeKind::NodeList,
                    _ => NodeKind::SyntaxKind(
                        SyntaxKind::from_repr(kind_raw as i16).unwrap_or(SyntaxKind::Unknown),
                    ),
                };

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
        kind: &NodeKind,
        node_data: u32,
        string_table: &StringTable<'a>,
        data: &[u8],
        header: &Header,
    ) -> Result<Option<Cow<'a, str>>> {
        let data_type = node_data & NODE_DATA_TYPE_MASK;

        match data_type {
            NODE_DATA_TYPE_STRING => {
                let string_index = (node_data & NODE_DATA_STRING_INDEX_MASK) as usize / 2;
                Ok(string_table.get(string_index))
            }
            NODE_DATA_TYPE_EXTENDED_DATA => match kind {
                NodeKind::SyntaxKind(SyntaxKind::SourceFile)
                | NodeKind::SyntaxKind(SyntaxKind::TemplateHead)
                | NodeKind::SyntaxKind(SyntaxKind::TemplateMiddle)
                | NodeKind::SyntaxKind(SyntaxKind::TemplateTail) => {
                    let extended_data_offset = header.extended_data_offset as usize
                        + (node_data & NODE_EXTENDED_DATA_MASK) as usize;

                    if extended_data_offset + 4 <= data.len() {
                        let mut cursor = Cursor::new(&data[extended_data_offset..]);
                        let string_index = cursor.read_u32::<LittleEndian>()? as usize / 2;
                        Ok(string_table.get(string_index))
                    } else {
                        Ok(None)
                    }
                }
                _ => Ok(None),
            },
            _ => Ok(None),
        }
    }

    #[allow(clippy::type_complexity)]
    fn decode_extended_data(
        kind: &NodeKind,
        node_data: u32,
        string_table: &StringTable<'a>,
        data: &[u8],
        header: &Header,
    ) -> Result<(
        Option<u32>,
        Option<SyntaxKind>,
        Option<TokenFlags>,
        Option<Cow<'a, str>>,
        Option<Cow<'a, str>>,
    )> {
        let data_type = node_data & NODE_DATA_TYPE_MASK;

        let flags = if kind == &NodeKind::SyntaxKind(SyntaxKind::VariableDeclarationList) {
            Some((node_data & (1 << 24 | 1 << 25)) >> 24)
        } else {
            None
        };

        let token = if kind == &NodeKind::SyntaxKind(SyntaxKind::ImportAttributes) {
            if (node_data & (1 << 25)) != 0 {
                Some(SyntaxKind::AssertKeyword)
            } else {
                Some(SyntaxKind::WithKeyword)
            }
        } else {
            None
        };

        let mut template_flags: Option<TokenFlags> = None;
        let mut file_name: Option<Cow<'a, str>> = None;
        let mut raw_text: Option<Cow<'a, str>> = None;

        if data_type == NODE_DATA_TYPE_EXTENDED_DATA {
            let extended_data_offset = header.extended_data_offset as usize
                + (node_data & NODE_EXTENDED_DATA_MASK) as usize;

            match kind {
                NodeKind::SyntaxKind(SyntaxKind::TemplateHead)
                | NodeKind::SyntaxKind(SyntaxKind::TemplateMiddle)
                | NodeKind::SyntaxKind(SyntaxKind::TemplateTail) => {
                    if extended_data_offset + 12 <= data.len() {
                        let mut cursor = Cursor::new(&data[extended_data_offset..]);
                        cursor.set_position(4); // raw_text is at offset 4
                        let raw_text_index = cursor.read_u32::<LittleEndian>()? as usize / 2;
                        raw_text = string_table.get(raw_text_index);

                        let raw = cursor.read_u32::<LittleEndian>()?;
                        template_flags = TokenFlags::from_bits(raw);
                    }
                }
                NodeKind::SyntaxKind(SyntaxKind::SourceFile) => {
                    if extended_data_offset + 8 <= data.len() {
                        let mut cursor = Cursor::new(&data[extended_data_offset..]);
                        cursor.set_position(4); // file_name is at offset 4
                        let file_name_index = cursor.read_u32::<LittleEndian>()? as usize / 2;
                        file_name = string_table.get(file_name_index);
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
    pub fn string_table(&self) -> &StringTable<'a> {
        &self.string_table
    }

    /// Get header
    pub fn header(&self) -> &Header {
        &self.header
    }

    /// Get raw data (for debugging)
    pub fn data(&self) -> &[u8] {
        self.data
    }
}
