# ══════════════════════════════════════════════════════════════════════════════
# Project Apex - API Server Dockerfile
# Multi-stage build for production Rust binary
# ══════════════════════════════════════════════════════════════════════════════

# Stage 1: Development (for docker-compose override with hot-reload)
FROM rust:1.75-bookworm AS development

WORKDIR /app

# Install protobuf compiler for gRPC and curl for health checks
RUN apt-get update && apt-get install -y protobuf-compiler curl && rm -rf /var/lib/apt/lists/*

# Install cargo-watch for hot-reload
RUN cargo install cargo-watch

# Copy Cargo files
COPY Cargo.toml Cargo.lock ./

# Create dummy project to cache dependencies
RUN mkdir src && echo 'fn main() {}' > src/main.rs
RUN cargo build && rm -rf src

# Copy actual source
COPY . .

ENV RUST_LOG=debug,apex_core=trace
ENV APEX__API__HOST=0.0.0.0
ENV APEX__API__HTTP_PORT=8080
ENV APEX__API__GRPC_PORT=50051

EXPOSE 8080 50051

CMD ["cargo", "watch", "-x", "run"]

# Stage 2: Build (production)
FROM rust:1.75-bookworm AS builder

WORKDIR /app

# Install protobuf compiler for gRPC
RUN apt-get update && apt-get install -y protobuf-compiler && rm -rf /var/lib/apt/lists/*

# Create dummy project to cache dependencies
RUN cargo new --bin apex-core
WORKDIR /app/apex-core

# Copy Cargo files
COPY Cargo.toml Cargo.lock ./

# Build dependencies (this layer is cached)
RUN cargo build --release && rm -rf src target/release/apex-core*

# Copy actual source
COPY . .

# Build the application
ENV SQLX_OFFLINE=true
RUN cargo build --release

# Stage 2: Runtime
FROM debian:bookworm-slim AS runtime

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN groupadd -r apex && useradd -r -g apex apex

WORKDIR /app

# Copy the binary
COPY --from=builder /app/apex-core/target/release/apex-core /app/apex-core

# Copy migrations (if needed at runtime)
COPY --from=builder /app/apex-core/migrations /app/migrations

# Set ownership
RUN chown -R apex:apex /app

# Switch to non-root user
USER apex

# Environment variables
ENV RUST_LOG=info,apex_core=debug
ENV APEX__API__HOST=0.0.0.0
ENV APEX__API__HTTP_PORT=8080
ENV APEX__API__GRPC_PORT=50051

# Expose ports
EXPOSE 8080 50051

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

# Run the binary
ENTRYPOINT ["/app/apex-core"]
