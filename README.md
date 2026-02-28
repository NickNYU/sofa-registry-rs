# SOFARegistry-RS

A high-performance, production-grade service registry for microservices, written in Rust. This is a Rust reimplementation of [SOFARegistry](https://github.com/sofastack/sofa-registry), originally developed by Ant Group.

SOFARegistry-RS uses an **AP architecture** (availability + partition tolerance) with real-time push notifications, consistent hashing via slot tables, and gRPC-based communication between all components.

## Features

- **Three-role architecture** — Meta, Data, and Session servers can run together or be deployed independently
- **Slot-based data partitioning** — Consistent hashing with configurable slot count and replica factor
- **Real-time push** — Server-side streaming pushes data changes to subscribers immediately
- **Leader election** — Distributed lock-based leader election for Meta server coordination
- **Session lease management** — Automatic eviction of stale clients and servers
- **Prometheus metrics** — Built-in `/metrics` endpoint on every server
- **MCP server** — Model Context Protocol server for AI-assisted registry lookups
- **Client SDK** — Async Rust client with HMAC-SHA256 authentication

## Quick Start

### Prerequisites

- Rust 1.75+ (2021 edition)
- Protocol Buffers compiler (`protoc`) — for building from source

### Build

```bash
cargo build --release
```

The binary is produced at `target/release/sofa-registry`.

### Run all servers (development mode)

```bash
# Uses defaults — no config file needed
cargo run -- all

# Or with a config file
cp config.example.toml config.toml
cargo run -- --config config.toml all
```

This starts Meta, Data, and Session servers in a single process with default ports:

| Server  | gRPC  | HTTP  |
|---------|-------|-------|
| Session | 9601  | 9602  |
| Meta    | 9611  | 9612  |
| Data    | 9621  | 9622  |

### Run individual servers

```bash
# Meta server (requires SQLite for persistence)
sofa-registry meta

# Data server (connects to Meta)
sofa-registry data

# Session server (connects to Meta, forwards writes to Data)
sofa-registry session

# MCP server (connects to all three via HTTP)
sofa-registry mcp
```

### Verify it's running

```bash
# Health check
curl http://localhost:9602/api/session/health

# Version info
curl http://localhost:9602/api/session/version

# Prometheus metrics
curl http://localhost:9602/metrics
```

## Configuration

Copy `config.example.toml` to `config.toml` and adjust as needed:

```toml
[common]
data_center = "DefaultDataCenter"
cluster_id = "DefaultCluster"

[meta]
grpc_port = 9611
http_port = 9612
db_url = "sqlite://sofa-registry-meta.db?mode=rwc"
meta_peers = ["127.0.0.1:9611"]
slot_num = 256
slot_replicas = 2

[data]
grpc_port = 9621
http_port = 9622
meta_server_addresses = ["127.0.0.1:9611"]

[session]
grpc_port = 9601
http_port = 9602
meta_server_addresses = ["127.0.0.1:9611"]

[mcp]
meta_http_url = "http://127.0.0.1:9612"
session_http_url = "http://127.0.0.1:9602"
data_http_url = "http://127.0.0.1:9622"
```

All fields have sensible defaults. The config file is optional for local development.

## CLI

```
sofa-registry [OPTIONS] <COMMAND>

Commands:
  all       Run all servers in one process (development mode)
  meta      Run meta server only
  data      Run data server only
  session   Run session server only
  mcp       Run MCP server for AI-assisted registry lookups

Options:
  -c, --config <FILE>    Config file path [default: config.toml]
      --log-level <LVL>  Log level [default: info]
  -h, --help             Print help
  -V, --version          Print version
```

## Testing

```bash
# Run all tests (575 tests)
cargo test --workspace

# Run tests for a specific crate
cargo test -p sofa-registry-core
cargo test -p sofa-registry-server-meta

# Run a single test
cargo test -p sofa-registry-core -- test_name

# Run with output
cargo test --workspace -- --nocapture
```

## Project Structure

```
sofa-registry-rs/
├── Cargo.toml               Workspace root
├── config.example.toml      Sample configuration
└── crates/
    ├── core/                 Shared types, constants, protobuf definitions
    ├── common/               Common utilities
    ├── client/               Client SDK (publish/subscribe API)
    ├── store/                Storage abstractions and implementations
    │   ├── traits/           DatumStorage, MetaServiceClient, etc.
    │   ├── memory/           In-memory storage for Data server
    │   └── jdbc/             SQLite persistence for Meta server
    ├── remoting/             gRPC server/client pool, HTTP server
    ├── server-shared/        Shared server code (metrics, MetaClient)
    ├── server-meta/          Meta server (leader election, slot table)
    ├── server-data/          Data server (storage, replication, change notification)
    ├── server-session/       Session server (client gateway, push, write forwarding)
    ├── mcp/                  MCP server for AI assistants
    ├── bin/                  Binary entry point
    └── integration-tests/    End-to-end server lifecycle tests
```

See [ARCHITECTURE.md](ARCHITECTURE.md) for detailed design documentation.

## License

This project is licensed under the [Apache License 2.0](LICENSE).
