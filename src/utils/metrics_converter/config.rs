//! Configuration and label enrichment for metrics conversion

use std::sync::Arc;

/// Get the CLK_TCK value from the system (equivalent to `getconf CLK_TCK`)
///
/// This is used to convert jiffies from /proc/stat to seconds.
/// The value represents the number of clock ticks per second.
///
/// Priority:
/// 1. `KATA_PULSE_CLK_TCK` environment variable (if set and valid)
/// 2. `sysconf(_SC_CLK_TCK)` on Unix systems
/// 3. Fallback to 100 Hz (standard on most Linux systems)
///
/// # Returns
/// The CLK_TCK value as a f64
fn get_clk_tck() -> f64 {
    // First, try environment variable override
    if let Ok(env_value) = std::env::var("KATA_PULSE_CLK_TCK") {
        if let Ok(clk_tck) = env_value.parse::<f64>() {
            if clk_tck > 0.0 {
                tracing::info!(
                    clk_tck = clk_tck,
                    source = "KATA_PULSE_CLK_TCK environment variable",
                    "CPU jiffy conversion factor obtained from environment variable"
                );
                return clk_tck;
            } else {
                tracing::warn!(
                    value = env_value,
                    "KATA_PULSE_CLK_TCK must be positive, falling back to system detection"
                );
            }
        } else {
            tracing::warn!(
                value = env_value,
                "KATA_PULSE_CLK_TCK is not a valid number, falling back to system detection"
            );
        }
    }

    // Try using sysconf if available
    #[cfg(unix)]
    {
        // sysconf(_SC_CLK_TCK) - this is the standard POSIX way
        let clk_tck = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };

        if clk_tck > 0 {
            let clk_tck_f64 = clk_tck as f64;
            tracing::info!(
                clk_tck = clk_tck_f64,
                source = "sysconf(_SC_CLK_TCK)",
                "CPU jiffy conversion factor obtained from system"
            );
            return clk_tck_f64;
        }
    }

    // Fallback to 100 Hz (standard on most Linux systems)
    // This is defined by Linux kernel as USER_HZ
    let default_clk_tck = 100.0;
    tracing::info!(
        clk_tck = default_clk_tck,
        source = "hardcoded default",
        "CPU jiffy conversion factor using fallback default (sysconf unavailable or returned invalid value)"
    );
    default_clk_tck
}

/// Enriched labels from CRI metadata
///
/// Contains typed fields for Kubernetes pod metadata obtained from CRI.
#[derive(Debug, Clone, Default)]
pub struct EnrichedLabels {
    /// Kubernetes pod UID
    pub pod_uid: String,
    /// Kubernetes pod name
    pub pod_name: String,
    /// Kubernetes namespace
    pub pod_namespace: String,
}

impl EnrichedLabels {
    /// Create enriched labels with all fields
    pub fn new(
        pod_uid: impl Into<String>,
        pod_name: impl Into<String>,
        pod_namespace: impl Into<String>,
    ) -> Self {
        Self {
            pod_uid: pod_uid.into(),
            pod_name: pod_name.into(),
            pod_namespace: pod_namespace.into(),
        }
    }
}

/// Supported hypervisor types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HypervisorType {
    /// Cloud Hypervisor (current implementation)
    CloudHypervisor,
    // Future hypervisors:
    // Qemu,
    // Firecracker,
}

/// Configuration for metrics conversion
#[derive(Clone)]
pub struct ConversionConfig {
    /// Which hypervisor metrics we're converting from
    pub hypervisor_type: HypervisorType,

    /// Label enricher for adding Kubernetes metadata
    pub label_enricher: Option<Arc<dyn LabelEnricher>>,

    /// Whether to include per-CPU breakdown
    pub include_per_cpu: bool,

    /// Whether to include per-interface network details
    pub include_per_interface: bool,

    /// Whether to include per-device disk details
    pub include_per_device: bool,

    /// Network interface filter: only include these patterns
    /// Default: ["eth0", "veth.*", "tap.*", "tun.*"]
    pub network_interface_patterns: Vec<String>,

