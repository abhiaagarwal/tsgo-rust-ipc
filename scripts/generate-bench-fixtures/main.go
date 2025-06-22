package main

import (
	"bytes"
	"encoding/binary"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"unicode/utf16"
	"unicode/utf8"

	"github.com/microsoft/typescript-go/internal/api/encoder"
	"github.com/microsoft/typescript-go/internal/ast"
	"github.com/microsoft/typescript-go/internal/core"
	"github.com/microsoft/typescript-go/internal/parser"
	"github.com/microsoft/typescript-go/internal/tspath"
)

// TestSettings mirrors the Rust type-runner TestSettings
type TestSettings struct {
	NoTypesAndSymbols    bool
	BaseURL              *string
	NoImplicitReferences bool
	IncludeBuiltFile     *string
	LibFiles             []string
}

// TestUnit mirrors the Rust type-runner TestUnit structure
type TestUnit struct {
	Path      string
	Settings  TestSettings
	FileNames []string
	FileContents []string
	Symlinks  map[string]string
}

// FileReadError represents file reading errors
type FileReadError struct {
	Path string
	Err  error
}

func (e FileReadError) Error() string {
	return fmt.Sprintf("failed to read file %s: %v", e.Path, e.Err)
}

// readFile handles UTF-8/16 BOM variants like type-runner
func readFile(path string) (string, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return "", FileReadError{Path: path, Err: err}
	}

	// Handle BOM variants
	if len(data) >= 3 {
		// UTF-8 BOM
		if bytes.Equal(data[:3], []byte{0xef, 0xbb, 0xbf}) {
			return string(data[3:]), nil
		}
		// UTF-16 BE BOM
		if bytes.Equal(data[:2], []byte{0xfe, 0xff}) {
			return decodeUTF16BE(data[2:])
		}
		// UTF-16 LE BOM
		if bytes.Equal(data[:2], []byte{0xff, 0xfe}) {
			return decodeUTF16LE(data[2:])
		}
	}

	// Default to UTF-8
	if !utf8.Valid(data) {
		return "", FileReadError{Path: path, Err: fmt.Errorf("invalid UTF-8")}
	}
	return string(data), nil
}

func decodeUTF16BE(data []byte) (string, error) {
	if len(data)%2 != 0 {
		return "", fmt.Errorf("invalid UTF-16 BE data length")
	}
	
	u16s := make([]uint16, len(data)/2)
	for i := 0; i < len(u16s); i++ {
		u16s[i] = binary.BigEndian.Uint16(data[i*2:])
	}
	return string(utf16.Decode(u16s)), nil
}

func decodeUTF16LE(data []byte) (string, error) {
	if len(data)%2 != 0 {
		return "", fmt.Errorf("invalid UTF-16 LE data length")
	}
	
	u16s := make([]uint16, len(data)/2)
	for i := 0; i < len(u16s); i++ {
		u16s[i] = binary.LittleEndian.Uint16(data[i*2:])
	}
	return string(utf16.Decode(u16s)), nil
}

// parseTestUnit parses a test unit file similar to type-runner's TestUnit::parse
func parseTestUnit(path, content string) (*TestUnit, error) {
	unit := &TestUnit{
		Path:         path,
		Settings:     TestSettings{},
		FileNames:    []string{},
		FileContents: []string{},
		Symlinks:     make(map[string]string),
	}

	lines := strings.Split(content, "\n")
	var fileStart *int
	fileName := filepath.Base(path)

	for i, line := range lines {
		line = strings.TrimRight(line, "\r")
		
		// Check for comment directives
		if strings.HasPrefix(line, "//") {
			rest := strings.TrimSpace(line[2:])
			if strings.HasPrefix(rest, "@") && strings.Contains(rest, ":") {
				// Complete current file if any
				if fileStart != nil {
					fileContent := strings.Join(lines[*fileStart:i], "\n")
					unit.FileNames = append(unit.FileNames, fileName)
					unit.FileContents = append(unit.FileContents, fileContent)
					fileStart = nil
				}

				// Parse directive
				colonIdx := strings.Index(rest, ":")
				name := strings.ToLower(strings.TrimSpace(rest[1:colonIdx]))
				value := strings.TrimSpace(rest[colonIdx+1:])

				switch name {
				case "filename":
					fileName = value
					nextLine := i + 1
					fileStart = &nextLine
				case "link":
					parts := strings.Fields(value)
					if len(parts) == 2 {
						unit.Symlinks[parts[0]] = parts[1]
					}
				case "baseurl":
					unit.Settings.BaseURL = &value
				case "noimplicitreferences":
					unit.Settings.NoImplicitReferences = strings.ToLower(value) == "true"
				case "includebuiltfile":
					unit.Settings.IncludeBuiltFile = &value
				case "libfiles":
					libFiles := []string{}
					for _, lib := range strings.Split(value, ",") {
						lib = strings.TrimSpace(lib)
						if lib != "" {
							libFiles = append(libFiles, lib)
						}
					}
					unit.Settings.LibFiles = libFiles
				case "notypesandsymbols":
					unit.Settings.NoTypesAndSymbols = strings.ToLower(value) == "true"
				}
			}
		} else if fileStart == nil && strings.TrimSpace(line) != "" {
			// Start of file content
			fileStart = &i
		}
	}

	// Complete final file
	if fileStart != nil {
		fileContent := strings.Join(lines[*fileStart:], "\n")
		unit.FileNames = append(unit.FileNames, fileName)
		unit.FileContents = append(unit.FileContents, fileContent)
	} else if len(unit.FileNames) == 0 {
		// Single file with no directives
		unit.FileNames = append(unit.FileNames, fileName)
		unit.FileContents = append(unit.FileContents, content)
	}

	return unit, nil
}

