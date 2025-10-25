# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**KataPulse** is a Rust-based real-time metrics agent for Kata Containers. cadvisor-compatible monitoring agent that provides:
- Metrics collection from multiple sandboxes (aggregation and per-sandbox)
- Sandbox management and lifecycle tracking
- Agent URL discovery for individual sandboxes
- HTTP API with both HTML and plain-text responses

The project was ported from Go to Rust and runs as a system-level daemon/container component.

## Common Development Commands

### Build and Run
```bash
cargo build                    # Debug build
cargo build --release         # Optimized release build
cargo run                      # Run with defaults
cargo run -- --help           # Show available CLI options
```

### Configuration
The application accepts these environment variables and CLI flags:
- `--listen-address` / `KATA_PULSE_LISTEN`: HTTP server listen address (default: `127.0.0.1:8090`)
- `--runtime-endpoint` / `RUNTIME_ENDPOINT`: CRI runtime socket path (default: `/run/containerd/containerd.sock`)
- `--log-level` / `RUST_LOG`: Log level (trace/debug/info/warn/error, default: info)
- `--metrics-interval-secs` / `KATA_PULSE_METRICS_INTERVAL`: Metrics collection interval in seconds (default: 60)

### Testing and Quality
```bash
cargo test                     # Run all tests
cargo check                    # Quick syntax/type check
cargo clippy                   # Linting checks
cargo fmt                      # Format code
cargo fmt -- --check          # Check formatting without modifying
```

### Docker
```bash
./build-docker.sh             # Build production Docker image
./build-docker-debug.sh       # Build debug Docker image with more logging
```

## Architecture

### Key Components

#### **Main Entry Point** (`src/main.rs`)
- Parses CLI arguments and environment variables
- Initializes logging (via tracing-subscriber)
- Initializes Prometheus metrics
- Spawns pod cache updater task
- Starts the HTTP server

#### **KataMonitor** (`src/monitor/mod.rs`)
The core monitoring orchestrator that:
- Manages sandbox cache (in-memory HashMap of sandbox metadata)
- Manages metrics cache (in-memory HashMap of metrics per sandbox)
- Monitors the sandbox filesystem directory (`/run/vc/sbs`) for new/deleted sandboxes
- Periodically syncs sandbox metadata from the CRI runtime
- Runs an async loop that updates cache every 5 seconds and checks filesystem every 5 seconds
- Spawns periodic metrics collection task (configurable interval, default 60s)
- Cleans up metrics cache when sandboxes are deleted

#### **SandboxCache** (`src/monitor/sandbox_cache.rs`)
Thread-safe cache (Arc<RwLock<HashMap>>) storing:
- `uid`: Kubernetes UID
- `name`: Pod/sandbox name
- `namespace`: Kubernetes namespace

#### **MetricsCache** (`src/monitor/metrics_cache.rs`)
Thread-safe cache (Arc<RwLock<HashMap>>) for Prometheus metrics:
- Stores parsed metrics per sandbox
- Tracks collection timestamp
- Supports add, get, delete operations
- Cleaned up when sandboxes are removed

#### **CRI Integration** (`src/monitor/cri.rs`, `src/monitor/cri_client.rs`)
- Connects to Kubernetes CRI (Container Runtime Interface) endpoint
- Uses `k8s-cri` crate for API types and `tonic` for gRPC
- Enriches sandbox cache with Kubernetes metadata (pod names, namespaces, UIDs)
- Handles connection timeouts and retries

#### **Shim Client** (`src/utils/shim_client.rs`)
- Communicates with per-sandbox monitoring sockets via Unix domain sockets
- Retrieves metrics (`/metrics`) from individual shims
- Supports both Go runtime (`/run/vc/sbs`) and Rust runtime (`/run/kata`) socket paths
- 3-second default timeout per request

#### **HTTP Server** (`src/server.rs`)
Actix-web based HTTP API with routes:
- `GET /`: Index page (HTML or plain text based on Accept header)
- `GET /metrics`: Aggregated metrics from all sandboxes (or per-sandbox if `?sandbox=<id>`)
- `GET /sandboxes`: List all running sandboxes

Handlers return 200 with text/plain on success, or appropriate error codes with error descriptions.

#### **Metrics** (`src/monitor/metrics.rs`)
Prometheus metrics exposed as:
- `kata_monitor_running_shim_count` (gauge): Number of active sandboxes
- `kata_monitor_scrape_count` (counter): Total metrics scrape requests
- `kata_monitor_scrape_failed_count` (counter): Failed scrape requests
- `kata_monitor_scrape_durations_histogram_milliseconds` (histogram): Scrape latency buckets (1ms to 512ms)

