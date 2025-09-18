package main

import (
	"fmt"
	"go/ast"
	"go/token"
	"sort"
	"strings"
)

// PropertyInfo holds information about a struct property
type PropertyInfo struct {
	Name         string
	GoType       string
	RustType     string
	Comment      string
	IsOptional   bool
	IsSlice      bool
	ElementType  string
	SpecificType string // More specific type based on comment analysis
}

// TypeAlias represents a Go type alias
type TypeAlias struct {
	Name        string
	TargetType  string
	Comment     string
	Category    string   // Primary category (Expression, Statement, etc.)
	Subcategory string   // Subcategory (LeftHandSideExpression, etc.)
	KindRanges  []string // Associated Kind ranges
}

// NodeCategory represents a category of nodes with hierarchy
type NodeCategory struct {
	Name       string
	Parent     string
	Children   []string
	NodeTypes  []string
	KindRanges []KindRange
}

// KindRange represents a range of SyntaxKind values
type KindRange struct {
	Start string
	End   string
	Name  string
}

// NodeInfo holds information about an AST node struct
type NodeInfo struct {
	Name            string
	Comment         string
	Properties      []PropertyInfo
	BaseTypes       []string
	IsExpression    bool
	IsStatement     bool
	IsTypeNode      bool
	IsDeclaration   bool
	Category        string // Primary category
	Subcategory     string // More specific subcategory
	NeedsAllocation bool   // Whether this struct needs allocation (has allocatable fields)
}

// NodeProcessor processes AST node struct definitions
type NodeProcessor struct {
	fileSet     *token.FileSet          // File set for parsing and comment extraction
	nodes       map[string]NodeInfo     // Map of struct name to node info
	typeAliases map[string]TypeAlias    // Map of type alias name to alias info
	categories  map[string]NodeCategory // Map of category name to category info
	kindRanges  map[string]KindRange    // Map of range name to kind range
}

// NewNodeProcessor creates a new node processor
func NewNodeProcessor() *NodeProcessor {
	return &NodeProcessor{
		nodes:       make(map[string]NodeInfo),
		typeAliases: make(map[string]TypeAlias),
		categories:  make(map[string]NodeCategory),
		kindRanges:  make(map[string]KindRange),
	}
}

// Process extracts node information from Go AST files
func (p *NodeProcessor) Process(files map[string]*ast.File) error {
	// First pass: parse type aliases and kind ranges
	for filename, file := range files {
		if strings.Contains(filename, "ast.go") {
			p.parseTypeAliases(file)
		}
		if strings.Contains(filename, "utilities.go") {
			p.parseCategoryFunctions(file)
		}
	}

	// Second pass: collect all struct definitions
	for filename, file := range files {
		if strings.Contains(filename, "ast.go") {
			p.processFile(file)
		}
	}

	// Third pass: determine node categories using enhanced analysis
	p.categorizeNodesAdvanced()

	// Fourth pass: build category hierarchy
	p.buildCategoryHierarchy()

	return nil
}

// processFile processes a single file for node struct definitions
func (p *NodeProcessor) processFile(file *ast.File) {
	ast.Inspect(file, func(n ast.Node) bool {
		if typeSpec, ok := n.(*ast.TypeSpec); ok {
			if structType, ok := typeSpec.Type.(*ast.StructType); ok {
				p.processStruct(typeSpec.Name.Name, structType, typeSpec.Comment)
			}
		}
		return true
	})
}

// processStruct processes a struct definition
func (p *NodeProcessor) processStruct(name string, structType *ast.StructType, comment *ast.CommentGroup) {
	// Skip if this is not an AST node (heuristic: must have certain patterns)
	if !p.isASTNode(name, structType) {
		return
	}

	node := NodeInfo{
		Name:       name,
		Properties: make([]PropertyInfo, 0),
		BaseTypes:  make([]string, 0),
	}

	// Extract comment
	if comment != nil {
		node.Comment = strings.TrimSpace(comment.Text())
	}

	// Process fields
	for _, field := range structType.Fields.List {
		if len(field.Names) == 0 {
			// Embedded struct (base type)
			baseType := p.extractTypeName(field.Type)
			if baseType != "" {
				node.BaseTypes = append(node.BaseTypes, baseType)
			}
		} else {
			// Regular property
			for _, fieldName := range field.Names {
				if !ast.IsExported(fieldName.Name) {
					continue // Skip unexported fields
				}

				prop := p.processProperty(fieldName.Name, field.Type, field.Comment)
				node.Properties = append(node.Properties, prop)
			}
		}
	}

	// Determine if this struct needs allocation
	node.NeedsAllocation = p.needsAllocation(node)

	p.nodes[name] = node
}

