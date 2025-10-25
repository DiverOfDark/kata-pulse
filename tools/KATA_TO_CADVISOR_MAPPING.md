# Kata Metrics to cAdvisor Metrics Mapping

## Overview

This document maps metrics from the Kata Containers monitoring system to the cAdvisor (Container Advisor) metrics format. The goal is to transform Kata metrics into cAdvisor-compatible metrics for seamless integration with container monitoring stacks.

### Key Differences

- **cAdvisor**: Provides container-level metrics from the host perspective (cgroups-based)
- **Kata**: Provides both guest VM-level metrics and host-side (shim) process metrics for each sandbox
- **Mapping Challenge**: Kata guest metrics represent VM-internal metrics; correlation to cAdvisor requires careful label mapping and calculation

---

## 1. CPU METRICS

### Mapping: CPU Time/Usage

| cAdvisor | Kata Source | Notes |
|----------|-------------|-------|
| `container_cpu_usage_seconds_total{cpu="total"}` | `kata_guest_cpu_time{item="total"}` | Total guest CPU time in seconds |
| `container_cpu_user_seconds_total` | `kata_guest_cpu_time{item="user"}` | User mode CPU time (requires parse from proc_stat or separate metric) |
| `container_cpu_system_seconds_total` | `kata_guest_cpu_time{item="system"}` | System mode CPU time (requires parse from proc_stat) |
| `container_cpu_load_average_10s` | `kata_guest_load{item="load_avg_10_second"}` | 10-second load average from guest VM |
| (no direct match) | `kata_shim_process_cpu_seconds_total` | **Shim process CPU time** - host-side overhead |
| (no direct match) | `kata_hypervisor_proc_stat` (user/system fields) | **Hypervisor (QEMU) CPU time** - hypervisor overhead |

### Mapping: CPU Limits/Quota

| cAdvisor | Kata Source | Notes |
|----------|-------------|-------|
| `container_spec_cpu_period` | *Not provided in Kata* | **MISSING**: CPU period (typically 100000 µs) |
| `container_spec_cpu_quota` | *Not provided in Kata* | **MISSING**: CPU quota in µs |
| `container_spec_cpu_shares` | *Not provided in Kata* | **MISSING**: CPU shares |
| `container_cpu_cfs_periods_total` | *Not provided in Kata* | **MISSING**: CFS period count |
| `container_cpu_cfs_throttled_periods_total` | *Not provided in Kata* | **MISSING**: Throttled periods |
| `container_cpu_cfs_throttled_seconds_total` | *Not provided in Kata* | **MISSING**: Throttled time |

---

## 2. MEMORY METRICS

### Mapping: Memory Usage

| cAdvisor | Kata Source | Notes |
|----------|-------------|-------|
| `container_memory_usage_bytes` | `kata_guest_meminfo{item="memtotal"} - kata_guest_meminfo{item="memfree"}` | Calculated from guest meminfo |
| `container_memory_working_set_bytes` | `kata_guest_meminfo{item="active"} + kata_guest_meminfo{item="inactive_file"}` | Active memory + inactive file cache |
| `container_memory_rss` | `kata_guest_meminfo{item="anon_pages"}` | Anonymous memory (RSS approximation) |
| `container_memory_cache` | `kata_guest_meminfo{item="cached"} + kata_guest_meminfo{item="buffers"}` | File cache + buffers |
| `container_memory_swap` | `kata_guest_meminfo{item="swaptotal"} - kata_guest_meminfo{item="swapfree"}` | Swap usage |
| `container_memory_mapped_file` | `kata_guest_meminfo{item="mapped"}` | Memory-mapped file size |
| `container_memory_kernel_usage` | `kata_guest_meminfo{item="kernel_stack"} + ...` | **PARTIAL**: Aggregate of kernel-related fields |
| `container_memory_max_usage_bytes` | *Not directly provided* | **MISSING**: Peak memory usage |
| `container_memory_failcnt` | *Not provided in Kata* | **MISSING**: Memory pressure/failure count |
| `container_memory_failures_total` | *Not provided in Kata* | **MISSING**: Total memory allocation failures |

### Mapping: Memory Limits

