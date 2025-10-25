# Comprehensive Label Analysis: All Metrics and Label Coverage

## Executive Summary

This analysis examines **ALL labels** that should be present in cAdvisor metrics according to the cadvisor_analysis_summary.txt, and compares them with what the CloudHypervisorConverter is actually populating.

**CRITICAL FINDINGS:**
- ✅ StandardLabels (6 fields): Mostly correct, but `id` field has wrong source
- ✅ Special labels for networks: `interface` - BEING POPULATED
- ✅ Special labels for CPU: `cpu="total"` - BEING POPULATED
- ❌ Special labels for disk: `major`, `minor` - **NOT BEING POPULATED** (empty strings)
- ❌ Special labels for memory: `failure_type`, `scope` - **NOT BEING POPULATED** (no failures captured)
- ❌ Special labels for processes: `state` - **NOT BEING POPULATED** (no task states captured)

---

## Detailed Label Analysis by Metric Type

### 1. CPU METRICS

#### Required Labels (from cAdvisor)
According to cadvisor_analysis_summary.txt (lines 40-67):
- Standard Labels: `container`, `id`, `image`, `name`, `namespace`, `pod` ✅
- Special Labels: `cpu="total"` (ONLY on `container_cpu_usage_seconds_total`)

#### Current Implementation Status

**StandardLabels:**
| Label | Source | Current Implementation | Status |
|-------|--------|---|---|
| `container` | N/A | Empty string (pod-level) | ✅ CORRECT |
| `id` | `sandbox_id` | Using `pod_uid` | ❌ WRONG |
| `image` | N/A | Empty string (not available) | ✅ CORRECT |
| `name` | `pod_name` | From CRI metadata | ✅ CORRECT |
| `namespace` | `pod_namespace` | From CRI metadata | ✅ CORRECT |
| `pod` | `pod_name` | From CRI metadata | ✅ CORRECT |

**Special Labels:**
| Label | Purpose | Current Implementation | Status |
|-------|---------|---|---|
| `cpu` | CPU identifier | Hardcoded `"total"` | ✅ POPULATED |

**Code Location:** CloudHypervisorConverter.convert_cpu(), CpuMetrics.to_prometheus_format()

---

### 2. MEMORY METRICS

#### Required Labels (from cAdvisor)
According to cadvisor_analysis_summary.txt (lines 70-97):
- Standard Labels: All 6 ✅
- Special Labels for memory failures:
  - `failure_type`: "pgfault" or "pgmajfault" (ONLY on `container_memory_failures_total`)
  - `scope`: "container" or "hierarchy" (ONLY on `container_memory_failures_total`)

#### Current Implementation Status

**StandardLabels:** Same as CPU (with same `id` issue)

**Special Labels:**
| Label | Purpose | Source in Kata Metrics | Current Implementation | Status |
|-------|---------|---|---|---|
| `failure_type` | Type of memory failure | `kata_guest_memory_failures{failure_type="pgfault"\|"pgmajfault"}` | **EMPTY - Never populated** | ❌ MISSING |
| `scope` | Failure scope | `kata_guest_memory_failures{scope="container"\|"hierarchy"}` | **EMPTY - Never populated** | ❌ MISSING |

**Problem Analysis:**
- The `MemoryMetrics` struct has `failures: HashMap<String, u64>` field (line 175 in cadvisor.rs)
- The PrometheusFormat implementation EMITS the failure labels correctly (lines 411-429)
- **BUT**: The CloudHypervisorConverter.convert_memory() method NEVER populates this HashMap!
- Search result: No "failure" or "pgfault" handling in convert_memory() method

**Code Location:** CloudHypervisorConverter.convert_memory() - **MISSING IMPLEMENTATION**

---

### 3. NETWORK METRICS

#### Required Labels (from cAdvisor)
According to cadvisor_analysis_summary.txt (lines 99-124):
- Standard Labels: All 6 ✅
- Special Labels:
  - `interface`: Network interface name (eth0, cilium_vxlan, lxc*, etc.) - **ALWAYS PRESENT on network metrics**

#### Current Implementation Status

**StandardLabels:** Same as CPU (with same `id` issue)

**Special Labels:**
| Label | Purpose | Source in Kata Metrics | Current Implementation | Status |
|-------|---------|---|---|---|
| `interface` | Network interface | `kata_guest_netdev_stat{interface="eth0"}` | **POPULATED via `InterfaceMetrics.name`** | ✅ CORRECT |

