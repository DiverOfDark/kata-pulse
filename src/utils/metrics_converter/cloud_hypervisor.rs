//! Cloud Hypervisor metrics converter
//!
//! Converts Kata/Cloud Hypervisor Prometheus metrics to cAdvisor format.
//! Implements the metric mappings documented in KATA_TO_CADVISOR_MAPPING.md

use crate::utils::metrics_converter::cadvisor::{
    DeviceMetrics, InterfaceMetrics, LoadAverage, StandardLabels,
};
use crate::utils::metrics_converter::config::{ConversionConfig, LabelEnricher};
use crate::utils::metrics_converter::{
    CpuMetrics, DiskMetrics, MemoryMetrics, MetricsConverter, NetworkMetrics, ProcessMetrics,
};
use crate::utils::prometheus_parser::PrometheusMetrics;
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::debug;

/// Cloud Hypervisor metrics converter
///
/// Converts Kata metrics (from Cloud Hypervisor) to cAdvisor-compatible format.
pub struct CloudHypervisorConverter {
    config: ConversionConfig,
    label_enricher: Option<Arc<dyn LabelEnricher>>,
    sandbox_id: Option<String>,
}

impl CloudHypervisorConverter {
    /// Create a new converter with label enricher and sandbox ID
    pub fn with_enricher(
        config: ConversionConfig,
        label_enricher: Arc<dyn LabelEnricher>,
        sandbox_id: String,
    ) -> Self {
        Self {
            config,
            label_enricher: Some(label_enricher),
            sandbox_id: Some(sandbox_id),
        }
    }

    /// Create standard cAdvisor labels from CRI enricher metadata
    fn create_standard_labels(&self) -> StandardLabels {
        // Get enriched labels from CRI enricher if available
        if let (Some(enricher), Some(ref sandbox_id)) = (&self.label_enricher, &self.sandbox_id) {
            let enriched = enricher.enrich(sandbox_id);
            StandardLabels::new(
                &enriched.pod_uid,
                &enriched.pod_name,
                &enriched.pod_namespace,
            )
        } else {
            StandardLabels::new("", "", "")
        }
    }
}

impl MetricsConverter for CloudHypervisorConverter {
    fn convert_cpu(&self, metrics: &PrometheusMetrics) -> Result<CpuMetrics> {
        debug!("Converting CPU metrics");

        let mut cpu_metrics = CpuMetrics::default();

        // Aggregate CPU time from the pre-computed "total" CPU metrics
        // Metrics are provided per-CPU (cpu="0", cpu="1") and as aggregated totals (cpu="total")
        // We use only the cpu="total" to avoid double-counting individual CPU cores
        let mut total_jiffies = 0u64;

        for metric in metrics.metrics.values() {
            if !metric.name.starts_with("kata_guest_cpu_time") {
                continue;
            }

            for sample in &metric.samples {
                let cpu = sample.labels.get("cpu").map(|s| s.as_str());
                let item = sample.labels.get("item").map(|s| s.as_str());
                let value = sample.value as u64;

                // Only use the pre-aggregated cpu="total" values
                // Ignore individual per-CPU metrics (cpu="0", cpu="1", etc.) to avoid double-counting
                if cpu == Some("total") {
                    match item {
                        Some("user") | Some("system") | Some("guest") | Some("nice") => {
                            total_jiffies += value;
                        }
                        _ => {}
                    }
                }
            }
        }

        cpu_metrics.usage_seconds_total =
            total_jiffies as f64 / self.config.cpu_jiffy_conversion_factor;

        // Extract individual components if available (using aggregated totals only)
        for metric in metrics.metrics.values() {
            if !metric.name.starts_with("kata_guest_cpu_time") {
                continue;
            }

            for sample in &metric.samples {
                let cpu = sample.labels.get("cpu").map(|s| s.as_str());
                let item = sample.labels.get("item").map(|s| s.as_str());
                let value = sample.value;

                // Only use the pre-aggregated cpu="total" values
                if cpu == Some("total") {
                    match item {
                        Some("user") => {
                            cpu_metrics.user_seconds_total +=
                                value / self.config.cpu_jiffy_conversion_factor;
                        }
                        Some("system") => {
                            cpu_metrics.system_seconds_total +=
                                value / self.config.cpu_jiffy_conversion_factor;
                        }
                        _ => {}
                    }
                }
            }
        }

        // Extract load average
        if let Some(load) = self.extract_load_average(metrics) {
            cpu_metrics.load_average = Some(load);
        }

        // Populate standard labels with CRI metadata during conversion
        cpu_metrics.standard_labels = self.create_standard_labels();

        Ok(cpu_metrics)
    }