| cAdvisor | Kata Source | Notes |
|----------|-------------|-------|
| `container_spec_memory_limit_bytes` | `kata_shim_pod_overhead_memory_in_bytes` | Pod overhead, but not full limit |
| `container_spec_memory_reservation_limit_bytes` | *Not provided in Kata* | **MISSING**: Memory reservation |
| `container_spec_memory_swap_limit_bytes` | *Not provided in Kata* | **MISSING**: Swap limit |
| `container_memory_total_active_file_bytes` | `kata_guest_meminfo{item="active_file"}` | Active file cache pages |
| `container_memory_total_inactive_file_bytes` | `kata_guest_meminfo{item="inactive_file"}` | Inactive file cache pages |

### Mapping: Memory Pressure (PSI)

| cAdvisor | Kata Source | Notes |
|----------|-------------|-------|
| `container_pressure_memory_waiting_seconds_total` | *Not provided in Kata* | **MISSING**: PSI memory waiting |
| `container_pressure_memory_stalled_seconds_total` | *Not provided in Kata* | **MISSING**: PSI memory stalled |

---

## 3. NETWORK METRICS

### Mapping: Network I/O

| cAdvisor | Kata Source | Notes |
|----------|-------------|-------|
| `container_network_receive_bytes_total` | `kata_guest_netdev_stat{interface="eth0|veth*", item="recv_bytes"}` | Aggregated per-interface RX bytes |
| `container_network_transmit_bytes_total` | `kata_guest_netdev_stat{interface="eth0|veth*", item="xmit_bytes"}` | Aggregated per-interface TX bytes |
| `container_network_receive_packets_total` | `kata_guest_netdev_stat{interface="eth0|veth*", item="recv_packets"}` | Aggregated RX packets |
| `container_network_transmit_packets_total` | `kata_guest_netdev_stat{interface="eth0|veth*", item="xmit_packets"}` | Aggregated TX packets |
| `container_network_receive_errors_total` | `kata_guest_netdev_stat{interface="eth0|veth*", item="recv_errs"}` | RX errors |
| `container_network_transmit_errors_total` | `kata_guest_netdev_stat{interface="eth0|veth*", item="xmit_errs"}` | TX errors |
| `container_network_receive_packets_dropped_total` | `kata_guest_netdev_stat{interface="eth0|veth*", item="recv_drop"}` | RX dropped packets |
| `container_network_transmit_packets_dropped_total` | `kata_guest_netdev_stat{interface="eth0|veth*", item="xmit_drop"}` | TX dropped packets |

### Notes on Network Metrics:
- **Guest vs. Hypervisor**: `kata_hypervisor_netdev` shows host-side (QEMU) network stats - these represent hypervisor overhead, not container traffic
- **Interface Selection**: Map container networks to eth0/veth*; skip loopback (lo) and internal bridges (docker0, br-*)
- **Aggregation**: Sum across all container-relevant interfaces

---

## 4. BLOCK I/O / DISK METRICS

### Mapping: Disk I/O Operations

| cAdvisor | Kata Source | Notes |
|----------|-------------|-------|
| `container_fs_reads_total` | `kata_guest_diskstat{disk="*", item="reads"}` | Total disk read operations (count) |
| `container_fs_writes_total` | `kata_guest_diskstat{disk="*", item="writes"}` | Total disk write operations (count) |
| `container_fs_reads_bytes_total` | `kata_guest_diskstat{disk="*", item="read_sectors"} * 512` | Read sectors → bytes |
| `container_fs_writes_bytes_total` | `kata_guest_diskstat{disk="*", item="write_sectors"} * 512` | Write sectors → bytes |
| `container_fs_reads_merged_total` | `kata_guest_diskstat{disk="*", item="merged"}` | **PARTIAL**: Merged I/O operations |
| `container_fs_writes_merged_total` | `kata_guest_diskstat{disk="*", item="merged"}` | **LIMITATION**: Can't separate read/write merges |
| `container_fs_sector_reads_total` | `kata_guest_diskstat{disk="*", item="read_sectors"}` | Read sectors |
| `container_fs_sector_writes_total` | `kata_guest_diskstat{disk="*", item="write_sectors"}` | Write sectors |

### Mapping: Disk I/O Time