// discoverTestFiles finds all TypeScript test files like type-runner's discover function
func discoverTestFiles(repoPath string) ([]string, error) {
	var testFiles []string
	
	testDirs := []string{
		filepath.Join(repoPath, "_submodules", "TypeScript", "tests", "cases", "compiler"),
		filepath.Join(repoPath, "_submodules", "TypeScript", "tests", "cases", "conformance"),
	}

	for _, testDir := range testDirs {
		err := filepath.Walk(testDir, func(path string, info os.FileInfo, err error) error {
			if err != nil {
				return err
			}
			
			if !info.IsDir() && strings.HasSuffix(path, ".ts") {
				// Skip problematic files like type-runner does
				baseName := filepath.Base(path)
				if baseName == "corrupted.ts" || 
				   baseName == "TransportStream.ts" || 
				   baseName == "checkJsFiles6.ts" || 
				   baseName == "jsFileCompilationWithoutJsExtensions.ts" {
					return nil
				}
				testFiles = append(testFiles, path)
			}
			return nil
		})
		if err != nil {
			return nil, fmt.Errorf("failed to walk directory %s: %v", testDir, err)
		}
	}

	sort.Strings(testFiles)
	return testFiles, nil
}

// formatEncodedSourceFile creates a string dump from encoded binary (copied from encoder_test.go)
func formatEncodedSourceFile(encoded []byte) string {
	var result strings.Builder
	var getIndent func(parentIndex uint32) string
	
	offsetNodes := binary.LittleEndian.Uint32(encoded[encoder.HeaderOffsetNodes:])
	offsetStringOffsets := binary.LittleEndian.Uint32(encoded[encoder.HeaderOffsetStringOffsets:])
	offsetStrings := binary.LittleEndian.Uint32(encoded[encoder.HeaderOffsetStringData:])
	
	getIndent = func(parentIndex uint32) string {
		if parentIndex == 0 {
			return ""
		}
		return "  " + getIndent(binary.LittleEndian.Uint32(encoded[int(offsetNodes)+int(parentIndex)*encoder.NodeSize+encoder.NodeOffsetParent:]))
	}
	
	j := 1
	for i := int(offsetNodes) + encoder.NodeSize; i < len(encoded); i += encoder.NodeSize {
		kind := binary.LittleEndian.Uint32(encoded[i+encoder.NodeOffsetKind:])
		pos := binary.LittleEndian.Uint32(encoded[i+encoder.NodeOffsetPos:])
		end := binary.LittleEndian.Uint32(encoded[i+encoder.NodeOffsetEnd:])
		parentIndex := binary.LittleEndian.Uint32(encoded[i+encoder.NodeOffsetParent:])
		
		result.WriteString(getIndent(parentIndex))
		if kind == encoder.SyntaxKindNodeList {
			result.WriteString("NodeList")
		} else {
			result.WriteString(ast.Kind(kind).String())
		}
		
		if ast.Kind(kind) == ast.KindIdentifier || ast.Kind(kind) == ast.KindStringLiteral {
			stringIndex := binary.LittleEndian.Uint32(encoded[i+encoder.NodeOffsetData:]) & encoder.NodeDataStringIndexMask
			strStart := binary.LittleEndian.Uint32(encoded[int(offsetStringOffsets+stringIndex*4):])
			strEnd := binary.LittleEndian.Uint32(encoded[int(offsetStringOffsets+stringIndex*4)+4:])
			str := string(encoded[offsetStrings+strStart : offsetStrings+strEnd])
			result.WriteString(fmt.Sprintf(` "%s"`, str))
		}
		
		fmt.Fprintf(&result, " [%d, %d), i=%d, next=%d", pos, end, j, encoded[i+encoder.NodeOffsetNext])
		result.WriteString("\n")
		j++
	}
	return result.String()
}