// needsAllocation determines if a struct needs allocation based on its properties
func (p *NodeProcessor) needsAllocation(node NodeInfo) bool {
	for _, prop := range node.Properties {
		// Check if the property type requires allocation
		rustType := prop.RustType
		if prop.SpecificType != "" {
			rustType = prop.SpecificType
		}

		// Types that require allocation - only heap-allocated types
		if strings.Contains(rustType, "Box<") ||
			strings.Contains(rustType, "Vec<") ||
			strings.Contains(rustType, "NodeList<") {
			return true
		}

		// Option types don't necessarily need allocation if their inner type doesn't
		// Only check if the inner type needs allocation
		if strings.Contains(rustType, "Option<") {
			innerType := strings.TrimPrefix(strings.TrimSuffix(rustType, ">"), "Option<")
			if strings.Contains(innerType, "Box<") ||
				strings.Contains(innerType, "Vec<") ||
				strings.Contains(innerType, "NodeList<") {
				return true
			}
		}

		// Check for specific types that need allocation
		if strings.Contains(rustType, "Expression<") ||
			strings.Contains(rustType, "Statement<") ||
			strings.Contains(rustType, "TypeNode<") ||
			strings.Contains(rustType, "Declaration<") ||
			strings.Contains(rustType, "AstNode<") {
			return true
		}
	}
	return false
}

// isASTNode determines if a struct represents an AST node
func (p *NodeProcessor) isASTNode(name string, structType *ast.StructType) bool {
	// Heuristics to identify AST nodes:
	// 1. Has embedded base types like "ExpressionBase", "StatementBase", etc.
	// 2. Or has common AST node patterns
	for _, field := range structType.Fields.List {
		if len(field.Names) == 0 {
			// Embedded struct
			baseType := p.extractTypeName(field.Type)
			if strings.HasSuffix(baseType, "Base") {
				return true
			}
		}
	}

	// Additional heuristics
	if strings.Contains(name, "Expression") || strings.Contains(name, "Statement") ||
		strings.Contains(name, "Declaration") || strings.Contains(name, "Type") {
		return true
	}

	return false
}

// processProperty processes a struct property
func (p *NodeProcessor) processProperty(name string, fieldType ast.Expr, comment *ast.CommentGroup) PropertyInfo {
	prop := PropertyInfo{
		Name: name,
	}

	// Extract comment
	if comment != nil {
		prop.Comment = strings.TrimSpace(comment.Text())
	}

	// Analyze type
	p.analyzePropertyType(&prop, fieldType)

	// Parse comment for more specific type information
	p.parseCommentType(&prop)

	return prop
}

// parseCommentType parses the comment to determine more specific types
func (p *NodeProcessor) parseCommentType(prop *PropertyInfo) {
	if prop.Comment == "" {
		return
	}

	comment := strings.TrimSpace(prop.Comment)

	// Parse specific type patterns from comments
	if strings.Contains(comment, "NodeList[") {
		// Extract element type from NodeList[*TypeNode] pattern
		start := strings.Index(comment, "NodeList[")
		if start != -1 {
			rest := comment[start+9:]
			end := strings.Index(rest, "]")
			if end != -1 {
				elementType := strings.TrimPrefix(rest[:end], "*")
				prop.SpecificType = fmt.Sprintf("NodeList<%s, A>", p.mapCommentTypeToRust(elementType))

				if strings.Contains(comment, "Optional") || prop.IsOptional {
					prop.SpecificType = fmt.Sprintf("Option<%s>", prop.SpecificType)
				}
			}
		}
	} else if strings.Contains(comment, "Expression") {
		prop.SpecificType = "Expression<'a, A>"
	} else if strings.Contains(comment, "Statement") {
		prop.SpecificType = "Statement<'a, A>"
	} else if strings.Contains(comment, "TypeNode") {
		prop.SpecificType = "TypeNode<'a, A>"
	} else if strings.Contains(comment, "Declaration") {
		prop.SpecificType = "Declaration<'a, A>"
	} else if strings.Contains(comment, "TokenNode") {
		if prop.IsOptional {
			prop.SpecificType = "Option<Token<'a, A>>"
		} else {
			prop.SpecificType = "Token<'a, A>"
		}
	}
}

