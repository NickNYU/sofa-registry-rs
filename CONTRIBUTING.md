# Contributing to SOFARegistry-RS

Thank you for your interest in contributing to SOFARegistry-RS. This document explains how to set up a development environment, run tests, and submit changes.

## Development Setup

### Prerequisites

- **Rust 1.75+** — Install via [rustup](https://rustup.rs/)
- **protoc** — Protocol Buffers compiler (proto files are compiled during `cargo build` via `tonic-build`)

### Clone and Build

```bash
git clone https://github.com/sofastack/sofa-registry-rs.git
cd sofa-registry-rs
cargo build --workspace
```

### IDE Setup

Any editor with rust-analyzer support works well. The workspace is structured as a standard Cargo workspace with 12 crates.

## Running Tests

```bash
# Full test suite (575 tests)
cargo test --workspace

# Single crate
cargo test -p sofa-registry-core

# Single test by name
cargo test -p sofa-registry-server-meta -- leader_election

# Integration tests (starts real servers on ephemeral ports)
cargo test -p sofa-registry-integration-tests
```

Integration tests allocate ports in the 19600+ range to avoid conflicts with locally running servers.

## Code Quality

### Formatting

```bash
cargo fmt --all --check   # Check formatting
cargo fmt --all           # Apply formatting
```

All code must pass `cargo fmt` before merging.

### Linting

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

Zero clippy warnings is enforced. If a warning is genuinely unavoidable, use a targeted `#[allow(...)]` with a comment explaining why.

### Build Checks

```bash
# Ensure everything compiles cleanly
cargo build --workspace

# Ensure tests pass
cargo test --workspace

# Ensure no warnings
cargo clippy --workspace --all-targets -- -D warnings
```

## Making Changes

### Branch Naming

- `feature/<description>` — New features
- `fix/<description>` — Bug fixes
- `refactor/<description>` — Code improvements without behavior changes
- `docs/<description>` — Documentation only

### Commit Messages

Write clear, concise commit messages. Use imperative mood in the subject line:

```
Add session lease eviction loop

The session server now periodically checks for idle client connections
and removes them after the configured timeout.
```

### Pull Requests

1. Create a feature branch from `main`
2. Make your changes, ensuring all checks pass (tests, fmt, clippy)
3. Write or update tests for your changes
4. Open a PR against `main` with a clear description of what changed and why

## Project Structure

The workspace has 12 crates. Understanding their dependency hierarchy helps when deciding where to place new code:

```
bin                     ← Entry point; depends on all server crates + mcp
├── server-meta         ← Meta server
├── server-data         ← Data server
├── server-session      ← Session server
├── mcp                 ← MCP server for AI tools
│
├── server-shared       ← Shared server utilities (MetaClient, metrics)
├── remoting            ← gRPC/HTTP transport layer
├── store               ← Storage traits + implementations
├── client              ← Client SDK
├── common              ← Shared utilities
└── core                ← Types, constants, protobuf, errors
```

**Dependency rule:** Crates lower in the tree must not depend on crates above them. `core` has no internal dependencies. `server-*` crates depend on `server-shared`, `remoting`, `store`, and `core`.

### Where to Put New Code

| Change type | Crate |
|---|---|
| New domain model or constant | `core` |
| New storage trait or implementation | `store` |
| New gRPC service definition | `core` (proto) + server crate (handler) |
| Shared server logic (metrics, config) | `server-shared` |
| Meta/Data/Session specific logic | Respective `server-*` crate |
| Client-facing API changes | `client` |
| Transport layer changes | `remoting` |

### Adding Protobuf Definitions

1. Edit or add `.proto` files in `crates/core/proto/`
2. Update `crates/core/build.rs` if adding a new `.proto` file
3. Run `cargo build -p sofa-registry-core` to regenerate
4. Generated types appear under `sofa_registry_core::pb::sofa::registry::*`

### Adding Tests

- **Unit tests** go in `#[cfg(test)] mod tests` within the source file
- **Integration tests** that require a running server go in `crates/integration-tests/`
- Integration tests use port offsets from 19600 to avoid conflicts

Example unit test:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_table_assigns_correct_epoch() {
        let table = SlotTable::new(42, vec![]);
        assert_eq!(table.epoch, 42);
    }

    #[tokio::test]
    async fn register_node_returns_slot_table() {
        // async test example
    }
}
```

## Architecture

See [ARCHITECTURE.md](ARCHITECTURE.md) for detailed design documentation covering the three-server architecture, slot-based partitioning, data flow, and key design decisions.

## License

By contributing to this project, you agree that your contributions will be licensed under the [Apache License 2.0](LICENSE).
