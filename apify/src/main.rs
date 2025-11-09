//! Application entry point, responsible for parsing CLI args, loading config, and starting services

use apify::{config::{Config, DatabaseConfig, OpenAPIConfig, ApiRef, ModulesConfig}, server::start_listener};
use clap::Parser;
use std::thread;
use std::path::Path;

/// Configurable HTTP server with route matching
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
    println!("Using config file: {}", cli.config);

    // Load main configuration from specified file path
    let config = Config::from_file(&cli.config)?;
    println!("Config loaded successfully");

    // Load database configuration if database.yaml exists
    let config_dir = Path::new(&cli.config).parent().unwrap_or(Path::new("."));
    let database_config = match DatabaseConfig::from_file(&config_dir.join("database.yaml").to_string_lossy()) {
        Ok(db_config) => {
            println!("Database config loaded successfully");
            Some(db_config)
        }
        Err(e) => {
            println!("No database config found or error loading: {}", e);
            None
        }
    };

    // Start worker threads (multiple threads per listener, sharing port via SO_REUSEPORT)
    let num_threads = 2; // Can be made configurable
    println!("Starting {} worker threads", num_threads);

    let mut handles = Vec::new();

    for (listener_idx, listener_config) in config.listeners.into_iter().enumerate() {
        // Load OpenAPI configurations for this listener
        let mut openapi_configs = Vec::new();
        if let Some(api_refs) = &listener_config.apis {
            for api_ref in api_refs {
                match api_ref {
                    ApiRef::Path(p) => {
                        let api_path = config_dir.join(p);
                        match OpenAPIConfig::from_file(&api_path.to_string_lossy()) {
                            Ok(openapi_config) => {
                                println!("OpenAPI config loaded from: {}", p);
                                openapi_configs.push((openapi_config, None));
                            }
                            Err(e) => eprintln!("Error loading OpenAPI config from {}: {}", p, e),
                        }
                    }
                    ApiRef::WithModules { path, modules } => {
                        let api_path = config_dir.join(path);
                        match OpenAPIConfig::from_file(&api_path.to_string_lossy()) {
                            Ok(openapi_config) => {
                                println!("OpenAPI config loaded from: {} (with modules)", path);
                                openapi_configs.push((openapi_config, modules.clone()));
                            }
                            Err(e) => eprintln!("Error loading OpenAPI config from {}: {}", path, e),
                        }
                    }
                }
            }
        }

        for thread_id in 0..num_threads {
            let listener_config_clone = listener_config.clone();
            let database_config_clone = database_config.clone();
            let openapi_configs_clone = openapi_configs.clone();
            let handle = thread::spawn(
                move || -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
                    println!(
                        "Starting thread {} for listener {} (port: {})",
                        thread_id, listener_idx, listener_config_clone.port
                    );
                    start_listener(
                        listener_config_clone,
                        thread_id,
                        database_config_clone,
                        openapi_configs_clone,
                    )?;
                    Ok(())
                },
            );
            handles.push(handle);
        }
    }

    // Wait for all threads to complete
    for (idx, handle) in handles.into_iter().enumerate() {
        match handle.join() {
            Ok(Ok(())) => {
                eprintln!("Thread {} exited normally", idx);
            }
            Ok(Err(e)) => {
                eprintln!("Thread {} execution error: {}", idx, e);
            }
            Err(e) => {
                eprintln!("Thread {} panicked: {:?}", idx, e);
            }
        }
    }
    
    eprintln!("All threads exited, main process terminating");

    Ok(())
}
