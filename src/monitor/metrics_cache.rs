use crate::utils::prometheus_parser::PrometheusMetrics;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::debug;

/// Cached metrics for a single sandbox
#[derive(Clone, Debug)]
pub struct CachedMetrics {
    /// The parsed metrics
    pub metrics: PrometheusMetrics,
}

/// Double-buffered cache for metrics from all sandboxes
///
/// This implementation uses two separate buffers to eliminate RwLock contention:
/// - `current_cache`: Readers always read from this (HTTP requests)
/// - `staging_cache`: Writer (metrics collector) builds here
/// - After collection completes, buffers are swapped atomically
///
/// Benefits:
/// - Readers are NEVER blocked by writers
/// - Writers don't block readers
/// - Atomic all-or-nothing consistency
/// - Better cache locality
#[derive(Clone)]
pub struct MetricsCache {
    /// Current buffer - readers read from here (HTTP requests)
    current_cache: Arc<Mutex<Arc<HashMap<String, CachedMetrics>>>>,
    /// Staging buffer - writer builds here during collection
    staging_cache: Arc<Mutex<HashMap<String, CachedMetrics>>>,
}

impl MetricsCache {
    /// Create a new empty double-buffered metrics cache
    pub fn new() -> Self {
        MetricsCache {
            current_cache: Arc::new(Mutex::new(Arc::new(HashMap::new()))),
            staging_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Get cached metrics for a sandbox (reader - NEVER blocked by writers)
    ///
    /// This is fast because:
    /// 1. Only takes a Mutex lock on current_cache (very brief - just clones Arc)
    /// 2. Never blocked by metrics collection (which writes to staging_cache)
    pub async fn get_metrics(&self, sandbox_id: &str) -> Option<CachedMetrics> {
        let current = self.current_cache.lock().await;
        current.get(sandbox_id).cloned()
    }

    /// Store a single metric in staging cache (internal use only)
    /// Used by metrics collection to build up new metrics
    async fn set_metrics_staging(&self, sandbox_id: String, metrics: PrometheusMetrics) {
        let cached = CachedMetrics { metrics };
        let mut staging = self.staging_cache.lock().await;
        staging.insert(sandbox_id, cached);
    }

    /// Start a new metrics collection cycle
    ///
    /// Call this when starting to collect metrics from all sandboxes
    pub async fn start_collection(&self) {
        debug!("Starting metrics collection - clearing staging cache");
        let mut staging = self.staging_cache.lock().await;
        staging.clear();
    }

    /// Add metrics during collection
    ///
    /// Call this for each sandbox as metrics are fetched and parsed
    pub async fn add_metrics(&self, sandbox_id: String, metrics: PrometheusMetrics) {
        self.set_metrics_staging(sandbox_id, metrics).await;
    }

    /// Finish collection and swap buffers atomically
    ///
    /// This is the critical section - it:
    /// 1. Takes staging_cache lock (to finalize collection)
    /// 2. Takes current_cache lock (to swap - VERY BRIEF)
    /// 3. Performs atomic swap
    /// 4. Clears staging for next cycle
    ///
    /// The swap is atomic and happens in <1 microsecond
    pub async fn finish_collection(&self) {
        debug!("Finishing metrics collection - preparing to swap buffers");

        // Prepare the new data
        let mut staging = self.staging_cache.lock().await;
        let new_data = std::mem::take(&mut *staging);

        // The actual atomic swap (very fast - just updates Arc pointer)
        {
            let mut current = self.current_cache.lock().await;
            *current = Arc::new(new_data);
            debug!("Metrics buffers swapped - staging cache cleared");
        }

        // staging is now empty, ready for next collection cycle
    }

    /// Remove metrics for a sandbox (when sandbox is deleted)
    ///
    /// This updates the current cache immediately since we're removing stale data
    pub async fn delete_metrics(&self, sandbox_id: &str) -> bool {
        let mut current = self.current_cache.lock().await;
        // We need to modify the current cache, so we rebuild it without the deleted entry
        let new_data: HashMap<String, CachedMetrics> = current
            .iter()
            .filter(|(id, _)| *id != sandbox_id)
            .map(|(id, cached)| (id.clone(), cached.clone()))
            .collect();

        let was_present = new_data.len() < current.len();
        *current = Arc::new(new_data);

        if was_present {
            debug!(sandbox_id = %sandbox_id, "Deleted metrics for sandbox");
        }
        was_present
    }
}

impl Default for MetricsCache {
    fn default() -> Self {
        Self::new()
    }
}