    /// CPU time conversion factor: jiffies to seconds
    /// jiffies from /proc/stat use USER_HZ (typically 100 Hz on Linux)
    pub cpu_jiffy_conversion_factor: f64,
}

impl Default for ConversionConfig {
    fn default() -> Self {
        Self {
            hypervisor_type: HypervisorType::CloudHypervisor,
            label_enricher: None,
            include_per_cpu: false,
            include_per_interface: false,
            include_per_device: false,
            network_interface_patterns: vec![
                "eth0".to_string(),
                "veth.*".to_string(),
                "tap.*".to_string(),
                "tun.*".to_string(),
            ],
            cpu_jiffy_conversion_factor: get_clk_tck(), // jiffies to seconds (obtained from system via sysconf)
        }
    }
}

impl std::fmt::Debug for ConversionConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConversionConfig")
            .field("hypervisor_type", &self.hypervisor_type)
            .field("label_enricher", &(self.label_enricher.is_some()))
            .field("include_per_cpu", &self.include_per_cpu)
            .field("include_per_interface", &self.include_per_interface)
            .field("include_per_device", &self.include_per_device)
            .field(
                "network_interface_patterns",
                &self.network_interface_patterns,
            )
            .field(
                "cpu_jiffy_conversion_factor",
                &self.cpu_jiffy_conversion_factor,
            )
            .finish()
    }
}

impl ConversionConfig {
    /// Check if an interface name matches the configured patterns
    pub fn matches_network_interface(&self, interface: &str) -> bool {
        self.network_interface_patterns.iter().any(|pattern| {
            if pattern.ends_with(".*") {
                // Simple glob-style matching: pattern.* matches interface starting with pattern
                let prefix = &pattern[..pattern.len() - 2];
                interface.starts_with(prefix)
            } else {
                // Exact match
                interface == pattern
            }
        })
    }
}

/// Trait for enriching metrics labels with Kubernetes metadata
///
/// This allows injecting pod name, namespace, and other K8s info
/// into the converted metrics by looking up CRI metadata.
pub trait LabelEnricher: Send + Sync {
    /// Enrich labels for a sandbox
    ///
    /// Given a sandbox ID, return enriched labels from CRI metadata.
    /// Returns empty EnrichedLabels if enrichment not available.
    fn enrich(&self, sandbox_id: &str) -> EnrichedLabels;
}

/// CRI-based label enricher that uses sandbox metadata
///
/// This enricher looks up pod name, namespace, and UID from the CRI sandbox cache
/// and enriches metrics labels with this information.
pub struct CRILabelEnricher {
    /// Reference to the sandbox cache for metadata lookup
    sandbox_cache: Arc<crate::monitor::sandbox_cache::SandboxCache>,
}

impl CRILabelEnricher {
    /// Create a new CRI-based label enricher
    pub fn new(sandbox_cache: Arc<crate::monitor::sandbox_cache::SandboxCache>) -> Self {
        CRILabelEnricher { sandbox_cache }
    }
}

impl LabelEnricher for CRILabelEnricher {
    fn enrich(&self, sandbox_id: &str) -> EnrichedLabels {
        // Try to get metadata from the sandbox cache (non-blocking)
        if let Some(metadata) = self.sandbox_cache.get_metadata_try(sandbox_id) {
            EnrichedLabels::new(metadata.uid, metadata.name, metadata.namespace)
        } else {
            EnrichedLabels::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ConversionConfig::default();
        assert_eq!(config.hypervisor_type, HypervisorType::CloudHypervisor);
        assert_eq!(config.cpu_jiffy_conversion_factor, 100.0);
    }

    #[test]
    fn test_interface_matching() {
        let config = ConversionConfig::default();

        // Should match
        assert!(config.matches_network_interface("eth0"));
        assert!(config.matches_network_interface("veth1234567890ab"));
        assert!(config.matches_network_interface("tap0"));
        assert!(config.matches_network_interface("tun1"));

        // Should not match
        assert!(!config.matches_network_interface("lo"));
        assert!(!config.matches_network_interface("docker0"));
        assert!(!config.matches_network_interface("br-abcdef"));
    }

    #[test]
    fn test_cri_label_enricher_with_metadata() {
        // Create a sandbox cache with test data
        let cache = Arc::new(crate::monitor::sandbox_cache::SandboxCache::new());

        // Synchronously set metadata using tokio runtime
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            cache
                .set_cri_metadata(
                    "sandbox-123",
                    crate::monitor::sandbox_cache::SandboxCRIMetadata {
                        uid: "uid-12345".to_string(),
                        name: "my-pod".to_string(),
                        namespace: "default".to_string(),
                    },
                )
                .await;
        });

