//! Sandbox cache manager - handles directory monitoring and CRI metadata synchronization
//!
//! Responsibilities:
//! - Watch sandbox directory for new/deleted sandboxes
//! - Synchronize CRI metadata (pod names, namespaces, UIDs)
//! - Maintain sandbox cache state
//! - Delete metrics when sandboxes are removed

use crate::config;
use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

use super::metrics_cache::MetricsCache;
use super::sandbox_cache::SandboxCache;

const FS_MONITOR_RETRY_DELAY_SECONDS: u64 = 60;
const POD_CACHE_REFRESH_DELAY_SECONDS: u64 = 5;
const FS_CHECK_INTERVAL_SECONDS: u64 = 5;

/// Manages sandbox cache and directory monitoring
///
/// Responsible for:
/// - Monitoring filesystem for sandbox additions/deletions
/// - Syncing CRI metadata from container runtime
/// - Managing sandbox lifecycle in the cache
/// - Cleaning up metrics when sandboxes are deleted
pub struct SandboxCacheManager {
    sandbox_cache: Arc<SandboxCache>,
    metrics_cache: Arc<MetricsCache>,
    runtime_endpoint: String,
}

impl SandboxCacheManager {
    /// Create a new sandbox cache manager
    pub fn new(
        sandbox_cache: Arc<SandboxCache>,
        metrics_cache: Arc<MetricsCache>,
        runtime_endpoint: String,
    ) -> Self {
        SandboxCacheManager {
            sandbox_cache,
            metrics_cache,
            runtime_endpoint,
        }
    }

    /// Start monitoring sandbox directory and syncing CRI metadata
    ///
    /// This is a long-running task that should be spawned as a background task.
    /// It will:
    /// 1. Read initial sandbox list from filesystem
    /// 2. Monitor filesystem for additions/deletions
    /// 3. Periodically sync CRI metadata
    pub async fn start(&self) -> Result<()> {
        let sandbox_dir = config::get_sandboxes_storage_path();
        info!(path = ?sandbox_dir, "Starting sandbox cache manager");

        // Try to monitor the sandbox directory
        loop {
            debug!(path = ?sandbox_dir, "Attempting to read sandbox directory");
            match tokio::fs::read_dir(&sandbox_dir).await {
                Ok(mut dir) => {
                    info!(path = ?sandbox_dir, "Successfully opened sandbox directory");
                    // Read initial sandbox list
                    let mut sandbox_list = Vec::new();
                    while let Some(entry) = dir.next_entry().await? {
                        if let Some(name) = entry.file_name().to_str() {
                            debug!(sandbox = %name, "Adding sandbox to initial list");
                            sandbox_list.push(name.to_string());
                            self.sandbox_cache
                                .put_if_not_exists(
                                    name,
                                    super::sandbox_cache::SandboxCRIMetadata {
                                        uid: String::new(),
                                        name: String::new(),
                                        namespace: String::new(),
                                    },
                                )
                                .await;
                        }
                    }
                    info!(
                        count = sandbox_list.len(),
                        "initial sync of sbs directory completed"
                    );

                    // Start monitoring directory for changes
                    info!(
                        count = sandbox_list.len(),
                        "Starting sandbox directory monitoring"
                    );
                    self.monitor_directory(&sandbox_list).await?;
                    break;
                }
                Err(e) => {
                    warn!(
                        error = %e,
                        path = ?sandbox_dir,
                        retry_delay_sec = FS_MONITOR_RETRY_DELAY_SECONDS,
                        "cannot monitor sandboxes, retrying"
                    );
                    sleep(Duration::from_secs(FS_MONITOR_RETRY_DELAY_SECONDS)).await;
                }
            }
        }

        Ok(())
    }

