//! harmony-agent main entry point
//!
//! This binary serves as the main entry point for the WireGuard agent.
//! It handles CLI parsing, logging setup, and daemon initialization.

use clap::{Parser, Subcommand};
use tracing::{error, info};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

use harmony_agent::{
    APP_NAME, VERSION,
    config::Config,
    service::{create_service, ServiceMode},
    monitoring::Monitor,
    control::{CommandHandler, ControlServer, DEFAULT_SOCKET_PATH},
};
use std::sync::Arc;
use std::path::PathBuf;
use axum::{
    routing::get,
    Router,
    http::StatusCode,
    response::IntoResponse,
};
use tokio::signal;

/// Cross-platform WireGuard network agent
#[derive(Parser, Debug)]
#[command(name = APP_NAME, version = VERSION, about, long_about = None)]
struct Cli {
    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Configuration file path
    #[arg(
        short,
        long,
        global = true,
        default_value = "/etc/harmony-agent/config.toml"
    )]
    config: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Start the agent daemon
    Start,

    /// Stop the agent daemon
    Stop,

    /// Check agent status
    Status,

    /// Show version information
    Version,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // Initialize logging
    init_logging(cli.verbose);

    info!("Starting {} v{}", APP_NAME, VERSION);

    // Execute command
    if let Err(e) = run(cli).await {
        error!("Error: {}", e);
        std::process::exit(1);
    }
}

/// Initialize structured logging with tracing
fn init_logging(verbose: bool) {
    let filter = if verbose {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("debug"))
    } else {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"))
    };

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer())
        .init();
}

/// Run the CLI command
async fn run(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        Commands::Start => {
            info!("Starting agent with config: {}", cli.config);
            let config = Config::from_file(&cli.config)?;
            let mode = ServiceMode::detect();
            info!("Service mode: {:?}", mode);
            let mut service = create_service(mode);
            service.init()?;
            service.start()?;
            service.notify_ready()?;
            info!("Service started successfully");
            
            // Create command handler and load configuration
            let handler = Arc::new(CommandHandler::new());
            handler.load_config(config.clone()).await;
            
            // Auto-start tunnels for networks with enable_wireguard = true
            info!("Checking for enabled WireGuard networks...");
            
            for (name, network) in &config.networks {
                if network.enable_wireguard {
                    info!("Auto-starting WireGuard tunnel for network: {}", name);
                    
                    match harmony_agent::wireguard::Tunnel::from_network_config(network) {
                        Ok(tunnel) => {
                            match tunnel.start().await {
                                Ok(()) => {
                                    info!("Tunnel '{}' started successfully", name);
                                    // Register tunnel with handler
                                    handler.register_tunnel(name.clone(), Arc::new(tunnel)).await;
                                }
                                Err(e) => {
                                    error!("Failed to start tunnel '{}': {}", name, e);
                                }
                            }
                        }
                        Err(e) => {
                            error!("Failed to create tunnel '{}': {}", name, e);
                        }
                    }
                }
            }
            
            let active_count = handler.list_networks().await.len();
            info!("Started {} WireGuard tunnel(s)", active_count);
            
            // Create control server
            let socket_path = PathBuf::from(DEFAULT_SOCKET_PATH);
            let control_server = Arc::new(ControlServer::new(socket_path.clone(), handler.clone()));
            
            // Spawn control server task
            info!("Spawning control server task...");
            let control_handle = {
                let server = control_server.clone();
                tokio::spawn(async move {
                    info!("Control server task started, calling start()...");
                    if let Err(e) = server.start().await {
                        error!("Control server error: {}", e);
                    }
                    info!("Control server task ended");
                })
            };
            
            // Start HTTP server for metrics and health endpoints
            let monitor = Arc::new(Monitor::new());
            let app = create_http_server(monitor);
            
            let addr = "127.0.0.1:9090";
            info!("Starting HTTP server on {}", addr);
            
            let listener = tokio::net::TcpListener::bind(addr).await?;
            info!("HTTP server listening on {}", addr);
            
            // Run HTTP server with graceful shutdown
            axum::serve(listener, app)
                .with_graceful_shutdown(shutdown_signal())
                .await?;
            
            info!("Shutting down agent");
            
            // Shutdown control server
            if let Err(e) = control_server.shutdown().await {
                error!("Failed to shutdown control server: {}", e);
            }
            
            // Abort control server task
            control_handle.abort();
            
            // Stop all tunnels
            for network in handler.list_networks().await {
                info!("Stopping tunnel: {}", network);
                if let Err(e) = handler.stop_tunnel(&network).await {
                    error!("Failed to stop tunnel '{}': {}", network, e);
                }
            }
            
            service.stop()?;
            Ok(())
        },
        Commands::Stop => {
            info!("Stopping agent");
            // To be implemented with proper process management
            Ok(())
        },
        Commands::Status => {
            info!("Checking agent status");
            // To be implemented with health checks
            println!("Agent status: Not yet implemented");
            Ok(())
        },
        Commands::Version => {
            println!("{} v{}", APP_NAME, VERSION);
            Ok(())
        },
    }
}