| cAdvisor | Kata Source | Notes |
|----------|-------------|-------|
| `container_fs_io_time_seconds_total` | `kata_guest_diskstat{disk="*", item="time_in_queue"} / 1000` | I/O in progress time (ms → seconds) |
| `container_fs_io_time_weighted_seconds_total` | *Not available* | **MISSING**: Weighted I/O time |
| `container_fs_read_seconds_total` | *Not available* | **MISSING**: Time spent reading |
| `container_fs_write_seconds_total` | *Not available* | **MISSING**: Time spent writing |
| `container_fs_io_current` | *Not available* | **MISSING**: I/O in progress count |

### Mapping: Block Device Level

| cAdvisor | Kata Source | Notes |
|----------|-------------|-------|
| `container_blkio_device_usage_total{operation="Read"}` | `kata_guest_diskstat` aggregated by device | Per-device read bytes (calculated) |
| `container_blkio_device_usage_total{operation="Write"}` | `kata_guest_diskstat` aggregated by device | Per-device write bytes (calculated) |

### Mapping: File System

| cAdvisor | Kata Source | Notes |
|----------|-------------|-------|
| `container_fs_usage_bytes` | *Not provided in Kata* | **MISSING**: Filesystem usage (du) |
| `container_fs_limit_bytes` | *Not provided in Kata* | **MISSING**: Filesystem limit |
| `container_fs_inodes_free` | *Not provided in Kata* | **MISSING**: Free inodes |
| `container_fs_inodes_total` | *Not provided in Kata* | **MISSING**: Total inodes |

---

## 5. PROCESS METRICS

### Mapping: Process Counts

| cAdvisor | Kata Source | Notes |
|----------|-------------|-------|
| `container_processes` | `kata_guest_tasks` | Number of processes in guest |
| `container_threads` | Sum of `kata_shim_threads`, `kata_guest_*_threads` | Total thread count across components |
| `container_threads_max` | *Not provided* | **MISSING**: Max thread limit |
| `container_sockets` | *Not provided* | **MISSING**: Open socket count |
| `container_file_descriptors` | Sum of `kata_shim_fds`, `kata_hypervisor_fds`, etc. | Aggregated FD count |

### Mapping: Process State

| cAdvisor | Kata Source | Notes |
|----------|-------------|-------|
| `container_tasks_state` | `kata_guest_vm_stat` (parse state fields) | Parse from guest VM stat (running, sleeping, etc.) |

### Mapping: OOM Events

| cAdvisor | Kata Source | Notes |
|----------|-------------|-------|
| `container_oom_events_total` | *Not provided in Kata* | **MISSING**: Out-of-Memory kill events |

---

## 6. PROCESS START TIME

| cAdvisor | Kata Source | Notes |
|----------|-------------|-------|
| `container_start_time_seconds` | *Not provided directly* | **MISSING**: Container start time timestamp |

---

## 7. PRESSURE METRICS (PSI - Process Stall Information)

| cAdvisor | Kata Source | Notes |
|----------|-------------|-------|
| `container_pressure_cpu_waiting_seconds_total` | *Not provided in Kata* | **MISSING**: CPU PSI waiting |
| `container_pressure_cpu_stalled_seconds_total` | *Not provided in Kata* | **MISSING**: CPU PSI stalled |
| `container_pressure_io_waiting_seconds_total` | *Not provided in Kata* | **MISSING**: I/O PSI waiting |
| `container_pressure_io_stalled_seconds_total` | *Not provided in Kata* | **MISSING**: I/O PSI stalled |
| `container_pressure_memory_waiting_seconds_total` | *Not provided in Kata* | **MISSING**: Memory PSI waiting |
| `container_pressure_memory_stalled_seconds_total` | *Not provided in Kata* | **MISSING**: Memory PSI stalled |

---

## 8. LABELS MAPPING

### cAdvisor Standard Labels
```
container_*{
  id="",              # cgroup path (e.g., /kubepods/pod-uuid/container-id)
  image="",           # Container image
  name="",            # Container name
  namespace="",       # Kubernetes namespace
  pod="",             # Pod name
  container="",       # Container name within pod
  device="",          # Device name (for blkio)
  major="",           # Device major number
  minor="",           # Device minor number
  operation="",       # Read/Write
  cpu="",             # CPU number or "total"
}
```