**Implementation Details:**
- CloudHypervisorConverter.convert_network() extracts interface from metrics (line 216)
- Creates InterfaceMetrics entries per interface (line 230)
- Sets `iface_metrics.name = interface` (line 230)
- PrometheusFormat emits per-interface metrics with interface label (lines 495, 507, 519, 531)

**Code Location:** CloudHypervisorConverter.convert_network(), NetworkMetrics.to_prometheus_format()

---

### 4. BLOCK I/O (DISK) METRICS

#### Required Labels (from cAdvisor)
According to cadvisor_analysis_summary.txt (lines 127-151):
- Standard Labels: All 6 ✅
- Special Labels (ALL REQUIRED on block I/O metrics):
  - `device`: Device path or empty (e.g., /dev/sda, /dev/sdb, or "")
  - `major`: Device major number (string: "7", "8", etc.)
  - `minor`: Device minor number (string: "0", "1", "2", etc.)
  - `operation`: "Read" or "Write"

**Example from cadvisor.out:**
```
container_blkio_device_usage_total{
  container="",
  device="",
  id="/kubepods/besteffort/pod37d6bc26...",
  image="",
  major="7",
  minor="0",
  name="",
  namespace="prometheus",
  operation="Read",
  pod="kube-prometheus-stack-prometheus-node-exporter-zr4xv"
} 179200
```

#### Current Implementation Status

**StandardLabels:** Same as CPU (with same `id` issue)

**Special Labels:**
| Label | Purpose | Source in Kata Metrics | Current Implementation | Status |
|-------|---------|---|---|---|
| `device` | Device path | `kata_guest_diskstat{disk="sda"}` | **POPULATED via `DeviceMetrics.device`** | ✅ CORRECT |
| `major` | Device major number | `kata_guest_diskstat_major` (?) | **EMPTY - Never populated** | ❌ MISSING |
| `minor` | Device minor number | `kata_guest_diskstat_minor` (?) | **EMPTY - Never populated** | ❌ MISSING |
| `operation` | Read or Write | Hardcoded in PrometheusFormat | **POPULATED as "Read"/"Write"** | ✅ CORRECT |

**Problem Analysis:**
- DeviceMetrics struct has `major` and `minor` fields (lines 270-272)
- PrometheusFormat implementation EMITS them correctly (lines 603-607, 617-620)
- **BUT**: CloudHypervisorConverter never sets these values!
- The converter only sets `device_metrics.device = disk;` (line 321 in cloud_hypervisor.rs)
- Search result for "major\|minor": No matches in cloud_hypervisor.rs

**Code Location:** CloudHypervisorConverter.convert_disk() - **MISSING major/minor extraction**

**How to Fix:**
Would need to either:
1. Extract from kata_guest_diskstat metrics (if available as separate metric with major/minor labels)
2. Use Linux device utilities to map disk name to major/minor numbers
3. Hardcode common devices (e.g., sda=8:0, sdb=8:1, loop0=7:0)

---

### 5. PROCESS METRICS

#### Required Labels (from cAdvisor)
According to cadvisor_analysis_summary.txt (lines 192-216):
- Standard Labels: All 6 ✅
- Special Labels:
  - `state`: Task state (ONLY on `container_tasks_state`)
    - Values: "running", "sleeping", "stopped", "uninterruptible", "iowaiting"
  - `ulimit`: Ulimit name (ONLY on `container_ulimits_soft`)
    - Values: "max_open_files"

#### Current Implementation Status

**StandardLabels:** Same as CPU (with same `id` issue)

**Special Labels:**

| Label | Purpose | Source in Kata Metrics | Current Implementation | Status |
|-------|---------|---|---|---|
| `state` | Task state | `kata_guest_tasks_state{state="running\|sleeping\|..."}` (?) | **EMPTY - Never populated** | ❌ MISSING |
| `ulimit` | Ulimit setting | Not currently implemented | Not implemented | ⚠️ OUT OF SCOPE |

**Problem Analysis:**
- ProcessMetrics struct has `tasks_by_state: HashMap<String, u64>` field (line 297)
- PrometheusFormat implementation EMITS state labels correctly (lines 676-685)
- **BUT**: CloudHypervisorConverter never populates this HashMap!
- The converter only extracts `cur` and `max` from `kata_guest_tasks` (lines 407-415)
- **No implementation for per-state task counts**

**Code Location:** CloudHypervisorConverter.convert_process() - **MISSING state extraction**

**How to Fix:**
Would need to:
1. Check if Cloud Hypervisor metrics provide per-state task counts
2. Or extract from `/proc/<pid>/stat` files within the guest VM
3. Map procfs state codes to cAdvisor state names

