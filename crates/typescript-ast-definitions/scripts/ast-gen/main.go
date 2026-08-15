package main

import (
	"fmt"
	"go/ast"
	"go/parser"
	"go/token"
	"log"
	"os"
	"path/filepath"
	"strings"
)

func main() {
	// Parse command line arguments
	if len(os.Args) < 3 {
		fmt.Fprintf(os.Stderr, "Usage: %s <tsgo-path> <output-dir>\n", os.Args[0])
		os.Exit(1)
	}

	tsgoPath := os.Args[1]
	outputDir := os.Args[2]

	// Ensure output directory exists
	if err := os.MkdirAll(outputDir, 0755); err != nil {
		log.Fatalf("Failed to create output directory: %v", err)
	}

	// Parse the AST files
	astPath := filepath.Join(tsgoPath, "internal", "ast")

	// Process different aspects of the AST
	processors := []Processor{
		NewFlagsProcessor(),
		NewKindProcessor(),
		NewNodeProcessor(),
	}

	// Parse all Go files in the AST directory
	fileSet, files, err := parseASTFiles(astPath)
	if err != nil {
		log.Fatalf("Failed to parse AST files: %v", err)
	}

	// Run each processor
	for _, processor := range processors {
		// Special handling for NodeProcessor which needs the fileSet
		if nodeProcessor, ok := processor.(*NodeProcessor); ok {
			nodeProcessor.fileSet = fileSet
		}
		if err := processor.Process(files); err != nil {
			log.Fatalf("Failed to process: %v", err)
		}

		// Generate Rust code
		rustCode := processor.GenerateRust()
		outputFile := filepath.Join(outputDir, processor.OutputFile())

		if err := os.WriteFile(outputFile, []byte(rustCode), 0644); err != nil {
			log.Fatalf("Failed to write %s: %v", outputFile, err)
		}

		fmt.Printf("Generated %s\n", outputFile)
	}
}

// Processor interface for different types of code generation
type Processor interface {
	Process(files map[string]*ast.File) error
	GenerateRust() string
	OutputFile() string
}

// parseASTFiles parses all Go files in the given directory
func parseASTFiles(dir string) (*token.FileSet, map[string]*ast.File, error) {
	fset := token.NewFileSet()
	files := make(map[string]*ast.File)

	err := filepath.Walk(dir, func(path string, info os.FileInfo, err error) error {
		if err != nil {
			return err
		}

		if strings.HasSuffix(path, ".go") && !strings.HasSuffix(path, "_test.go") {
			file, err := parser.ParseFile(fset, path, nil, parser.ParseComments)
			if err != nil {
				return fmt.Errorf("failed to parse %s: %w", path, err)
			}
			files[path] = file
		}

		return nil
	})

	return fset, files, err
}
