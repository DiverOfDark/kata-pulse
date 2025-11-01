//! Application context and dependency injection
//!
//! This module provides a centralized container for all application singletons.
//! All services are created once during startup and accessed through this context.

use anyhow::Result;
use std::sync::Arc;

use crate::monitor::metrics_cache::MetricsCache;
use crate::monitor::metrics_collector::MetricsCollector;
use crate::monitor::sandbox_cache::SandboxCache;
use crate::monitor::sandbox_cache_manager::SandboxCacheManager;
use crate::utils::metrics_converter::{CRILabelEnricher, LabelEnricher};

/// Application context holding all singleton instances
///
/// This is the single source of truth for all application dependencies.
/// Services are created once at startup and accessed through this context
/// in all handlers and other components.
#[derive(Clone)]
pub struct AppContext {
    /// Sandbox cache - stores sandbox metadata (pod names, namespaces, UIDs)
    sandbox_cache: Arc<SandboxCache>,

    /// Metrics cache - double-buffered cache for metrics from all sandboxes
    metrics_cache: Arc<MetricsCache>,

    /// Sandbox cache manager - handles directory monitoring and CRI metadata sync
    sandbox_cache_manager: Arc<SandboxCacheManager>,

    /// Metrics collector - handles periodic metrics collection
    metrics_collector: Arc<MetricsCollector>,

    /// CRI label enricher - enriches metrics with pod metadata
    cri_enricher: Arc<dyn LabelEnricher>,
}

impl AppContext {
    /// Create a new application context with all singletons initialized
    ///
    /// This should be called once during startup before creating the HTTP server.
    /// All services are created and stored as Arc for shared ownership.
    pub fn new(runtime_endpoint: String, metrics_interval_secs: u64) -> Result<Self> {
        tracing::info!("Initializing application context");

        if runtime_endpoint.is_empty() {
            return Err(anyhow::anyhow!("runtime endpoint missing"));
        }

        // Validate metrics interval
        if metrics_interval_secs == 0 {
            return Err(anyhow::anyhow!(
                "metrics_interval_secs must be > 0, got {}",
                metrics_interval_secs
            ));
        }

        // Create the core caches
        let sandbox_cache = Arc::new(SandboxCache::new());
        let metrics_cache = Arc::new(MetricsCache::new());
        tracing::info!("Core caches initialized");

        // Create sandbox cache manager (directory monitoring + CRI sync)
        let sandbox_cache_manager = Arc::new(SandboxCacheManager::new(
            sandbox_cache.clone(),
            metrics_cache.clone(),
            runtime_endpoint,
        ));
        tracing::info!("Sandbox cache manager initialized");

        // Create metrics collector (periodic metrics collection)
        let metrics_collector = Arc::new(MetricsCollector::new(
            sandbox_cache.clone(),
            metrics_cache.clone(),
            metrics_interval_secs,
        ));
        tracing::info!("Metrics collector initialized");

        // Create the CRI label enricher
        let cri_enricher: Arc<dyn LabelEnricher> =
            Arc::new(CRILabelEnricher::new(sandbox_cache.clone()));
        tracing::info!("CRI label enricher initialized");

        Ok(AppContext {
            sandbox_cache,
            metrics_cache,
            sandbox_cache_manager,
            metrics_collector,
            cri_enricher,
        })
    }

    /// Start background tasks for sandbox cache management and metrics collection
    ///
    /// This spawns two long-running background tasks:
    /// - Sandbox cache manager (directory monitoring + CRI metadata sync)
    /// - Metrics collector (periodic metrics collection)
    ///
    /// Note: We clone the Arc<T> (cheap - just increments reference count),
    /// not the underlying data. All tasks share the same singleton instances.
    pub fn start(&self) -> Result<()> {
        // Spawn the sandbox cache manager task (directory monitoring + CRI sync)
        // Clone the Arc to move into the async task (cheap - just ref counting)
        let sandbox_cache_manager = self.sandbox_cache_manager.clone();
        tokio::spawn(async move {
            if let Err(e) = sandbox_cache_manager.start().await {
                tracing::error!(error = %e, "Sandbox cache manager error");
            }
        });

        // Spawn the metrics collector task (periodic metrics collection)
        // Clone the Arc to move into the async task (cheap - just ref counting)
        let metrics_collector = self.metrics_collector.clone();
        tokio::spawn(async move {
            if let Err(e) = metrics_collector.start().await {
                tracing::error!(error = %e, "Metrics collector error");
            }
        });

        Ok(())
    }

    /// Get reference to the sandbox cache
    pub fn sandbox_cache(&self) -> &Arc<SandboxCache> {
        &self.sandbox_cache
    }

    /// Get reference to the metrics cache
    pub fn metrics_cache(&self) -> &Arc<MetricsCache> {
        &self.metrics_cache
    }

    /// Get reference to the CRI label enricher
    pub fn cri_enricher(&self) -> &Arc<dyn LabelEnricher> {
        &self.cri_enricher
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_context_creation() {
        let context = AppContext::new("/tmp/test.sock".to_string(), 1);
        assert!(context.is_ok());

        let ctx = context.unwrap();
        // Verify key singletons were created
        let _ = ctx.sandbox_cache();
        let _ = ctx.metrics_cache();
        let _ = ctx.cri_enricher();
    }

    #[test]
    fn test_app_context_clone() {
        let context = AppContext::new("/tmp/test.sock".to_string(), 1).unwrap();
        let cloned = context.clone();

        // Both should reference the same sandbox cache instance (same Arc pointer)
        let ptr1 = Arc::as_ptr(context.sandbox_cache());
        let ptr2 = Arc::as_ptr(cloned.sandbox_cache());
        assert_eq!(ptr1, ptr2);
    }

    #[test]
    fn test_app_context_empty_endpoint() {
        let context = AppContext::new(String::new(), 1);
        assert!(context.is_err());
    }

    #[test]
    fn test_app_context_zero_metrics_interval() {
        let context = AppContext::new("/tmp/test.sock".to_string(), 0);
        assert!(context.is_err(), "Should reject zero metrics_interval_secs");
    }

    #[test]
    fn test_app_context_valid_metrics_interval() {
        let context = AppContext::new("/tmp/test.sock".to_string(), 60);
        assert!(
            context.is_ok(),
            "Should accept valid metrics_interval_secs > 0"
        );
    }
}