---

## Summary Table: All Labels Coverage

### StandardLabels (Present on ALL metrics)

| Label | Expected Source | Current Implementation | Status |
|-------|---|---|---|
| `container` | Empty (pod-level) | Empty | ✅ |
| `id` | `sandbox_id` | `pod_uid` | ❌ WRONG |
| `image` | Empty (not available) | Empty | ✅ |
| `name` | `pod_name` | `pod_name` | ✅ |
| `namespace` | `pod_namespace` | `pod_namespace` | ✅ |
| `pod` | `pod_name` | `pod_name` | ✅ |

**Score: 5/6 (83%)**

---

### Special Labels by Metric Type

| Metric Type | Label | Expected | Current | Status |
|---|---|---|---|---|
| **CPU** | `cpu` | "total" | "total" | ✅ POPULATED |
| **Memory** | `failure_type` | "pgfault", "pgmajfault" | Empty | ❌ NOT POPULATED |
| **Memory** | `scope` | "container", "hierarchy" | Empty | ❌ NOT POPULATED |
| **Network** | `interface` | Interface names | From metrics | ✅ POPULATED |
| **Disk** | `device` | Device path | From metrics | ✅ POPULATED |
| **Disk** | `major` | Major number | Empty | ❌ NOT POPULATED |
| **Disk** | `minor` | Minor number | Empty | ❌ NOT POPULATED |
| **Disk** | `operation` | "Read", "Write" | Hardcoded | ✅ POPULATED |
| **Process** | `state` | Task states | Empty | ❌ NOT POPULATED |

**Score: 6/9 (67%)**

---

## Priority Fixes Required

### CRITICAL (Blocks metric correlation)
1. **Change `id` label source**: Use `sandbox_id` instead of `pod_uid`
   - Impact: ALL metrics affected
   - Files: CloudHypervisorConverter.create_standard_labels()

### HIGH (Missing important metrics)
2. **Populate disk device `major`/`minor` labels**:
   - Impact: Block I/O metrics won't match cAdvisor
   - Files: CloudHypervisorConverter.convert_disk()
   - Effort: Medium (need device number mapping)

3. **Populate memory failure labels**:
   - Impact: Memory failure metrics won't emit
   - Files: CloudHypervisorConverter.convert_memory()
   - Effort: Medium (need to extract from metrics or other sources)

4. **Populate process task state labels**:
   - Impact: Task state metrics won't emit
   - Files: CloudHypervisorConverter.convert_process()
   - Effort: Medium-High (need to extract per-state counts)

### MEDIUM (Nice-to-have)
5. **Implement ulimit labels**: Not currently implemented
   - Impact: Limited (ulimit metrics rarely used)
   - Files: ProcessMetrics, CloudHypervisorConverter
   - Effort: Low-Medium

---

## Data Availability Assessment

Based on KATA_TO_CADVISOR_MAPPING.md and cadvisor.out analysis:

### Available in Cloud Hypervisor Metrics ✅
- `kata_guest_netdev_stat{interface="eth0", item="recv_bytes"}` → network interface data
- `kata_guest_diskstat{disk="sda", item="reads"}` → disk data
- `kata_guest_meminfo{item="..."}` → memory data
- `kata_guest_cpu_time{item="user|system|..."}` → CPU data

### NOT Currently Available ❌
- Memory failure counts per type/scope (pgfault, pgmajfault, container, hierarchy)
- Task state breakdown (running, sleeping, stopped, etc.)
- Device major/minor numbers (could be derived from disk names)

### Can Be Derived/Extracted ⚠️
- Device major/minor: Can hardcode common mappings or extract from sysfs
- Task states: May be available from guest /proc/stat or separate metric
- Memory failures: May be in separate kata_guest_memory_failures metric

---

## Recommendations

1. **Immediate**: Fix the `id` label to use `sandbox_id` instead of `pod_uid`
2. **Short-term**: Add device major/minor extraction (hardcode common values or derive from device name)
3. **Medium-term**: Investigate availability of memory failure and task state metrics in Cloud Hypervisor
4. **Long-term**: Complete implementation once data source is confirmed

---

## Testing Impact

Tests currently validate:
- StandardLabels population (with pod_uid in id - WRONG)
- Network interface labels (CORRECT)
- Disk device labels (CORRECT, but missing major/minor)
- Memory/Process labels (EMPTY, no tests for special labels)

After fixes:
- Tests should validate `id == sandbox_id`
- Tests should validate `major` and `minor` values
- Tests should validate memory failure labels when populated
- Tests should validate task state labels when populated