        let enricher = CRILabelEnricher::new(cache);
        let labels = enricher.enrich("sandbox-123");

        // Verify all labels were enriched
        assert_eq!(labels.pod_name, "my-pod");
        assert_eq!(labels.pod_namespace, "default");
        assert_eq!(labels.pod_uid, "uid-12345");
    }

    #[test]
    fn test_cri_label_enricher_missing_metadata() {
        let cache = Arc::new(crate::monitor::sandbox_cache::SandboxCache::new());
        let enricher = CRILabelEnricher::new(cache);

        // Enrich for non-existent sandbox
        let labels = enricher.enrich("non-existent-sandbox");

        // Should return empty labels
        assert_eq!(labels.pod_uid, "");
        assert_eq!(labels.pod_name, "");
        assert_eq!(labels.pod_namespace, "");
    }

    #[test]
    fn test_cri_label_enricher_multiple_sandboxes() {
        let cache = Arc::new(crate::monitor::sandbox_cache::SandboxCache::new());

        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            // Add metadata for two sandboxes
            cache
                .set_cri_metadata(
                    "sandbox-1",
                    crate::monitor::sandbox_cache::SandboxCRIMetadata {
                        uid: "uid-1".to_string(),
                        name: "pod-1".to_string(),
                        namespace: "ns-1".to_string(),
                    },
                )
                .await;

            cache
                .set_cri_metadata(
                    "sandbox-2",
                    crate::monitor::sandbox_cache::SandboxCRIMetadata {
                        uid: "uid-2".to_string(),
                        name: "pod-2".to_string(),
                        namespace: "ns-2".to_string(),
                    },
                )
                .await;
        });

        let enricher = CRILabelEnricher::new(cache);

        // Check first sandbox
        let labels1 = enricher.enrich("sandbox-1");
        assert_eq!(labels1.pod_name, "pod-1");
        assert_eq!(labels1.pod_namespace, "ns-1");
        assert_eq!(labels1.pod_uid, "uid-1");

        // Check second sandbox
        let labels2 = enricher.enrich("sandbox-2");
        assert_eq!(labels2.pod_name, "pod-2");
        assert_eq!(labels2.pod_namespace, "ns-2");
        assert_eq!(labels2.pod_uid, "uid-2");
    }

    #[test]
    fn test_get_clk_tck_with_valid_env_override() {
        // Test that environment variable override works
        std::env::set_var("KATA_PULSE_CLK_TCK", "250");
        // Note: We can't directly test get_clk_tck() since it's private and called at config creation
        // But we can verify config creation respects environment
        let config = ConversionConfig::default();
        // The conversion factor should be 250 if the env var was picked up
        assert_eq!(config.cpu_jiffy_conversion_factor, 250.0);
        std::env::remove_var("KATA_PULSE_CLK_TCK");
    }

    #[test]
    fn test_get_clk_tck_with_invalid_env_override() {
        // Test that invalid env values fall back to system/default
        std::env::set_var("KATA_PULSE_CLK_TCK", "not_a_number");
        let config = ConversionConfig::default();
        // Should fall back to sysconf or default (100)
        assert!(config.cpu_jiffy_conversion_factor > 0.0);
        std::env::remove_var("KATA_PULSE_CLK_TCK");
    }

    #[test]
    fn test_get_clk_tck_with_negative_env_override() {
        // Test that negative env values are rejected
        std::env::set_var("KATA_PULSE_CLK_TCK", "-50");
        let config = ConversionConfig::default();
        // Should fall back to sysconf or default (100)
        assert!(config.cpu_jiffy_conversion_factor > 0.0);
        std::env::remove_var("KATA_PULSE_CLK_TCK");
    }
}
