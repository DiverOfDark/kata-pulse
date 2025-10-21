use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer};
use tracing::{debug, info, warn};

use crate::context::AppContext;
use crate::utils::metrics_converter::cadvisor::PrometheusFormat;
use crate::utils::metrics_converter::ConversionConfig;

/// Index page handler
async fn index_page(req: HttpRequest, _ctx: web::Data<AppContext>) -> HttpResponse {
    info!(method = %req.method(), path = %req.path(), "Index page request received");
    let html_accepted = is_html_request(&req);
    debug!(accept_html = html_accepted, "Determined response format");

    if html_accepted {
        info!("Responding with HTML format");
        index_page_html()
    } else {
        info!("Responding with text format");
        index_page_text()
    }
}

/// Text version of index page
fn index_page_text() -> HttpResponse {
    let endpoints = vec![
        ("/metrics", "Get metrics from sandboxes"),
        ("/sandboxes", "List all Kata Containers sandboxes"),
    ];

    let mut body = String::from("Available HTTP endpoints:\n");
    let max_len = endpoints.iter().map(|(p, _)| p.len()).max().unwrap_or(0) + 3;

    for (path, desc) in endpoints {
        body.push_str(&format!("{:<width$}: {}\n", path, desc, width = max_len));
    }

    HttpResponse::Ok()
        .content_type("text/plain; charset=utf-8")
        .body(body)
}

/// HTML version of index page
fn index_page_html() -> HttpResponse {
    let html = r#"
        <h1>Available HTTP endpoints:</h1>
        <ul>
            <li><b><a href='/metrics'>/metrics</a></b>: Get metrics from sandboxes</li>
            <li><b><a href='/sandboxes'>/sandboxes</a></b>: List all Kata Containers sandboxes</li>
        </ul>
    "#;

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html)
}

