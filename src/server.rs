use axum::{
    extract::Query,
    http::header::HeaderMap,
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use serde::Deserialize;
use std::sync::Arc;
use tracing::{debug, info, warn};

use crate::context::AppContext;
use crate::utils::metrics_converter::cadvisor::PrometheusFormat;
use crate::utils::metrics_converter::ConversionConfig;

/// Extract sandbox ID from query parameters
#[derive(Deserialize)]
pub struct SandboxQuery {
    sandbox: Option<String>,
}

/// Create the HTTP server router
pub fn create_router(app_context: Arc<AppContext>) -> Router {
    let app_context_clone1 = app_context.clone();
    let app_context_clone2 = app_context.clone();

    Router::new()
        .route("/", get(index_page))
        .route(
            "/metrics",
            get(move |Query(params): Query<SandboxQuery>| async move {
                let ctx = app_context_clone1.clone();
                metrics_handler(ctx, params).await
            }),
        )
        .route(
            "/sandboxes",
            get(move || async move { sandboxes_handler(app_context_clone2.clone()).await }),
        )
}

/// Index page handler
async fn index_page(headers: HeaderMap) -> impl IntoResponse {
    info!("Index page request received");
    let html = r#"<html>
    <head><title>Kata Pulse</title></head>
    <body>
    <h1>Available HTTP endpoints:</h1>
    <ul>
    <li><b><a href='/metrics'>/metrics</a></b>: Get metrics from sandboxes</li>
    <li><b><a href='/sandboxes'>/sandboxes</a></b>: List all Kata Containers sandboxes</li>
    </ul>
    </body>
    </html>"#;
    Html(html).into_response()
}

/// Text version of index page
/// Metrics endpoint handler
async fn metrics_handler(ctx: Arc<AppContext>, params: SandboxQuery) -> impl IntoResponse {
    info!("Metrics request received");

    debug!("Processing metrics request");

    // Check if specific sandbox requested
    if let Some(sandbox_id) = params.sandbox {
        info!(sandbox_id = %sandbox_id, "Fetching metrics for specific sandbox");
        let metrics_cache = ctx.metrics_cache();
        debug!("Acquired metrics_cache, calling get_metrics");

        match metrics_cache.get_metrics(&sandbox_id).await {
            Some(cached_metrics) => {
                info!(sandbox_id = %sandbox_id, "Found cached metrics for sandbox");

                // Convert to cAdvisor format with CRI enrichment
                debug!(sandbox_id = %sandbox_id, "Converting to cAdvisor metrics format with CRI enrichment");
                let config = ConversionConfig::default();
                let cri_enricher = ctx.cri_enricher().clone();
                let converter = crate::utils::metrics_converter::create_converter(
                    config,
                    cri_enricher,
                    sandbox_id.clone(),
                );

                // Try to convert to cAdvisor format, fall back to raw format if conversion fails
                match converter.convert_all(&cached_metrics.metrics) {
                    Ok(cadvisor_metrics) => {
                        debug!(sandbox_id = %sandbox_id, "Successfully converted to cAdvisor format");
                        let output = cadvisor_metrics.to_prometheus_format(Some(&sandbox_id));
                        info!(sandbox_id = %sandbox_id, output_size = output.len(), "Returning converted metrics");
                        return (
                            axum::http::StatusCode::OK,
                            [("Content-Type", "text/plain; charset=utf-8")],
                            output,
                        )
                            .into_response();
                    }
                    Err(e) => {
                        warn!(sandbox_id = %sandbox_id, error = %e, "Failed to convert metrics, falling back to raw format");
                        let output = cached_metrics.metrics.to_prometheus_format(None);
                        return (
                            axum::http::StatusCode::OK,
                            [("Content-Type", "text/plain; charset=utf-8")],
                            output,
                        )
                            .into_response();
                    }
                }
            }
            None => {
                warn!(sandbox_id = %sandbox_id, "No cached metrics available for sandbox");
                return (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    [("Content-Type", "text/plain; charset=utf-8")],
                    "No cached metrics available for this sandbox".to_string(),
                )
                    .into_response();
            }
        }
    }

    // Aggregate metrics from all sandboxes
    let cache = ctx.sandbox_cache();
    let sandboxes = cache.get_sandboxes_with_metadata().await;
    let metrics_cache = ctx.metrics_cache();

    let mut output = String::new();
    for (sandbox_id, _metadata) in &sandboxes {
        debug!(sandbox_id = %sandbox_id, "Processing metrics for sandbox");

        // Get metrics first (async operation)
        let metrics_opt = metrics_cache.get_metrics(sandbox_id).await;

        // Then process with converter (sync operation, no awaits)
        if let Some(cached_metrics) = metrics_opt {
            let config = ConversionConfig::default();
            let cri_enricher = ctx.cri_enricher().clone();
            let converter = crate::utils::metrics_converter::create_converter(
                config,
                cri_enricher,
                sandbox_id.clone(),
            );

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
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            [("Content-Type", "text/plain; charset=utf-8")],
            "No cached metrics available".to_string(),
        )
            .into_response()
    } else {
        info!(output_size = output.len(), "Returning aggregated metrics");
        (
            axum::http::StatusCode::OK,
            [("Content-Type", "text/plain; charset=utf-8")],
            output,
        )
            .into_response()
    }
}

/// Sandboxes listing handler
async fn sandboxes_handler(ctx: Arc<AppContext>) -> impl IntoResponse {
    info!("Sandboxes listing request received");
    let cache = ctx.sandbox_cache();
    debug!("Acquiring sandbox cache");
    let sandboxes = cache.get_sandboxes_with_metadata().await;
    info!(
        sandbox_count = sandboxes.len(),
        "Returning list of sandboxes"
    );

    let json_output = serde_json::to_string(&sandboxes).unwrap_or_else(|e| {
        warn!("Failed to serialize sandboxes: {}", e);
        "[]".to_string()
    });

    (
        axum::http::StatusCode::OK,
        [("Content-Type", "application/json; charset=utf-8")],
        json_output,
    )
        .into_response()
}

/// Start the HTTP server
pub async fn start_server(listen_address: &str, app_context: AppContext) -> anyhow::Result<()> {
    let app_context = Arc::new(app_context);
    let router = create_router(app_context);

    let listener = tokio::net::TcpListener::bind(listen_address).await?;
    info!("Server listening on {}", listen_address);

    axum::serve(listener, router).await?;

    Ok(())
}