### Kata Labels in Sources
```
kata_guest_*{
  sandbox_id="",      # Sandbox ID (unique identifier)
  cri_uid="",         # Kubernetes UID
  cri_name="",        # Pod name
  cri_namespace="",   # Kubernetes namespace
  item="",            # Metric item (field from /proc/stat, /proc/meminfo, etc.)
  disk="",            # Disk name
  interface="",       # Network interface
}

kata_shim_*{
  sandbox_id="",      # Sandbox ID
  (similar labels)
}
```

### Label Transformation Rules

| cAdvisor Label | Kata Equivalent | Notes |
|---|---|---|
| `id` | `sandbox_id` | Direct mapping |
| `namespace` | `cri_namespace` | Direct mapping |
| `pod` | `cri_name` | Direct mapping |
| `image` | *Not in Kata metrics* | **MISSING** - Would need to query CRI separately |
| `container` | *Not in Kata metrics* | **MISSING** - Would need container-level breakdown |
| `name` | `cri_name` | Same as pod |

---

## 9. MAPPING SUMMARY: COVERAGE ANALYSIS

### Fully Mappable (Direct or Calculated)
- ✅ CPU time (guest)
- ✅ Memory usage (guest - from meminfo)
- ✅ Network I/O (guest)
- ✅ Disk I/O operations (guest)
- ✅ Process/thread counts

### Partially Mappable (Estimates/Aggregation Needed)
- ⚠️ Memory RSS (approx from anon_pages)
- ⚠️ Memory working set (needs careful aggregation)
- ⚠️ Disk I/O time (available but limited detail)
- ⚠️ Process state (available but requires parsing)

### Not Available in Kata Metrics
- ❌ CPU limits/quota/shares
- ❌ CPU throttling metrics
- ❌ Memory limits (spec)
- ❌ Memory peak usage
- ❌ PSI (Process Stall Information) metrics
- ❌ OOM events
- ❌ Filesystem usage (du)
- ❌ Container image information
- ❌ Per-container breakdown within pod
- ❌ Container start time
- ❌ Weighted I/O time
- ❌ Memory allocation failures

---

## 10. ADDITIONAL KATA METRICS (No cAdvisor Equivalent)

### Shim Process Metrics
- `kata_shim_process_*` - Shim Go process metrics (CPU, memory, FDs)
- `kata_shim_go_memstats_*` - Shim Go runtime memory stats
- `kata_shim_go_threads` - Shim goroutine counts
- `kata_shim_go_gc_duration_seconds` - Garbage collection pauses
- `kata_shim_agent_rpc_durations_histogram_milliseconds_*` - Agent RPC call latencies
- `kata_shim_rpc_durations_histogram_milliseconds_*` - Shim RPC call latencies
- `kata_shim_pod_overhead_*` - Pod overhead measurements

### Hypervisor (QEMU) Metrics
- `kata_hypervisor_proc_stat` - QEMU process CPU usage
- `kata_hypervisor_proc_status` - QEMU process state
- `kata_hypervisor_netdev` - QEMU network interface stats (host-side)
- `kata_hypervisor_io_stat` - QEMU disk I/O stats
- `kata_hypervisor_threads` - QEMU thread count
- `kata_hypervisor_fds` - QEMU open file descriptors

### Agent Metrics
- `kata_agent_*_stat` - Agent process CPU/IO stats
- `kata_agent_threads` - Agent thread count
- `kata_agent_*_rss`, `_vm` - Agent memory usage

### VirtioFS Daemon Metrics
- `kata_virtiofsd_proc_stat` - VirtioFS process CPU
- `kata_virtiofsd_threads` - VirtioFS thread count
- `kata_virtiofsd_fds` - VirtioFS file descriptors
- `kata_virtiofsd_io_stat` - VirtioFS I/O stats

### Monitor Process Metrics
- `kata_monitor_process_*` - Monitor daemon resource usage
- `kata_monitor_go_memstats_*` - Monitor Go runtime metrics
- `kata_monitor_running_shim_count` - Count of active sandboxes
- `kata_monitor_scrape_*` - Monitor scrape operation metrics

---

## 11. IMPLEMENTATION RECOMMENDATIONS