/// Metrics endpoint handler
async fn metrics_handler(ctx: web::Data<AppContext>, req: HttpRequest) -> HttpResponse {
    let query_string = req.query_string().to_string();

    info!(method = %req.method(), path = %req.path(), query = %query_string, "Metrics request received");

    debug!("Processing metrics request");

    // Check if specific sandbox requested
    if let Some(sandbox_id) = req.query_string().split('=').nth(1) {
        info!(sandbox_id = %sandbox_id, "Fetching metrics for specific sandbox");
        let metrics_cache = ctx.metrics_cache();
        debug!("Acquired metrics_cache, calling get_metrics");

        match metrics_cache.get_metrics(sandbox_id).await {
            Some(cached_metrics) => {
                info!(sandbox_id = %sandbox_id, "Found cached metrics for sandbox");

                // Convert to cAdvisor format with CRI enrichment
                debug!(sandbox_id = %sandbox_id, "Converting to cAdvisor metrics format with CRI enrichment");
                let config = ConversionConfig::default();
                let cri_enricher = ctx.cri_enricher().clone();
                let converter = crate::utils::metrics_converter::create_converter(
                    config,
                    cri_enricher,
                    sandbox_id.to_string(),
                );

                return match converter.convert_all(&cached_metrics.metrics) {
                    Ok(cadvisor_metrics) => {
                        debug!(sandbox_id = %sandbox_id, "Successfully converted to cAdvisor format");
                        let cadvisor_output =
                            cadvisor_metrics.to_prometheus_format(Some(sandbox_id));

                        info!(sandbox_id = %sandbox_id, output_size = cadvisor_output.len(), "Returning converted metrics");

                        HttpResponse::Ok()
                            .content_type("text/plain; charset=utf-8")
                            .body(cadvisor_output)
                    }
                    Err(e) => {
                        warn!(sandbox_id = %sandbox_id, error = %e, "Failed to convert metrics to cAdvisor format, falling back to raw format");
                        // Fall back to raw Prometheus format on conversion error
                        let prometheus_output =
                            (&cached_metrics.metrics).to_prometheus_format(None);
                        info!(sandbox_id = %sandbox_id, output_size = prometheus_output.len(), "Returning fallback raw metrics");

                        HttpResponse::Ok()
                            .content_type("text/plain; charset=utf-8")
                            .body(prometheus_output)
                    }
                };
            }
            None => {
                warn!(sandbox_id = %sandbox_id, "No cached metrics found for sandbox");
                return HttpResponse::NotFound()
                    .content_type("text/plain; charset=utf-8")
                    .body(format!(
                        "No cached metrics found for sandbox: {}",
                        sandbox_id
                    ));
            }
        }
    }

    // Return aggregated metrics from all sandboxes
    info!("Fetching aggregated metrics from all sandboxes");
    let sandbox_cache = ctx.sandbox_cache();
    let metrics_cache = ctx.metrics_cache();

    debug!("Acquiring sandbox list");
    let sandboxes = sandbox_cache.get_sandbox_list().await;
    info!(sandbox_count = sandboxes.len(), "Retrieved sandbox list");

    if sandboxes.is_empty() {
        info!("No sandboxes running, returning empty response");
        return HttpResponse::Ok()
            .content_type("text/plain; charset=utf-8")
            .body("# No sandboxes running\n");
    }

    // Get cached metrics from all sandboxes and convert to cAdvisor format
    debug!("Collecting metrics from {} sandboxes", sandboxes.len());
    let mut output = String::new();
    let config = ConversionConfig::default();
    let cri_enricher = ctx.cri_enricher().clone();

    for sandbox_id in &sandboxes {
        debug!(sandbox_id = %sandbox_id, "Fetching cached metrics for sandbox");
        if let Some(cached_metrics) = metrics_cache.get_metrics(sandbox_id).await {
            debug!(sandbox_id = %sandbox_id, "Found metrics for sandbox, converting to cAdvisor format with CRI enrichment");
            // Add sandbox comment to output
            output.push_str(&format!("# Metrics from sandbox: {}\n", sandbox_id));

            // Create converter with enricher for this sandbox
            let converter = crate::utils::metrics_converter::create_converter(
                config.clone(),
                cri_enricher.clone(),
                sandbox_id.clone(),
            );

            // Try to convert to cAdvisor format, fall back to raw format if conversion fails
            match converter.convert_all(&cached_metrics.metrics) {
                Ok(cadvisor_metrics) => {
                    debug!(sandbox_id = %sandbox_id, "Successfully converted to cAdvisor format");
                    output.push_str(&cadvisor_metrics.to_prometheus_format(Some(sandbox_id)));
                }
                Err(e) => {
                    warn!(sandbox_id = %sandbox_id, error = %e, "Failed to convert metrics, falling back to raw format");
                    let metrics = &cached_metrics.metrics;
                    output.push_str(&metrics.to_prometheus_format(None));
                }
            }
            output.push('\n');
            debug!(sandbox_id = %sandbox_id, output_size = output.len(), "Added metrics to output");
        } else {
            warn!(sandbox_id = %sandbox_id, "No cached metrics available for sandbox");
        }
    }

    if output.is_empty() {
        warn!(
            "Output is empty after collecting metrics from {} sandboxes",
            sandboxes.len()
        );
        HttpResponse::InternalServerError()
            .content_type("text/plain; charset=utf-8")
            .body("No cached metrics available")
    } else {
        info!(output_size = output.len(), "Returning aggregated metrics");
        HttpResponse::Ok()
            .content_type("text/plain; charset=utf-8")
            .body(output)
    }
}

/// Sandboxes listing handler
async fn sandboxes_handler(ctx: web::Data<AppContext>, req: HttpRequest) -> HttpResponse {
    info!(method = %req.method(), path = %req.path(), "Sandboxes listing request received");
    let cache = ctx.sandbox_cache();
    debug!("Acquiring sandbox cache");
    let sandboxes = cache.get_sandboxes_with_metadata().await;
    info!(
        sandbox_count = sandboxes.len(),
        "Retrieved sandboxes with metadata"
    );

    let html_accepted = is_html_request(&req);
    debug!(accept_html = html_accepted, "Determined response format");

    if html_accepted {
        info!("Responding with HTML format");
        sandboxes_html(&sandboxes)
    } else {
        info!("Responding with text format");
        sandboxes_text(&sandboxes)
    }
}