    /// Monitor sandbox directory for changes
    async fn monitor_directory(&self, initial_list: &[String]) -> Result<()> {
        let sandbox_dir = config::get_sandboxes_storage_path();
        let sandbox_dir_str = sandbox_dir.to_string_lossy().to_string();
        let mut sandbox_list = initial_list.to_vec();
        let mut next_cache_update =
            tokio::time::Instant::now() + Duration::from_secs(POD_CACHE_REFRESH_DELAY_SECONDS);
        let mut next_fs_check =
            tokio::time::Instant::now() + Duration::from_secs(FS_CHECK_INTERVAL_SECONDS);

        loop {
            let now = tokio::time::Instant::now();

            // Handle cache update if it's time
            if now >= next_cache_update {
                next_cache_update = now + Duration::from_secs(POD_CACHE_REFRESH_DELAY_SECONDS);
                self.sync_cri_metadata(&mut sandbox_list).await;
            }

            // Handle filesystem check if it's time
            if now >= next_fs_check {
                next_fs_check = now + Duration::from_secs(FS_CHECK_INTERVAL_SECONDS);
                self.check_filesystem_changes(&sandbox_dir_str, &mut sandbox_list)
                    .await;
            }

            // Sleep for a short period before checking again
            sleep(Duration::from_millis(100)).await;
        }
    }

    /// Sync CRI metadata for sandboxes
    async fn sync_cri_metadata(&self, sandbox_list: &mut Vec<String>) {
        debug!(sandboxes = ?sandbox_list, "retrieve pods metadata from the container manager");

        match super::cri::sync_sandboxes(
            &self.runtime_endpoint,
            &self.sandbox_cache,
            sandbox_list.clone(),
        )
        .await
        {
            Ok(remaining) => {
                *sandbox_list = remaining;
                if !sandbox_list.is_empty() {
                    debug!(
                        remaining = sandbox_list.len(),
                        "sandboxes still missing metadata"
                    );
                }
            }
            Err(e) => {
                error!(error = %e, "failed to sync sandboxes");
            }
        }
    }

    /// Check filesystem for sandbox additions/deletions
    async fn check_filesystem_changes(&self, sandbox_dir: &str, sandbox_list: &mut Vec<String>) {
        use tokio::fs;

        if let Ok(mut dir) = fs::read_dir(sandbox_dir).await {
            let mut current_list = Vec::new();
            while let Ok(Some(entry)) = dir.next_entry().await {
                if let Some(name) = entry.file_name().to_str() {
                    current_list.push(name.to_string());
                }
            }

            // Check for new sandboxes
            for sandbox in &current_list {
                if !sandbox_list.contains(sandbox)
                    && !self
                        .sandbox_cache
                        .get_sandbox_list()
                        .await
                        .contains(sandbox)
                {
                    if self
                        .sandbox_cache
                        .put_if_not_exists(
                            sandbox,
                            super::sandbox_cache::SandboxCRIMetadata {
                                uid: String::new(),
                                name: String::new(),
                                namespace: String::new(),
                            },
                        )
                        .await
                    {
                        info!(sandbox = %sandbox, "sandbox cache: added pod");
                        sandbox_list.push(sandbox.clone());
                    }
                }
            }

            // Check for deleted sandboxes
            let mut to_remove = Vec::new();
            for sandbox in &*sandbox_list {
                if !current_list.contains(sandbox) {
                    if self.sandbox_cache.delete_if_exists(sandbox).await {
                        // Also remove metrics cache for deleted sandbox
                        self.metrics_cache.delete_metrics(sandbox).await;
                        info!(sandbox = %sandbox, "sandbox cache: removed pod and cleared metrics");
                        to_remove.push(sandbox.clone());
                    }
                }
            }
            for sandbox in to_remove {
                sandbox_list.retain(|x| x != &sandbox);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sandbox_cache_manager_creation() {
        let sandbox_cache = Arc::new(SandboxCache::new());
        let metrics_cache = Arc::new(MetricsCache::new());
        let manager = SandboxCacheManager::new(
            sandbox_cache,
            metrics_cache,
            "/run/containerd/containerd.sock".to_string(),
        );
        assert_eq!(manager.runtime_endpoint, "/run/containerd/containerd.sock");
    }
}