### Priority 1: Core Metrics (Highest Value)
1. **Memory**: Map `kata_guest_meminfo` → `container_memory_*`
2. **CPU**: Map `kata_guest_cpu_time` → `container_cpu_usage_seconds_total`
3. **Network**: Map `kata_guest_netdev_stat` → `container_network_*`
4. **Disk I/O**: Map `kata_guest_diskstat` → `container_fs_*`

### Priority 2: Support Metrics
1. Process/thread counts
2. Host-side overhead (shim, hypervisor)
3. Load averages
4. Network/disk errors and drops

### Priority 3: Handle Missing Data
1. Set CPU quota/period to sensible defaults based on pod specification (requires separate query)
2. Skip PSI metrics or mark as unavailable
3. Leave OOM, memory failures as zeros or "not available"
4. For memory peak: track and store separately (not available from Kata guest metrics)

### Label Strategy
- Use `sandbox_id` as primary identifier
- Enhance labels by joining with CRI metadata (uid, name, namespace) if available
- For image: Requires separate CRI call (pod spec)
- For per-container metrics: May require aggregating across pod if full breakdown unavailable

### Unit Conversions
- **Disk sectors → bytes**: Multiply by 512
- **Memory**: Already in bytes in meminfo
- **Time**: Convert ms to seconds (divide by 1000)
- **CPU**: Convert jiffies if needed (platform-dependent, typically 100 jiffies/sec on Linux)

---

## 12. EXAMPLE TRANSFORMATION

### Input (Kata Metrics)
```
kata_guest_meminfo{item="memtotal",sandbox_id="abc123",...} 1024000000
kata_guest_meminfo{item="memfree",sandbox_id="abc123",...} 512000000
kata_guest_cpu_time{item="total",sandbox_id="abc123",...} 1234.5
kata_guest_diskstat{disk="sda",item="read_sectors",sandbox_id="abc123",...} 2000000
kata_guest_diskstat{disk="sda",item="reads",sandbox_id="abc123",...} 50000
kata_guest_netdev_stat{interface="eth0",item="recv_bytes",sandbox_id="abc123",...} 1000000000
```

### Output (cAdvisor-style Metrics)
```
container_memory_usage_bytes{id="abc123",namespace="default",pod="test-pod",...} 512000000
container_cpu_usage_seconds_total{id="abc123",cpu="total",...} 1234.5
container_fs_reads_total{id="abc123",...} 50000
container_fs_reads_bytes_total{id="abc123",...} 1024000000
container_network_receive_bytes_total{id="abc123",...} 1000000000
```

---

## 13. TESTING CHECKLIST & VALIDATION RESULTS

### ✅ Validated Against Real Data

- [x] **Verify memory calculation accuracy** (mem_total - mem_free)
  - ✅ Both fields present in kata_guest_meminfo
  - ✅ mem_available also available as direct metric
  - **DATA**: mem_total, mem_free, mem_available, active, inactive all present
  - **ACTION**: Can calculate multiple memory metrics with high accuracy

- [x] **Validate CPU time breakdown (should have user, system, etc.)**
  - ✅ Excellent data available: user, system, idle, iowait, irq, softirq, steal, guest, guest_nice
  - ✅ CPU time per CPU available (cpu label: 0, 1, 2...)
  - ✅ Data is in jiffies (not seconds) - need conversion (divide by 100 on Linux)
  - **FINDING**: Can map to container_cpu_usage_seconds_total = sum(user+system+guest+nice) / 100
  - **BONUS**: Can also map iowait separately if needed

- [x] **Test network I/O aggregation across multiple interfaces**
  - ✅ 19 sandboxes analyzed, 304 eth0 entries
  - ✅ Also docker0 (224 entries) - SHOULD BE SKIPPED (internal bridge)
  - ✅ Also lo (304 entries) - SHOULD BE SKIPPED (loopback)
  - **RULE**: Filter interfaces: eth0, veth*, tap*, tun* only
  - **ACTION**: Aggregate recv_bytes, xmit_bytes across remaining interfaces

- [x] **Confirm disk I/O sector → byte conversion (×512)**
  - ✅ sectors_read and sectors_written present
  - ✅ Conversion is valid: sectors * 512 = bytes
  - **DATA**: Multiple disk devices per sandbox (loop0, loop1, loop2...)
  - **ACTION**: Implement sector-to-byte conversion in mapping code

