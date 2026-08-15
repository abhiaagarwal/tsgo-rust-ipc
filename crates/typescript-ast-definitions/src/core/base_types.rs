use std::{collections::HashMap, marker::PhantomData};

use allocator_api2::{
    alloc::{Allocator, Global},
    boxed::Box,
};

use crate::{
    core::{
        Symbol, TextRange,
        node::{Node, NodeId, NodeList},
    },
    generated::nodes::{
        AstNode, ClassElement, DeclarationName, HeritageClause, ParameterDeclaration, TypeNode,
        TypeParameterDeclaration,
    },
};

/// Core Node structure containing basic AST node data
/// This mirrors NodeDefault and NodeBase in Go
#[derive(Debug, Clone, PartialEq)]
pub struct NodeBase<'a, A: Allocator + Clone = Global>(pub Node<'a, A>);

/// Base for declaration nodes
#[derive(Debug, Clone, PartialEq)]
pub struct DeclarationBase<'a, A: Allocator + Clone = Global> {
    pub symbol: Option<Symbol>,
    pub _phantom: PhantomData<&'a A>,
}

/// Base for exportable nodes
#[derive(Debug, Clone, PartialEq)]
pub struct ExportableBase<'a, A: Allocator + Clone = Global> {
    pub local_symbol: Option<Symbol>,
    pub _phantom: PhantomData<&'a A>,
}

/// Base for nodes with modifiers
#[derive(Debug, Clone, PartialEq)]
pub struct ModifiersBase<'a, A: Allocator + Clone = Global> {
    pub modifiers: Option<NodeList<AstNode<'a, A>, A>>,
}

/// Base for nodes that can contain local symbols
#[derive(Debug, Clone, PartialEq)]
pub struct LocalsContainerBase<'a, A: Allocator + Clone = Global> {
    pub locals: Option<HashMap<String, Symbol>>,
    pub next_container: Option<Box<AstNode<'a, A>>>,
}

/// Base for flow control nodes
#[derive(Debug, Clone, PartialEq)]
pub struct FlowNodeBase<'a, A: Allocator + Clone = Global> {
    // Flow node data would go here
    pub _phantom: PhantomData<&'a A>,
}

/// Base for function-like nodes
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionLikeBase<'a, A: Allocator + Clone = Global> {
    pub locals_container: LocalsContainerBase<'a, A>,
    pub type_parameters: Option<NodeList<TypeParameterDeclaration<'a, A>, A>>,
    pub parameters: NodeList<ParameterDeclaration<'a, A>, A>,
    pub r#type: Option<TypeNode<'a, A>>,
    pub full_signature: Option<TypeNode<'a, A>>,
}

/// Base for statement nodes
#[derive(Debug, Clone, PartialEq)]
pub struct StatementBase<'a, A: Allocator + Clone = Global> {
    pub node_base: NodeBase<'a, A>,
    pub flow_node: Option<FlowNodeBase<'a, A>>,
}

/// Base for expression nodes
#[derive(Debug, Clone, PartialEq)]
pub struct ExpressionBase<'a, A: Allocator + Clone = Global> {
    pub node_base: NodeBase<'a, A>,
}

/// Base for type nodes
#[derive(Debug, Clone, PartialEq)]
pub struct TypeNodeBase<'a, A: Allocator + Clone = Global> {
    pub node_base: NodeBase<'a, A>,
}

/// Base for class element nodes
#[derive(Debug, Clone, PartialEq)]
pub struct ClassElementBase<'a, A: Allocator + Clone = Global> {
    _phantom: PhantomData<&'a A>,
    // Empty base struct
}

/// Base for type element nodes
#[derive(Debug, Clone, PartialEq)]
pub struct TypeElementBase<'a, A: Allocator + Clone = Global> {
    _phantom: PhantomData<&'a A>,
    // Empty base struct
}

/// Base for object literal element nodes
#[derive(Debug, Clone, PartialEq)]
pub struct ObjectLiteralElementBase<'a, A: Allocator + Clone = Global> {
    _phantom: PhantomData<&'a A>,
    // Empty base struct
}

/// Base for class-like nodes
#[derive(Debug, Clone, PartialEq)]
pub struct ClassLikeBase<'a, A: Allocator + Clone = Global> {
    pub name: Option<DeclarationName<'a, A>>,
    pub type_parameters: Option<NodeList<TypeParameterDeclaration<'a, A>, A>>,
    pub heritage_clauses: Option<NodeList<HeritageClause<'a, A>, A>>,
    pub members: NodeList<ClassElement<'a, A>, A>,
}
