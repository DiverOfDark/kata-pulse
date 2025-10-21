//! Metrics conversion module for transforming hypervisor-specific metrics to cAdvisor format
//!
//! This module provides extensible metrics conversion supporting multiple hypervisors.
//! Currently implements CloudHypervisor metrics conversion to cAdvisor-compatible format.
//!
//! ## Architecture
//!
//! The conversion pipeline is structured as:
//! 1. **MetricsConverter** trait - Main conversion interface (hypervisor-agnostic)
//! 2. **CloudHypervisorConverter** - Cloud hypervisor specific implementation
//! 3. **CadvisorMetrics** - Output model (cAdvisor-compatible format)
//! 4. **LabelEnricher** - Enriches labels with Kubernetes metadata
//!
//! ## Extensibility
//!
//! To support a new hypervisor (e.g., QEMU, Firecracker):
//! 1. Create new module `src/utils/metrics_converter/qemu.rs`
//! 2. Implement `MetricsConverter` trait
//! 3. Register in factory function
//!
//! Example:
//! ```ignore
//! pub struct QemuConverter { /* ... */ }
//!
//! impl MetricsConverter for QemuConverter {
//!     fn convert_cpu(&self, metrics: &PrometheusMetrics) -> Result<CpuMetrics> { /* ... */ }
//!     fn convert_memory(&self, metrics: &PrometheusMetrics) -> Result<MemoryMetrics> { /* ... */ }
//!     // ... implement other conversions
//! }
//! ```

pub mod cadvisor;
pub mod cloud_hypervisor;
pub mod config;

pub use cadvisor::{
    CadvisorMetrics, CpuMetrics, DiskMetrics, MemoryMetrics, NetworkMetrics, ProcessMetrics,
};
pub use cloud_hypervisor::CloudHypervisorConverter;
pub use config::{CRILabelEnricher, ConversionConfig, LabelEnricher};

use crate::utils::prometheus_parser::PrometheusMetrics;
use anyhow::Result;
use std::sync::Arc;

/// Main trait for metrics conversion
///
/// Implementations convert hypervisor-specific metrics to cAdvisor format.
/// This trait is hypervisor-agnostic and allows plugging in different implementations.
pub trait MetricsConverter {
    /// Convert CPU metrics
    fn convert_cpu(&self, metrics: &PrometheusMetrics) -> Result<CpuMetrics>;

    /// Convert memory metrics
    fn convert_memory(&self, metrics: &PrometheusMetrics) -> Result<MemoryMetrics>;

    /// Convert network metrics
    fn convert_network(&self, metrics: &PrometheusMetrics) -> Result<NetworkMetrics>;

    /// Convert disk I/O metrics
    fn convert_disk(&self, metrics: &PrometheusMetrics) -> Result<DiskMetrics>;

    /// Convert process metrics
    fn convert_process(&self, metrics: &PrometheusMetrics) -> Result<ProcessMetrics>;

    /// Complete conversion: CPU + Memory + Network + Disk + Process
    fn convert_all(&self, metrics: &PrometheusMetrics) -> Result<CadvisorMetrics> {
        let cpu = self.convert_cpu(metrics)?;
        let memory = self.convert_memory(metrics)?;
        let network = self.convert_network(metrics)?;
        let disk = self.convert_disk(metrics)?;
        let process = self.convert_process(metrics)?;

        Ok(CadvisorMetrics {
            cpu,
            memory,
            network,
            disk,
            process,
        })
    }
}

/// Factory function to create a converter with CRI label enricher
pub fn create_converter(
    config: ConversionConfig,
    label_enricher: Arc<dyn LabelEnricher>,
    sandbox_id: String,
) -> Box<dyn MetricsConverter> {
    match config.hypervisor_type {
        config::HypervisorType::CloudHypervisor => Box::new(
            CloudHypervisorConverter::with_enricher(config, label_enricher, sandbox_id),
        ), // Future: Add more hypervisor types
           // config::HypervisorType::Qemu => Box::new(QemuConverter::with_enricher(config, label_enricher, sandbox_id)),
           // config::HypervisorType::Firecracker => Box::new(FirecrackerConverter::with_enricher(config, label_enricher, sandbox_id)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_factory_creates_cloud_hypervisor_converter() {
        let config = ConversionConfig::default();
        let cache = Arc::new(crate::monitor::sandbox_cache::SandboxCache::new());
        let enricher = Arc::new(CRILabelEnricher::new(cache));
        let converter = create_converter(config, enricher, "test".parse().unwrap());
        // Just verify it doesn't crash - actual conversion tested in cloud_hypervisor tests
        assert!(std::mem::size_of_val(&*converter) > 0);
    }
}