- [x] **NEW FINDING: Disk I/O Time Metrics ARE Available!**
  - ✅ time_reading, time_writing, time_in_progress ALL PRESENT
  - ✅ weighted_time_in_progress also available
  - ✅ Values in milliseconds (need to divide by 1000 for seconds)
  - **UPGRADE**: Can now map to container_fs_read_seconds_total, container_fs_write_seconds_total
  - **IMPACT**: Closes gap identified in section 4 of mapping doc

- [x] **Validate label transformation with real pod data**
  - ⚠️ **CRITICAL FINDING**: cri_uid, cri_name, cri_namespace are EMPTY in metrics!
  - ✅ sandbox_id is always populated and unique
  - ⚠️ Labels must come from separate CRI enrichment call
  - **ACTION**: Labels cannot be inferred from metrics alone - need CRI metadata lookup
  - **IMPACT**: Label enrichment is a separate step, not embedded in metrics

- [x] **Test with multiple sandboxes**
  - ✅ 19 unique sandboxes in test data
  - ✅ Each sandbox has consistent metric structure
  - ✅ No empty/missing metric values (all values populated)

- [x] **Verify data consistency**
  - ✅ All timestamps consistent within sandbox
  - ✅ Process count (tasks cur/max) reasonable: cur=1-2, max=78
  - ✅ Thread counts per component reasonable: hypervisor=13 threads each
  - ✅ Load averages reasonable (0.0-0.02)

- [ ] Test with missing/empty Kata metrics (cannot test - all populated in sample)
- [ ] Handle label enrichment failures (will need CRI fallback)

---

---

## 14. CRITICAL CORRECTIONS FROM ACTUAL DATA ANALYSIS

### CPU Time Units - IMPORTANT!
- **FINDING**: CPU time in kata_guest_cpu_time is in **jiffies**, NOT seconds
- **Conversion**: jiffies / 100 = seconds (on Linux, typically 100 jiffies per second)
- **FORMULA**: `container_cpu_usage_seconds_total = (user + system + guest + nice) / 100`
- **Example Data**: user=37160 jiffies → 371.6 seconds

### Disk I/O Time - MAJOR UPDATE!
- **FINDING**: container_fs_read_seconds_total and container_fs_write_seconds_total CAN be mapped!
- **Source**: `kata_guest_diskstat{item="time_reading"}` and `kata_guest_diskstat{item="time_writing"}`
- **Units**: Milliseconds (need to divide by 1000)
- **Aggregation**: Sum across all disks per sandbox
- **IMPACT**: Improves coverage from 70-75% to estimated 75-80%

### Label Enrichment - CRITICAL LIMITATION
- **CURRENT STATE**: `cri_uid`, `cri_name`, `cri_namespace` are ALL EMPTY in metrics
- **IMPLICATION**: Cannot generate cAdvisor-style labels (namespace, pod, container) from metrics alone
- **REQUIREMENT**: Must implement separate CRI metadata lookup or accept unlabeled metrics
- **WORKAROUND OPTIONS**:
  1. Keep only sandbox_id as label (minimal information)
  2. Add CRI enrichment layer to query K8s API for pod metadata
  3. Cache CRI metadata and correlate with metrics
- **RECOMMENDATION**: Implement option 2 for production use

### Memory Calculation Precision
- **BEST MAPPING**: `container_memory_usage_bytes = mem_total - mem_free` (exact match)
- **ALTERNATIVE**: `container_memory_usage_bytes = mem_available` (already calculated by kernel)
- **WORKING SET**: `container_memory_working_set_bytes = active + inactive_file`
- **SAMPLE DATA VERIFIED**: Values are consistent and reasonable (e.g., mem_total=1.024GB in test VM)

### Network Interface Filtering
- **MUST SKIP**: lo (loopback), docker0 (internal bridge), br-* (bridge devices)
- **MUST INCLUDE**: eth0, veth*, tap*, tun* (real container interfaces)
- **DATA VERIFIED**: 304 eth0 entries across 19 sandboxes confirms primary interface pattern
- **IMPLEMENTATION**: Filter by interface name regex before aggregating