// mapCommentTypeToRust maps comment type strings to Rust types
func (p *NodeProcessor) mapCommentTypeToRust(commentType string) string {
	switch commentType {
	case "Expression":
		return "Expression<'a, A>"
	case "Statement":
		return "Statement<'a, A>"
	case "TypeNode":
		return "TypeNode<'a, A>"
	case "Declaration":
		return "Declaration<'a, A>"
	case "TokenNode":
		return "Token<'a, A>"
	default:
		// For specific node types, return the node type
		if strings.HasSuffix(commentType, "Node") {
			return fmt.Sprintf("%s<'a, A>", commentType)
		}
		return "AstNode<'a, A>"
	}
}

// analyzePropertyType analyzes a property type and sets the appropriate fields
func (p *NodeProcessor) analyzePropertyType(prop *PropertyInfo, fieldType ast.Expr) {
	switch t := fieldType.(type) {
	case *ast.StarExpr:
		// Pointer type - indicates optional
		prop.IsOptional = true
		prop.GoType = "*" + p.extractTypeName(t.X)
		prop.RustType = p.convertToRustType(prop.GoType, true, false)

	case *ast.Ident:
		// Simple identifier type
		prop.GoType = t.Name
		prop.RustType = p.convertToRustType(prop.GoType, false, false)

	case *ast.SelectorExpr:
		// Package-qualified type (e.g., core.TextRange)
		pkg := p.extractTypeName(t.X)
		typeName := t.Sel.Name
		prop.GoType = pkg + "." + typeName
		prop.RustType = p.convertToRustType(prop.GoType, false, false)

	case *ast.ArrayType:
		// Slice type
		prop.IsSlice = true
		prop.ElementType = p.extractTypeName(t.Elt)
		prop.GoType = "[]" + prop.ElementType
		prop.RustType = p.convertToRustType(prop.GoType, false, true)

	default:
		// Fallback
		prop.GoType = p.extractTypeName(fieldType)
		prop.RustType = p.convertToRustType(prop.GoType, false, false)
	}
}

// extractTypeName extracts the type name from an AST expression
func (p *NodeProcessor) extractTypeName(expr ast.Expr) string {
	switch t := expr.(type) {
	case *ast.Ident:
		return t.Name
	case *ast.StarExpr:
		return "*" + p.extractTypeName(t.X)
	case *ast.SelectorExpr:
		return p.extractTypeName(t.X) + "." + t.Sel.Name
	case *ast.ArrayType:
		return "[]" + p.extractTypeName(t.Elt)
	default:
		return ""
	}
}

// convertToRustType converts a Go type to its Rust equivalent
func (p *NodeProcessor) convertToRustType(goType string, isOptional, isSlice bool) string {
	var baseType string

	// Strip pointer prefix for analysis
	cleanType := strings.TrimPrefix(goType, "*")

	// Check if this is a type alias we've parsed
	if alias, exists := p.typeAliases[cleanType]; exists {
		// For type aliases, use the alias name with proper lifetime
		if strings.Contains(alias.TargetType, "NodeList") {
			baseType = fmt.Sprintf("%s<'a, A>", cleanType)
		} else {
			baseType = fmt.Sprintf("%s<'a, A>", cleanType)
		}
	} else {
		// Type mapping for non-alias types
		switch {
		case cleanType == "string":
			baseType = "String"
		case cleanType == "bool":
			baseType = "bool"
		case cleanType == "int" || cleanType == "int64":
			baseType = "i64"
		case cleanType == "uint64":
			baseType = "u64"
		case cleanType == "core.TextRange":
			baseType = "TextRange"
		case cleanType == "core.LanguageVariant":
			baseType = "LanguageVariant"
		case cleanType == "core.ScriptKind":
			baseType = "ScriptKind"
		case cleanType == "core.Tristate":
			baseType = "Tristate"
		case cleanType == "core.ResolutionMode":
			baseType = "ResolutionMode"
		case cleanType == "core.TextPos":
			baseType = "TextPos"
		case cleanType == "core.Pattern":
			baseType = "Pattern"
		case cleanType == "NodeList":
			baseType = "NodeList<AstNode<'a, A>, A>"
		case cleanType == "Symbol":
			baseType = "Symbol"
		case cleanType == "map[string]string":
			baseType = "HashMap<String, String>"
		case cleanType == "collections.Set[string]":
			baseType = "HashSet<String>"
		case cleanType == "Kind":
			baseType = "SyntaxKind"
		case cleanType == "TokenFlags":
			baseType = "TokenFlags"
		case cleanType == "any":
			baseType = "AstNode<'a, A>"
		case strings.HasSuffix(cleanType, "Node"):
			baseType = "Box<AstNode<'a, A>>"
		case cleanType == "Expression":
			baseType = "Box<AstNode<'a, A>>"
		case cleanType == "Statement":
			baseType = "Box<AstNode<'a, A>>"
		case cleanType == "TypeNode":
			baseType = "Box<AstNode<'a, A>>"
		case cleanType == "Declaration":
			baseType = "Box<AstNode<'a, A>>"
		default:
			// Default to the type name for unknown types
			baseType = cleanType
		}
	}

	// Handle slices
	if isSlice || strings.HasPrefix(goType, "[]") {
		if strings.Contains(baseType, "AstNode") {
			baseType = "NodeList<AstNode<'a, A>, A>"
		} else {
			baseType = fmt.Sprintf("Vec<%s, A>", baseType)
		}
	}

	// Handle optionals
	if isOptional || strings.HasPrefix(goType, "*") {
		baseType = fmt.Sprintf("Option<%s>", baseType)
	}

	return baseType
}

