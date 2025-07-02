# TypeScript-Go Rust IPC Client

Highly experimental Rust client library for communicating with Microsoft's Typescript rewrite in Go (tsgo) IPC-based API server, as implemented in [tsgo](https://github.com/microsoft/typescript-go/pull/711).

## Motivation

Go is the language of choice for the TypeScript compiler, but Go's main problem is that it's hard to embed in other languages. Since a lot of modern JS/TS tooling is written in Rust, this provides a way to provide TypeScript type information for that tooling by making requests to the tsgo server, running as a subprocess.

## Testing

### Unit Tests (no tsgo)
```bash
cargo build --workspace
cargo test  --workspace --exclude tsgo-rust-ipc-integration-tests
```

### Integration Tests (needs tsgo)
```bash
# one-off setup
git submodule update --init --recursive
cd tsgo && npm ci && npx hereby build && cd ..

TSGO_PATH=./tsgo/built/local/tsgo \
  cargo test -p tsgo-rust-ipc-integration-tests
```
