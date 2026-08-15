/// Placeholder for Symbol type - this would need to be properly implemented
#[derive(Debug, Clone, PartialEq)]
pub struct Symbol;

/// Language variant for TypeScript/JavaScript
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LanguageVariant {
    Standard,
    JSX,
}

/// Script kind for TypeScript/JavaScript
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScriptKind {
    Unknown,
    JS,
    JSX,
    TS,
    TSX,
    External,
    Deferred,
}

/// Tristate enum
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tristate {
    False,
    True,
    Unknown,
}

/// Resolution mode
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ResolutionMode {
    None,
    ESM,
    CommonJS,
}

/// Text position
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextPos {
    pub line: usize,
    pub character: usize,
}

/// Pattern type
#[derive(Debug, Clone, PartialEq)]
pub struct Pattern;

#[derive(Debug, Clone, PartialEq)]
pub struct TextRange {
    pub start: usize,
    pub end: usize,
}

pub mod base_types;
pub mod node;
