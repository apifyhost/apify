//! Application entry point, responsible for parsing CLI args, loading config, and starting services

use apify::{config::Config, server::start_listener};
use clap::Parser;
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

    // Load configuration from specified file path
    let config = Config::from_file(&cli.config)?;
    println!("Config loaded successfully");

    // Start worker threads (multiple threads per listener, sharing port via SO_REUSEPORT)
    let num_threads = 2; // Can be made configurable
    println!("Starting {} worker threads", num_threads);

    let mut handles = Vec::new();

    for (listener_idx, listener_config) in config.listeners.into_iter().enumerate() {
        for thread_id in 0..num_threads {
            let listener_config_clone = listener_config.clone();
            let handle = thread::spawn(
                move || -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
                    println!(
                        "Starting thread {} for listener {} (port: {})",
                        thread_id, listener_idx, listener_config_clone.port
                    );
                    start_listener(listener_config_clone, thread_id)?;
                    Ok(())
                },
            );
            handles.push(handle);
        }
    }

    // Wait for all threads to complete
    for handle in handles {
        if let Err(e) = handle.join() {
            eprintln!("Thread execution error: {:?}", e);
        }
    }

    Ok(())
}