/// Text version of sandboxes listing
fn sandboxes_text(
    sandboxes: &[(String, crate::monitor::sandbox_cache::SandboxCRIMetadata)],
) -> HttpResponse {
    let mut body = String::new();

    for (id, metadata) in sandboxes {
        body.push_str(&format!("ID: {}\n", id));
        body.push_str(&format!(
            "  UID: {}\n",
            if metadata.uid.is_empty() {
                "<unknown>"
            } else {
                &metadata.uid
            }
        ));
        body.push_str(&format!(
            "  Name: {}\n",
            if metadata.name.is_empty() {
                "<unknown>"
            } else {
                &metadata.name
            }
        ));
        body.push_str(&format!(
            "  Namespace: {}\n",
            if metadata.namespace.is_empty() {
                "<unknown>"
            } else {
                &metadata.namespace
            }
        ));
        body.push('\n');
    }

    if body.is_empty() {
        body = "No sandboxes running\n".to_string();
    }

    HttpResponse::Ok()
        .content_type("text/plain; charset=utf-8")
        .body(body)
}

/// HTML version of sandboxes listing
fn sandboxes_html(
    sandboxes: &[(String, crate::monitor::sandbox_cache::SandboxCRIMetadata)],
) -> HttpResponse {
    let mut html = String::from("<h1>Sandbox list</h1>\n");

    if sandboxes.is_empty() {
        html.push_str("<p>No sandboxes running</p>\n");
    } else {
        html.push_str("<table border='1' style='border-collapse:collapse'>\n");
        html.push_str(
            "<tr><th>ID</th><th>UID</th><th>Name</th><th>Namespace</th><th>Actions</th></tr>\n",
        );

        for (id, metadata) in sandboxes {
            let uid_display = if metadata.uid.is_empty() {
                "<unknown>"
            } else {
                &metadata.uid
            };
            let name_display = if metadata.name.is_empty() {
                "<unknown>"
            } else {
                &metadata.name
            };
            let namespace_display = if metadata.namespace.is_empty() {
                "<unknown>"
            } else {
                &metadata.namespace
            };

            html.push_str(&format!(
                "<tr><td><code>{}</code></td><td>{}</td><td>{}</td><td>{}</td><td><a href='/metrics?sandbox={}'>metrics</a></td></tr>\n",
                id, uid_display, name_display, namespace_display, id
            ));
        }

        html.push_str("</table>\n");
    }

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html)
}

/// Check if request accepts HTML response
fn is_html_request(req: &HttpRequest) -> bool {
    if let Some(accept) = req.headers().get("accept") {
        if let Ok(accept_str) = accept.to_str() {
            return accept_str.contains("text/html");
        }
    }
    false
}

