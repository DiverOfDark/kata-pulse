//! cAdvisor-compatible metrics output models
//!
//! This module defines the output format for converted metrics,
//! matching cAdvisor's metric structure and naming conventions.

use std::collections::HashMap;

/// Trait for converting metrics to Prometheus text format
///
/// Implementations handle the serialization of metrics into valid Prometheus exposition format,
/// including HELP and TYPE annotations.
pub trait PrometheusFormat {
    /// Convert this metric to Prometheus text format
    /// Optional sandbox_id parameter can be used to add labels
    fn to_prometheus_format(&self, _sandbox_id: Option<&str>) -> String;
}

/// Helper function to escape label values for Prometheus format
fn escape_label_value(value: &str) -> String {
    let mut result = String::new();
    for ch in value.chars() {
        match ch {
            '\\' => result.push_str("\\\\"),
            '"' => result.push_str("\\\""),
            '\n' => result.push_str("\\n"),
            _ => result.push(ch),
        }
    }
    result
}

/// Standard cAdvisor labels present on all container metrics
#[derive(Debug, Clone, Default)]
pub struct StandardLabels {
    /// Container ID (empty for pod-level aggregates)
    pub container: String,
    /// Cgroup path (e.g., /kubepods/burstable/pod<uuid>)
    pub id: String,
    /// Container image URI (empty if not available)
    pub image: String,
    /// Container/pod name (usually pod name, empty for pod-level)
    pub name: String,
    /// Kubernetes namespace
    pub namespace: String,
    /// Kubernetes pod name
    pub pod: String,
}

impl StandardLabels {
    /// Create StandardLabels from CRI metadata components
    ///
    /// # Arguments
    /// * `pod_uid` - Kubernetes pod UID (from CRI metadata)
    /// * `pod_name` - Kubernetes pod name (from CRI metadata)
    /// * `pod_namespace` - Kubernetes namespace (from CRI metadata)
    pub fn new(
        pod_uid: impl Into<String>,
        pod_name: impl Into<String>,
        pod_namespace: impl Into<String>,
    ) -> Self {
        let pod_name_str = pod_name.into();
        let pod_namespace_str = pod_namespace.into();
        let pod_uid_str = pod_uid.into();

        StandardLabels {
            container: "kata".to_string(), // Empty for sandbox-level metrics
            id: pod_uid_str,
            image: "unknown".to_string(), // Not available from Cloud Hypervisor metrics
            name: pod_name_str.clone(), // Use pod name as container name
            namespace: pod_namespace_str,
            pod: pod_name_str,
        }
    }