### Process Count Data Granularity
- **Available**: `kata_guest_tasks{item="cur"}` (current tasks) and `{item="max"}` (max tasks)
- **Sample Values**: cur=1-2, max=78 (reasonable for VMs with 8 CPUs)
- **Mapping**: Use "cur" for `container_processes`, "max" for `container_threads_max`
- **NOTE**: These are guest OS counts, not per-container breakdown

---

## 15. DATA QUALITY OBSERVATIONS

### Strengths
- **Completeness**: All metrics fields populated (no nulls/empty values)
- **Consistency**: 19 sandboxes with identical metric structure
- **Granularity**: Per-CPU, per-disk, per-interface breakdown available
- **Process Details**: Rich proc_stat and proc_status data from multiple components
- **Historical Data**: Process CPU times track lifetime (utime, stime fields)

### Limitations
- **No Label Enrichment**: CRI metadata (pod name, namespace) not embedded
- **VM-Level Only**: Metrics are VM/sandbox-level, not per-container within pod
- **No PSI Metrics**: Process Stall Information not available
- **No OOM Tracking**: Out-of-memory events not tracked
- **No Limits**: CPU/memory quota information not available
- **Binary Format**: metrics are numeric, no text fields for troubleshooting

### Missing from cAdvisor but Present in Kata
- Host overhead metrics (shim, hypervisor, virtiofsd, agent)
- RPC latency histograms (agent, shim communication)
- Pod overhead measurements
- Go runtime metrics (GC pauses, memory stats)

---

## 16. IMPLEMENTATION PRIORITIES (REVISED)

### Phase 1: Core Mappings (READY TO IMPLEMENT)
1. **CPU Time**: Map user+system+guest+nice from kata_guest_cpu_time, convert from jiffies
2. **Memory**: Map mem_total-mem_free or use mem_available directly
3. **Network**: Aggregate eth0 recv/xmit bytes
4. **Disk I/O Operations**: Map reads/writes counts and sector→byte conversion
5. **Disk I/O Time**: Map time_reading/time_writing with ms→seconds conversion (NEW!)

### Phase 2: Support Metrics
1. Process/thread counts (cur/max from tasks)
2. Network errors and dropped packets
3. Disk errors and merged operations
4. Load averages (1, 5, 15 minute)

### Phase 3: Label Enrichment (Separate Component)
1. Implement CRI metadata lookup service
2. Cache mapping: sandbox_id → (uid, name, namespace)
3. Enhance metric labels post-collection

### Phase 4: Observe Not Map (No cAdvisor Equivalent)
1. Shim/hypervisor/agent metrics (for Kata-specific monitoring)
2. Pod overhead measurements
3. RPC latency histograms

---

## 17. REVISED COVERAGE ESTIMATE

### Before Analysis: 70-75%
### After Analysis: **75-80%**

**Reason**: Disk I/O time metrics found and confirmed
- Fixed: container_fs_read_seconds_total ✅
- Fixed: container_fs_write_seconds_total ✅
- Fixed: container_fs_io_time_weighted_seconds_total (via weighted_time_in_progress) ✅

**Still Missing (5-20% gap)**:
- CPU quotas/limits (spec fields) - Requires separate K8s API
- PSI metrics (6 metrics) - Not collected by Kata
- Memory peak (max_usage_bytes) - Not tracked
- OOM events - Not tracked
- Filesystem usage - Not collected
- Per-container breakdown - Only pod-level
- Label enrichment - Requires CRI lookup

---

## Version History

- **v1.1** - Analysis validation report
  - Ran comprehensive tests against actual kata_metrics_example.out (19 sandboxes)
  - Discovered disk I/O time metrics ARE available (major upgrade!)
  - Identified critical label enrichment gap (CRI metadata empty in metrics)
  - Validated memory, CPU, network, process count mappings
  - Revised coverage estimate: 70-75% → 75-80%
  - Documented jiffies-to-seconds conversion for CPU time
  - Created implementation priorities

- **v1.0** - Initial mapping document
  - Analyzed Kata metrics format and structure
  - Mapped core CPU, memory, network, and disk metrics
  - Identified ~15 missing metrics
  - Documented label transformation strategy
  - Estimated 70-75% metric coverage vs cAdvisor