/// Start the HTTP server
pub async fn start_server(listen_addr: &str, app_ctx: AppContext) -> std::io::Result<()> {
    tracing::info!(address = listen_addr, workers = 8, "Starting HTTP server");

    let server = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(app_ctx.clone()))
            .route("/", web::get().to(index_page))
            .route("/metrics", web::get().to(metrics_handler))
            .route("/sandboxes", web::get().to(sandboxes_handler))
    })
    .workers(8)
    .bind(listen_addr)?;

    tracing::info!(
        address = listen_addr,
        "HTTP server bound successfully, running..."
    );
    server.run().await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::metrics_converter::cadvisor::{LoadAverage, StandardLabels};
    use crate::utils::metrics_converter::CadvisorMetrics;
    use std::collections::HashMap;

    #[test]
    fn test_cadvisor_metrics_to_prometheus_cpu_metrics() {
        let metrics = CadvisorMetrics {
            cpu: crate::utils::metrics_converter::CpuMetrics {
                usage_seconds_total: 100.5,
                user_seconds_total: 60.0,
                system_seconds_total: 40.5,
                load_average: Some(LoadAverage {
                    one_minute: 1.5,
                    five_minute: 1.2,
                    fifteen_minute: 1.0,
                }),
                per_cpu: Default::default(),
                standard_labels: StandardLabels {
                    container: "".to_string(),
                    id: "test-sandbox".to_string(),
                    image: "".to_string(),
                    name: "test-pod".to_string(),
                    namespace: "default".to_string(),
                    pod: "test-pod".to_string(),
                },
            },
            memory: Default::default(),
            network: Default::default(),
            disk: Default::default(),
            process: Default::default(),
        };

        let sandbox_id = Some("test-sandbox");
        let output = metrics.to_prometheus_format(sandbox_id);

        // Verify CPU metrics are in output
        assert!(output.contains("container_cpu_usage_seconds_total"));
        assert!(output.contains("100.5"));
        assert!(output.contains("container_cpu_user_seconds_total"));
        assert!(output.contains("60"));
        assert!(output.contains("container_cpu_system_seconds_total"));
        assert!(output.contains("40.5"));

        // Verify load average
        assert!(output.contains("container_load_average_1m"));
        assert!(output.contains("1.5"));
        assert!(output.contains("container_load_average_5m"));
        assert!(output.contains("1.2"));

        // Verify standard labels
        assert!(output.contains(r#"id="test-sandbox""#));
        assert!(output.contains(r#"name="test-pod""#));
        assert!(output.contains(r#"pod="test-pod""#));
        assert!(output.contains(r#"namespace="default""#));
    }

    #[test]
    fn test_cadvisor_metrics_to_prometheus_memory_metrics() {
        let metrics = CadvisorMetrics {
            cpu: Default::default(),
            memory: crate::utils::metrics_converter::MemoryMetrics {
                usage_bytes: 536870912,             // 512 MB
                working_set_bytes: Some(268435456), // 256 MB
                cache_bytes: Some(268435456),       // 256 MB
                rss_bytes: Some(268435456),         // 256 MB
                swap_bytes: Some(0),
                mapped_file_bytes: None,
                failures: HashMap::new(),
                standard_labels: StandardLabels::default(),
            },
            network: Default::default(),
            disk: Default::default(),
            process: Default::default(),
        };

        let output = metrics.to_prometheus_format(None);

        // Verify memory metrics
        assert!(output.contains("container_memory_usage_bytes"));
        assert!(output.contains("536870912"));
        assert!(output.contains("container_memory_working_set_bytes"));
        assert!(output.contains("268435456"));
        assert!(output.contains("container_memory_cache_bytes"));
        assert!(output.contains("container_memory_rss_bytes"));
        assert!(output.contains("container_memory_swap_bytes"));
    }

    #[test]
    fn test_cadvisor_metrics_to_prometheus_network_metrics() {
        let metrics = CadvisorMetrics {
            cpu: Default::default(),
            memory: Default::default(),
            network: crate::utils::metrics_converter::NetworkMetrics {
                receive_bytes_total: 1024000,
                transmit_bytes_total: 2048000,
                receive_packets_total: 10000,
                transmit_packets_total: 20000,
                receive_errors_total: Some(5),
                transmit_errors_total: Some(3),
                receive_packets_dropped_total: None,
                transmit_packets_dropped_total: None,
                per_interface: Default::default(),
                standard_labels: StandardLabels::default(),
            },
            disk: Default::default(),
            process: Default::default(),
        };

        let sandbox_id = Some("sandbox-1");
        let output = metrics.to_prometheus_format(sandbox_id);

        // Verify network metrics
        assert!(output.contains("container_network_receive_bytes_total"));
        assert!(output.contains("1024000"));
        assert!(output.contains("container_network_transmit_bytes_total"));
        assert!(output.contains("2048000"));
        assert!(output.contains("container_network_receive_packets_total"));
        assert!(output.contains("10000"));
        assert!(output.contains("container_network_transmit_packets_total"));
        assert!(output.contains("20000"));
        assert!(output.contains("container_network_receive_errors_total"));
        assert!(output.contains("5"));
    }

    #[test]
    fn test_cadvisor_metrics_to_prometheus_disk_metrics() {
        let metrics = CadvisorMetrics {
            cpu: Default::default(),
            memory: Default::default(),
            network: Default::default(),
            disk: crate::utils::metrics_converter::DiskMetrics {
                reads_total: 1000,
                writes_total: 2000,
                reads_bytes_total: 10485760,  // 10 MB
                writes_bytes_total: 20971520, // 20 MB
                read_seconds_total: 1.5,
                write_seconds_total: 2.5,
                io_time_seconds_total: Some(4.0),
                io_time_weighted_seconds_total: None,
                per_device: Default::default(),
                standard_labels: StandardLabels::default(),
            },
            process: Default::default(),
        };

        let output = metrics.to_prometheus_format(None);

        // Verify disk metrics
        assert!(output.contains("container_disk_io_reads_total"));
        assert!(output.contains("1000"));
        assert!(output.contains("container_disk_io_writes_total"));
        assert!(output.contains("2000"));
        assert!(output.contains("container_disk_io_read_bytes_total"));
        assert!(output.contains("10485760"));
        assert!(output.contains("container_disk_io_write_bytes_total"));
        assert!(output.contains("20971520"));
    }

    #[test]
    fn test_cadvisor_metrics_to_prometheus_process_metrics() {
        let metrics = CadvisorMetrics {
            cpu: Default::default(),
            memory: Default::default(),
            network: Default::default(),
            disk: Default::default(),
            process: crate::utils::metrics_converter::ProcessMetrics {
                count: 42,
                thread_count: 128,
                thread_count_max: Some(256),
                file_descriptors: 512,
                tasks_by_state: HashMap::new(),
                standard_labels: StandardLabels::default(),
            },
        };

        let sandbox_id = Some("test-pod");
        let output = metrics.to_prometheus_format(sandbox_id);

        // Verify process metrics
        assert!(output.contains("container_processes_count"));
        assert!(output.contains("42"));
        assert!(output.contains("container_threads_count"));
        assert!(output.contains("128"));
        assert!(output.contains("container_threads_max_count"));
        assert!(output.contains("256"));
        assert!(output.contains("container_file_descriptors"));
        assert!(output.contains("512"));
    }

    #[test]
    fn test_cadvisor_conversion_pipeline() {
        // This test verifies the end-to-end conversion from Prometheus format to CadvisorMetrics
        // and back to Prometheus format

        let metrics = CadvisorMetrics {
            cpu: crate::utils::metrics_converter::CpuMetrics {
                usage_seconds_total: 50.0,
                user_seconds_total: 30.0,
                system_seconds_total: 20.0,
                load_average: Some(LoadAverage {
                    one_minute: 2.0,
                    five_minute: 1.5,
                    fifteen_minute: 1.0,
                }),
                per_cpu: Default::default(),
                standard_labels: StandardLabels::default(),
            },
            memory: crate::utils::metrics_converter::MemoryMetrics {
                usage_bytes: 1073741824,            // 1 GB
                working_set_bytes: Some(536870912), // 512 MB
                cache_bytes: None,
                rss_bytes: None,
                swap_bytes: None,
                mapped_file_bytes: None,
                failures: HashMap::new(),
                standard_labels: StandardLabels::default(),
            },
            network: crate::utils::metrics_converter::NetworkMetrics {
                receive_bytes_total: 5000000,
                transmit_bytes_total: 3000000,
                receive_packets_total: 50000,
                transmit_packets_total: 30000,
                receive_errors_total: None,
                transmit_errors_total: None,
                receive_packets_dropped_total: None,
                transmit_packets_dropped_total: None,
                per_interface: Default::default(),
                standard_labels: StandardLabels::default(),
            },
            disk: crate::utils::metrics_converter::DiskMetrics {
                reads_total: 5000,
                writes_total: 8000,
                reads_bytes_total: 52428800,  // 50 MB
                writes_bytes_total: 83886080, // 80 MB
                read_seconds_total: 5.0,
                write_seconds_total: 8.0,
                io_time_seconds_total: None,
                io_time_weighted_seconds_total: None,
                per_device: Default::default(),
                standard_labels: StandardLabels::default(),
            },
            process: crate::utils::metrics_converter::ProcessMetrics {
                count: 25,
                thread_count: 64,
                thread_count_max: Some(512),
                file_descriptors: 256,
                tasks_by_state: HashMap::new(),
                standard_labels: StandardLabels::default(),
            },
        };

        let sandbox_id = Some("test-sandbox");
        let output = metrics.to_prometheus_format(sandbox_id);

        // Verify all major metric categories are present
        assert!(output.contains("container_cpu_usage_seconds_total"));
        assert!(output.contains("container_memory_usage_bytes"));
        assert!(output.contains("container_network_receive_bytes_total"));
        assert!(output.contains("container_disk_io_reads_total"));
        assert!(output.contains("container_processes_count"));

        // Verify HELP and TYPE lines are present
        assert!(output.contains("# HELP container_cpu_usage_seconds_total"));
        assert!(output.contains("# TYPE container_cpu_usage_seconds_total"));
    }
}
