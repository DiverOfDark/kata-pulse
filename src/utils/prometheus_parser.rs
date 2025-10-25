use crate::utils::metrics_converter::cadvisor::PrometheusFormat;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Represents a single Prometheus metric
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PrometheusMetric {
    /// Metric name
    pub name: String,
    /// Metric type (counter, gauge, histogram, summary, etc.)
    pub metric_type: Option<String>,
    /// Metric help/description
    pub help: Option<String>,
    /// Samples for this metric (value, labels)
    pub samples: Vec<MetricSample>,
}

/// Represents a single sample of a metric with its labels and value
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MetricSample {
    /// Full metric name with suffix (e.g., "requests_total", "duration_seconds_bucket")
    pub name: String,
    /// Label key-value pairs (e.g., {"method": "GET", "status": "200"})
    pub labels: HashMap<String, String>,
    /// Metric value
    pub value: f64,
    /// Timestamp (optional)
    pub timestamp: Option<i64>,
}

/// Parsed Prometheus metrics text format
#[derive(Clone, Debug)]
pub struct PrometheusMetrics {
    /// Metrics grouped by base name (mutable to support aggregation)
    pub metrics: std::collections::HashMap<String, PrometheusMetric>,
}

impl PrometheusMetrics {
    /// Create an empty metrics container
    pub fn new() -> Self {
        PrometheusMetrics {
            metrics: HashMap::new(),
        }
    }

    /// Get or create a metric entry
    fn get_or_create_metric(&mut self, base_name: String) -> &mut PrometheusMetric {
        self.metrics
            .entry(base_name.clone())
            .or_insert_with(|| PrometheusMetric {
                name: base_name,
                ..Default::default()
            })
    }

    /// Parse Prometheus text format metrics
    pub fn parse(content: &str) -> Result<Self> {
        let mut metrics = PrometheusMetrics::new();

        for line in content.lines() {
            let trimmed = line.trim();

            // Skip empty lines and other comments
            if trimmed.is_empty()
                || trimmed.starts_with('#')
                    && !trimmed.starts_with("# HELP ")
                    && !trimmed.starts_with("# TYPE ")
            {
                continue;
            }

            // Handle HELP lines
            if let Some((metric_name, help)) = parse_metadata_line(trimmed, "# HELP ") {
                let base_name = extract_base_metric_name(&metric_name);
                metrics.get_or_create_metric(base_name).help = Some(help);
                continue;
            }

            // Handle TYPE lines
            if let Some((metric_name, metric_type)) = parse_metadata_line(trimmed, "# TYPE ") {
                let base_name = extract_base_metric_name(&metric_name);
                metrics.get_or_create_metric(base_name).metric_type = Some(metric_type);
                continue;
            }

            // Parse sample line
            if let Ok(sample) = parse_metric_sample(trimmed) {
                let base_name = extract_base_metric_name(&sample.name);
                metrics.get_or_create_metric(base_name).samples.push(sample);
            }
        }

        Ok(metrics)
    }
}

/// Parse metadata line (HELP or TYPE)
/// Returns (metric_name, value) if successful
fn parse_metadata_line(line: &str, prefix: &str) -> Option<(String, String)> {
    line.strip_prefix(prefix).and_then(|rest| {
        rest.find(' ')
            .map(|idx| (rest[..idx].to_string(), rest[idx + 1..].to_string()))
    })
}

