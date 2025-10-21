//! CRI gRPC Client
//!
//! Provides a high-level interface for communicating with Kubernetes
//! container runtimes (containerd, CRI-O) via the CRI API.
//! Uses k8s-cri for official Kubernetes CRI proto types.

use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use containerd_client::tonic::transport::Channel;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

// Re-export k8s-cri proto definitions
pub use k8s_cri::v1 as runtime;
use k8s_cri::v1::runtime_service_client::RuntimeServiceClient;

/// Configuration for the CRI client
#[derive(Clone, Debug)]
pub struct CRIClientConfig {
    /// Path to the CRI runtime socket
    pub endpoint: String,

    /// Connection timeout
    pub timeout: Duration,

    /// Maximum number of retries for transient failures
    pub max_retries: u32,

    /// Retry backoff duration
    pub retry_backoff: Duration,
}

impl Default for CRIClientConfig {
    fn default() -> Self {
        CRIClientConfig {
            endpoint: "/run/containerd/containerd.sock".to_string(),
            timeout: Duration::from_secs(10),
            max_retries: 3,
            retry_backoff: Duration::from_millis(100),
        }
    }
}

impl CRIClientConfig {
    /// Create a new config with endpoint
    pub fn with_endpoint(endpoint: impl Into<String>) -> Self {
        CRIClientConfig {
            endpoint: endpoint.into(),
            ..Default::default()
        }
    }

    /// Set the connection timeout
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set the maximum retries
    pub fn with_max_retries(mut self, retries: u32) -> Self {
        self.max_retries = retries;
        self
    }
}

/// CRI Runtime Service Client
///
/// Provides methods for interacting with Kubernetes container runtimes
/// via the CRI API over Unix sockets.
pub struct CRIClient {
    config: CRIClientConfig,
    // Channel is kept in Arc<Mutex<>> for shared access across async tasks
    channel: Arc<Mutex<Option<Channel>>>,
}

impl CRIClient {
    /// Create a new CRI client with the given configuration
    pub fn new(config: CRIClientConfig) -> Self {
        CRIClient {
            config,
            channel: Arc::new(Mutex::new(None)),
        }
    }

    /// Connect to the CRI endpoint
    pub async fn connect(&mut self) -> Result<()> {
        debug!(
            endpoint = %self.config.endpoint,
            timeout_ms = self.config.timeout.as_millis(),
            "Connecting to CRI endpoint"
        );

        debug!(path = %self.config.endpoint, "Creating gRPC channel for Unix socket");

        // Extract the actual socket path from the endpoint
        // Handle both "unix:///path" and "/path" formats
        let socket_path_str = if self.config.endpoint.starts_with("unix://") {
            self.config
                .endpoint
                .strip_prefix("unix://")
                .unwrap_or(&self.config.endpoint)
                .to_string()
        } else {
            self.config.endpoint.clone()
        };

        debug!(
            original_endpoint = %self.config.endpoint,
            socket_path = %socket_path_str,
            "Attempting Unix socket connection"
        );

        // Use containerd_client but with JUST the socket path (no unix:// prefix)
        // containerd_client::connect expects raw filesystem paths for Unix sockets
        let connect_path = socket_path_str.clone();

        debug!(
            socket_path = %connect_path,
            "Using containerd_client::connect with direct path"
        );

        // Connect using containerd_client with just the path
        // It internally handles the unix:// URL construction
        match tokio::time::timeout(
            self.config.timeout,
            containerd_client::connect(&connect_path),
        )
        .await
        {
            Ok(Ok(channel)) => {
                info!(
                    path = %self.config.endpoint,
                    "Successfully created gRPC channel to containerd"
                );

                let mut stored_channel = self.channel.lock().await;
                *stored_channel = Some(channel);

                info!(endpoint = %self.config.endpoint, "Successfully connected to CRI");
                Ok(())
            }
            Ok(Err(e)) => {
                warn!(
                    endpoint = %self.config.endpoint,
                    socket_path = %connect_path,
                    error = %e,
                    "gRPC channel creation failed"
                );

                // Additional diagnostic: try to understand the error
                if e.to_string().contains("permission") || e.to_string().contains("Permission") {
                    warn!("ERROR appears to be permission-related - verify socket permissions and container privileges");
                } else if e.to_string().contains("connection")
                    || e.to_string().contains("Connection")
                {
                    warn!("ERROR appears to be connection-related - verify socket exists and is accessible");
                } else if e.to_string().contains("timeout") {
                    warn!("ERROR appears to be timeout-related - containerd may be slow or hung");
                } else if e.to_string().contains("ENOENT") || e.to_string().contains("No such file")
                {
                    warn!("ERROR is file-not-found - socket path may be incorrect or socket not created yet");
                }

                Err(anyhow!(
                    "Failed to connect to containerd socket at {}: {}. \
                     Possible causes: (1) containerd not running, (2) socket permissions (run as root?), \
                     (3) SELinux policies, (4) network namespace mismatch, (5) socket is not a valid Unix socket",
                    &self.config.endpoint,
                    e
                ))
            }
            Err(_) => {
                warn!(
                    endpoint = %self.config.endpoint,
                    timeout_secs = self.config.timeout.as_secs(),
                    "Connection timeout - containerd is not responding within timeout period"
                );
                Err(anyhow!(
                    "Timeout connecting to containerd socket at {} ({}s). \
                     Possible causes: (1) containerd service is slow/hung, (2) gRPC serialization overhead, \
                     (3) high system load",
                    &self.config.endpoint,
                    self.config.timeout.as_secs()
                ))
            }
        }
    }