    fn convert_memory(&self, metrics: &PrometheusMetrics) -> Result<MemoryMetrics> {
        debug!("Converting memory metrics");

        let mut memory_metrics = MemoryMetrics::default();
        let mut meminfo: HashMap<String, u64> = HashMap::new();

        // Extract all meminfo values
        for metric in metrics.metrics.values() {
            if !metric.name.starts_with("kata_guest_meminfo") {
                continue;
            }

            for sample in &metric.samples {
                if let Some(item) = sample.labels.get("item") {
                    meminfo.insert(item.clone(), sample.value as u64);
                }
            }
        }

        // Calculate memory usage: mem_total - mem_free
        if let (Some(&total), Some(&free)) = (meminfo.get("memtotal"), meminfo.get("memfree")) {
            memory_metrics.usage_bytes = total.saturating_sub(free);
        }

        // Calculate working set: active + inactive_file
        if let (Some(&active), Some(&inactive_file)) =
            (meminfo.get("active"), meminfo.get("inactive_file"))
        {
            memory_metrics.working_set_bytes = Some(active + inactive_file);
        }

        // Memory cache: cached + buffers
        if let (Some(&cached), Some(&buffers)) = (meminfo.get("cached"), meminfo.get("buffers")) {
            memory_metrics.cache_bytes = Some(cached + buffers);
        }

        // RSS: anonymous pages
        if let Some(&anon) = meminfo.get("anon_pages") {
            memory_metrics.rss_bytes = Some(anon);
        }

        // Swap: swap_total - swap_free
        if let (Some(&swap_total), Some(&swap_free)) =
            (meminfo.get("swaptotal"), meminfo.get("swapfree"))
        {
            memory_metrics.swap_bytes = Some(swap_total.saturating_sub(swap_free));
        }

        // Mapped file pages
        if let Some(&mapped) = meminfo.get("mapped") {
            memory_metrics.mapped_file_bytes = Some(mapped);
        }

        // Populate standard labels with CRI metadata during conversion
        memory_metrics.standard_labels = self.create_standard_labels();

        Ok(memory_metrics)
    }

    fn convert_network(&self, metrics: &PrometheusMetrics) -> Result<NetworkMetrics> {
        debug!("Converting network metrics");

        let mut network_metrics = NetworkMetrics::default();
        let mut interfaces: HashMap<String, InterfaceMetrics> = HashMap::new();

        // Extract network stats per interface
        for metric in metrics.metrics.values() {
            if !metric.name.starts_with("kata_guest_netdev_stat") {
                continue;
            }

            for sample in &metric.samples {
                let interface = match sample.labels.get("interface") {
                    Some(iface) => iface.clone(),
                    None => continue,
                };

                // Filter interfaces: only include eth0, veth*, tap*, tun*
                if !self.config.matches_network_interface(&interface) {
                    continue;
                }

                let item = sample.labels.get("item").map(|s| s.as_str());
                let value = sample.value as u64;

                let iface_metrics = interfaces.entry(interface.clone()).or_default();
                iface_metrics.name = interface;

                match item {
                    Some("recv_bytes") => {
                        iface_metrics.receive_bytes = value;
                        network_metrics.receive_bytes_total += value;
                    }
                    Some("xmit_bytes") => {
                        iface_metrics.transmit_bytes = value;
                        network_metrics.transmit_bytes_total += value;
                    }
                    Some("recv_packets") => {
                        iface_metrics.receive_packets = value;
                        network_metrics.receive_packets_total += value;
                    }
                    Some("xmit_packets") => {
                        iface_metrics.transmit_packets = value;
                        network_metrics.transmit_packets_total += value;
                    }
                    Some("recv_errs") => {
                        iface_metrics.receive_errors = Some(value);
                        network_metrics.receive_errors_total =
                            Some(network_metrics.receive_errors_total.unwrap_or(0) + value);
                    }
                    Some("xmit_errs") => {
                        iface_metrics.transmit_errors = Some(value);
                        network_metrics.transmit_errors_total =
                            Some(network_metrics.transmit_errors_total.unwrap_or(0) + value);
                    }
                    Some("recv_drop") => {
                        iface_metrics.receive_dropped = Some(value);
                        network_metrics.receive_packets_dropped_total = Some(
                            network_metrics.receive_packets_dropped_total.unwrap_or(0) + value,
                        );
                    }
                    Some("xmit_drop") => {
                        iface_metrics.transmit_dropped = Some(value);
                        network_metrics.transmit_packets_dropped_total = Some(
                            network_metrics.transmit_packets_dropped_total.unwrap_or(0) + value,
                        );
                    }
                    _ => {}
                }
            }
        }

        if self.config.include_per_interface {
            network_metrics.per_interface = interfaces;
        }

        // Populate standard labels with CRI metadata during conversion
        network_metrics.standard_labels = self.create_standard_labels();

        Ok(network_metrics)
    }

