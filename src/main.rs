//! wg-agent main entry point
//!
//! This binary serves as the main entry point for the WireGuard agent.
//! It handles CLI parsing, logging setup, and daemon initialization.

use clap::{Parser, Subcommand};
use tracing::{error, info};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

use wg_agent::{APP_NAME, VERSION, config::Config, service::ServiceManager};

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
        default_value = "/etc/wg-agent/config.toml"
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
            let _config = Config::from_file(&cli.config)?;
            let service = ServiceManager::new()?;
            service.run().await?;
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