    /// Convert to label string for Prometheus format
    fn to_label_string(&self) -> String {
        let labels = [
            format!(r#"container="{}""#, escape_label_value(&self.container)),
            format!(r#"id="{}""#, escape_label_value(&self.id)),
            format!(r#"image="{}""#, escape_label_value(&self.image)),
            format!(r#"name="{}""#, escape_label_value(&self.name)),
            format!(r#"namespace="{}""#, escape_label_value(&self.namespace)),
            format!(r#"pod="{}""#, escape_label_value(&self.pod)),
        ];
        format!("{{{}}}", labels.join(","))
    }

    /// Convert to label string with additional labels
    fn to_label_string_with_extras(&self, extras: &[(&str, &str)]) -> String {
        let mut labels = vec![
            format!(r#"container="{}""#, escape_label_value(&self.container)),
            format!(r#"id="{}""#, escape_label_value(&self.id)),
            format!(r#"image="{}""#, escape_label_value(&self.image)),
            format!(r#"name="{}""#, escape_label_value(&self.name)),
            format!(r#"namespace="{}""#, escape_label_value(&self.namespace)),
            format!(r#"pod="{}""#, escape_label_value(&self.pod)),
        ];

        for (key, value) in extras {
            labels.push(format!(r#"{}="{}""#, key, escape_label_value(value)));
        }

        format!("{{{}}}", labels.join(","))
    }
}

/// Complete set of converted cAdvisor metrics
#[derive(Debug, Clone)]
pub struct CadvisorMetrics {
    pub cpu: CpuMetrics,
    pub memory: MemoryMetrics,
    pub network: NetworkMetrics,
    pub disk: DiskMetrics,
    pub process: ProcessMetrics,
}

/// CPU metrics in cAdvisor format
#[derive(Debug, Clone, Default)]
pub struct CpuMetrics {
    /// Total CPU usage in seconds (all CPUs combined)
    pub usage_seconds_total: f64,

    /// User mode CPU time in seconds
    pub user_seconds_total: f64,

    /// System mode CPU time in seconds
    pub system_seconds_total: f64,

    /// Load average (1-minute, 5-minute, 15-minute)
    pub load_average: Option<LoadAverage>,

    /// Per-CPU breakdown (optional, for detailed monitoring)
    #[allow(dead_code)]
    pub per_cpu: HashMap<String, f64>,

    /// Standard cAdvisor labels (container, id, image, name, namespace, pod)
    pub standard_labels: StandardLabels,
}

/// Load average breakdown
#[derive(Debug, Clone)]
pub struct LoadAverage {
    pub one_minute: f64,
    pub five_minute: f64,
    pub fifteen_minute: f64,
}

/// Memory metrics in cAdvisor format
#[derive(Debug, Clone, Default)]
pub struct MemoryMetrics {
    /// Total memory in use (in bytes)
    pub usage_bytes: u64,

    /// Working set size (in bytes)
    pub working_set_bytes: Option<u64>,

    /// Memory cache (in bytes)
    pub cache_bytes: Option<u64>,

    /// Resident set size (in bytes)
    pub rss_bytes: Option<u64>,

    /// Swap usage (in bytes)
    pub swap_bytes: Option<u64>,

    /// Memory-mapped file size (in bytes)
    pub mapped_file_bytes: Option<u64>,

    /// Memory failure counts - mapped by "failure_type:scope" key (e.g., "pgfault:container")
    /// failure_type: "pgfault" or "pgmajfault"
    /// scope: "container" or "hierarchy"
    pub failures: HashMap<String, u64>,

    /// Standard cAdvisor labels (container, id, image, name, namespace, pod)
    pub standard_labels: StandardLabels,
}

/// Network metrics in cAdvisor format
#[derive(Debug, Clone, Default)]
pub struct NetworkMetrics {
    /// Total bytes received
    pub receive_bytes_total: u64,

    /// Total bytes transmitted
    pub transmit_bytes_total: u64,

    /// Total packets received
    pub receive_packets_total: u64,

    /// Total packets transmitted
    pub transmit_packets_total: u64,

    /// Total receive errors
    pub receive_errors_total: Option<u64>,

    /// Total transmit errors
    pub transmit_errors_total: Option<u64>,

    /// Total dropped receive packets
    pub receive_packets_dropped_total: Option<u64>,

    /// Total dropped transmit packets
    pub transmit_packets_dropped_total: Option<u64>,

    /// Per-interface breakdown
    pub per_interface: HashMap<String, InterfaceMetrics>,

    /// Standard cAdvisor labels (container, id, image, name, namespace, pod)
    pub standard_labels: StandardLabels,
}

/// Per-interface network metrics
#[derive(Debug, Clone, Default)]
pub struct InterfaceMetrics {
    /// Interface name (eth0, cilium_vxlan, etc.) - used for the interface label
    pub name: String,
    pub receive_bytes: u64,
    pub transmit_bytes: u64,
    pub receive_packets: u64,
    pub transmit_packets: u64,
    pub receive_errors: Option<u64>,
    pub transmit_errors: Option<u64>,
    pub receive_dropped: Option<u64>,
    pub transmit_dropped: Option<u64>,
}

/// Disk I/O metrics in cAdvisor format
#[derive(Debug, Clone, Default)]
pub struct DiskMetrics {
    /// Total disk read operations
    pub reads_total: u64,

    /// Total disk write operations
    pub writes_total: u64,

    /// Total bytes read
    pub reads_bytes_total: u64,

    /// Total bytes written
    pub writes_bytes_total: u64,

    /// Total time spent reading (in seconds)
    pub read_seconds_total: f64,

    /// Total time spent writing (in seconds)
    pub write_seconds_total: f64,

    /// Total I/O time (in seconds)
    pub io_time_seconds_total: Option<f64>,

    /// Weighted I/O time (in seconds)
    pub io_time_weighted_seconds_total: Option<f64>,

    /// Per-device breakdown
    pub per_device: HashMap<String, DeviceMetrics>,

    /// Standard cAdvisor labels (container, id, image, name, namespace, pod)
    pub standard_labels: StandardLabels,
}

/// Per-device disk metrics for block I/O
#[derive(Debug, Clone, Default)]
pub struct DeviceMetrics {
    /// Device name/path for the device label (e.g., /dev/sda, /dev/sdb, or empty "")
    pub device: String,
    /// Device major number (for block I/O metrics)
    pub major: String,
    /// Device minor number (for block I/O metrics)
    pub minor: String,
    pub reads: u64,
    pub writes: u64,
    pub reads_bytes: u64,
    pub writes_bytes: u64,
    pub read_time_seconds: f64,
    pub write_time_seconds: f64,
}

/// Process metrics in cAdvisor format
#[derive(Debug, Clone, Default)]
pub struct ProcessMetrics {
    /// Number of running processes
    pub count: u64,

    /// Total thread count
    pub thread_count: u64,

    /// Maximum thread count allowed
    pub thread_count_max: Option<u64>,

    /// Total open file descriptors
    pub file_descriptors: u64,

    /// Task counts by state - mapped by state (e.g., "running", "sleeping", "stopped", "uninterruptible", "iowaiting")
    pub tasks_by_state: HashMap<String, u64>,

    /// Standard cAdvisor labels (container, id, image, name, namespace, pod)
    pub standard_labels: StandardLabels,
}

// PrometheusFormat trait implementations for each metric type

impl PrometheusFormat for CpuMetrics {
    fn to_prometheus_format(&self, _sandbox_id: Option<&str>) -> String {
        let mut output = String::new();
        let labels_with_cpu = self
            .standard_labels
            .to_label_string_with_extras(&[("cpu", "total")]);

        output
            .push_str("# HELP container_cpu_usage_seconds_total Total CPU time used in seconds\n");
        output.push_str("# TYPE container_cpu_usage_seconds_total counter\n");
        output.push_str(&format!(
            "container_cpu_usage_seconds_total{} {}\n",
            labels_with_cpu, self.usage_seconds_total
        ));

        if self.user_seconds_total > 0.0 {
            output
                .push_str("# HELP container_cpu_user_seconds_total CPU time spent in user mode\n");
            output.push_str("# TYPE container_cpu_user_seconds_total counter\n");
            output.push_str(&format!(
                "container_cpu_user_seconds_total{} {}\n",
                labels_with_cpu, self.user_seconds_total
            ));
        }

        if self.system_seconds_total > 0.0 {
            output.push_str(
                "# HELP container_cpu_system_seconds_total CPU time spent in system mode\n",
            );
            output.push_str("# TYPE container_cpu_system_seconds_total counter\n");
            output.push_str(&format!(
                "container_cpu_system_seconds_total{} {}\n",
                labels_with_cpu, self.system_seconds_total
            ));
        }

        if let Some(load) = &self.load_average {
            output.push_str("# HELP container_load_average_1m 1-minute load average\n");
            output.push_str("# TYPE container_load_average_1m gauge\n");
            output.push_str(&format!(
                "container_load_average_1m{} {}\n",
                labels_with_cpu, load.one_minute
            ));

            output.push_str("# HELP container_load_average_5m 5-minute load average\n");
            output.push_str("# TYPE container_load_average_5m gauge\n");
            output.push_str(&format!(
                "container_load_average_5m{} {}\n",
                labels_with_cpu, load.five_minute
            ));

            output.push_str("# HELP container_load_average_15m 15-minute load average\n");
            output.push_str("# TYPE container_load_average_15m gauge\n");
            output.push_str(&format!(
                "container_load_average_15m{} {}\n",
                labels_with_cpu, load.fifteen_minute
            ));
        }

        output
    }
}

impl PrometheusFormat for MemoryMetrics {
    fn to_prometheus_format(&self, _sandbox_id: Option<&str>) -> String {
        let mut output = String::new();
        let labels_suffix = self.standard_labels.to_label_string();

        output.push_str("# HELP container_memory_usage_bytes Memory usage in bytes\n");
        output.push_str("# TYPE container_memory_usage_bytes gauge\n");
        output.push_str(&format!(
            "container_memory_usage_bytes{} {}\n",
            labels_suffix, self.usage_bytes
        ));

        if let Some(working_set) = self.working_set_bytes {
            output
                .push_str("# HELP container_memory_working_set_bytes Working set size in bytes\n");
            output.push_str("# TYPE container_memory_working_set_bytes gauge\n");
            output.push_str(&format!(
                "container_memory_working_set_bytes{} {}\n",
                labels_suffix, working_set
            ));
        }

        if let Some(cache) = self.cache_bytes {
            output.push_str("# HELP container_memory_cache_bytes Memory cache in bytes\n");
            output.push_str("# TYPE container_memory_cache_bytes gauge\n");
            output.push_str(&format!(
                "container_memory_cache_bytes{} {}\n",
                labels_suffix, cache
            ));
        }

        if let Some(rss) = self.rss_bytes {
            output.push_str("# HELP container_memory_rss_bytes Resident set size in bytes\n");
            output.push_str("# TYPE container_memory_rss_bytes gauge\n");
            output.push_str(&format!(
                "container_memory_rss_bytes{} {}\n",
                labels_suffix, rss
            ));
        }

        if let Some(swap) = self.swap_bytes {
            output.push_str("# HELP container_memory_swap_bytes Swap usage in bytes\n");
            output.push_str("# TYPE container_memory_swap_bytes gauge\n");
            output.push_str(&format!(
                "container_memory_swap_bytes{} {}\n",
                labels_suffix, swap
            ));
        }

        // Emit memory failure metrics if available
        if !self.failures.is_empty() {
            output.push_str("# HELP container_memory_failures_total Memory failure count\n");
            output.push_str("# TYPE container_memory_failures_total counter\n");
            for (key, count) in &self.failures {
                // Key format is "failure_type:scope" e.g., "pgfault:container"
                let parts: Vec<&str> = key.split(':').collect();
                if parts.len() == 2 {
                    let failure_type = parts[0];
                    let scope = parts[1];
                    let failure_labels = self.standard_labels.to_label_string_with_extras(&[
                        ("failure_type", failure_type),
                        ("scope", scope),
                    ]);
                    output.push_str(&format!(
                        "container_memory_failures_total{} {}\n",
                        failure_labels, count
                    ));
                }
            }
        }

        output
    }
}

impl PrometheusFormat for NetworkMetrics {
    fn to_prometheus_format(&self, _sandbox_id: Option<&str>) -> String {
        let mut output = String::new();
        let labels_suffix = self.standard_labels.to_label_string();

        if self.receive_bytes_total > 0 || self.transmit_bytes_total > 0 {
            output.push_str("# HELP container_network_receive_bytes_total Total bytes received\n");
            output.push_str("# TYPE container_network_receive_bytes_total counter\n");
            output.push_str(&format!(
                "container_network_receive_bytes_total{} {}\n",
                labels_suffix, self.receive_bytes_total
            ));

            output.push_str(
                "# HELP container_network_transmit_bytes_total Total bytes transmitted\n",
            );
            output.push_str("# TYPE container_network_transmit_bytes_total counter\n");
            output.push_str(&format!(
                "container_network_transmit_bytes_total{} {}\n",
                labels_suffix, self.transmit_bytes_total
            ));

            output.push_str(
                "# HELP container_network_receive_packets_total Total packets received\n",
            );
            output.push_str("# TYPE container_network_receive_packets_total counter\n");
            output.push_str(&format!(
                "container_network_receive_packets_total{} {}\n",
                labels_suffix, self.receive_packets_total
            ));

            output.push_str(
                "# HELP container_network_transmit_packets_total Total packets transmitted\n",
            );
            output.push_str("# TYPE container_network_transmit_packets_total counter\n");
            output.push_str(&format!(
                "container_network_transmit_packets_total{} {}\n",
                labels_suffix, self.transmit_packets_total
            ));
        }

        if let Some(errors) = self.receive_errors_total {
            output.push_str("# HELP container_network_receive_errors_total Receive errors\n");
            output.push_str("# TYPE container_network_receive_errors_total counter\n");
            output.push_str(&format!(
                "container_network_receive_errors_total{} {}\n",
                labels_suffix, errors
            ));
        }

        if let Some(errors) = self.transmit_errors_total {
            output.push_str("# HELP container_network_transmit_errors_total Transmit errors\n");
            output.push_str("# TYPE container_network_transmit_errors_total counter\n");
            output.push_str(&format!(
                "container_network_transmit_errors_total{} {}\n",
                labels_suffix, errors
            ));
        }

        // Emit per-interface metrics if available
        if !self.per_interface.is_empty() {
            output.push_str(
                "# HELP container_network_receive_bytes_total Total bytes received per interface\n",
            );
            output.push_str("# TYPE container_network_receive_bytes_total counter\n");
            for iface in self.per_interface.values() {
                if iface.receive_bytes > 0 {
                    let iface_labels = self
                        .standard_labels
                        .to_label_string_with_extras(&[("interface", &iface.name)]);
                    output.push_str(&format!(
                        "container_network_receive_bytes_total{} {}\n",
                        iface_labels, iface.receive_bytes
                    ));
                }
            }

            output.push_str("# HELP container_network_transmit_bytes_total Total bytes transmitted per interface\n");
            output.push_str("# TYPE container_network_transmit_bytes_total counter\n");
            for iface in self.per_interface.values() {
                if iface.transmit_bytes > 0 {
                    let iface_labels = self
                        .standard_labels
                        .to_label_string_with_extras(&[("interface", &iface.name)]);
                    output.push_str(&format!(
                        "container_network_transmit_bytes_total{} {}\n",
                        iface_labels, iface.transmit_bytes
                    ));
                }
            }

            output.push_str("# HELP container_network_receive_packets_total Total packets received per interface\n");
            output.push_str("# TYPE container_network_receive_packets_total counter\n");
            for iface in self.per_interface.values() {
                if iface.receive_packets > 0 {
                    let iface_labels = self
                        .standard_labels
                        .to_label_string_with_extras(&[("interface", &iface.name)]);
                    output.push_str(&format!(
                        "container_network_receive_packets_total{} {}\n",
                        iface_labels, iface.receive_packets
                    ));
                }
            }

            output.push_str("# HELP container_network_transmit_packets_total Total packets transmitted per interface\n");
            output.push_str("# TYPE container_network_transmit_packets_total counter\n");
            for iface in self.per_interface.values() {
                if iface.transmit_packets > 0 {
                    let iface_labels = self
                        .standard_labels
                        .to_label_string_with_extras(&[("interface", &iface.name)]);
                    output.push_str(&format!(
                        "container_network_transmit_packets_total{} {}\n",
                        iface_labels, iface.transmit_packets
                    ));
                }
            }
        }

        output
    }
}

impl PrometheusFormat for DiskMetrics {
    fn to_prometheus_format(&self, _sandbox_id: Option<&str>) -> String {
        let mut output = String::new();
        let labels_suffix = self.standard_labels.to_label_string();

        if self.reads_total > 0 || self.writes_total > 0 {
            output.push_str("# HELP container_disk_io_reads_total Total disk read operations\n");
            output.push_str("# TYPE container_disk_io_reads_total counter\n");
            output.push_str(&format!(
                "container_disk_io_reads_total{} {}\n",
                labels_suffix, self.reads_total
            ));

            output.push_str("# HELP container_disk_io_writes_total Total disk write operations\n");
            output.push_str("# TYPE container_disk_io_writes_total counter\n");
            output.push_str(&format!(
                "container_disk_io_writes_total{} {}\n",
                labels_suffix, self.writes_total
            ));

            output
                .push_str("# HELP container_disk_io_read_bytes_total Total bytes read from disk\n");
            output.push_str("# TYPE container_disk_io_read_bytes_total counter\n");
            output.push_str(&format!(
                "container_disk_io_read_bytes_total{} {}\n",
                labels_suffix, self.reads_bytes_total
            ));

            output.push_str(
                "# HELP container_disk_io_write_bytes_total Total bytes written to disk\n",
            );
            output.push_str("# TYPE container_disk_io_write_bytes_total counter\n");
            output.push_str(&format!(
                "container_disk_io_write_bytes_total{} {}\n",
                labels_suffix, self.writes_bytes_total
            ));

            if self.read_seconds_total > 0.0 {
                output.push_str(
                    "# HELP container_disk_io_read_seconds_total Total time spent reading\n",
                );
                output.push_str("# TYPE container_disk_io_read_seconds_total counter\n");
                output.push_str(&format!(
                    "container_disk_io_read_seconds_total{} {}\n",
                    labels_suffix, self.read_seconds_total
                ));
            }

            if self.write_seconds_total > 0.0 {
                output.push_str(
                    "# HELP container_disk_io_write_seconds_total Total time spent writing\n",
                );
                output.push_str("# TYPE container_disk_io_write_seconds_total counter\n");
                output.push_str(&format!(
                    "container_disk_io_write_seconds_total{} {}\n",
                    labels_suffix, self.write_seconds_total
                ));
            }
        }

        // Emit per-device block I/O metrics if available
        if !self.per_device.is_empty() {
            // cAdvisor emits block I/O as container_blkio_device_usage_total with device, major, minor, operation labels
            for device in self.per_device.values() {
                // Read operations
                if device.reads > 0 {
                    let dev_labels = self.standard_labels.to_label_string_with_extras(&[
                        ("device", &device.device),
                        ("major", &device.major),
                        ("minor", &device.minor),
                        ("operation", "Read"),
                    ]);
                    output.push_str(&format!(
                        "container_blkio_device_usage_total{} {}\n",
                        dev_labels, device.reads
                    ));
                }
                // Write operations
                if device.writes > 0 {
                    let dev_labels = self.standard_labels.to_label_string_with_extras(&[
                        ("device", &device.device),
                        ("major", &device.major),
                        ("minor", &device.minor),
                        ("operation", "Write"),
                    ]);
                    output.push_str(&format!(
                        "container_blkio_device_usage_total{} {}\n",
                        dev_labels, device.writes
                    ));
                }
            }
        }

        output
    }
}

impl PrometheusFormat for ProcessMetrics {
    fn to_prometheus_format(&self, _sandbox_id: Option<&str>) -> String {
        let mut output = String::new();
        let labels_suffix = self.standard_labels.to_label_string();

        if self.count > 0 {
            output.push_str("# HELP container_processes_count Number of running processes\n");
            output.push_str("# TYPE container_processes_count gauge\n");
            output.push_str(&format!(
                "container_processes_count{} {}\n",
                labels_suffix, self.count
            ));
        }

        if self.thread_count > 0 {
            output.push_str("# HELP container_threads_count Number of threads\n");
            output.push_str("# TYPE container_threads_count gauge\n");
            output.push_str(&format!(
                "container_threads_count{} {}\n",
                labels_suffix, self.thread_count
            ));
        }

        if let Some(max) = self.thread_count_max {
            output
                .push_str("# HELP container_threads_max_count Maximum number of threads allowed\n");
            output.push_str("# TYPE container_threads_max_count gauge\n");
            output.push_str(&format!(
                "container_threads_max_count{} {}\n",
                labels_suffix, max
            ));
        }

        if self.file_descriptors > 0 {
            output.push_str("# HELP container_file_descriptors Number of open file descriptors\n");
            output.push_str("# TYPE container_file_descriptors gauge\n");
            output.push_str(&format!(
                "container_file_descriptors{} {}\n",
                labels_suffix, self.file_descriptors
            ));
        }

        // Emit task state metrics if available
        if !self.tasks_by_state.is_empty() {
            output.push_str("# HELP container_tasks_state Number of tasks in each state\n");
            output.push_str("# TYPE container_tasks_state gauge\n");
            for (state, count) in &self.tasks_by_state {
                let state_labels = self
                    .standard_labels
                    .to_label_string_with_extras(&[("state", state)]);
                output.push_str(&format!(
                    "container_tasks_state{} {}\n",
                    state_labels, count
                ));
            }
        }

        output
    }
}

impl PrometheusFormat for CadvisorMetrics {
    fn to_prometheus_format(&self, sandbox_id: Option<&str>) -> String {
        let mut output = String::new();
        output.push_str(&self.cpu.to_prometheus_format(sandbox_id));
        output.push_str(&self.memory.to_prometheus_format(sandbox_id));
        output.push_str(&self.network.to_prometheus_format(sandbox_id));
        output.push_str(&self.disk.to_prometheus_format(sandbox_id));
        output.push_str(&self.process.to_prometheus_format(sandbox_id));
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cadvisor_metrics_creation() {
        let metrics = CadvisorMetrics {
            cpu: CpuMetrics {
                usage_seconds_total: 100.0,
                user_seconds_total: 80.0,
                system_seconds_total: 20.0,
                load_average: Some(LoadAverage {
                    one_minute: 1.5,
                    five_minute: 1.2,
                    fifteen_minute: 1.0,
                }),
                per_cpu: Default::default(),
                standard_labels: StandardLabels::default(),
            },
            memory: MemoryMetrics {
                usage_bytes: 1024 * 1024 * 512, // 512 MB
                working_set_bytes: Some(256 * 1024 * 1024),
                cache_bytes: Some(256 * 1024 * 1024),
                rss_bytes: Some(256 * 1024 * 1024),
                swap_bytes: Some(0),
                mapped_file_bytes: None,
                failures: HashMap::new(),
                standard_labels: StandardLabels::default(),
            },
            network: Default::default(),
            disk: Default::default(),
            process: ProcessMetrics {
                count: 42,
                thread_count: 128,
                thread_count_max: Some(256),
                file_descriptors: 256,
                tasks_by_state: HashMap::new(),
                standard_labels: StandardLabels::default(),
            },
        };

        assert_eq!(metrics.cpu.usage_seconds_total, 100.0);
        assert_eq!(metrics.memory.usage_bytes, 1024 * 1024 * 512);
        assert_eq!(metrics.process.count, 42);
    }

    #[test]
    fn test_load_average_breakdown() {
        let load = LoadAverage {
            one_minute: 2.5,
            five_minute: 2.0,
            fifteen_minute: 1.5,
        };

        assert!(load.one_minute > load.five_minute);
        assert!(load.five_minute > load.fifteen_minute);
    }

    #[test]
    fn test_cpu_metrics_prometheus_format() {
        let cpu = CpuMetrics {
            usage_seconds_total: 100.5,
            user_seconds_total: 60.0,
            system_seconds_total: 40.5,
            load_average: Some(LoadAverage {
                one_minute: 1.5,
                five_minute: 1.2,
                fifteen_minute: 1.0,
            }),
            per_cpu: Default::default(),
            standard_labels: StandardLabels {
                container: "".to_string(),
                id: "test-pod".to_string(),
                image: "".to_string(),
                name: "test-pod".to_string(),
                namespace: "default".to_string(),
                pod: "test-pod".to_string(),
            },
        };

        let output = cpu.to_prometheus_format(Some("test-pod"));
        assert!(output.contains("container_cpu_usage_seconds_total"));
        assert!(output.contains("100.5"));
        assert!(output.contains("container_cpu_user_seconds_total"));
        assert!(output.contains("container_load_average_1m"));
        // Verify standard labels
        assert!(output.contains(r#"id="test-pod""#));
        assert!(output.contains(r#"name="test-pod""#));
        assert!(output.contains(r#"pod="test-pod""#));
    }

    #[test]
    fn test_memory_metrics_prometheus_format() {
        let memory = MemoryMetrics {
            usage_bytes: 536870912,
            working_set_bytes: Some(268435456),
            cache_bytes: Some(268435456),
            rss_bytes: Some(268435456),
            swap_bytes: Some(0),
            mapped_file_bytes: None,
            failures: HashMap::new(),
            standard_labels: StandardLabels::default(),
        };

        let output = memory.to_prometheus_format(None);
        assert!(output.contains("container_memory_usage_bytes"));
        assert!(output.contains("536870912"));
        assert!(output.contains("container_memory_working_set_bytes"));
        assert!(output.contains("container_memory_cache_bytes"));
    }

    #[test]
    fn test_network_metrics_prometheus_format() {
        let network = NetworkMetrics {
            receive_bytes_total: 1024000,
            transmit_bytes_total: 2048000,
            receive_packets_total: 10000,
            transmit_packets_total: 20000,
            receive_errors_total: Some(5),
            transmit_errors_total: None,
            receive_packets_dropped_total: None,
            transmit_packets_dropped_total: None,
            per_interface: Default::default(),
            standard_labels: StandardLabels::default(),
        };

        let output = network.to_prometheus_format(Some("sandbox-1"));
        assert!(output.contains("container_network_receive_bytes_total"));
        assert!(output.contains("1024000"));
        assert!(output.contains("container_network_transmit_bytes_total"));
        assert!(output.contains("container_network_receive_errors_total"));
    }

    #[test]
    fn test_disk_metrics_prometheus_format() {
        let disk = DiskMetrics {
            reads_total: 1000,
            writes_total: 2000,
            reads_bytes_total: 10485760,
            writes_bytes_total: 20971520,
            read_seconds_total: 1.5,
            write_seconds_total: 2.5,
            io_time_seconds_total: None,
            io_time_weighted_seconds_total: None,
            per_device: Default::default(),
            standard_labels: StandardLabels::default(),
        };

        let output = disk.to_prometheus_format(None);
        assert!(output.contains("container_disk_io_reads_total"));
        assert!(output.contains("1000"));
        assert!(output.contains("container_disk_io_writes_total"));
        assert!(output.contains("container_disk_io_read_seconds_total"));
    }

    #[test]
    fn test_process_metrics_prometheus_format() {
        let process = ProcessMetrics {
            count: 42,
            thread_count: 128,
            thread_count_max: Some(256),
            file_descriptors: 512,
            tasks_by_state: HashMap::new(),
            standard_labels: StandardLabels {
                container: "".to_string(),
                id: "app-pod".to_string(),
                image: "".to_string(),
                name: "app-pod".to_string(),
                namespace: "default".to_string(),
                pod: "app-pod".to_string(),
            },
        };

        let output = process.to_prometheus_format(Some("app-pod"));
        assert!(output.contains("container_processes_count"));
        assert!(output.contains("42"));
        assert!(output.contains("container_threads_count"));
        assert!(output.contains("128"));
        assert!(output.contains("container_threads_max_count"));
        // Verify standard labels
        assert!(output.contains(r#"id="app-pod""#));
        assert!(output.contains(r#"name="app-pod""#));
        assert!(output.contains(r#"pod="app-pod""#));
    }

    #[test]
    fn test_cadvisor_metrics_prometheus_format() {
        let metrics = CadvisorMetrics {
            cpu: CpuMetrics {
                usage_seconds_total: 50.0,
                user_seconds_total: 30.0,
                system_seconds_total: 20.0,
                load_average: None,
                per_cpu: Default::default(),
                standard_labels: StandardLabels::default(),
            },
            memory: MemoryMetrics {
                usage_bytes: 1073741824,
                working_set_bytes: Some(536870912),
                cache_bytes: None,
                rss_bytes: None,
                swap_bytes: None,
                mapped_file_bytes: None,
                failures: HashMap::new(),
                standard_labels: StandardLabels::default(),
            },
            network: NetworkMetrics {
                receive_bytes_total: 5000000,
                transmit_bytes_total: 3000000,
                receive_packets_total: 50000,
                transmit_packets_total: 30000,
                receive_errors_total: None,
                transmit_errors_total: None,
                receive_packets_dropped_total: None,
                transmit_packets_dropped_total: None,
                per_interface: Default::default(),
                standard_labels: StandardLabels::default(),
            },
            disk: DiskMetrics {
                reads_total: 5000,
                writes_total: 8000,
                reads_bytes_total: 52428800,
                writes_bytes_total: 83886080,
                read_seconds_total: 5.0,
                write_seconds_total: 8.0,
                io_time_seconds_total: None,
                io_time_weighted_seconds_total: None,
                per_device: Default::default(),
                standard_labels: StandardLabels::default(),
            },
            process: ProcessMetrics {
                count: 25,
                thread_count: 64,
                thread_count_max: Some(512),
                file_descriptors: 256,
                tasks_by_state: HashMap::new(),
                standard_labels: StandardLabels::default(),
            },
        };

        let output = metrics.to_prometheus_format(Some("test-sandbox"));

        // Verify all metric categories are present
        assert!(output.contains("container_cpu_usage_seconds_total"));
        assert!(output.contains("container_memory_usage_bytes"));
        assert!(output.contains("container_network_receive_bytes_total"));
        assert!(output.contains("container_disk_io_reads_total"));
        assert!(output.contains("container_processes_count"));

        // Verify specific values
        assert!(output.contains("50")); // CPU usage
        assert!(output.contains("1073741824")); // Memory usage
        assert!(output.contains("5000000")); // Network receive
        assert!(output.contains("5000")); // Disk reads
        assert!(output.contains("25")); // Process count
    }
}
