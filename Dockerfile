# Build stage
FROM rust:1.84-bookworm AS builder

# Install protoc for tonic-build
RUN apt-get update && apt-get install -y protobuf-compiler && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy manifests first for dependency caching
COPY Cargo.toml Cargo.lock ./
COPY crates/core/Cargo.toml crates/core/Cargo.toml
COPY crates/common/Cargo.toml crates/common/Cargo.toml
COPY crates/client/Cargo.toml crates/client/Cargo.toml
COPY crates/store/Cargo.toml crates/store/Cargo.toml
COPY crates/remoting/Cargo.toml crates/remoting/Cargo.toml
COPY crates/server-shared/Cargo.toml crates/server-shared/Cargo.toml
COPY crates/server-meta/Cargo.toml crates/server-meta/Cargo.toml
COPY crates/server-data/Cargo.toml crates/server-data/Cargo.toml
COPY crates/server-session/Cargo.toml crates/server-session/Cargo.toml
COPY crates/mcp/Cargo.toml crates/mcp/Cargo.toml
COPY crates/bin/Cargo.toml crates/bin/Cargo.toml
COPY crates/integration-tests/Cargo.toml crates/integration-tests/Cargo.toml

# Create dummy source files so cargo can resolve the workspace and cache deps
RUN mkdir -p crates/core/src crates/common/src crates/client/src \
    crates/store/src crates/remoting/src crates/server-shared/src \
    crates/server-meta/src crates/server-data/src crates/server-session/src \
    crates/mcp/src crates/bin/src crates/integration-tests/src \
    crates/core/proto && \
    for d in core common client store remoting server-shared server-meta server-data server-session mcp integration-tests; do \
        echo "// dummy" > crates/$d/src/lib.rs; \
    done && \
    echo "fn main() {}" > crates/bin/src/main.rs && \
    touch crates/core/proto/registry_model.proto crates/core/proto/meta_service.proto \
          crates/core/proto/data_service.proto crates/core/proto/session_service.proto && \
    echo 'fn main() { }' > crates/core/build.rs

# Pre-build dependencies (this layer is cached unless Cargo.toml/Cargo.lock change)
RUN cargo build --release 2>/dev/null || true

# Copy actual source code
COPY . .

# Touch all source files so cargo rebuilds them (not the cached deps)
RUN find crates -name "*.rs" -exec touch {} + && \
    touch crates/core/build.rs

# Build the real binary
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/sofa-registry /usr/local/bin/sofa-registry

# Meta: 9611 (gRPC), 9612 (HTTP)
# Data: 9621 (gRPC), 9622 (HTTP)
# Session: 9601 (gRPC), 9602 (HTTP)
EXPOSE 9601 9602 9611 9612 9621 9622

ENTRYPOINT ["sofa-registry"]
CMD ["--help"]