// GenerateRust generates Rust code for all AST nodes with a single giant enum
func (p *NodeProcessor) GenerateRust() string {
	var output strings.Builder

	output.WriteString("// Generated by ast-gen - DO NOT EDIT\n\n")
	output.WriteString("use crate::{core::*, core::base_types::*, core::node::NodeList, SyntaxKind, TokenFlags};\n\n")
	output.WriteString("use allocator_api2::alloc::{Allocator, Global};\n")
	output.WriteString("use allocator_api2::boxed::Box;\n")
	output.WriteString("use std::collections::{HashMap, HashSet};\n")
	output.WriteString("use core::marker::PhantomData;\n\n")

	// NodeBase is now defined in core::base_types as a newtype wrapper around Node
	output.WriteString("/// NodeBase is defined in core::base_types as a newtype wrapper around Node\n\n")

	// Sort nodes for consistent output
	var nodeNames []string
	for name := range p.nodes {
		nodeNames = append(nodeNames, name)
	}
	sort.Strings(nodeNames)

	// Generate type aliases
	for _, alias := range p.typeAliases {
		rustTarget := p.parseCommentForSpecificType(alias.Comment, alias.TargetType, alias.Name)
		if rustTarget != "" {
			// Add comment if available
			if alias.Comment != "" {
				output.WriteString(fmt.Sprintf("/// %s\n", alias.Comment))
			}
			fmt.Fprintf(&output, "pub type %s<'a, A: Allocator + Clone = Global> = %s;\n", alias.Name, rustTarget)
		}
	}
	output.WriteString("\n")

	// Generate individual struct definitions (skip base types)
	baseTypeNames := map[string]bool{
		"NodeBase": true, "DeclarationBase": true, "ExportableBase": true,
		"ModifiersBase": true, "LocalsContainerBase": true, "FlowNodeBase": true,
		"FunctionLikeBase": true, "StatementBase": true, "ExpressionBase": true,
		"TypeNodeBase": true, "ClassElementBase": true, "TypeElementBase": true,
		"ObjectLiteralElementBase": true, "ClassLikeBase": true,
	}

	for _, name := range nodeNames {
		// Skip base types since they're already generated
		if baseTypeNames[name] {
			continue
		}
		node := p.nodes[name]
		p.generateNodeStruct(&output, node)
		output.WriteString("\n")
	}

	// Generate the single giant AstNode enum
	p.generateSingleAstNodeEnum(&output, nodeNames)

	// Generate helper implementations for the single enum
	p.generateSingleEnumHelperImpls(&output, nodeNames)

	return output.String()
}

// determineNodeBases determines which base types a node should embed based on its name and category
func (p *NodeProcessor) determineNodeBases(nodeName, category string) []string {
	var bases []string

	// Don't include base types as their own base - they are base types!
	baseTypeNames := map[string]bool{
		"NodeBase": true, "DeclarationBase": true, "ExportableBase": true,
		"ModifiersBase": true, "LocalsContainerBase": true, "FlowNodeBase": true,
		"FunctionLikeBase": true, "StatementBase": true, "ExpressionBase": true,
		"TypeNodeBase": true, "ClassElementBase": true, "TypeElementBase": true,
		"ObjectLiteralElementBase": true, "ClassLikeBase": true,
	}

	if baseTypeNames[nodeName] {
		// This is a base type itself, don't add any bases
		return bases
	}

	// Determine primary base types based on naming patterns and categories
	if strings.HasSuffix(nodeName, "Statement") || category == "Statement" {
		bases = append(bases, "StatementBase")
	} else if strings.HasSuffix(nodeName, "Expression") || category == "Expression" ||
		nodeName == "Identifier" || nodeName == "PrivateIdentifier" ||
		strings.Contains(nodeName, "Literal") {
		bases = append(bases, "ExpressionBase")
	} else if strings.HasSuffix(nodeName, "TypeNode") || strings.Contains(nodeName, "Type") || category == "TypeNode" ||
		nodeName == "TypeParameter" || strings.HasSuffix(nodeName, "Type") {
		bases = append(bases, "TypeNodeBase")
	} else {
		// Default to NodeBase for nodes that don't fit other categories
		bases = append(bases, "NodeBase")
	}

	// Add specific base types based on patterns (avoid self-inclusion)
	if (strings.HasSuffix(nodeName, "Declaration") || category == "Declaration") && nodeName != "DeclarationBase" {
		bases = append(bases, "DeclarationBase")
	}

	if strings.Contains(nodeName, "Function") || strings.Contains(nodeName, "Method") ||
		strings.Contains(nodeName, "Constructor") || strings.Contains(nodeName, "Accessor") {
		bases = append(bases, "FunctionLikeBase<'a, A>")
	}

	if strings.Contains(nodeName, "Class") && (strings.Contains(nodeName, "Declaration") || strings.Contains(nodeName, "Expression")) {
		bases = append(bases, "ClassLikeBase<'a, A>")
	}

	return bases
}

