use std::marker::PhantomData;

use allocator_api2::{
    alloc::{Allocator, Global},
    boxed::Box,
    vec::Vec,
};

use crate::{NodeFlags, SyntaxKind, core::TextRange};

/// Unique identifier for AST nodes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub u64);

#[derive(Debug, Clone, PartialEq)]
pub struct Node<'a, A: Allocator + Clone = Global> {
    pub kind: SyntaxKind,
    pub flags: NodeFlags,
    pub location: TextRange,
    pub id: NodeId,
    pub parent: Option<Box<Node<'a, A>, A>>,
    pub _phantom: PhantomData<&'a A>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NodeList<T, A: Allocator + Clone = Global> {
    pub nodes: Vec<T, A>,
    pub text_range: TextRange,
}