// sanitizeFileName converts file paths to safe filenames
func sanitizeFileName(path string) string {
	// Replace path separators and other problematic characters
	safe := strings.ReplaceAll(path, "/", "__")
	safe = strings.ReplaceAll(safe, "\\", "__")
	safe = strings.ReplaceAll(safe, ":", "_")
	safe = strings.ReplaceAll(safe, "*", "_")
	safe = strings.ReplaceAll(safe, "?", "_")
	safe = strings.ReplaceAll(safe, "\"", "_")
	safe = strings.ReplaceAll(safe, "<", "_")
	safe = strings.ReplaceAll(safe, ">", "_")
	safe = strings.ReplaceAll(safe, "|", "_")
	return safe
}

func main() {
	if len(os.Args) != 2 {
		fmt.Fprintf(os.Stderr, "Usage: %s <tsgo-repo-path>\n", os.Args[0])
		os.Exit(1)
	}

	repoPath := os.Args[1]
	
	// Create output directories
	testDataDir := filepath.Join(repoPath, "..", "test_data")
	encodedDir := filepath.Join(testDataDir, "encoded")
	dumpsGoDir := filepath.Join(testDataDir, "dumps", "go")
	
	for _, dir := range []string{encodedDir, dumpsGoDir} {
		if err := os.MkdirAll(dir, 0755); err != nil {
			fmt.Fprintf(os.Stderr, "Failed to create directory %s: %v\n", dir, err)
			os.Exit(1)
		}
	}

	// Discover test files
	fmt.Println("Discovering test files...")
	testFiles, err := discoverTestFiles(repoPath)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Failed to discover test files: %v\n", err)
		os.Exit(1)
	}

	fmt.Printf("Found %d test files\n", len(testFiles))

	var processed, skipped, failed int

	for _, testFile := range testFiles {
		relPath, _ := filepath.Rel(filepath.Join(repoPath, "_submodules", "TypeScript"), testFile)
		fmt.Printf("Processing %s...\n", relPath)

		// Read and parse test unit
		content, err := readFile(testFile)
		if err != nil {
			fmt.Printf("  Skipped: %v\n", err)
			skipped++
			continue
		}

		unit, err := parseTestUnit(testFile, content)
		if err != nil {
			fmt.Printf("  Failed to parse: %v\n", err)
			failed++
			continue
		}

		// Skip if @noTypesAndSymbols
		if unit.Settings.NoTypesAndSymbols {
			fmt.Printf("  Skipped: @noTypesAndSymbols\n")
			skipped++
			continue
		}

		// Process each file in the unit
		for i, fileName := range unit.FileNames {
			fileContent := unit.FileContents[i]
			
			// Skip non-TypeScript files
			if !strings.HasSuffix(fileName, ".ts") && !strings.HasSuffix(fileName, ".tsx") {
				continue
			}

			// Skip empty files
			if strings.TrimSpace(fileContent) == "" {
				continue
			}

			// Normalize Windows paths and create a safe virtual file path
			normalizedFileName := strings.ReplaceAll(fileName, "\\", "/")
			if filepath.IsAbs(normalizedFileName) || strings.Contains(normalizedFileName, ":") {
				// For absolute paths or Windows drives, create a safe relative path
				normalizedFileName = filepath.Base(normalizedFileName)
			}
			
			// Create absolute path for the virtual file
			absFileName, err := filepath.Abs(filepath.Join(filepath.Dir(testFile), normalizedFileName))
			if err != nil {
				absFileName = filepath.Join(filepath.Dir(testFile), normalizedFileName)
			}
			
			// Parse and encode
			sourceFile := parser.ParseSourceFile(ast.SourceFileParseOptions{
				FileName: absFileName,
				Path:     tspath.Path(absFileName),
			}, fileContent, core.ScriptKindTS)

			encoded, err := encoder.EncodeSourceFile(sourceFile, "")
			if err != nil {
				fmt.Printf("  Failed to encode %s: %v\n", fileName, err)
				failed++
				continue
			}

			// Generate output filename
			outputName := fmt.Sprintf("%s__%s", sanitizeFileName(relPath[:len(relPath)-3]), sanitizeFileName(fileName))
			
			// Write binary
			binPath := filepath.Join(encodedDir, outputName+".bin")
			if err := os.WriteFile(binPath, encoded, 0644); err != nil {
				fmt.Printf("  Failed to write binary %s: %v\n", binPath, err)
				failed++
				continue
			}

			// Generate and write dump
			dump := formatEncodedSourceFile(encoded)
			dumpPath := filepath.Join(dumpsGoDir, outputName+".txt")
			if err := os.WriteFile(dumpPath, []byte(dump), 0644); err != nil {
				fmt.Printf("  Failed to write dump %s: %v\n", dumpPath, err)
				failed++
				continue
			}

			fmt.Printf("  Generated: %s\n", outputName)
			processed++
		}
	}

	fmt.Printf("\nSummary:\n")
	fmt.Printf("  Processed: %d files\n", processed)
	fmt.Printf("  Skipped: %d files\n", skipped)
	fmt.Printf("  Failed: %d files\n", failed)
	
	if failed > 0 {
		os.Exit(1)
	}
} 