### Data Flow

1. **Startup**:
   - Monitor starts, reads initial sandbox list from filesystem
   - Spawns pod cache updater task
   - Spawns metrics collector task
   - Starts HTTP server

2. **Pod Cache Update Loop** (every 5 seconds):
   - Query CRI runtime for metadata on known sandboxes
   - Poll filesystem for new/deleted sandboxes
   - Update sandbox cache
   - When sandbox deleted: also delete from metrics cache

3. **Metrics Collection Loop** (configurable interval, default 60s):
   - Get current list of active sandboxes
   - Query each sandbox's shim for metrics via Unix socket
   - Parse Prometheus format into object model
   - Store parsed metrics in metrics cache

4. **Request Processing**:
   - HTTP request arrives for `/metrics`
   - For metrics: return cached metrics (no I/O wait)
   - Aggregates responses or returns per-sandbox data
   - Updates Prometheus metrics on success/failure

## Important Implementation Details

### Sandbox Directory Monitoring
The monitor watches `/run/vc/sbs` (Go runtime) and can also access `/run/kata` (Rust runtime):
- Initial read populates the cache
- Periodic filesystem checks detect new sandboxes
- `SandboxCache::put_if_not_exists()` prevents duplicates
- CRI client attempts to resolve metadata for sandboxes without it

### Socket Path Resolution
`config::client_socket_address()` tries both:
1. `/run/vc/sbs/{sandbox_id}/shim-monitor.sock` (Go)
2. `/run/kata/{sandbox_id}/shim-monitor.sock` (Rust)

Returns first match or error if neither exists.

### Error Handling
- All public async functions return `Result<T>` (anyhow::Result)
- Errors are logged via tracing macros but don't crash the daemon
- HTTP handlers gracefully degrade (return 500 with error message)
- Metrics track failure rates

### Concurrency Model
- Uses Tokio async runtime for all I/O operations
- `SandboxCache` uses Arc<RwLock<>> for thread-safe, concurrent access
- Main loop is a single-threaded async loop with yield points via `sleep()`
- HTTP server runs in Actix worker pool (configurable, default depends on CPU count)

## Dependencies of Note

- **actix-web**: Web framework
- **tokio**: Async runtime
- **tracing**: Structured logging
- **prometheus**: Metrics collection
- **tonic**: gRPC client (for CRI)
- **k8s-cri**: Kubernetes CRI API definitions
- **containerd-client**: Unix socket connection helpers

## Testing Guidance

- Unit tests exist for metrics and cache logic
- No integration tests currently; primarily tested via Docker/Kubernetes deployments
- Manual testing: Use `curl` against HTTP endpoints with real or mock sandboxes
- Debug build includes verbose tracing output

## Kubernetes Deployment

See `daemonset.yaml` and `daemonset.debug.yaml` for K8s deployment patterns. The application is designed to run as a DaemonSet (one per node) to monitor all local sandboxes.

## Claude Code Guidelines

### When Working on This Codebase

1. **Avoid Redundant Reports**
   - Do NOT create a summary or report after completing each task
   - Do NOT create documentation files unless explicitly requested
   - Focus on code changes only
   - Use existing documentation as reference

2. **Communicate Efficiently**
   - Provide brief status updates: "✅ Done" or "❌ Issue: ..."
   - Include test results: "All 15 tests pass" or "3 tests fail"
   - List changes made (file paths only)
   - Identify any blockers or decisions needed

3. **Task Completion Criteria**
   - Code compiles (`cargo check`)
   - All tests pass (`cargo test`)
   - Changes are backward compatible
   - No breaking API changes

4. **Only Create Reports When Requested**
   - "Write a summary" → Create documentation
   - "Document this" → Create documentation
   - "Explain X" → Create documentation
   - Default behavior → Only code changes, minimal communication

5. **Use Existing Docs**
   - Reference existing .md files for context
   - Update existing docs rather than creating new ones
   - Keep documentation in sync with code changes

6. **Git Commits - NEVER Commit Automatically**
   - **NEVER** commit changes by yourself
   - **ALWAYS** show the changes with `git status` and `git diff`
   - **ALWAYS** let the user review and approve before committing
   - User must explicitly say "commit this" or similar
   - Wait for user review and explicit approval
   - This ensures complete control over what goes into the repository