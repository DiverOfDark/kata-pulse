//! Metrics collector - handles periodic collection of metrics from sandboxes
//!
//! Responsibilities:
//! - Periodically collect metrics from all sandboxes
//! - Parse Prometheus format metrics
//! - Store metrics in double-buffered cache
//! - Track collection statistics (success/failure counts, timing)

use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info, warn};

use super::metrics_cache::MetricsCache;
use super::sandbox_cache::SandboxCache;

/// Collects metrics from sandboxes at regular intervals
///
/// Responsible for:
/// - Fetching metrics from sandbox shims in parallel
/// - Parsing Prometheus format metrics
/// - Storing in double-buffered cache (atomic buffer swap)
/// - Reporting collection statistics
pub struct MetricsCollector {
    sandbox_cache: Arc<SandboxCache>,
    metrics_cache: Arc<MetricsCache>,
    metrics_interval_secs: u64,
}

impl MetricsCollector {
    /// Create a new metrics collector
    pub fn new(
        sandbox_cache: Arc<SandboxCache>,
        metrics_cache: Arc<MetricsCache>,
        metrics_interval_secs: u64,
    ) -> Self {
        MetricsCollector {
            sandbox_cache,
            metrics_cache,
            metrics_interval_secs,
        }
    }

    /// Start the periodic metrics collection task
    ///
    /// This spawns a background task that collects metrics at the specified interval.
    /// The task will:
    /// 1. Get list of active sandboxes
    /// 2. Fetch metrics from all sandboxes in parallel
    /// 3. Parse Prometheus format metrics
    /// 4. Store in double-buffered cache with atomic buffer swap
    /// 5. Report timing and success/failure statistics
    pub async fn start(&self) -> Result<()> {
        let sandbox_cache = self.sandbox_cache.clone();
        let metrics_cache = self.metrics_cache.clone();

        let interval_secs = self.metrics_interval_secs;

        info!(
            interval_secs = interval_secs,
            "Starting metrics collector task"
        );

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));

            loop {
                interval.tick().await;

                let cycle_start = std::time::Instant::now();
                info!("Starting metrics collection cycle (double-buffered)");

                // Get current list of sandboxes
                let sandboxes = sandbox_cache.get_sandbox_list().await;
                debug!(
                    sandbox_count = sandboxes.len(),
                    "Retrieved sandbox list for metrics collection"
                );

                if sandboxes.is_empty() {
                    debug!("No sandboxes running, skipping metrics collection");
                    continue;
                }

                let total_sandboxes = sandboxes.len();
                info!(
                    sandbox_count = total_sandboxes,
                    "Collecting metrics from sandboxes (parallel, double-buffered)"
                );

                // Start collection - prepare staging cache
                metrics_cache.start_collection().await;

                // Collect metrics from all sandboxes in parallel
                let futures: Vec<_> = sandboxes
                    .into_iter()
                    .map(|sandbox_id| {
                        async move {
                            debug!(sandbox_id = %sandbox_id, "Attempting to fetch metrics from sandbox");
                            let fetch_result = crate::utils::shim_client::do_get(&sandbox_id, crate::config::METRICS_URL).await;
                            (sandbox_id, fetch_result)
                        }
                    })
                    .collect();

                let results = futures::future::join_all(futures).await;

                // Process results and add to staging cache
                let mut success_count = 0;
                let mut failure_count = 0;

                for (sandbox_id, result) in results {
                    match result {
                        Ok(data) => {
                            debug!(sandbox_id = %sandbox_id, data_size = data.len(), "Received metrics data from shim");
                            let metrics_text = String::from_utf8_lossy(&data);
                            match crate::utils::prometheus_parser::PrometheusMetrics::parse(
                                &metrics_text,
                            ) {
                                Ok(parsed_metrics) => {
                                    // Add to staging cache (not yet visible to readers)
                                    metrics_cache
                                        .add_metrics(sandbox_id.clone(), parsed_metrics)
                                        .await;
                                    success_count += 1;
                                    debug!(sandbox_id = %sandbox_id, "Metrics collected and added to staging");
                                }
                                Err(e) => {
                                    failure_count += 1;
                                    warn!(sandbox_id = %sandbox_id, error = %e, "Failed to parse metrics");
                                }
                            }
                        }
                        Err(e) => {
                            failure_count += 1;
                            warn!(sandbox_id = %sandbox_id, error = %e, "Failed to collect metrics from sandbox");
                        }
                    }
                }

                // Finish collection - atomic swap of buffers
                let swap_start = std::time::Instant::now();
                metrics_cache.finish_collection().await;
                let swap_duration_us = swap_start.elapsed().as_micros();

                let cycle_duration_ms = cycle_start.elapsed().as_millis();
                info!(
                    success = success_count,
                    failure = failure_count,
                    total = total_sandboxes,
                    duration_ms = cycle_duration_ms,
                    swap_duration_us = swap_duration_us,
                    "Metrics collection cycle completed (buffers swapped atomically)"
                );
            }
        });

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_collector_creation() {
        let sandbox_cache = Arc::new(SandboxCache::new());
        let metrics_cache = Arc::new(MetricsCache::new());
        let collector = MetricsCollector::new(sandbox_cache, metrics_cache, 30);
        // Verify it's created successfully
        assert!(std::mem::size_of_val(&collector) > 0);
    }
}
