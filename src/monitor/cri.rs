use anyhow::Result;
use std::time::Duration;
use tracing::{debug, error, info, warn};

pub use super::cri_client::{CRIClient, CRIClientConfig};
use crate::monitor::sandbox_cache::{SandboxCRIMetadata, SandboxCache};

// Re-export proto definitions from cri_client
#[allow(unused_imports)]
pub mod runtime {
    pub use crate::monitor::cri_client::runtime::*;
}

/// Global CRI client instance for reuse across monitor operations
static CRI_CLIENT: once_cell::sync::OnceCell<CRIClient> = once_cell::sync::OnceCell::new();

/// Initialize the CRI client with the given endpoint
pub fn init_cri_client(endpoint: impl Into<String>) -> Result<CRIClient> {
    let config = CRIClientConfig::with_endpoint(endpoint)
        .with_timeout(Duration::from_secs(10))
        .with_max_retries(3);

    let client = CRIClient::new(config);
    Ok(client)
}

/// Get the global CRI client instance
pub fn get_cri_client() -> Option<&'static CRIClient> {
    CRI_CLIENT.get()
}

/// Set the global CRI client instance
pub fn set_cri_client(client: CRIClient) -> Result<()> {
    CRI_CLIENT
        .set(client)
        .map_err(|_| anyhow::anyhow!("CRI client already initialized"))
}

/// Sync sandboxes with CRI runtime metadata
///
/// Attempts to connect to the CRI endpoint and retrieve pod metadata
/// for all known sandboxes. This enriches our sandbox cache with
/// Kubernetes pod information (name, namespace, UID).
pub async fn sync_sandboxes(
    endpoint: &str,
    cache: &SandboxCache,
    mut sandbox_list: Vec<String>,
) -> Result<Vec<String>> {
    debug!(
        endpoint = %endpoint,
        sandbox_count = sandbox_list.len(),
        "Starting CRI sandbox metadata sync"
    );

    // Create or get the CRI client
    let client = match get_cri_client() {
        Some(c) => c.clone(),
        None => {
            let mut c = init_cri_client(endpoint)?;

            // Try to connect - if it fails, we'll return the sandbox list as-is
            match c.connect().await {
                Ok(_) => {
                    set_cri_client(c.clone())?;
                    c
                }
                Err(e) => {
                    warn!(
                        error = %e,
                        "Failed to connect to CRI endpoint, skipping metadata sync"
                    );
                    return Ok(sandbox_list);
                }
            }
        }
    };

    // Try to retrieve pod list from CRI
    let pods = match client.list_pod_sandboxes().await {
        Ok(pods) => pods,
        Err(e) => {
            error!(error = %e, "Failed to retrieve pod sandboxes from CRI");
            // Return original list - we'll try again next cycle
            return Ok(sandbox_list);
        }
    };

    debug!(pod_count = pods.len(), "Retrieved pods from CRI");

    // Match pods to our known sandboxes and extract metadata
    for pod in pods {
        if let Some(pos) = sandbox_list.iter().position(|s| pod.id == *s) {
            let sandbox_id = sandbox_list[pos].clone();
            let metadata = pod
                .metadata
                .as_ref()
                .map(|m| SandboxCRIMetadata {
                    uid: m.uid.clone(),
                    name: m.name.clone(),
                    namespace: m.namespace.clone(),
                })
                .unwrap_or_else(|| SandboxCRIMetadata {
                    uid: String::new(),
                    name: String::new(),
                    namespace: String::new(),
                });

            cache.set_cri_metadata(&sandbox_id, metadata).await;

            // Remove from the list of unsync'd sandboxes
            sandbox_list.remove(pos);

            info!(
                sandbox_id = %sandbox_id,
                pod_name = %pod.metadata.as_ref().map(|m| &m.name).unwrap_or(&"unknown".to_string()),
                pod_namespace = %pod.metadata.as_ref().map(|m| &m.namespace).unwrap_or(&"unknown".to_string()),
                "Synced KATA POD metadata from CRI"
            );
        }
    }

    debug!(
        remaining = sandbox_list.len(),
        "CRI sandbox metadata sync completed"
    );

    Ok(sandbox_list)
}
