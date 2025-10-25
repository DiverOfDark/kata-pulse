# Kata Metrics to cAdvisor Mapping - Documentation Index

## Overview

This directory contains comprehensive documentation for mapping Kata Container metrics to cAdvisor-compatible Prometheus format. The mapping enables Kata Containers to expose metrics in the same format as cAdvisor, facilitating integration with existing container monitoring infrastructure.

**Status**: ✅ Analysis Complete (Validated against 19 real Kata sandboxes)
**Coverage**: 75% of cAdvisor metrics (48/64 metrics mappable)
**Last Updated**: 2025-10-23

---

## 📚 Documents

### 1. **VALIDATION_SUMMARY.txt** (Quick Read - 11KB)
**For**: Project managers, decision makers, quick overview
- Validation results against real data (19 sandboxes)
- Coverage estimate: 75%
- Unit conversions verified
- Implementation roadmap
- Effort estimate: 4-5 weeks
- **Start here for executive summary**

### 2. **MAPPING_QUICK_REFERENCE.md** (Developer Reference - 9KB)
**For**: Implementation engineers, code developers
- Ready-to-use mapping formulas
- Critical implementation notes
- Code pseudocode
- Testing checklist
- Coverage matrix
- Conversion rules
- **Start here to begin implementation**

### 3. **KATA_TO_CADVISOR_MAPPING.md** (Comprehensive Guide - 26KB)
**For**: Architects, detailed technical review, complete reference
- Detailed metric-by-metric mapping
- Label transformation strategy
- Missing metrics documentation
- Unit conversion explanations
- Implementation recommendations
- Testing validation results
- **Reference for questions, edge cases, detailed specs**

---

## 🎯 Quick Start by Role

### For Project Managers
1. Read: **VALIDATION_SUMMARY.txt** (2 min)
   - Key metrics: 75% coverage, 4-5 weeks effort
   - Risk: Label enrichment requires CRI integration

2. Decision: Label enrichment approach
   - Option A: Implement CRI metadata lookup (recommended)
   - Option B: Accept metrics without K8s labels
   - Option C: Do this later as Phase 3

### For Architects/Tech Leads
1. Read: **VALIDATION_SUMMARY.txt** (5 min)
2. Read: **KATA_TO_CADVISOR_MAPPING.md** sections 1-3, 14-17 (20 min)
3. Review: Implementation roadmap (4 phases)
4. Plan: CRI integration approach

### For Implementation Engineers
1. Read: **MAPPING_QUICK_REFERENCE.md** (10 min)
   - All formulas, conversions, rules
2. Skim: **KATA_TO_CADVISOR_MAPPING.md** sections 8-12 (10 min)
   - Label details, implementation recommendations
3. Start coding: Phase 1 core mappings
4. Reference: Full mapping doc for edge cases

### For QA/Test Engineers
1. Read: **MAPPING_QUICK_REFERENCE.md** "Testing Checklist" (5 min)
2. Read: **VALIDATION_SUMMARY.txt** "Testing Checklist" (5 min)
3. Read: **KATA_TO_CADVISOR_MAPPING.md** section 13 (10 min)
4. Create test cases: See detailed validation results

---

## 🔑 Key Findings Summary

### ✅ Fully Mappable (100% Coverage)
- **Network Metrics** (8/8): All network I/O operations
- **Process Metrics** (5/5): Task counts, threads, file descriptors
- **CPU Time** (5/5): User, system, guest, nice, idle (need jiffies→seconds conversion)
- **Disk I/O Operations** (12/15): Reads, writes, sectors, I/O time

### ⚠️ Partially Mappable (80-90%)
- **Memory** (10/14): Usage, RSS, cache, swap (missing: peak, limits, OOM events)
- **Disk I/O Time** (12/15): time_reading/writing available! (missing: filesystem usage, inodes)

### ❌ Not Mappable (0% Coverage)
- **PSI Metrics** (0/6): Not collected by Kata
- **CPU Quotas** (0/3): Need Kubernetes API
- **Label Enrichment** (1/8): CRI metadata empty in metrics

### 🚨 Critical Discovery
**Disk I/O Time metrics ARE available!**
- `kata_guest_diskstat{item="time_reading"}` ✅
- `kata_guest_diskstat{item="time_writing"}` ✅
- `kata_guest_diskstat{item="time_in_progress"}` ✅
- This improved coverage from 70% to 75%

---

