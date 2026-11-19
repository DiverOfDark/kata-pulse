# Multi-stage Dockerfile for KataPulse
# Real-time metrics for Kata Containers. cadvisor-compatible monitoring agent.
FROM rust:1.90 AS builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    protobuf-compiler \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

# cargo-auditable embeds the resolved crate list in the binary (.dep-v0
# section, survives strip) so Trivy can report Rust CVEs from the image.
RUN cargo install cargo-auditable@0.7.7 --locked

# Copy Cargo files
COPY Cargo.toml Cargo.lock ./

# Copy source code
COPY src ./src

# Build the release binary with cache mounts
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    cargo auditable build --release && \
    cp /app/target/release/kata-pulse /kata-pulse

# Verify the binary works
RUN /kata-pulse --help || true

# Runtime stage - minimal distroless image with CA certificates only
FROM gcr.io/distroless/cc-debian12:latest

# Run as root (system component)
USER root

# Default environment variables
ENV KATA_PULSE_LISTEN="0.0.0.0:8090" \
    RUST_LOG=info

# Copy the built binary from builder stage
COPY --from=builder /kata-pulse /usr/local/bin/kata-pulse

# Expose metrics port
EXPOSE 8090

# Run KataPulse
ENTRYPOINT ["/usr/local/bin/kata-pulse"]