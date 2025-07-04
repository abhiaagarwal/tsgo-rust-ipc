#!/bin/bash
set -e

# Get the directory of this script
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
CRATE_DIR="$( cd "$SCRIPT_DIR/.." && pwd )"
PROJECT_ROOT="$( cd "$CRATE_DIR/../.." && pwd )"

# Path to tsgo
TSGO_PATH="$PROJECT_ROOT/tsgo"

# Output directory for generated Rust files
OUTPUT_DIR="$CRATE_DIR/src/generated"

# Ensure tsgo exists
if [ ! -d "$TSGO_PATH" ]; then
    echo "Error: tsgo directory not found at $TSGO_PATH"
    echo "Make sure the tsgo submodule is initialized:"
    echo "  git submodule init"
    echo "  git submodule update"
    exit 1
fi

# Create output directory
mkdir -p "$OUTPUT_DIR"

# Build the generator
echo "Building AST generator..."
cd "$SCRIPT_DIR/ast-gen"
go build -o ast-gen

# Run the generator
echo "Generating Rust AST from TypeScript-Go..."
./ast-gen "$TSGO_PATH" "$OUTPUT_DIR"

# Clean up
rm ast-gen

echo "Done! Generated files are in $OUTPUT_DIR" 