    /// Get the stored channel
    async fn get_channel(&self) -> Result<Channel> {
        let channel = self.channel.lock().await;
        channel
            .clone()
            .ok_or_else(|| anyhow!("CRI client not connected. Call connect() first."))
    }

    /// List pod sandboxes with retry logic
    pub async fn list_pod_sandboxes(&self) -> Result<Vec<runtime::PodSandbox>> {
        self.list_pod_sandboxes_with_filter(None).await
    }

    /// List pod sandboxes with optional filter and retry logic
    pub async fn list_pod_sandboxes_with_filter(
        &self,
        filter: Option<runtime::PodSandboxFilter>,
    ) -> Result<Vec<runtime::PodSandbox>> {
        let mut last_error = None;

        for attempt in 0..=self.config.max_retries {
            match self.list_pod_sandboxes_internal(filter.clone()).await {
                Ok(pods) => {
                    debug!(
                        pod_count = pods.len(),
                        "Successfully retrieved pod sandboxes"
                    );
                    return Ok(pods);
                }
                Err(e) => {
                    last_error = Some(e);
                    if attempt < self.config.max_retries {
                        warn!(
                            attempt = attempt + 1,
                            max_retries = self.config.max_retries,
                            backoff_ms = self.config.retry_backoff.as_millis(),
                            "Failed to list pod sandboxes, retrying..."
                        );
                        tokio::time::sleep(self.config.retry_backoff).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            anyhow!(
                "Failed to list pod sandboxes after {} retries",
                self.config.max_retries
            )
        }))
    }

    /// Internal implementation of list_pod_sandboxes
    async fn list_pod_sandboxes_internal(
        &self,
        filter: Option<runtime::PodSandboxFilter>,
    ) -> Result<Vec<runtime::PodSandbox>> {
        debug!("Sending ListPodSandbox request to CRI");

        let channel = self.get_channel().await?;
        let mut client = RuntimeServiceClient::new(channel);

        let request = runtime::ListPodSandboxRequest { filter };
        let response = client
            .list_pod_sandbox(request)
            .await
            .map_err(|e| anyhow!("ListPodSandbox RPC failed: {}", e))?;

        Ok(response.into_inner().items)
    }
}

impl Clone for CRIClient {
    fn clone(&self) -> Self {
        CRIClient {
            config: self.config.clone(),
            channel: Arc::clone(&self.channel),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cri_client_config_default() {
        let config = CRIClientConfig::default();
        assert_eq!(config.endpoint, "/run/containerd/containerd.sock");
        assert_eq!(config.timeout, Duration::from_secs(10));
        assert_eq!(config.max_retries, 3);
    }

    #[test]
    fn test_cri_client_config_with_endpoint() {
        let config = CRIClientConfig::with_endpoint("/run/crio/crio.sock");
        assert_eq!(config.endpoint, "/run/crio/crio.sock");
    }

    #[test]
    fn test_cri_client_config_builder() {
        let config = CRIClientConfig::with_endpoint("/tmp/test.sock")
            .with_timeout(Duration::from_secs(20))
            .with_max_retries(5);

        assert_eq!(config.endpoint, "/tmp/test.sock");
        assert_eq!(config.timeout, Duration::from_secs(20));
        assert_eq!(config.max_retries, 5);
    }

    #[test]
    fn test_cri_client_creation() {
        let config = CRIClientConfig::default();
        let _client = CRIClient::new(config);
        // Just verify it creates without panicking
    }

    #[test]
    fn test_cri_client_clone() {
        let config = CRIClientConfig::default();
        let client1 = CRIClient::new(config);
        let client2 = client1.clone();

        assert_eq!(client1.config.endpoint, client2.config.endpoint);
    }

    #[test]
    fn test_cri_client_config_retry_backoff() {
        let config = CRIClientConfig::default();
        assert_eq!(config.retry_backoff, Duration::from_millis(100));
    }
}
