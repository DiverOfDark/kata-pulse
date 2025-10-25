# kata-pulse Helm Chart

Real-time metrics for Kata Containers - A cadvisor-compatible monitoring agent deployed as a DaemonSet with Prometheus PodMonitor.

## Prerequisites

- Kubernetes 1.20+
- Helm 3.0+
- Prometheus Operator (for PodMonitor)
- Kata Containers runtime installed on nodes

## Installation

### Install from GitHub

```bash
helm install kata-pulse oci://ghcr.io/diverofdark/kata-pulse/helm/kata-pulse
```

Or clone the repository and install locally:

```bash
git clone https://github.com/diverofdark/kata-pulse.git
cd kata-pulse
helm install kata-pulse ./helm/kata-pulse
```

### Install to Specific Namespace

```bash
helm install kata-pulse oci://ghcr.io/diverofdark/kata-pulse/helm/kata-pulse -n monitoring --create-namespace
```

### Custom Configuration

```bash
helm install kata-pulse oci://ghcr.io/diverofdark/kata-pulse/helm/kata-pulse \
  --set config.logLevel=debug \
  --set config.metricsIntervalSecs=30 \
  --set resources.limits.memory=512Mi
```

## Configuration

### Basic Values

| Key | Default | Description |
|-----|---------|-------------|
| `image.repository` | `ghcr.io/kata-containers/kata-pulse` | Docker image repository |
| `image.tag` | Chart appVersion | Docker image tag |
| `image.pullPolicy` | `IfNotPresent` | Image pull policy |
| `config.logLevel` | `info` | Log level (trace/debug/info/warn/error) |
| `config.metricsIntervalSecs` | `60` | Metrics collection interval in seconds |
| `config.listenAddress` | `0.0.0.0:8090` | HTTP server listen address |
| `config.runtimeEndpoint` | `/run/containerd/containerd.sock` | CRI runtime socket path |

### Resource Limits

| Key | Default | Description |
|-----|---------|-------------|
| `resources.requests.cpu` | `100m` | CPU request |
| `resources.requests.memory` | `64Mi` | Memory request |
| `resources.limits.cpu` | `500m` | CPU limit |
| `resources.limits.memory` | `256Mi` | Memory limit |

### PodMonitor

| Key | Default | Description |
|-----|---------|-------------|
| `podMonitor.enabled` | `true` | Enable PodMonitor for Prometheus |
| `podMonitor.interval` | `30s` | Scrape interval |
| `podMonitor.scrapeTimeout` | `10s` | Scrape timeout |
| `podMonitor.namespace` | Current namespace | PodMonitor namespace |

### Security

| Key | Default | Description |
|-----|---------|-------------|
| `podSecurityContext.runAsNonRoot` | `true` | Run as non-root user |
| `podSecurityContext.runAsUser` | `65532` | User ID (distroless nonroot) |
| `securityContext.readOnlyRootFilesystem` | `true` | Read-only root filesystem |
| `securityContext.allowPrivilegeEscalation` | `false` | Prevent privilege escalation |

## Deployed Objects

The chart deploys the following Kubernetes objects:

- **DaemonSet** (`kata-pulse`): Runs one pod per node to monitor local Kata Containers
- **PodMonitor** (`kata-pulse`): Prometheus service monitor for automatic metric scraping

### Node Selection

| Key | Default | Description |
|-----|---------|-------------|
| `nodeSelector` | `{}` | Node selector for pod scheduling |
| `tolerations` | `[]` | Tolerations for node taints |
| `affinity` | `{}` | Affinity rules for pod scheduling |

## Volume Mounts

The chart mounts the following host paths:

| Mount Point | Host Path | Purpose |
|-------------|-----------|---------|
| `/run/vc/sbs` | `/run/vc/sbs` | Go runtime sandboxes |
| `/run/kata` | `/run/kata` | Rust runtime sandboxes |
| `/run/containerd` | `/run/containerd` | Containerd socket |

## Monitoring

### PodMonitor Configuration

The chart automatically creates a PodMonitor resource to scrape metrics. Prometheus Operator discovers the PodMonitor and scrapes metrics at the configured interval.

Example Prometheus scrape config (if not using Operator):

```yaml
- job_name: 'kata-pulse'
  kubernetes_sd_configs:
    - role: pod
  relabel_configs:
    - source_labels: [__meta_kubernetes_pod_label_app_kubernetes_io_name]
      action: keep
      regex: kata-pulse
    - source_labels: [__meta_kubernetes_pod_container_port_number]
      action: keep
      regex: "8090"
```

### Metrics Endpoints

- `GET /metrics` - Aggregated metrics from all sandboxes (Prometheus format)
- `GET /sandboxes` - List of running sandboxes (JSON)

## Examples

### Production Deployment

```bash
helm install kata-pulse kata-containers/kata-pulse \
  --set resources.limits.cpu=1000m \
  --set resources.limits.memory=512Mi \
  --set config.logLevel=warn \
  --set config.metricsIntervalSecs=30 \
  --set podMonitor.enabled=true \
  --set podMonitor.interval=30s
```

### Debug Deployment

```bash
helm install kata-pulse kata-containers/kata-pulse \
  --set config.logLevel=debug \
  --set config.metricsIntervalSecs=10 \
  --set podMonitor.interval=10s
```

### Custom Node Selection

```bash
helm install kata-pulse kata-containers/kata-pulse \
  --set nodeSelector.kata=true \
  --set tolerations[0].key=kata \
  --set tolerations[0].operator=Equal \
  --set tolerations[0].value=true \
  --set tolerations[0].effect=NoSchedule
```

## Uninstall

```bash
helm uninstall kata-pulse
```

## Upgrading

```bash
helm upgrade kata-pulse kata-containers/kata-pulse --values values.yaml
```

## Troubleshooting

### Pod Not Starting

Check logs:
```bash
kubectl logs -l app.kubernetes.io/name=kata-pulse --all-containers=true
```

Check pod status:
```bash
kubectl describe pod -l app.kubernetes.io/name=kata-pulse
```

### No Metrics Being Scraped

1. Verify PodMonitor was created:
   ```bash
   kubectl get podmonitor
   ```

2. Check Prometheus targets:
   - Access Prometheus UI
   - Look for `kata-pulse` job

3. Verify metrics endpoint:
   ```bash
   kubectl port-forward ds/kata-pulse 8090:8090
   curl http://localhost:8090/metrics
   ```

### Permission Issues

Ensure host paths exist and are readable:
```bash
ls -la /run/vc/sbs
ls -la /run/kata
ls -la /run/containerd/containerd.sock
```

## Contributing

Contributions are welcome! Please submit issues and pull requests to the [kata-pulse repository](https://github.com/diverofdark/kata-pulse).

## License

Apache License 2.0 - See LICENSE file for details.