## 📊 Unit Conversions (Verified)

| Source | Target | Formula | Notes |
|--------|--------|---------|-------|
| CPU jiffies | seconds | `/100` | Linux standard (100 jiffies/sec) |
| Disk sectors | bytes | `×512` | Standard sector size |
| Disk time (ms) | seconds | `/1000` | iostat standard |
| Memory | bytes | no-op | Already in bytes |

---

## 🔴 Critical Limitations

### 1. Label Enrichment Gap
**Problem**: `cri_uid`, `cri_name`, `cri_namespace` are EMPTY in all metrics
**Impact**: Cannot generate pod/namespace labels from metrics alone
**Solution**: Implement separate CRI metadata lookup service

### 2. Per-Container Breakdown Not Available
- Metrics are VM/sandbox-level only
- Multi-container pods show aggregate VM stats only
- No per-container CPU/memory isolation

### 3. Missing Information (20% gap)
- CPU quotas/limits (need K8s pod spec)
- PSI metrics (not collected)
- Memory peak usage (not tracked)
- OOM events (not collected)
- Filesystem usage (not available)

---

## 🚀 Implementation Roadmap

### Phase 1: Core Mappings (Week 1)
- CPU time calculation (jiffies→seconds)
- Memory usage (mem_total - mem_free)
- Network aggregation (eth0 only)
- Disk I/O (sectors→bytes, time conversions)
- Process counts (tasks, threads, FDs)

### Phase 2: Enhanced Metrics (Week 2)
- Memory working set
- Network errors/drops
- Disk merged operations
- Load averages
- Integration tests

### Phase 3: Label Enrichment (Week 3)
- CRI metadata lookup service
- Caching layer
- Pod/namespace label enrichment
- Failure handling

### Phase 4: Polish (Week 4)
- Performance optimization
- Edge case handling
- Documentation
- Production readiness

**Total Estimate**: 4-5 weeks to production-ready

---

## ✅ Validation Against Real Data

**Test Dataset**:
- 19 unique Kata sandboxes
- 23,655 metric lines analyzed
- 72+ unique metric types
- 100% data completeness

**Validation Results**:
- ✅ Memory calculation: Formula verified, values reasonable (1GB+)
- ✅ CPU time: Jiffies format confirmed, per-CPU data present
- ✅ Network: 304 eth0 entries confirmed, filtering rules validated
- ✅ Disk I/O: Sector conversion confirmed (×512), time metrics found!
- ✅ Process counts: Values reasonable (cur=1-2, max=78)
- ⚠️ Label enrichment: All CRI fields empty, confirmed requirement for external lookup

---

## 📋 Implementation Formulas

### CPU (Verified)
```
container_cpu_usage_seconds_total =
    SUM(kata_guest_cpu_time{item="user"}) +
    SUM(kata_guest_cpu_time{item="system"}) +
    SUM(kata_guest_cpu_time{item="guest"}) +
    SUM(kata_guest_cpu_time{item="nice"})
    / 100  # Jiffies to seconds
```

### Memory (Verified)
```
container_memory_usage_bytes =
    kata_guest_meminfo{item="mem_total"} -
    kata_guest_meminfo{item="mem_free"}
```

### Network (Filter First!)
```
# Include: eth0, veth*, tap*, tun*
# Exclude: lo, docker0, br-*, vxlan*, flannel*

container_network_receive_bytes_total =
    SUM(kata_guest_netdev_stat{interface~="eth0|veth.*|tap.*|tun.*", item="recv_bytes"})
```

### Disk I/O (NEW!)
```
container_fs_reads_bytes_total =
    SUM(kata_guest_diskstat{item="sectors_read"}) * 512

container_fs_read_seconds_total =
    SUM(kata_guest_diskstat{item="time_reading"}) / 1000
```

---

## 🔍 How to Use This Documentation

### Scenario 1: "I need to implement this"
1. Start with **MAPPING_QUICK_REFERENCE.md**
2. Copy the formulas
3. Implement Phase 1
4. Reference full doc for edge cases

### Scenario 2: "I need to review this"
1. Read **VALIDATION_SUMMARY.txt**
2. Skim **KATA_TO_CADVISOR_MAPPING.md** sections 1-5
3. Review section 14-17 for decisions
4. Check testing checklist (section 13)

### Scenario 3: "I found an edge case"
1. Search **KATA_TO_CADVISOR_MAPPING.md** for metric name
2. Check section 8 for label details
3. Review section 12 for implementation notes
4. See section 13 for validation rules

