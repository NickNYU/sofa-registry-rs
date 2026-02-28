# Architecture

This document describes the architecture of SOFARegistry-RS, a Rust reimplementation of [SOFARegistry](https://github.com/sofastack/sofa-registry) — a production-grade service registry for microservices.

## Overview

SOFARegistry uses an **AP architecture** (availability + partition tolerance). It prioritizes availability and partition tolerance over strong consistency, converging to eventual consistency through slot-based data partitioning and real-time push notifications.

The system has three server roles that coordinate through gRPC:

```
┌─────────────────────────────────────────────────────────────────┐
│                        Clients (SDK)                            │
│            publish / subscribe / heartbeat                      │
└──────────────────────────┬──────────────────────────────────────┘
                           │ gRPC
                           ▼
┌──────────────────────────────────────────────────────────────────┐
│                      Session Server                              │
│  Client gateway: manages connections, push streams, write        │
│  forwarding. Caches data versions from Data server.              │
└──────────┬───────────────────────────────────────┬───────────────┘
           │ gRPC (write forwarding)               │ gRPC (change notification)
           ▼                                       ▼
┌──────────────────────────────────────────────────────────────────┐
│                       Data Server                                │
│  Stores publisher data, partitioned by slot. Handles             │
│  replication across replicas. Notifies Session on changes.       │
└──────────────────────────────┬───────────────────────────────────┘
                               │ gRPC (register, heartbeat, slot table)
                               ▼
┌──────────────────────────────────────────────────────────────────┐
│                       Meta Server                                │
│  Cluster coordinator: leader election, slot table management,    │
│  node lease tracking. Persistence via SQLite.                    │
└──────────────────────────────────────────────────────────────────┘
```

## Server Roles

### Meta Server

The Meta server is the cluster brain. It coordinates all other servers.

**Responsibilities:**
- **Leader election** — Uses a distributed lock backed by SQLite to elect a single leader among Meta peers. The leader manages the slot table; followers forward requests to the leader.
- **Slot table management** — Creates and maintains the slot table that maps data partitions (slots) to Data server addresses. Handles replica assignment and rebalancing when nodes join or leave.
- **Node lease management** — Tracks heartbeats from Data and Session servers. Evicts nodes that fail to renew their lease within the configured timeout.
- **Admin HTTP API** — Exposes health checks, leader status, node lists, and slot table information.

**Key components:**
- `MetaLeaderElector` — Periodic lock acquisition loop; only the holder acts as leader
- `MetaSlotManager` — Builds and updates the slot table with leader/follower assignment
- `DataServerManager` / `SessionServerManager` — Track registered nodes and their lease expiry
- `MetaGrpcServiceImpl` — Handles `RegisterNode`, `RenewNode`, `GetSlotTable`, `GetLeader` RPCs

**Persistence:**
- SQLite via `sqlx` for distributed locks and metadata
- Schema migrations run automatically at startup

### Data Server

The Data server stores the actual service registry data (publisher registrations).

**Responsibilities:**
- **Data storage** — Stores publishers keyed by `(data_center, data_info_id)`, partitioned into slots via CRC32 hashing
- **Slot leadership** — Each slot has a leader Data server. Writes are only accepted by the slot leader.
- **Change notification** — When data changes, notifies connected Session servers so they can push updates to subscribers
- **Replication** — `SlotDiffSyncer` handles delta synchronization between slot leader and followers
- **Session lease tracking** — Tracks Session server leases and evicts publishers from expired sessions

**Key components:**
- `LocalDatumStorage` — In-memory storage using `DashMap` for concurrent access, organized per-slot
- `DataSlotManager` — Maintains the local view of the slot table; determines leadership
- `DataChangeEventCenter` — Event bus with debouncing that batches change notifications to Session servers
- `SessionLeaseManager` — Tracks Session server heartbeats; triggers publisher cleanup on expiry
- `DataGrpcService` — Handles `PublishData`, `GetData`, `BatchPutData`, `NotifyDataChange` RPCs

### Session Server

The Session server is the client-facing gateway. All client SDK interactions go through Session.

**Responsibilities:**
- **Client registration** — Accepts publisher and subscriber registrations from the client SDK
- **Write forwarding** — Forwards publisher writes to the appropriate Data server based on slot ownership
- **Push notifications** — Maintains server-side gRPC streams to clients; pushes data changes in real time
- **Connection management** — Tracks active client connections; evicts idle clients after timeout
- **Data caching** — Caches data versions received from Data server for efficient comparison

**Key components:**
- `PublisherRegistry` / `SubscriberRegistry` — Local registries indexed by `(data_center, data_info_id)`
- `WriteDataAcceptor` — Queues incoming writes and dispatches them to the correct Data server
- `PushService` + `StreamRegistry` — Manages per-client gRPC streams for server-push
- `ConnectionService` — Tracks client heartbeats and connection state
- `SessionCacheService` — Caches data versions for diff-based push decisions
- `SessionSlotManager` — Local slot table used to route writes to the correct Data server

## Data Flow

### Publish Flow

```
Client SDK                Session Server              Data Server
    │                          │                          │
    │── RegisterPublisher ────>│                          │
    │                          │── PublishData ──────────>│
    │                          │   (routed by slot)       │
    │                          │                          │── store in DashMap
    │                          │                          │── emit DataChangeEvent
    │<── RegisterResponse ─────│                          │
    │                          │<── NotifyDataChange ─────│
    │                          │── push to subscribers    │
```

1. Client calls `RegisterPublisher` on Session server
2. Session stores the registration locally and forwards the write to the Data server that owns the slot
3. Data server stores the publisher, emits a change event
4. Data server notifies the Session server of the change
5. Session server pushes the updated data to all subscribed clients via server-side streaming

### Subscribe Flow

```
Client SDK                Session Server
    │                          │
    │── RegisterSubscriber ───>│
    │                          │── store subscription locally
    │── Subscribe (stream) ───>│
    │                          │── register stream in StreamRegistry
    │                          │
    │  ... later, on data change ...
    │                          │
    │<── push data via stream ─│
```

### Slot Table Distribution

```
Meta Server ──── GetSlotTable ────> Data Server (periodic poll)
Meta Server ──── GetSlotTable ────> Session Server (periodic poll)
```

Both Data and Session servers periodically poll Meta for the latest slot table. When the epoch advances, they update their local slot table and adjust routing accordingly.

## Slot-Based Partitioning

Data is partitioned across Data servers using slots:

1. **Hash function:** `CRC32(data_info_id) % slot_num` determines the slot
2. **Slot count:** Configurable (default: 256)
3. **Replicas:** Each slot has a leader and configurable number of followers (default: 2 replicas)
4. **Assignment:** Meta server assigns slots to Data servers using round-robin with jitter

```
data_info_id = "com.example.FooService"
slot = CRC32("com.example.FooService") % 256 = 42

Slot 42:
  leader:    data-server-1:9621
  followers: [data-server-2:9621, data-server-3:9621]
```

Only the slot leader accepts writes. Followers replicate via `SlotDiffSyncer`.

## Communication

### Protocols

| Path | Protocol | Framework |
|---|---|---|
| Client <-> Session | gRPC | tonic |
| Session <-> Data | gRPC | tonic |
| Data/Session <-> Meta | gRPC | tonic |
| Admin APIs | HTTP/REST | axum |

### gRPC Services

**MetaService** (`meta_service.proto`):
- `RegisterNode` — Register a Data or Session server
- `RenewNode` — Renew a node's lease
- `GetSlotTable` — Fetch the current slot table (returns unchanged if epoch matches)
- `GetLeader` — Get the current Meta leader address

**DataService** (`data_service.proto`):
- `PublishData` — Store a publisher registration
- `GetData` — Retrieve publishers for a data ID
- `BatchPutData` — Bulk write for replication
- `NotifyDataChange` — Notify Session servers of data changes
- `SlotDiffPublisher` / `SlotDiffSubscriber` — Delta sync between replicas

**SessionService** (`session_service.proto`):
- `RegisterPublisher` — Register a publisher (forwarded to Data)
- `RegisterSubscriber` — Register a subscriber
- `Subscribe` — Server-side streaming for push notifications
- `Heartbeat` — Client keep-alive
- `Unregister` — Remove a registration

### Connection Pooling

Outbound gRPC connections are managed by `GrpcClientPool`, which maintains a map of `address -> tonic::Channel`. Channels are created on first use and reused for subsequent calls to the same address.

## Error Handling

The codebase uses a unified error hierarchy:

```
MetaError (store/traits)
    ↓ From<MetaError>
RegistryError (core/error)
    ↓ From<RegistryError>
tonic::Status (gRPC responses)
```

`RegistryError` variants map to specific gRPC status codes:

| Error | gRPC Code |
|---|---|
| `NotLeader` | `FAILED_PRECONDITION` |
| `SlotMoved` | `UNAVAILABLE` |
| `NotFound` | `NOT_FOUND` |
| `Auth` | `UNAUTHENTICATED` |
| `Timeout` | `DEADLINE_EXCEEDED` |
| `Duplicate` | `ALREADY_EXISTS` |
| `Config` | `INVALID_ARGUMENT` |
| `Connection` / `Remoting` | `UNAVAILABLE` |

## Concurrency Model

- **Runtime:** Tokio multi-threaded async runtime
- **Shared state:** `DashMap` for registries (lock-free reads, sharded writes)
- **State sharing:** `Arc<ServerState>` passed to both HTTP and gRPC handlers
- **Graceful shutdown:** `CancellationToken` from `tokio-util` propagated to all background tasks
- **Background loops:** Each server spawns `tokio::spawn` tasks for heartbeats, slot sync, lease eviction, change notification

## Observability

### Metrics

Every server exposes a `/metrics` endpoint in Prometheus exposition format. Key metrics:

- `data_slot_table_epoch` — Current slot table epoch on Data server
- `data_active_session_leases` — Number of active Session server leases on Data server
- Standard HTTP/gRPC request metrics via `metrics` crate

### Logging

Structured logging via `tracing` with configurable level (`--log-level`). Supports `RUST_LOG` environment variable for fine-grained control:

```bash
RUST_LOG=sofa_registry_server_meta=debug,info cargo run -- all
```

## MCP Server

The MCP (Model Context Protocol) server exposes SOFARegistry data to AI assistants via JSON-RPC 2.0 over stdio or HTTP. It connects to the HTTP admin APIs of Meta, Data, and Session servers to answer queries about registered services, cluster health, and slot distribution.

## Crate Dependency Graph

```
sofa-registry-bin
├── sofa-registry-server-meta
│   ├── sofa-registry-server-shared
│   │   ├── sofa-registry-remoting
│   │   │   ├── sofa-registry-core
│   │   │   └── sofa-registry-common
│   │   └── sofa-registry-store
│   │       └── sofa-registry-core
│   └── sofa-registry-store
├── sofa-registry-server-data
│   ├── sofa-registry-server-shared
│   ├── sofa-registry-remoting
│   └── sofa-registry-store
├── sofa-registry-server-session
│   ├── sofa-registry-server-shared
│   ├── sofa-registry-remoting
│   └── sofa-registry-store
├── sofa-registry-mcp
│   └── sofa-registry-core
└── sofa-registry-client
    └── sofa-registry-core
```

## Key Design Decisions

| Decision | Rationale |
|---|---|
| **gRPC (tonic) over custom binary protocol** | The Java version uses SOFABolt, a custom Netty-based protocol. tonic provides equivalent performance in Rust with less code and ecosystem compatibility. |
| **SQLite for Meta persistence** | Lightweight, zero-config persistence suitable for leader election and metadata. The Java version uses MySQL; SQLite eliminates the external dependency for small deployments. |
| **DashMap for in-memory storage** | Lock-free concurrent reads with sharded writes. Ideal for the read-heavy, write-occasional pattern of a service registry. |
| **CancellationToken for shutdown** | Clean, composable shutdown propagation across independent `tokio::spawn` tasks without shared mutable state. |
| **Trait abstractions for storage and meta client** | `DatumStorage`, `MetaServiceClient`, `DistributeLockRepository` traits enable mock-based testing without starting real servers or databases. |
| **Workspace with fine-grained crates** | Keeps compile times manageable, enforces dependency boundaries, and allows selective compilation/testing. |