/// Create HTTP server with routes
fn create_http_server(monitor: Arc<Monitor>) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/metrics", get(move || metrics(monitor.clone())))
}

/// Health check endpoint
async fn healthz() -> impl IntoResponse {
    (StatusCode::OK, "OK")
}

/// Metrics endpoint (Prometheus format)
async fn metrics(monitor: Arc<Monitor>) -> impl IntoResponse {
    let stats = monitor.get_all_stats();
    let metrics_collector = monitor.metrics();
    
    let mut output = String::new();
    
    // Agent info
    output.push_str("# HELP harmony_agent_info Agent information\n");
    output.push_str("# TYPE harmony_agent_info gauge\n");
    output.push_str(&format!("harmony_agent_info{{version=\"{}\"}} 1\n\n", VERSION));
    
    // Network stats
    for (network, stat) in stats.iter() {
        output.push_str(&format!("# HELP wg_network_state Network connection state (0=disconnected, 1=connecting, 2=connected, 3=degraded, 4=failed)\n"));
        output.push_str(&format!("# TYPE wg_network_state gauge\n"));
        let state_value = match stat.state {
            harmony_agent::monitoring::ConnectionState::Disconnected => 0,
            harmony_agent::monitoring::ConnectionState::Connecting => 1,
            harmony_agent::monitoring::ConnectionState::Connected => 2,
            harmony_agent::monitoring::ConnectionState::Degraded => 3,
            harmony_agent::monitoring::ConnectionState::Failed => 4,
        };
        output.push_str(&format!("wg_network_state{{network=\"{}\"}} {}\n\n", network, state_value));
        
        output.push_str(&format!("# HELP wg_bytes_transmitted Total bytes transmitted\n"));
        output.push_str(&format!("# TYPE wg_bytes_transmitted counter\n"));
        output.push_str(&format!("wg_bytes_transmitted{{network=\"{}\"}} {}\n\n", network, stat.tx_bytes));
        
        output.push_str(&format!("# HELP wg_bytes_received Total bytes received\n"));
        output.push_str(&format!("# TYPE wg_bytes_received counter\n"));
        output.push_str(&format!("wg_bytes_received{{network=\"{}\"}} {}\n\n", network, stat.rx_bytes));
        
        output.push_str(&format!("# HELP wg_peers_total Total number of peers\n"));
        output.push_str(&format!("# TYPE wg_peers_total gauge\n"));
        output.push_str(&format!("wg_peers_total{{network=\"{}\"}} {}\n\n", network, stat.total_peers));
        
        output.push_str(&format!("# HELP wg_peers_active Active peers\n"));
        output.push_str(&format!("# TYPE wg_peers_active gauge\n"));
        output.push_str(&format!("wg_peers_active{{network=\"{}\"}} {}\n\n", network, stat.active_peers));
    }
    
    // Add metrics from collector
    output.push_str(&metrics_collector.export_prometheus());
    
    (StatusCode::OK, output)
}

/// Wait for shutdown signal (Ctrl+C or SIGTERM)
async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Received Ctrl+C signal");
        },
        _ = terminate => {
            info!("Received SIGTERM signal");
        },
    }
}
