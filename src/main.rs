mod config;
mod context;
mod monitor;
mod server;
mod utils;

use anyhow::Result;
use clap::Parser;
use tracing::info;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

const APP_NAME: &str = "kata-pulse";
const VERSION: &str = env!("CARGO_PKG_VERSION");
const DEFAULT_LISTEN_ADDRESS: &str = "127.0.0.1:8090";
const DEFAULT_RUNTIME_ENDPOINT: &str = "/run/containerd/containerd.sock";
const DEFAULT_LOG_LEVEL: &str = "info";
const DEFAULT_METRICS_INTERVAL_SECS: u64 = 60;

const BANNER: &str = r#"
╔═══════════════════════════════════════════════════════════════════╗
║                                                                   ║
║                        🐳  KATA-PULSE 🐳                            ║
║          Real-time metrics for Kata Containers                    ║
║                                                                   ║
║  Repository: https://github.com/diverofdark/kata-pulse            ║
║  Author: Kirill Orlov (@diverofdark)                              ║
║                                                                   ║
║  Thanks to Kata Containers for a great product!                   ║
║                                                                   ║
╚═══════════════════════════════════════════════════════════════════╝
"#;

#[derive(Parser, Debug)]
#[command(
    name = APP_NAME,
    version = VERSION,
    about = "Real-time metrics for Kata Containers",
    long_about = "KataPulse: cadvisor-compatible monitoring agent for Kata Containers. Provides metrics collection, sandbox management, and agent URL discovery"
)]
struct Args {
    /// The address to listen on for HTTP requests
    #[arg(
        long,
        env = "KATA_PULSE_LISTEN",
        default_value = DEFAULT_LISTEN_ADDRESS,
        help = "The address to listen on for HTTP requests"
    )]
    listen_address: String,

    /// Endpoint of CRI container runtime service
    #[arg(
        long,
        env = "RUNTIME_ENDPOINT",
        default_value = DEFAULT_RUNTIME_ENDPOINT,
        help = "Endpoint of CRI container runtime service"
    )]
    runtime_endpoint: String,

    /// Log level
    #[arg(
        long,
        env = "RUST_LOG",
        default_value = DEFAULT_LOG_LEVEL,
        help = "Log level (trace/debug/info/warn/error)"
    )]
    log_level: String,

    /// Metrics collection interval in seconds
    #[arg(
        long,
        env = "KATA_PULSE_METRICS_INTERVAL",
        default_value_t = DEFAULT_METRICS_INTERVAL_SECS,
        help = "Metrics collection interval in seconds"
    )]
    metrics_interval_secs: u64,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    // Initialize logging
    if let Err(e) = init_logging(&args.log_level) {
        eprintln!("Failed to initialize logging: {}", e);
        return;
    }

    // Print banner
    println!("{}", BANNER);

    // Log startup information
    info!(
        app = APP_NAME,
        version = VERSION,
        listen_address = %args.listen_address,
        runtime_endpoint = %args.runtime_endpoint,
        log_level = %args.log_level,
        metrics_interval_secs = args.metrics_interval_secs,
        "announcement"
    );

    // Create application context with all singletons
    let app_context =
        match context::AppContext::new(args.runtime_endpoint, args.metrics_interval_secs) {
            Ok(ctx) => ctx,
            Err(e) => {
                eprintln!("Failed to initialize application context: {}", e);
                return;
            }
        };

    match app_context.start() {
        Ok(_) => (),
        Err(e) => {
            eprintln!("Failed to start application: {}", e);
            return;
        }
    };

    // Start HTTP server
    tracing::debug!(listen_address = %args.listen_address, "Starting HTTP server");
    if let Err(e) = server::start_server(&args.listen_address, app_context).await {
        tracing::error!(error = %e, "Server error");
    }
}

/// Initialize the logging system
fn init_logging(log_level: &str) -> Result<()> {
    let env_filter = match log_level {
        "trace" => EnvFilter::new("trace"),
        "debug" => EnvFilter::new("debug"),
        "info" => EnvFilter::new("info"),
        "warn" => EnvFilter::new("warn"),
        "error" => EnvFilter::new("error"),
        _ => EnvFilter::new("info"),
    };

    tracing_subscriber::registry()
        .with(
            fmt::layer()
                .with_writer(std::io::stderr)
                .with_thread_ids(true),
        )
        .with(env_filter)
        .init();

    Ok(())
}
