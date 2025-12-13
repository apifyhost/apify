//! Control Plane application entry point

use apify::{
    config::Config,
    startup::{build_runtime, init_database, setup_logging},
};
use clap::Parser;

/// Apify Control Plane Server
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Path to the configuration file (YAML format)
    #[arg(short, long, default_value = "config.yaml")]
    config: String,
}

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Parse command-line arguments
    let cli = Cli::parse();

    // Load main configuration from specified file path
    let config = Config::from_file(&cli.config)?;

    // Setup logging
    let (_tracing_enabled, _otlp_endpoint, _log_level) = setup_logging(&config)?;

    // Initialize Runtime
    let rt = build_runtime()?;

    rt.block_on(async {
        // Initialize Database
        let db = init_database(&config).await?;

        if let Some(cp_config) = config.control_plane {
            tracing::info!("Starting Control Plane Server");

            // Initialize metadata schema
            db.initialize_schema(apify::control_plane::get_metadata_schemas())
                .await
                .map_err(|e| e.to_string())?;

            apify::control_plane::start_control_plane_server(cp_config, db).await?;
            Ok(())
        } else {
            Err("Control plane configuration missing in config file".into())
        }
    })
}