// generateNodeStruct generates a Rust struct for an AST node using composition
func (p *NodeProcessor) generateNodeStruct(output *strings.Builder, node NodeInfo) {
	if node.Comment != "" {
		output.WriteString(fmt.Sprintf("/// %s\n", node.Comment))
	}
	output.WriteString("#[derive(Debug, Clone, PartialEq)]\n")
	output.WriteString(fmt.Sprintf("pub struct %s<'a, A: Allocator + Clone = Global> {\n", node.Name))

	// Determine which base types this node should embed
	bases := p.determineNodeBases(node.Name, node.Category)

	// Add base struct fields
	for _, base := range bases {
		baseFieldName := toSnakeCase(strings.TrimSuffix(base, "<'a, A>"))
		// Use the hardcoded base types from core::base_types with proper lifetime and generic parameters
		if strings.Contains(base, "<'a, A>") {
			output.WriteString(fmt.Sprintf("    pub %s: crate::core::base_types::%s,\n", baseFieldName, base))
		} else {
			// Add lifetime and generic parameters for base types that need them
			output.WriteString(fmt.Sprintf("    pub %s: crate::core::base_types::%s<'a, A>,\n", baseFieldName, base))
		}
	}

	// Properties
	for _, prop := range node.Properties {
		// Skip core fields that are now handled by base types
		propName := strings.ToLower(prop.Name)
		if propName == "textrange" || propName == "parent" || propName == "id" {
			continue
		}

		if prop.Comment != "" {
			output.WriteString(fmt.Sprintf("    /// %s\n", prop.Comment))
		}
		rustName := toSnakeCase(prop.Name)

		// Use specific type if available, otherwise fall back to RustType
		rustType := prop.RustType
		if prop.SpecificType != "" {
			rustType = prop.SpecificType
		}

		fmt.Fprintf(output, "    pub %s: %s,\n", rustName, rustType)
	}

	// Add PhantomData if the struct doesn't need allocation
	if !node.NeedsAllocation {
		output.WriteString("    pub _phantom: PhantomData<&'a A>,\n")
	}

	output.WriteString("}\n")
}

// OutputFile returns the output file name
func (p *NodeProcessor) OutputFile() string {
	return "nodes.rs"
}

// toSnakeCase converts PascalCase to snake_case and handles Rust keywords
func toSnakeCase(s string) string {
	var result []rune
	for i, r := range s {
		if i > 0 && 'A' <= r && r <= 'Z' {
			result = append(result, '_')
		}
		if 'A' <= r && r <= 'Z' {
			result = append(result, r-'A'+'a')
		} else {
			result = append(result, r)
		}
	}

	snakeCase := string(result)

	// Handle Rust keywords by adding r# prefix
	rustKeywords := map[string]bool{
		"type":     true,
		"match":    true,
		"fn":       true,
		"let":      true,
		"mut":      true,
		"const":    true,
		"static":   true,
		"if":       true,
		"else":     true,
		"while":    true,
		"for":      true,
		"loop":     true,
		"break":    true,
		"continue": true,
		"return":   true,
		"struct":   true,
		"enum":     true,
		"trait":    true,
		"impl":     true,
		"mod":      true,
		"pub":      true,
		"use":      true,
		"super":    true,
		"self":     true,
		"Self":     true,
		"crate":    true,
		"extern":   true,
		"unsafe":   true,
		"async":    true,
		"await":    true,
		"move":     true,
		"ref":      true,
		"where":    true,
		"as":       true,
		"in":       true,
		"box":      true,
		"dyn":      true,
	}

	if rustKeywords[snakeCase] {
		return "r#" + snakeCase
	}

	return snakeCase
}