/// Parse a single metric sample line
/// Format: metric_name{label1="value1",label2="value2"} value [timestamp]
fn parse_metric_sample(line: &str) -> Result<MetricSample> {
    let (name, labels_str, rest) = if let Some(brace_start) = line.find('{') {
        // Has labels: extract up to }
        let brace_end = line
            .find('}')
            .ok_or_else(|| anyhow::anyhow!("Missing closing brace in metric line: {}", line))?;
        let metric_name = line[..brace_start].to_string();
        let labels_str = &line[brace_start + 1..brace_end];
        let rest = line[brace_end + 1..].trim();
        (metric_name, Some(labels_str), rest)
    } else {
        // No labels: split on first space
        let (metric_name, rest) = line
            .split_once(' ')
            .ok_or_else(|| anyhow::anyhow!("Invalid metric format: {}", line))?;
        (metric_name.to_string(), None, rest.trim())
    };

    // Parse value and optional timestamp
    let mut parts = rest.split_whitespace();
    let value = parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("Missing value in metric line: {}", line))?
        .parse::<f64>()?;

    let timestamp = parts.next().and_then(|ts| ts.parse::<i64>().ok());

    // Parse labels
    let labels = if let Some(labels_str) = labels_str {
        parse_labels(labels_str)?
    } else {
        HashMap::new()
    };

    Ok(MetricSample {
        name,
        labels,
        value,
        timestamp,
    })
}

/// Parse label pairs from a label string
/// Format: label1="value1",label2="value2"
fn parse_labels(labels_str: &str) -> Result<HashMap<String, String>> {
    labels_str
        .split(',')
        .filter(|pair| !pair.is_empty())
        .try_fold(HashMap::new(), |mut acc, pair| {
            let pair = pair.trim();
            let (key, val) = pair
                .split_once('=')
                .ok_or_else(|| anyhow::anyhow!("Invalid label pair: {}", pair))?;

            let key = key.trim().to_string();
            let val = val
                .trim_matches('"')
                .replace("\\\"", "\"")
                .replace("\\n", "\n")
                .replace("\\t", "\t")
                .replace("\\\\", "\\");

            acc.insert(key, val);
            Ok(acc)
        })
}

/// Extract the base metric name from a full metric name (removing suffixes like _total, _count, _bucket, etc.)
fn extract_base_metric_name(full_name: &str) -> String {
    // Common Prometheus suffixes
    for suffix in &["_total", "_count", "_sum", "_bucket", "_info", "_created"] {
        if let Some(base) = full_name.strip_suffix(suffix) {
            return base.to_string();
        }
    }
    full_name.to_string()
}

/// Escape label values for Prometheus format (internal helper)
fn escape_label_value(value: &str) -> String {
    let mut result = String::new();
    for ch in value.chars() {
        match ch {
            '\\' => result.push_str("\\\\"),
            '"' => result.push_str("\\\""),
            '\n' => result.push_str("\\n"),
            _ => result.push(ch),
        }
    }
    result
}

