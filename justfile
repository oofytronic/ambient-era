# One entry point for every check CI runs. `just ci` before you commit.
set shell := ["bash", "-euo", "pipefail", "-c"]

# Crates that cannot target wasm: the relay, the FFI layer, the CLI, and the desktop window.
wasm_excludes := "--exclude ambient-relay --exclude ambient-ffi --exclude ambient-cli --exclude ambient-desktop"

default:
    @just --list

# Format all code.
fmt:
    cargo fmt --all

# Verify formatting without changing files.
fmt-check:
    cargo fmt --all --check

# Type-check everything, including tests.
check:
    cargo check --workspace --all-targets --locked

# Lint with warnings denied.
lint:
    cargo clippy --workspace --all-targets --locked -- -D warnings

# Unit, integration, and doc tests. Remove `--no-tests=pass` once every crate has tests.
test:
    cargo nextest run --workspace --locked --no-tests=pass
    cargo test --workspace --doc --locked

# Build docs with warnings (including broken links) denied.
doc:
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked

# Protocol crates must keep compiling for the browser.
wasm-check:
    cargo check --workspace {{wasm_excludes}} --target wasm32-unknown-unknown --locked

# Licenses, banned crates, sources, and security advisories.
deny:
    cargo deny check

# Everything CI runs.
ci: fmt-check lint test doc wasm-check deny
