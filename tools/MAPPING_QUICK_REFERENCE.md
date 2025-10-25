# Kata to cAdvisor Metrics Mapping - Quick Reference

## TL;DR - Implementation Formulas

### Memory Mapping
```
container_memory_usage_bytes = kata_guest_meminfo{item="mem_total"} - kata_guest_meminfo{item="mem_free"}
# Alternative (more accurate):
container_memory_usage_bytes = kata_guest_meminfo{item="mem_available"}

container_memory_working_set_bytes = kata_guest_meminfo{item="active"} + kata_guest_meminfo{item="inactive_file"}

container_memory_cache = kata_guest_meminfo{item="cached"} + kata_guest_meminfo{item="buffers"}

container_memory_rss = kata_guest_meminfo{item="anon_pages"}

container_memory_swap = kata_guest_meminfo{item="swap_total"} - kata_guest_meminfo{item="swap_free"}
```

### CPU Mapping
```
# Container CPU time (sum across all CPUs)
container_cpu_usage_seconds_total =
    SUM(kata_guest_cpu_time{item="user"}) +
    SUM(kata_guest_cpu_time{item="system"}) +
    SUM(kata_guest_cpu_time{item="guest"}) +
    SUM(kata_guest_cpu_time{item="nice"})
    / 100  # Convert from jiffies to seconds

container_cpu_system_seconds_total =
    SUM(kata_guest_cpu_time{item="system"}) / 100

container_cpu_user_seconds_total =
    SUM(kata_guest_cpu_time{item="user"}) / 100

container_cpu_load_average_10s = kata_guest_load{item="load5"}  # Use 5min as approximation
```

### Network Mapping (Filter interfaces first!)
```
# Filter: eth0, veth*, tap*, tun* (skip lo, docker0, br-*)
container_network_receive_bytes_total =
    SUM(kata_guest_netdev_stat{interface=~"eth0|veth.*|tap.*|tun.*", item="recv_bytes"})

container_network_transmit_bytes_total =
    SUM(kata_guest_netdev_stat{interface=~"eth0|veth.*|tap.*|tun.*", item="xmit_bytes"})

container_network_receive_packets_total =
    SUM(kata_guest_netdev_stat{interface=~"eth0|veth.*|tap.*|tun.*", item="recv_packets"})

container_network_transmit_packets_total =
    SUM(kata_guest_netdev_stat{interface=~"eth0|veth.*|tap.*|tun.*", item="xmit_packets"})

container_network_receive_errors_total =
    SUM(kata_guest_netdev_stat{interface=~"eth0|veth.*|tap.*|tun.*", item="recv_errs"})

container_network_transmit_errors_total =
    SUM(kata_guest_netdev_stat{interface=~"eth0|veth.*|tap.*|tun.*", item="xmit_errs"})

container_network_receive_packets_dropped_total =
    SUM(kata_guest_netdev_stat{interface=~"eth0|veth.*|tap.*|tun.*", item="recv_drop"})

container_network_transmit_packets_dropped_total =
    SUM(kata_guest_netdev_stat{interface=~"eth0|veth.*|tap.*|tun.*", item="xmit_drop"})
```

### Disk I/O Mapping
```
# Operations (count)
container_fs_reads_total =
    SUM(kata_guest_diskstat{item="reads"})

container_fs_writes_total =
    SUM(kata_guest_diskstat{item="writes"})

# Sectors -> Bytes conversion
container_fs_reads_bytes_total =
    SUM(kata_guest_diskstat{item="sectors_read"}) * 512

container_fs_writes_bytes_total =
    SUM(kata_guest_diskstat{item="sectors_written"}) * 512

# I/O Time (milliseconds -> seconds)
container_fs_read_seconds_total =
    SUM(kata_guest_diskstat{item="time_reading"}) / 1000

container_fs_write_seconds_total =
    SUM(kata_guest_diskstat{item="time_writing"}) / 1000

container_fs_io_time_seconds_total =
    SUM(kata_guest_diskstat{item="time_in_progress"}) / 1000

container_fs_io_time_weighted_seconds_total =
    SUM(kata_guest_diskstat{item="weighted_time_in_progress"}) / 1000

# Per-device (if needed, don't aggregate)
container_blkio_device_usage_total{operation="Read"} =
    kata_guest_diskstat{disk="<device>", item="sectors_read"} * 512

container_blkio_device_usage_total{operation="Write"} =
    kata_guest_diskstat{disk="<device>", item="sectors_written"} * 512
```

### Process Metrics Mapping
```
container_processes = kata_guest_tasks{item="cur"}

container_threads =
    kata_shim_threads +
    kata_hypervisor_threads +
    kata_agent_threads +
    kata_virtiofsd_threads  # Sum across all components

container_threads_max = kata_guest_tasks{item="max"}

container_file_descriptors =
    kata_shim_fds +
    kata_hypervisor_fds +
    kata_virtiofsd_fds +
    kata_agent_fds  # Sum across all components
```

---

## Critical Implementation Notes

### 1. Unit Conversions Required
- **CPU time**: Divide by 100 (jiffies → seconds)
- **Disk time**: Divide by 1000 (milliseconds → seconds)
- **Disk sectors**: Multiply by 512 (sectors → bytes)
- **Memory**: Already in bytes, no conversion needed

