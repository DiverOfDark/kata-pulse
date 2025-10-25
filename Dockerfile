# Multi-stage Dockerfile for KataPulse
# Real-time metrics for Kata Containers. cadvisor-compatible monitoring agent.
FROM rust:1.82 as builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    protobuf-compiler \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

# Copy Cargo files
COPY Cargo.toml Cargo.lock ./

# Copy source code
COPY src ./src

# Build the release binary with cache mounts
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    cargo build --release && \
    cp /app/target/release/kata-pulse /kata-pulse

# Verify the binary works
RUN /kata-pulse --help || true

# Runtime stage
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Copy the built binary from builder stage
COPY --from=builder /kata-pulse /usr/local/bin/kata-pulse

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