impl PrometheusFormat for PrometheusMetrics {
    /// Convert parsed Prometheus metrics back to text format
    fn to_prometheus_format(&self, _sandbox_id: Option<&str>) -> String {
        let mut output = String::new();

        for metric in self.metrics.values() {
            // Write HELP line if available
            if let Some(help) = &metric.help {
                output.push_str(&format!("# HELP {} {}\n", metric.name, help));
            }

            // Write TYPE line if available
            if let Some(metric_type) = &metric.metric_type {
                output.push_str(&format!("# TYPE {} {}\n", metric.name, metric_type));
            }

            // Write samples
            for sample in &metric.samples {
                output.push_str(&sample.name);

                // Write labels if present
                if !sample.labels.is_empty() {
                    output.push('{');
                    let mut first = true;
                    for (label_name, label_value) in &sample.labels {
                        if !first {
                            output.push(',');
                        }
                        first = false;
                        output.push_str(&format!(
                            "{}=\"{}\"",
                            label_name,
                            escape_label_value(label_value)
                        ));
                    }
                    output.push('}');
                }

                // Write value
                output.push(' ');
                output.push_str(&sample.value.to_string());

                // Write timestamp if present
                if let Some(timestamp) = sample.timestamp {
                    output.push(' ');
                    output.push_str(&timestamp.to_string());
                }

                output.push('\n');
            }
        }

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_metric() {
        let content = r#"# HELP requests_total Total requests
# TYPE requests_total counter
requests_total 42
"#;
        let metrics = PrometheusMetrics::parse(content).unwrap();
        // Metrics are stored by base name (without _total suffix)
        assert!(metrics.metrics.contains_key("requests"));
        let metric = metrics.metrics.get("requests").unwrap();
        assert_eq!(metric.samples.len(), 1);
        assert_eq!(metric.samples[0].value, 42.0);
    }

    #[test]
    fn test_parse_metric_with_labels() {
        let content = r#"# HELP http_requests_total Total HTTP requests
# TYPE http_requests_total counter
http_requests_total{method="GET",status="200"} 100
http_requests_total{method="POST",status="201"} 10
"#;
        let metrics = PrometheusMetrics::parse(content).unwrap();
        // Metrics are stored by base name (without _total suffix)
        let metric = metrics.metrics.get("http_requests").unwrap();
        assert_eq!(metric.samples.len(), 2);
        assert_eq!(metric.samples[0].labels.get("method").unwrap(), "GET");
        assert_eq!(metric.samples[1].labels.get("status").unwrap(), "201");
    }

    #[test]
    fn test_parse_histogram() {
        let content = r#"# HELP request_duration_seconds Request duration
# TYPE request_duration_seconds histogram
request_duration_seconds_bucket{le="0.1"} 10
request_duration_seconds_bucket{le="1.0"} 50
request_duration_seconds_sum 123.45
request_duration_seconds_count 60
"#;
        let metrics = PrometheusMetrics::parse(content).unwrap();
        // Base name is extracted from the full metric names
        let metric = metrics.metrics.get("request_duration_seconds").unwrap();
        assert_eq!(metric.samples.len(), 4);
    }

    #[test]
    fn test_parse_with_timestamp() {
        let content = "request_total{path=\"/api\"} 42 1234567890";
        let sample = parse_metric_sample(content).unwrap();
        assert_eq!(sample.value, 42.0);
        assert_eq!(sample.timestamp, Some(1234567890));
        assert_eq!(sample.labels.get("path").unwrap(), "/api");
    }

    #[test]
    fn test_prometheus_metrics_to_format() {
        let content = r#"# HELP requests_total Total requests
# TYPE requests_total counter
requests_total 42
"#;
        let metrics = PrometheusMetrics::parse(content).unwrap();
        let output = metrics.to_prometheus_format(None);

        // Verify the output contains expected elements
        assert!(output.contains("# HELP requests Total requests"));
        assert!(output.contains("# TYPE requests counter"));
        assert!(output.contains("requests_total 42"));
    }

    #[test]
    fn test_prometheus_metrics_with_labels_to_format() {
        let content = r#"# HELP http_requests_total Total HTTP requests
# TYPE http_requests_total counter
http_requests_total{method="GET",status="200"} 100
http_requests_total{method="POST",status="201"} 10
"#;
        let metrics = PrometheusMetrics::parse(content).unwrap();
        let output = metrics.to_prometheus_format(None);

        // Verify output contains expected elements
        assert!(output.contains("http_requests_total"));
        assert!(output.contains("method=\"GET\""));
        assert!(output.contains("status=\"200\""));
        assert!(output.contains("100"));
        assert!(output.contains("method=\"POST\""));
        assert!(output.contains("10"));
    }

    #[test]
    fn test_prometheus_metrics_roundtrip() {
        // Test that we can parse and convert back to format
        let content = r#"# HELP test_metric Test metric
# TYPE test_metric gauge
test_metric{label1="value1",label2="value2"} 123.45 1234567890
test_metric{label1="value3"} 456.78
"#;
        let metrics = PrometheusMetrics::parse(content).unwrap();
        let output = metrics.to_prometheus_format(None);

        // Verify roundtrip
        assert!(output.contains("test_metric"));
        assert!(output.contains("label1=\"value1\""));
        assert!(output.contains("123.45"));
        assert!(output.contains("1234567890"));
        assert!(output.contains("456.78"));
    }
}
