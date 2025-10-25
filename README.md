# kata-pulse

[![Build and Push](https://github.com/kata-containers/kata-pulse/actions/workflows/build-and-push.yml/badge.svg)](https://github.com/kata-containers/kata-pulse/actions)
[![Tests](https://github.com/kata-containers/kata-pulse/actions/workflows/test.yml/badge.svg)](https://github.com/kata-containers/kata-pulse/actions)

Real-time metrics for Kata Containers. A cadvisor-compatible monitoring agent providing metrics collection for Kata Containers environments.

## Overview

**kata-pulse** is a lightweight Rust-based monitoring daemon that:
- 📊 Collects metrics from Kata Container sandboxes
- 🔄 Aggregates metrics across all running sandboxes
- 🏷️ Maps Cloud Hypervisor metrics to cAdvisor-compatible format
- 🔗 Discovers per-sandbox monitoring agents
- 🎯 Integrates seamlessly with Kubernetes and Prometheus monitoring stacks

## Features

### Core Capabilities
- **Multi-Sandbox Metrics Collection** - Monitor metrics from multiple Kata sandboxes simultaneously
- **Automatic Sandbox Discovery** - Detects new sandboxes from filesystem and CRI runtime
- **Kubernetes Integration** - Enriches metrics with pod names, namespaces, and UIDs from CRI
- **cAdvisor Compatibility** - Outputs metrics in cAdvisor-compatible Prometheus format
- **Automatic Cleanup** - Removes cached metrics when sandboxes are deleted

### Performance
- **Zero-Copy Architecture** - Uses Arc for efficient memory management
- **Async/Await** - Tokio-based asynchronous operations for high throughput
- **Caching Strategy** - In-memory caches for sandbox metadata and metrics
- **Configurable Intervals** - Adjustable metrics collection frequency
- **Layer Caching** - Docker multi-stage build with dependency caching

### Monitoring
- **Prometheus Metrics** - Exposes health and performance metrics
- **Health Checks** - HTTP endpoint for container health verification
- **Structured Logging** - Tracing-based logging with configurable levels
- **Error Tracking** - Comprehensive error handling and reporting

## Quick Start

### Docker

```bash
# Pull from GitHub Container Registry
docker pull ghcr.io/kata-containers/kata-pulse:latest

# Run with defaults
docker run -d \
  --name kata-pulse \
  -p 8090:8090 \
  -v /run/kata:/run/kata:ro \
  -v /run/vc/sbs:/run/vc/sbs:ro \
  -v /run/containerd/containerd.sock:/run/containerd/containerd.sock:ro \
  ghcr.io/kata-containers/kata-pulse:latest

# Check metrics
curl http://localhost:8090/metrics
```

### From Source

```bash
# Clone repository
git clone https://github.com/kata-containers/kata-pulse.git
cd kata-pulse

# Build release binary
cargo build --release

# Run
./target/release/kata-pulse

# Or with custom config
./target/release/kata-pulse \
  --listen-address 0.0.0.0:8090 \
  --runtime-endpoint /run/containerd/containerd.sock \
  --log-level info
```

## Configuration

### Environment Variables

```bash
# HTTP server configuration
KATA_PULSE_LISTEN=127.0.0.1:8090              # Listen address (default)
RUST_LOG=info                                   # Log level (trace/debug/info/warn/error)

# Container runtime
RUNTIME_ENDPOINT=/run/containerd/containerd.sock  # CRI socket path

# Metrics collection
KATA_MONITOR_METRICS_INTERVAL=60                # Interval in seconds (default: 60)
```

### Command Line Arguments

```bash
./target/release/kata-pulse --help

OPTIONS:
  -l, --listen-address <LISTEN_ADDRESS>
          HTTP server listen address
          [default: 127.0.0.1:8090]
          [env: KATA_PULSE_LISTEN]

  -r, --runtime-endpoint <RUNTIME_ENDPOINT>
          CRI runtime socket path
          [default: /run/containerd/containerd.sock]
          [env: RUNTIME_ENDPOINT]

  -m, --metrics-interval-secs <METRICS_INTERVAL_SECS>
          Metrics collection interval in seconds
          [default: 60]
          [env: KATA_MONITOR_METRICS_INTERVAL]

  -h, --help
          Print help
```

## API Endpoints

### GET /

Returns HTML or plain text index page (based on Accept header)

```bash
curl http://localhost:8090/
```

### GET /metrics

Aggregated metrics from all sandboxes in Prometheus format

```bash
curl http://localhost:8090/metrics
curl http://localhost:8090/metrics?sandbox=sandbox-123  # Per-sandbox
```

### GET /sandboxes

List all running sandboxes

```bash
curl http://localhost:8090/sandboxes

[
  {
    "sandbox_id": "abc123...",
    "pod_name": "my-pod",
    "namespace": "default",
    "uid": "12345-67890"
  }
]
```

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                   kata-pulse Daemon                     │
├─────────────────────────────────────────────────────────┤
│                                                         │
│  ┌──────────────────┐         ┌──────────────────┐      │
│  │  HTTP Server     │         │  Metrics Cache   │      │
│  │  (Actix-web)     │──────→  │  (Arc<RwLock>)   │      │
│  └──────────────────┘         └──────────────────┘      │
│       ↓                              ↑                  │
│  GET /metrics                   Updated every           │
│  GET /sandboxes                 60 seconds              │
│                                                         │
│  ┌──────────────────┐         ┌──────────────────┐      │
│  │ Metrics Collector│         │ Sandbox Cache    │      │
│  │ (Periodic Task)  │──────→  │ (Arc<RwLock>)    │      │
│  └──────────────────┘         └──────────────────┘      │
│       ↓                             ↑                   │
│   Per-sandbox shim    ← CRI Sync Task (every 5s)        │
│                                                         │
│  ┌──────────────────┐         ┌──────────────────┐      │
│  │ Sandbox Manager  │         │ Directory        │      │
│  │ (SandboxCache    │ ←────→  │ Monitor          │      │
│  │  + CRI Client)   │         │ (/run/vc/sbs)    │      │
│  └──────────────────┘         └──────────────────┘      │
│                                                         │
└─────────────────────────────────────────────────────────┘
```

### Key Components

1. **HTTP Server** - Serves metrics and manages requests
2. **Metrics Collector** - Periodic task collecting metrics from sandboxes
3. **Sandbox Cache Manager** - Monitors filesystem and syncs with CRI
4. **CRI Client** - Communicates with container runtime (Kubernetes)
5. **Metrics Converter** - Transforms Cloud Hypervisor metrics to cAdvisor format

## Metrics Format

Output is cAdvisor-compatible Prometheus format:

```prometheus
# CPU metrics
container_cpu_usage_seconds_total{container="",id="/kubepods/...",image="",name="my-pod",namespace="default",pod="my-pod",cpu="total"} 1234.5

# Memory metrics
container_memory_usage_bytes{container="",id="/kubepods/...",image="",name="my-pod",namespace="default",pod="my-pod"} 536870912

# Network metrics (per-interface)
container_network_receive_bytes_total{container="",id="/kubepods/...",image="",name="my-pod",namespace="default",pod="my-pod",interface="eth0"} 1024000

# Disk I/O metrics (per-device)
container_blkio_device_usage_total{container="",device="",id="/kubepods/...",image="",major="8",minor="0",name="my-pod",namespace="default",operation="Read",pod="my-pod"} 2000000

# Process/task metrics
container_processes_count{container="",id="/kubepods/...",image="",name="my-pod",namespace="default",pod="my-pod"} 42
```

## Development

### Build

```bash
# Debug build
cargo build

# Release build (optimized)
cargo build --release

# Check without building
cargo check
```

### Testing

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_name

# Run with output
cargo test -- --nocapture

# Check coverage
cargo tarpaulin --out Html
```

### Code Quality

```bash
# Format code
cargo fmt

# Check formatting
cargo fmt -- --check

# Lint with clippy
cargo clippy -- -D warnings

# Security audit
cargo audit
```

## Kubernetes Deployment

### DaemonSet Example

```yaml
apiVersion: apps/v1
kind: DaemonSet
metadata:
  name: kata-pulse
  namespace: kube-system
spec:
  selector:
    matchLabels:
      app: kata-pulse
  template:
    metadata:
      labels:
        app: kata-pulse
    spec:
      hostNetwork: true
      containers:
      - name: kata-pulse
        image: ghcr.io/kata-containers/kata-pulse:latest
        ports:
        - name: metrics
          containerPort: 8090
          hostPort: 8090
        env:
        - name: RUST_LOG
          value: info
        volumeMounts:
        - name: sandbox-dir
          mountPath: /run/vc/sbs
          readOnly: true
        - name: kata-dir
          mountPath: /run/kata
          readOnly: true
        - name: containerd-socket
          mountPath: /run/containerd
          readOnly: true
        livenessProbe:
          httpGet:
            path: /
            port: metrics
          initialDelaySeconds: 5
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /
            port: metrics
          initialDelaySeconds: 5
          periodSeconds: 10
      volumes:
      - name: sandbox-dir
        hostPath:
          path: /run/vc/sbs
      - name: kata-dir
        hostPath:
           path: /run/kata
      - name: containerd-socket
        hostPath:
          path: /run/containerd
```

## Troubleshooting

### No metrics appearing

1. Check logs
   ```bash
   docker logs kata-pulse
   RUST_LOG=debug ./target/release/kata-pulse
   ```

2. Verify sandbox connectivity
   ```bash
   ls /run/vc/sbs  # Should see sandbox directories
   ```

3. Check CRI socket
   ```bash
   ls -la /run/containerd/containerd.sock
   ```

### High memory usage

- Adjust metrics cache cleanup
- Reduce metrics collection frequency
- Monitor number of active sandboxes

### Slow metrics collection

- Check network connectivity to CRI
- Review Prometheus scrape interval
- Check system load and available resources

## Performance Considerations

| Metric | Value |
|--------|-------|
| Memory per sandbox | ~2-5 MB |
| Metrics latency | <1 second |
| CPU overhead | <1% per 100 sandboxes |
| Typical startup time | <2 seconds |

## Contributing

Contributions are welcome! Please:

1. Fork the repository
2. Create a feature branch (`git checkout -b feat/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feat/amazing-feature`)
5. Open a Pull Request

### Development Guidelines

- Follow Rust naming conventions
- Write tests for new features
- Update documentation
- Run `cargo fmt` and `cargo clippy`
- Ensure all tests pass: `cargo test`

## License

This project is licensed under the Apache License 2.0 - see individual source files for details.

## References

- [Kata Containers](https://katacontainers.io/)
- [cAdvisor Metrics Format](https://github.com/google/cadvisor)
- [Prometheus Format](https://prometheus.io/docs/instrumenting/exposition_formats/)
- [Kubernetes CRI](https://github.com/kubernetes/cri-api)
- [Cloud Hypervisor](https://www.cloudhypervisor.org/)

## Support

For issues, questions, or suggestions:

1. Check [GitHub Issues](https://github.com/kata-containers/kata-pulse/issues)
2. Review [documentation](.github/workflows)
3. Open a new issue with detailed information

## Acknowledgments

Built with:
- 🦀 Rust
- ⚡ Tokio async runtime
- 🌐 Actix-web HTTP framework
- 📊 Prometheus metrics format
- 🐳 Docker containerization
