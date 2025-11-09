//! Application entry point, responsible for parsing CLI args, loading config, and starting services

use apify::{
    config::{ApiRef, Config, OpenAPIConfig},
    server::start_listener,
};
use clap::Parser;
use std::path::Path;
use std::thread;

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

    // Use datasources from config if available
    let datasources = config.datasource.clone();
    if let Some(ref ds) = datasources {
        println!("Found {} datasource(s) in config", ds.len());
    }

    // Use consumers from config (global or listener-level)
    let global_consumers = config.consumers.clone().unwrap_or_default();

    // Start worker threads (multiple threads per listener, sharing port via SO_REUSEPORT)
    // Allow override via APIFY_THREADS env var (useful for tests)
    let num_threads: usize = std::env::var("APIFY_THREADS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(2); // default 2
    println!("Starting {} worker threads", num_threads);

    let mut handles = Vec::new();

    let config_dir = Path::new(&cli.config).parent().unwrap_or(Path::new("."));

    for (listener_idx, listener_config) in config.listeners.into_iter().enumerate() {
        // Merge global consumers with listener-specific consumers
        let mut all_consumers = global_consumers.clone();
        if let Some(ref listener_consumers) = listener_config.consumers {
            all_consumers.extend(listener_consumers.clone());
        }

        // Load OpenAPI configurations for this listener with datasource info
        let mut openapi_configs = Vec::new();
        if let Some(api_refs) = &listener_config.apis {
            for api_ref in api_refs {
                match api_ref {
                    ApiRef::Path(p) => {
                        let api_path = config_dir.join(p);
                        match OpenAPIConfig::from_file(&api_path.to_string_lossy()) {
                            Ok(openapi_config) => {
                                println!("OpenAPI config loaded from: {}", p);
                                openapi_configs.push((openapi_config, None, None));
                            }
                            Err(e) => eprintln!("Error loading OpenAPI config from {}: {}", p, e),
                        }
                    }
                    ApiRef::WithConfig {
                        path,
                        modules,
                        datasource,
                    } => {
                        let api_path = config_dir.join(path);
                        match OpenAPIConfig::from_file(&api_path.to_string_lossy()) {
                            Ok(openapi_config) => {
                                let ds_info = if let Some(ds_name) = datasource {
                                    println!(
                                        "OpenAPI config loaded from: {} (datasource: {})",
                                        path, ds_name
                                    );
                                    Some(ds_name.clone())
                                } else {
                                    println!("OpenAPI config loaded from: {}", path);
                                    None
                                };
                                openapi_configs.push((openapi_config, modules.clone(), ds_info));
                            }
                            Err(e) => {
                                eprintln!("Error loading OpenAPI config from {}: {}", path, e)
                            }
                        }
                    }
                }
            }
        }

        for thread_id in 0..num_threads {
            let listener_config_clone = listener_config.clone();
            let datasources_clone = datasources.clone();
            let openapi_configs_clone = openapi_configs.clone();
            let consumers_clone = all_consumers.clone();
            let handle = thread::spawn(
                move || -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
                    println!(
                        "Starting thread {} for listener {} (port: {})",
                        thread_id, listener_idx, listener_config_clone.port
                    );
                    start_listener(
                        listener_config_clone,
                        thread_id,
                        datasources_clone,
                        openapi_configs_clone,
                        consumers_clone,
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
