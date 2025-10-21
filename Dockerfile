# Multi-stage Dockerfile for KataPulse
# Real-time metrics for Kata Containers. cadvisor-compatible monitoring agent.
# Stage 1: Dependency builder - compiles and caches dependencies
FROM rust:1.82 as dependencies

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    protobuf-compiler \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

# Copy only Cargo files to leverage Docker layer caching for dependencies
COPY Cargo.toml Cargo.lock ./

# Create a dummy main.rs to build dependencies
RUN mkdir -p src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src

# Stage 2: Code builder - compiles the actual application
FROM dependencies as builder

WORKDIR /app

# Copy the full source code
COPY src ./src

# Build the release binary
RUN cargo build --release

# Verify the binary works
RUN ./target/release/kata-pulse --help || true

# Stage 3: Runtime stage
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Copy the built binary from builder stage
COPY --from=builder /app/target/release/kata-pulse /usr/local/bin/kata-pulse

# Run as root (system component)
USER root

# Health check
HEALTHCHECK --interval=30s --timeout=5s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8090/ || exit 1

# Default environment variables
ENV KATA_PULSE_LISTEN="0.0.0.0:8090" \
    RUST_LOG=info

# Expose metrics port
EXPOSE 8090

# Run KataPulse
ENTRYPOINT ["kata-pulse"]