    fn convert_disk(&self, metrics: &PrometheusMetrics) -> Result<DiskMetrics> {
        debug!("Converting disk metrics");

        let mut disk_metrics = DiskMetrics::default();
        let mut devices: HashMap<String, DeviceMetrics> = HashMap::new();

        // Extract disk stats per device
        for metric in metrics.metrics.values() {
            if !metric.name.starts_with("kata_guest_diskstat") {
                continue;
            }

            for sample in &metric.samples {
                let disk = match sample.labels.get("disk") {
                    Some(d) => d.clone(),
                    None => continue,
                };

                let item = sample.labels.get("item").map(|s| s.as_str());
                let value = sample.value;

                let device_metrics = devices.entry(disk.clone()).or_default();
                device_metrics.device = disk;

                match item {
                    Some("reads") => {
                        let count = value as u64;
                        device_metrics.reads = count;
                        disk_metrics.reads_total += count;
                    }
                    Some("writes") => {
                        let count = value as u64;
                        device_metrics.writes = count;
                        disk_metrics.writes_total += count;
                    }
                    Some("sectors_read") => {
                        // Convert sectors to bytes: multiply by 512
                        let bytes = (value as u64) * 512;
                        device_metrics.reads_bytes = bytes;
                        disk_metrics.reads_bytes_total += bytes;
                    }
                    Some("sectors_written") => {
                        // Convert sectors to bytes: multiply by 512
                        let bytes = (value as u64) * 512;
                        device_metrics.writes_bytes = bytes;
                        disk_metrics.writes_bytes_total += bytes;
                    }
                    Some("time_reading") => {
                        // Convert milliseconds to seconds
                        let seconds = value / 1000.0;
                        device_metrics.read_time_seconds = seconds;
                        disk_metrics.read_seconds_total += seconds;
                    }
                    Some("time_writing") => {
                        // Convert milliseconds to seconds
                        let seconds = value / 1000.0;
                        device_metrics.write_time_seconds = seconds;
                        disk_metrics.write_seconds_total += seconds;
                    }
                    Some("time_in_progress") => {
                        let seconds = value / 1000.0;
                        disk_metrics.io_time_seconds_total =
                            Some(disk_metrics.io_time_seconds_total.unwrap_or(0.0) + seconds);
                    }
                    Some("weighted_time_in_progress") => {
                        let seconds = value / 1000.0;
                        disk_metrics.io_time_weighted_seconds_total = Some(
                            disk_metrics.io_time_weighted_seconds_total.unwrap_or(0.0) + seconds,
                        );
                    }
                    _ => {}
                }
            }
        }

        if self.config.include_per_device {
            disk_metrics.per_device = devices;
        }

        // Populate standard labels with CRI metadata during conversion
        disk_metrics.standard_labels = self.create_standard_labels();

        Ok(disk_metrics)
    }

