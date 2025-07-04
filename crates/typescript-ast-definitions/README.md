# Typescript AST Definitions

This crate contains Rust code that is partially generated from parsing the TypeScript-Go AST implementation.

## Usage

### Prerequisites

1. Go 1.24 or later
2. The `tsgo` submodule must be initialized:
   ```bash
   git submodule init
   git submodule update
   ```

### Running the Generator

From the project root:

```bash
cd crates/typescript-ast-definitions/scripts
./generate.sh
```

### Generated Files

- `src/generated/flags.rs` - Bitflag types (NodeFlags, ModifierFlags, etc.)
- `src/generated/syntax_kind.rs` - SyntaxKind enum
