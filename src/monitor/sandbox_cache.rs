use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone, Debug, Serialize)]
pub struct SandboxCRIMetadata {
    pub uid: String,
    pub name: String,
    pub namespace: String,
}

#[derive(Clone)]
pub struct SandboxCache {
    sandboxes: Arc<RwLock<HashMap<String, SandboxCRIMetadata>>>,
}

impl SandboxCache {
    /// Create a new sandbox cache
    pub fn new() -> Self {
        SandboxCache {
            sandboxes: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get list of all sandbox IDs
    pub async fn get_sandbox_list(&self) -> Vec<String> {
        let map = self.sandboxes.read().await;
        map.keys().cloned().collect()
    }

    /// Delete a sandbox if it exists
    /// Returns true if the sandbox was deleted, false if it didn't exist
    pub async fn delete_if_exists(&self, id: &str) -> bool {
        let mut map = self.sandboxes.write().await;
        map.remove(id).is_some()
    }

    /// Put a sandbox in the cache if it doesn't already exist
    /// Returns true if the sandbox was inserted, false if it already existed
    pub async fn put_if_not_exists(&self, id: &str, value: SandboxCRIMetadata) -> bool {
        let mut map = self.sandboxes.write().await;
        if map.contains_key(id) {
            false
        } else {
            map.insert(id.to_string(), value);
            true
        }
    }

    /// Set CRI metadata for a sandbox (inserts or updates)
    pub async fn set_cri_metadata(&self, id: &str, value: SandboxCRIMetadata) {
        let mut map = self.sandboxes.write().await;
        map.insert(id.to_string(), value);
    }

    /// Get all sandboxes with their CRI metadata
    pub async fn get_sandboxes_with_metadata(&self) -> Vec<(String, SandboxCRIMetadata)> {
        let map = self.sandboxes.read().await;
        map.iter()
            .map(|(id, metadata)| (id.clone(), metadata.clone()))
            .collect()
    }

    /// Get CRI metadata for a specific sandbox (blocking variant)
    ///
    /// This variant tries to get the metadata without blocking for long.
    /// If the lock is already held, it returns None to avoid deadlocks.
    /// This is safe to call from sync contexts like the LabelEnricher.
    pub fn get_metadata_try(&self, id: &str) -> Option<SandboxCRIMetadata> {
        // Try to acquire the read lock without blocking
        // This is safe because we're just reading, and RwLock allows multiple readers
        match self.sandboxes.try_read() {
            Ok(map) => map.get(id).cloned(),
            // If the lock is busy (write in progress), we gracefully return None
            // rather than blocking or panicking
            Err(_) => None,
        }
    }
}

impl Default for SandboxCache {
    fn default() -> Self {
        Self::new()
    }
}