    fn convert_process(&self, metrics: &PrometheusMetrics) -> Result<ProcessMetrics> {
        debug!("Converting process metrics");

        let mut process_metrics = ProcessMetrics::default();

        // Extract task counts
        for metric in metrics.metrics.values() {
            if !metric.name.starts_with("kata_guest_tasks") {
                continue;
            }

            for sample in &metric.samples {
                let item = sample.labels.get("item").map(|s| s.as_str());
                let value = sample.value as u64;

                match item {
                    Some("cur") => process_metrics.count = value,
                    Some("max") => process_metrics.thread_count_max = Some(value),
                    _ => {}
                }
            }
        }

        // Aggregate thread count across all components
        for metric in metrics.metrics.values() {
            let should_count = metric.name.ends_with("_threads")
                && (metric.name.contains("shim")
                    || metric.name.contains("hypervisor")
                    || metric.name.contains("agent")
                    || metric.name.contains("virtiofsd"));

            if should_count {
                for sample in &metric.samples {
                    process_metrics.thread_count += sample.value as u64;
                }
            }
        }

        // Aggregate file descriptors across all components
        for metric in metrics.metrics.values() {
            let should_count = metric.name.ends_with("_fds")
                && (metric.name.contains("shim")
                    || metric.name.contains("hypervisor")
                    || metric.name.contains("agent")
                    || metric.name.contains("virtiofsd"));

            if should_count {
                for sample in &metric.samples {
                    process_metrics.file_descriptors += sample.value as u64;
                }
            }
        }

        // Populate standard labels with CRI metadata during conversion
        process_metrics.standard_labels = self.create_standard_labels();

        Ok(process_metrics)
    }
}