### Scenario 4: "I need to explain this to management"
1. Use **VALIDATION_SUMMARY.txt** for talking points
2. Key numbers: 75% coverage, 4-5 weeks, 3 phases
3. Key risk: Label enrichment (CRI integration)
4. Key opportunity: Disk I/O time metrics available

---

## 📖 Document Map

```
METRICS_MAPPING_README.md (this file)
├── VALIDATION_SUMMARY.txt (executive summary)
├── MAPPING_QUICK_REFERENCE.md (implementation guide)
└── KATA_TO_CADVISOR_MAPPING.md (comprehensive reference)
    ├── Section 1-5: Core mappings (CPU, Memory, Network, Disk, Process)
    ├── Section 6-8: Advanced mappings (misc metrics, labels)
    ├── Section 9-12: Implementation guide
    ├── Section 13-14: Validation & corrections
    ├── Section 15-17: Final recommendations & timeline
```

---

## 🎓 Reference Data

### Available Metrics in Kata (72 unique types)

**Highest frequency**:
- `kata_guest_diskstat` (8,398 entries)
- `kata_guest_vm_stat` (2,812 entries)
- `kata_shim_agent_rpc_durations_histogram_milliseconds_bucket` (2,519 entries)
- `kata_hypervisor_netdev` (1,824 entries)
- `kata_shim_rpc_durations_histogram_milliseconds_bucket` (1,265 entries)

**Categories**:
- Guest metrics: meminfo, diskstat, netdev_stat, cpu_time, load, tasks, vm_stat
- Shim metrics: proc_stat, proc_status, threads, fds, go_memstats, RPC histograms
- Hypervisor metrics: netdev, proc_stat, proc_status, threads, fds, io_stat
- Agent metrics: proc_stat, proc_status, threads, io_stat, RPC histograms
- VirtioFS metrics: proc_stat, proc_status, threads, fds, io_stat
- Monitor metrics: go_memstats, process stats, scrape stats

---

## ❓ FAQ

**Q: Why are labels empty in metrics?**
A: Kata metrics are collected at the shim level before CRI enrichment. Labels must come from a separate CRI lookup service.

**Q: Can I map CPU quotas?**
A: No, quota information is in Kubernetes pod spec, not in Kata metrics. Requires K8s API query.

**Q: Why divide CPU by 100?**
A: CPU times are in jiffies (Linux kernel time units). Default is 100 jiffies per second on most systems.

**Q: Can I use docker0 network metrics?**
A: No, docker0 is an internal bridge device. Only use eth0 and veth* (container interfaces).

**Q: Is 75% coverage enough?**
A: Yes. The 25% missing are mostly quota limits and PSI metrics which require external data sources.

---

## 📞 Support & Questions

For questions on:
- **Mapping logic**: See KATA_TO_CADVISOR_MAPPING.md
- **Implementation details**: See MAPPING_QUICK_REFERENCE.md
- **Why certain decisions**: See VALIDATION_SUMMARY.txt section "KEY IMPLEMENTATION DECISIONS"
- **What's missing**: See KATA_TO_CADVISOR_MAPPING.md section 1 & 8

---

## 📝 Version History

- **v1.1** (2025-10-23): Validation report with real data analysis
  - Analyzed 19 real Kata sandboxes
  - Discovered disk I/O time metrics (upgraded coverage to 75%)
  - Identified label enrichment gap
  - Created implementation roadmap

- **v1.0** (2025-10-23): Initial mapping analysis
  - Analyzed metrics structure
  - Mapped core CPU, memory, network, disk metrics
  - Estimated 70-75% coverage

---

## 📄 Document Sizes

| Document | Size | Time to Read | Best For |
|----------|------|--------------|----------|
| VALIDATION_SUMMARY.txt | 11KB | 5-10 min | Overview, decision making |
| MAPPING_QUICK_REFERENCE.md | 9KB | 10-15 min | Implementation, formulas |
| KATA_TO_CADVISOR_MAPPING.md | 26KB | 30-45 min | Deep dive, reference |
| Total | 46KB | 45-70 min | Complete understanding |

---

**Ready to start implementation?** → Begin with MAPPING_QUICK_REFERENCE.md
**Need an overview?** → Start with VALIDATION_SUMMARY.txt
**Want all details?** → Read KATA_TO_CADVISOR_MAPPING.md