// parseCommentForSpecificType parses comments to extract specific type information
func (p *NodeProcessor) parseCommentForSpecificType(comment, targetType, aliasName string) string {
	if comment == "" {
		// Fallback to generic type based on target
		switch targetType {
		case "Node":
			return "AstNode<'a, A>"
		case "NodeList":
			return "NodeList<AstNode<'a, A>, A>"
		default:
			if strings.Contains(targetType, "NodeList") {
				return "NodeList<AstNode<'a, A>, A>"
			}
			return "AstNode<'a, A>"
		}
	}

	// Check for NodeList patterns first
	if strings.Contains(comment, "NodeList[") {
		// Extract the type inside NodeList[*Type] or NodeList[Type]
		start := strings.Index(comment, "NodeList[")
		if start != -1 {
			end := strings.Index(comment[start:], "]")
			if end != -1 {
				innerType := comment[start+9 : start+end]      // Skip "NodeList["
				innerType = strings.TrimPrefix(innerType, "*") // Remove pointer prefix

				// Convert to specific Rust type if we know it
				rustInnerType := p.convertSpecificNodeType(innerType)
				return fmt.Sprintf("NodeList<%s, A>", rustInnerType)
			}
		}
		return "NodeList<AstNode<'a, A>, A>"
	}

	// Check for union types (Type1 | Type2 | Type3)
	if strings.Contains(comment, " | ") {
		types := strings.Split(comment, " | ")
		if len(types) > 1 {
			// For union types, we'll use an enum in the future, but for now use AstNode
			// TODO: Generate proper union enums for these
			return "AstNode<'a, A>"
		}
	}

	// Check for "Node with XxxBase" pattern
	if strings.Contains(comment, "Node with ") && strings.Contains(comment, "Base") {
		return "AstNode<'a, A>"
	}

	// Check for subset patterns
	if strings.Contains(comment, "subset of ") {
		return "AstNode<'a, A>"
	}

	// Check if it's a single specific type
	trimmed := strings.TrimSpace(comment)
	if !strings.Contains(trimmed, " ") && !strings.Contains(trimmed, "|") {
		// Single type, convert to Rust type
		rustType := p.convertSpecificNodeType(trimmed)
		return rustType
	}

	// Default fallback
	switch targetType {
	case "Node":
		return "AstNode<'a, A>"
	case "NodeList":
		return "NodeList<AstNode<'a, A>, A>"
	default:
		if strings.Contains(targetType, "NodeList") {
			return "NodeList<AstNode<'a, A>, A>"
		}
		return "AstNode<'a, A>"
	}
}

// convertSpecificNodeType converts a specific node type name to its Rust equivalent
func (p *NodeProcessor) convertSpecificNodeType(typeName string) string {
	// Remove common prefixes/suffixes
	typeName = strings.TrimSpace(typeName)

	// Check if this is a known struct in our nodes map
	if _, exists := p.nodes[typeName]; exists {
		return fmt.Sprintf("%s<'a, A>", typeName)
	}

	// Check if this is a type alias we know about
	if _, exists := p.typeAliases[typeName]; exists {
		return fmt.Sprintf("%s<'a, A>", typeName)
	}

	// Handle special cases
	switch typeName {
	case "Expression":
		return "Expression<'a, A>"
	case "Statement":
		return "Statement<'a, A>"
	case "TypeNode":
		return "TypeNode<'a, A>"
	case "Declaration":
		return "Declaration<'a, A>"
	case "Token":
		return "Token<'a, A>"
	default:
		// For unknown types, check if it ends with common suffixes
		if strings.HasSuffix(typeName, "Node") || strings.HasSuffix(typeName, "Declaration") ||
			strings.HasSuffix(typeName, "Expression") || strings.HasSuffix(typeName, "Statement") ||
			strings.HasSuffix(typeName, "Type") {
			return fmt.Sprintf("%s<'a, A>", typeName)
		}

		// Default to generic AstNode
		return "AstNode<'a, A>"
	}
}