impl CloudHypervisorConverter {
    /// Extract load average from metrics
    fn extract_load_average(&self, metrics: &PrometheusMetrics) -> Option<LoadAverage> {
        let mut loads = HashMap::new();

        for metric in metrics.metrics.values() {
            if !metric.name.starts_with("kata_guest_load") {
                continue;
            }

            for sample in &metric.samples {
                if let Some(item) = sample.labels.get("item") {
                    loads.insert(item.clone(), sample.value);
                }
            }
        }

        if loads.is_empty() {
            return None;
        }

        Some(LoadAverage {
            one_minute: loads.get("load1").copied().unwrap_or(0.0),
            five_minute: loads.get("load5").copied().unwrap_or(0.0),
            fifteen_minute: loads.get("load15").copied().unwrap_or(0.0),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::metrics_converter::cadvisor::PrometheusFormat;
    use crate::utils::metrics_converter::config::EnrichedLabels;
    use crate::utils::metrics_converter::CRILabelEnricher;
    use crate::utils::prometheus_parser::{MetricSample, PrometheusMetrics};

    #[test]
    fn test_cpu_conversion() {
        let mut metrics = PrometheusMetrics::new();
        let cpu_metric = metrics
            .metrics
            .entry("kata_guest_cpu_time".to_string())
            .or_insert_with(|| crate::utils::prometheus_parser::PrometheusMetric {
                name: "kata_guest_cpu_time".to_string(),
                metric_type: Some("gauge".to_string()),
                help: None,
                samples: vec![],
            });

        // Add samples using the pre-aggregated cpu="total" values (from real data)
        // This avoids double-counting individual CPU cores
        cpu_metric.samples.push(MetricSample {
            name: "kata_guest_cpu_time".to_string(),
            labels: {
                let mut map = HashMap::new();
                map.insert("cpu".to_string(), "total".to_string());
                map.insert("item".to_string(), "user".to_string());
                map
            },
            value: 56160.0,
            timestamp: None,
        });

        cpu_metric.samples.push(MetricSample {
            name: "kata_guest_cpu_time".to_string(),
            labels: {
                let mut map = HashMap::new();
                map.insert("cpu".to_string(), "total".to_string());
                map.insert("item".to_string(), "system".to_string());
                map
            },
            value: 82060.0,
            timestamp: None,
        });

        let cache = Arc::new(crate::monitor::sandbox_cache::SandboxCache::new());
        let enricher = Arc::new(CRILabelEnricher::new(cache));
        let converter = CloudHypervisorConverter::with_enricher(
            ConversionConfig::default(),
            enricher,
            "test-sandbox".to_string(),
        );
        let cpu_metrics = converter.convert_cpu(&metrics).unwrap();

        // (56160 + 82060) / 100 = 1382.2 seconds (jiffies from /proc/stat with USER_HZ=100)
        assert_eq!(cpu_metrics.usage_seconds_total, 1382.2);
        assert_eq!(cpu_metrics.user_seconds_total, 561.6);
        assert_eq!(cpu_metrics.system_seconds_total, 820.6);
    }

    #[test]
    fn test_memory_conversion() {
        let mut metrics = PrometheusMetrics::new();
        let mem_metric = metrics
            .metrics
            .entry("kata_guest_meminfo".to_string())
            .or_insert_with(|| crate::utils::prometheus_parser::PrometheusMetric {
                name: "kata_guest_meminfo".to_string(),
                metric_type: Some("gauge".to_string()),
                help: None,
                samples: vec![],
            });

        // Add samples: mem_total=1000, mem_free=400
        mem_metric.samples.push(MetricSample {
            name: "kata_guest_meminfo".to_string(),
            labels: {
                let mut map = HashMap::new();
                map.insert("item".to_string(), "memtotal".to_string());
                map
            },
            value: 1000.0,
            timestamp: None,
        });

        mem_metric.samples.push(MetricSample {
            name: "kata_guest_meminfo".to_string(),
            labels: {
                let mut map = HashMap::new();
                map.insert("item".to_string(), "memfree".to_string());
                map
            },
            value: 400.0,
            timestamp: None,
        });

        let cache = Arc::new(crate::monitor::sandbox_cache::SandboxCache::new());
        let enricher = Arc::new(CRILabelEnricher::new(cache));
        let converter = CloudHypervisorConverter::with_enricher(
            ConversionConfig::default(),
            enricher,
            "test-sandbox".to_string(),
        );
        let mem_metrics = converter.convert_memory(&metrics).unwrap();

        // 1000 - 400 = 600
        assert_eq!(mem_metrics.usage_bytes, 600);
    }

    #[test]
    fn test_interface_filtering() {
        let config = ConversionConfig::default();
        assert!(config.matches_network_interface("eth0"));
        assert!(!config.matches_network_interface("lo"));
    }

    // Mock label enricher for testing
    struct MockLabelEnricher {
        enriched_labels: EnrichedLabels,
    }

    impl MockLabelEnricher {
        fn new(pod_name: &str, namespace: &str, uid: &str) -> Self {
            Self {
                enriched_labels: EnrichedLabels::new(uid, pod_name, namespace),
            }
        }
    }

    impl crate::utils::metrics_converter::config::LabelEnricher for MockLabelEnricher {
        fn enrich(&self, _sandbox_id: &str) -> EnrichedLabels {
            self.enriched_labels.clone()
        }
    }

    #[test]
    fn test_cpu_conversion_with_enrichment() {
        let mut metrics = PrometheusMetrics::new();
        let cpu_metric = metrics
            .metrics
            .entry("kata_guest_cpu_time".to_string())
            .or_insert_with(|| crate::utils::prometheus_parser::PrometheusMetric {
                name: "kata_guest_cpu_time".to_string(),
                metric_type: Some("gauge".to_string()),
                help: None,
                samples: vec![],
            });

        // Add samples using the pre-aggregated cpu="total" values (from real data)
        cpu_metric.samples.push(MetricSample {
            name: "kata_guest_cpu_time".to_string(),
            labels: {
                let mut map = HashMap::new();
                map.insert("cpu".to_string(), "total".to_string());
                map.insert("item".to_string(), "user".to_string());
                map
            },
            value: 56160.0,
            timestamp: None,
        });

        cpu_metric.samples.push(MetricSample {
            name: "kata_guest_cpu_time".to_string(),
            labels: {
                let mut map = HashMap::new();
                map.insert("cpu".to_string(), "total".to_string());
                map.insert("item".to_string(), "system".to_string());
                map
            },
            value: 82060.0,
            timestamp: None,
        });

        let config = ConversionConfig::default();
        let enricher = Arc::new(MockLabelEnricher::new("my-pod", "default", "12345-67890"));
        let converter =
            CloudHypervisorConverter::with_enricher(config, enricher, "sandbox-123".to_string());

        let cpu_metrics = converter.convert_cpu(&metrics).unwrap();

        // Verify metrics conversion: (56160 + 82060) / 100 = 1382.2 seconds (jiffies with USER_HZ=100)
        assert_eq!(cpu_metrics.usage_seconds_total, 1382.2);
        assert_eq!(cpu_metrics.user_seconds_total, 561.6);

        // Verify enrichment happened during conversion (enriched labels are now in standard_labels)
        assert_eq!(cpu_metrics.standard_labels.name, "my-pod");
        assert_eq!(cpu_metrics.standard_labels.namespace, "default");
        assert_eq!(cpu_metrics.standard_labels.pod, "my-pod");
        assert_eq!(cpu_metrics.standard_labels.id, "12345-67890"); // pod_uid from enricher
    }

    #[test]
    fn test_memory_conversion_with_enrichment() {
        let mut metrics = PrometheusMetrics::new();
        let mem_metric = metrics
            .metrics
            .entry("kata_guest_meminfo".to_string())
            .or_insert_with(|| crate::utils::prometheus_parser::PrometheusMetric {
                name: "kata_guest_meminfo".to_string(),
                metric_type: Some("gauge".to_string()),
                help: None,
                samples: vec![],
            });

        // Add samples
        mem_metric.samples.push(MetricSample {
            name: "kata_guest_meminfo".to_string(),
            labels: {
                let mut map = HashMap::new();
                map.insert("item".to_string(), "memtotal".to_string());
                map
            },
            value: 1000.0,
            timestamp: None,
        });

        mem_metric.samples.push(MetricSample {
            name: "kata_guest_meminfo".to_string(),
            labels: {
                let mut map = HashMap::new();
                map.insert("item".to_string(), "memfree".to_string());
                map
            },
            value: 400.0,
            timestamp: None,
        });

        let config = ConversionConfig::default();
        let enricher = Arc::new(MockLabelEnricher::new(
            "test-app",
            "production",
            "abc-123-def",
        ));
        let converter =
            CloudHypervisorConverter::with_enricher(config, enricher, "sandbox-xyz".to_string());

        let mem_metrics = converter.convert_memory(&metrics).unwrap();

        // Verify metrics conversion
        assert_eq!(mem_metrics.usage_bytes, 600);

        // Verify enrichment happened during conversion (enriched labels are now in standard_labels)
        assert_eq!(mem_metrics.standard_labels.name, "test-app");
        assert_eq!(mem_metrics.standard_labels.namespace, "production");
        assert_eq!(mem_metrics.standard_labels.pod, "test-app");
    }

    #[test]
    fn test_enrichment_renders_in_prometheus_format() {
        let mut metrics = PrometheusMetrics::new();
        let cpu_metric = metrics
            .metrics
            .entry("kata_guest_cpu_time".to_string())
            .or_insert_with(|| crate::utils::prometheus_parser::PrometheusMetric {
                name: "kata_guest_cpu_time".to_string(),
                metric_type: Some("gauge".to_string()),
                help: None,
                samples: vec![],
            });

        cpu_metric.samples.push(MetricSample {
            name: "kata_guest_cpu_time".to_string(),
            labels: {
                let mut map = HashMap::new();
                map.insert("cpu".to_string(), "total".to_string());
                map.insert("item".to_string(), "user".to_string());
                map
            },
            value: 100.0,
            timestamp: None,
        });

        let config = ConversionConfig::default();
        let enricher = Arc::new(MockLabelEnricher::new("nginx-app", "web", "xyz-789"));
        let converter =
            CloudHypervisorConverter::with_enricher(config, enricher, "sandbox-abc".to_string());

        let cpu_metrics = converter.convert_cpu(&metrics).unwrap();

        // Render to Prometheus format
        let output = cpu_metrics.to_prometheus_format(Some("sandbox-abc"));

        // Verify standard labels appear in output (enriched labels are converted to standard labels)
        assert!(output.contains(r#"name="nginx-app""#)); // pod_name becomes name
        assert!(output.contains(r#"pod="nginx-app""#)); // pod_name becomes pod
        assert!(output.contains(r#"namespace="web""#)); // pod_namespace becomes namespace
        assert!(output.contains(r#"id="xyz-789""#)); // pod_uid from enricher becomes id

        // Note: enriched_labels like pod_uid are deprecated and no longer emitted in Prometheus format
        // Only standard_labels (container, id, image, name, namespace, pod) are now emitted
    }
}