### 2. Label Strategy
```
# Source metric labels:
kata_guest_*{
    sandbox_id="...",        # Unique VM identifier
    cri_uid="",              # EMPTY! Must be populated from CRI
    cri_name="",             # EMPTY! Must be populated from CRI
    cri_namespace="",        # EMPTY! Must be populated from CRI
    ...
}

# Target metric labels (must be enriched):
container_*{
    id="<sandbox_id>",                    # From metric
    pod="<cri_name>",                     # From CRI lookup
    namespace="<cri_namespace>",          # From CRI lookup
    container="",                         # Can't map (VM-level data)
    image="",                             # Can't map (need CRI pod spec)
}
```

### 3. Aggregation Strategy
- **SUM**: When aggregating across CPUs, disks, interfaces (total view)
- **PER-DEVICE**: Keep separate for blkio metrics
- **PER-CPU**: Keep separate if per-cpu metrics needed
- **PER-INTERFACE**: Aggregate with filter (eth0 only)

### 4. Interface Filtering Regex
```
Match: eth0, veth[a-f0-9]+, tap[0-9]+, tun[0-9]+
Skip: lo, docker0, br-.*, vxlan.*, flannel.*
```

### 5. Label Enrichment Requirement
**CRITICAL**: CRI labels are empty in metrics!
- Option A: Query K8s API separately for sandbox → pod mapping
- Option B: Cache pod info from Kata monitor startup
- Option C: Accept metrics without K8s labels (sandbox_id only)

Recommendation: **Implement Option A** - Add CRI lookup service

---

## Coverage Matrix

| Metric Category | cAdvisor Metrics | Kata Coverage | Notes |
|---|---|---|---|
| **CPU** | 8 metrics | 5/8 | Missing: quota, period, shares, throttling |
| **Memory** | 14 metrics | 10/14 | Missing: peak, limits, OOM events |
| **Network** | 8 metrics | 8/8 | ✅ Fully mapped |
| **Disk I/O** | 15 metrics | 12/15 | Missing: filesystem usage, inode counts |
| **Process** | 5 metrics | 5/5 | ✅ Fully mapped |
| **Pressure** | 6 metrics | 0/6 | PSI not collected by Kata |
| **Labels** | 8 labels | 3/8 | Missing: image, container, pod name/ns |
| **TOTAL** | 64 metrics | ~48/64 | **~75% coverage** |

---

## Data Validation Rules

```
# Memory sanity checks
meminfo{mem_total} > meminfo{mem_free}  # Always true
meminfo{mem_available} <= meminfo{mem_total}  # Always true
meminfo{active} + meminfo{inactive} <= meminfo{mem_total}  # Usually true

# CPU sanity checks
cpu_time{user} + cpu_time{system} + cpu_time{idle} >= cpu_time{nice}  # Monotonic
Per-CPU times can be summed for total

# Network sanity checks
recv_packets >= recv_drop  # Drops are subset of receives
xmit_packets >= xmit_drop  # Drops are subset of transmits

# Disk sanity checks
sectors_read * 512 = bytes_read (approximately)
time_reading / reads = avg time per read (estimate)
```

---

## Common Transformation Pseudocode

```python
def kata_to_cadvisor(kata_metrics):
    """Transform Kata metrics to cAdvisor format"""

    result = {}

    # CPU: Aggregate across CPUs and convert jiffies
    total_cpu_jiffies = sum([
        kata_metrics.guest_cpu_time.user,
        kata_metrics.guest_cpu_time.system,
        kata_metrics.guest_cpu_time.guest,
        kata_metrics.guest_cpu_time.nice
    ])
    result['container_cpu_usage_seconds_total'] = total_cpu_jiffies / 100

    # Memory: Direct calculation
    result['container_memory_usage_bytes'] = (
        kata_metrics.guest_meminfo.mem_total -
        kata_metrics.guest_meminfo.mem_free
    )

    # Network: Filter interfaces and sum
    net_if = filter(lambda x: x in ['eth0', 'veth*', 'tap*', 'tun*'],
                    kata_metrics.guest_netdev_stat.keys())
    result['container_network_receive_bytes_total'] = sum([
        kata_metrics.guest_netdev_stat[iface].recv_bytes
        for iface in net_if
    ])

    # Disk: Aggregate and convert sectors
    result['container_fs_reads_bytes_total'] = (
        sum([d.sectors_read for d in kata_metrics.guest_diskstat]) * 512
    )
    result['container_fs_read_seconds_total'] = (
        sum([d.time_reading for d in kata_metrics.guest_diskstat]) / 1000
    )

    # Process counts
    result['container_processes'] = kata_metrics.guest_tasks.cur
    result['container_threads'] = (
        kata_metrics.shim_threads +
        kata_metrics.hypervisor_threads +
        kata_metrics.agent_threads +
        kata_metrics.virtiofsd_threads
    )

    # Labels (must enrich with CRI data)
    result['labels'] = {
        'id': kata_metrics.sandbox_id,
        'pod': cri_lookup(kata_metrics.sandbox_id).pod_name,
        'namespace': cri_lookup(kata_metrics.sandbox_id).namespace,
    }

    return result
```

---

## Testing Checklist for Implementation

- [ ] Memory calculation: verify `mem_used = mem_total - mem_free` matches `mem_available`
- [ ] CPU time monotonicity: each sample > previous sample
- [ ] Network aggregation: eth0 bytes > individual interface bytes
- [ ] Disk conversion: sectors_read * 512 is reasonable byte count
- [ ] Time conversion: time values after /1000 and /100 are sensible
- [ ] Label enrichment: pod/namespace resolved from CRI
- [ ] Multiple sandboxes: metrics correctly separated by sandbox_id
- [ ] Performance: mapping code executes in <100ms per metric batch