// parseTypeAliases parses type aliases from Go AST files
func (p *NodeProcessor) parseTypeAliases(file *ast.File) {
	// Create a comment map for extracting inline comments
	commentMap := ast.NewCommentMap(p.fileSet, file, file.Comments)

	ast.Inspect(file, func(n ast.Node) bool {
		if typeDecl, ok := n.(*ast.TypeSpec); ok {
			// Check if this is a type alias (type Name = TargetType)
			if typeDecl.Assign != 0 {
				aliasName := typeDecl.Name.Name
				targetType := p.extractTypeName(typeDecl.Type)

				// Extract comment if available
				var comment string

				// First try block comment above declaration
				if typeDecl.Doc != nil {
					comment = strings.TrimSpace(typeDecl.Doc.Text())
				}

				// If no block comment, try to find inline comment
				if comment == "" {
					if comments := commentMap[typeDecl]; len(comments) > 0 {
						// Look for inline comments (starting with //)
						for _, commentGroup := range comments {
							for _, c := range commentGroup.List {
								if strings.HasPrefix(c.Text, "//") {
									// Extract the text after // and trim whitespace
									inlineComment := strings.TrimSpace(c.Text[2:])
									if inlineComment != "" {
										comment = inlineComment
										break
									}
								}
							}
							if comment != "" {
								break
							}
						}
					}
				}

				// Determine category based on target type and comment
				category := p.determineAliasCategory(aliasName, targetType, comment)

				alias := TypeAlias{
					Name:        aliasName,
					TargetType:  targetType,
					Comment:     comment,
					Category:    category,
					Subcategory: p.determineAliasSubcategory(aliasName, targetType, comment),
				}

				p.typeAliases[aliasName] = alias
			}
		}
		return true
	})
}

// determineAliasCategory determines the primary category of a type alias
func (p *NodeProcessor) determineAliasCategory(aliasName, targetType, comment string) string {
	// Check for common patterns in the alias name
	if strings.Contains(aliasName, "Statement") {
		return "Statement"
	}
	if strings.Contains(aliasName, "Expression") {
		return "Expression"
	}
	if strings.Contains(aliasName, "TypeNode") || strings.Contains(aliasName, "Type") {
		return "TypeNode"
	}
	if strings.Contains(aliasName, "Declaration") {
		return "Declaration"
	}
	if strings.Contains(aliasName, "Element") {
		return "Element"
	}
	if strings.Contains(aliasName, "List") {
		return "List"
	}

	// Check target type for clues
	if strings.Contains(targetType, "Node") {
		return "Node"
	}

	return "Other"
}

// determineAliasSubcategory determines the subcategory of a type alias
func (p *NodeProcessor) determineAliasSubcategory(aliasName, targetType, comment string) string {
	// Extract subcategory from comment if available
	if comment != "" {
		// Look for patterns like "Node with TypeNodeBase" or "UnionTypeNode | IntersectionTypeNode"
		if strings.Contains(comment, "with") {
			parts := strings.Split(comment, "with")
			if len(parts) > 1 {
				return strings.TrimSpace(parts[1])
			}
		}
		if strings.Contains(comment, "|") {
			// For union types, use the first part as subcategory
			parts := strings.Split(comment, "|")
			if len(parts) > 1 {
				return strings.TrimSpace(parts[0])
			}
		}
	}

	// Fallback to alias name analysis
	if strings.Contains(aliasName, "LeftHandSide") {
		return "LeftHandSideExpression"
	}
	if strings.Contains(aliasName, "Literal") {
		return "Literal"
	}
	if strings.Contains(aliasName, "Access") {
		return "AccessExpression"
	}

	return ""
}

// parseCategoryFunctions parses categorization functions from utilities.go
func (p *NodeProcessor) parseCategoryFunctions(file *ast.File) {
	ast.Inspect(file, func(n ast.Node) bool {
		if funcDecl, ok := n.(*ast.FuncDecl); ok {
			funcName := funcDecl.Name.Name

			// Look for categorization functions
			if strings.HasSuffix(funcName, "Kind") &&
				(strings.Contains(funcName, "Expression") ||
					strings.Contains(funcName, "Statement") ||
					strings.Contains(funcName, "TypeNode") ||
					strings.Contains(funcName, "Declaration")) {

				// Extract category information from function name
				var category string
				if strings.Contains(funcName, "Expression") {
					category = "Expression"
				} else if strings.Contains(funcName, "Statement") {
					category = "Statement"
				} else if strings.Contains(funcName, "TypeNode") {
					category = "TypeNode"
				} else if strings.Contains(funcName, "Declaration") {
					category = "Declaration"
				}

				if category != "" {
					nodeCategory := NodeCategory{
						Name: category,
					}
					p.categories[category] = nodeCategory
				}
			}
		}
		return true
	})
}

// categorizeNodesAdvanced determines node categories using enhanced analysis
func (p *NodeProcessor) categorizeNodesAdvanced() {
	for name, node := range p.nodes {
		// Determine categories based on base types
		for _, baseType := range node.BaseTypes {
			switch baseType {
			case "ExpressionBase":
				node.IsExpression = true
				node.Category = "Expression"
			case "StatementBase":
				node.IsStatement = true
				node.Category = "Statement"
			case "TypeNodeBase":
				node.IsTypeNode = true
				node.Category = "TypeNode"
			case "DeclarationBase":
				node.IsDeclaration = true
				node.Category = "Declaration"
			}
		}

		// Additional heuristics based on name patterns
		if strings.Contains(name, "Expression") && node.Category == "" {
			node.IsExpression = true
			node.Category = "Expression"
		} else if strings.Contains(name, "Statement") && node.Category == "" {
			node.IsStatement = true
			node.Category = "Statement"
		} else if strings.Contains(name, "Declaration") && node.Category == "" {
			node.IsDeclaration = true
			node.Category = "Declaration"
		} else if strings.Contains(name, "Type") && node.Category == "" {
			node.IsTypeNode = true
			node.Category = "TypeNode"
		}

		// Determine subcategories based on specific patterns
		if node.Category == "Expression" {
			if strings.Contains(name, "Call") || strings.Contains(name, "New") {
				node.Subcategory = "CallLikeExpression"
			} else if strings.Contains(name, "Literal") {
				node.Subcategory = "LiteralExpression"
			} else if strings.Contains(name, "Binary") || strings.Contains(name, "Unary") {
				node.Subcategory = "OperatorExpression"
			}
		} else if node.Category == "Declaration" {
			if strings.Contains(name, "Function") || strings.Contains(name, "Method") {
				node.Subcategory = "FunctionLikeDeclaration"
			} else if strings.Contains(name, "Class") {
				node.Subcategory = "ClassLikeDeclaration"
			}
		}

		p.nodes[name] = node
	}
}

// buildCategoryHierarchy builds the hierarchical category structure
func (p *NodeProcessor) buildCategoryHierarchy() {
	// Build hierarchy based on parsed type aliases and categories
	for categoryName, category := range p.categories {
		// Collect nodes belonging to this category
		for nodeName, node := range p.nodes {
			if node.Category == categoryName {
				category.NodeTypes = append(category.NodeTypes, nodeName)
			}
		}
		p.categories[categoryName] = category
	}
}

// generateCategoryEnums generates hierarchical category enums
func (p *NodeProcessor) generateCategoryEnums(output *strings.Builder, nodeNames []string) {
	// This function is no longer used with the single enum approach
}

// generateTopLevelAstNodeEnum generates the top-level AstNode enum
func (p *NodeProcessor) generateTopLevelAstNodeEnum(output *strings.Builder) {
	// This function is no longer used with the single enum approach
}

// generateSingleAstNodeEnum generates a single giant enum for all AST node types
func (p *NodeProcessor) generateSingleAstNodeEnum(output *strings.Builder, nodeNames []string) {
	output.WriteString("/// Top-level AST node that encompasses all TypeScript node types\n")
	output.WriteString("/// Provides a hierarchical structure for better type safety and ergonomics\n")
	output.WriteString("#[derive(Debug, Clone, PartialEq)]\n")
	output.WriteString("pub enum AstNode<'a, A: Allocator + Clone> {\n")

	for _, name := range nodeNames {
		output.WriteString(fmt.Sprintf("    %s(Box<%s<'a, A>>),\n", name, name))
	}

	output.WriteString("}\n\n")
}

// generateSingleEnumHelperImpls generates helper implementations for the single giant enum
func (p *NodeProcessor) generateSingleEnumHelperImpls(output *strings.Builder, nodeNames []string) {
	// Generate implementations for the single enum
	output.WriteString("impl<'a, A: Allocator + Clone> AstNode<'a, A> {\n")

	// Generate category type guards
	for _, name := range nodeNames {
		snakeName := toSnakeCase(name)
		output.WriteString(fmt.Sprintf("\n    pub fn is_%s(&self) -> bool {\n", snakeName))
		output.WriteString(fmt.Sprintf("        matches!(self, AstNode::%s(_))\n", name))
		output.WriteString("    }\n")

		output.WriteString(fmt.Sprintf("\n    pub fn as_%s(&self) -> Option<&%s<'a, A>> {\n", snakeName, name))
		output.WriteString("        match self {\n")
		output.WriteString(fmt.Sprintf("            AstNode::%s(node) => Some(node),\n", name))
		output.WriteString("            _ => None,\n")
		output.WriteString("        }\n")
		output.WriteString("    }\n")
	}

	output.WriteString("}\n")
}
