mod loader;
mod memory;
mod plugin;
mod preprocessor;
mod runtime;
mod settings;
mod test_runner;
use loader::Loader;
use plugin::Plugin;
use runtime::Runtime;
use sdk::otel::init_tracing_subscriber;
use sdk::{tracing, use_log};
use settings::Settings;
mod scripts;

#[cfg(all(feature = "mimalloc", target_env = "musl"))]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[cfg(all(feature = "jemalloc", target_env = "musl"))]
#[global_allocator]
static GLOBAL: jemallocator::Jemalloc = jemallocator::Jemalloc;

#[cfg(target_os = "macos")]
pub const MODULE_EXTENSION: &str = "dylib";
#[cfg(target_os = "linux")]
pub const MODULE_EXTENSION: &str = "so";
#[cfg(target_os = "windows")]
pub const MODULE_EXTENSION: &str = "dll";

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
pub const RUNTIME_ARCH: &str = "aarch64-apple-darwin";
#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
pub const RUNTIME_ARCH: &str = "x86_64-apple-darwin";
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
pub const RUNTIME_ARCH: &str = "aarch64-unknown-linux-gnu";
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub const RUNTIME_ARCH: &str = "x86_64-unknown-linux-gnu";

#[tokio::main]
async fn main() {
    use_log!();
    log::debug!("Starting APIFY Runtime");

    let settings = match Settings::try_load() {
        Ok(settings) => settings,
        Err(err) => {
            log::error!("Error loading settings: {err:?}");
            std::process::exit(1);
        }
    };

    if let Some(publish_path) = settings.plugin_path.clone() {
        match Plugin::try_from(publish_path) {
            Ok(publish) => {
                if let Err(err) = publish.run() {
                    log::error!("Error publishing module: {err:?}");
                    return;
                }
            }
            Err(err) => {
                log::error!("Error creating publish instance: {err:?}");
                return;
            }
        }
    }

    let mut loader =
        match Loader::load(&settings.script_main_absolute_path, settings.print_yaml).await {
            Ok(main) => main,
            Err(err) => {
                log::error!("Runtime Error Main File: {err:?}");
                return;
            }
        };

    if settings.no_run {
        return;
    }

    let guard = init_tracing_subscriber(loader.app_data.clone());

    if let Err(err) = tracing::dispatcher::set_global_default(guard.dispatch.clone()) {
        log::error!("Failed to set global subscriber: {err:?}");
        std::process::exit(1);
    }

    let dispatch = guard.dispatch.clone();
    let fut = async {
        if settings.download
            && let Err(err) = loader
                .download(&settings.default_plugin_repository_url)
                .await
        {
            log::error!("Download failed: {err:?}");
            return;
        }

        loader.update_info();

        if !settings.only_download_modules {
            if settings.test {
                log::debug!("Run test");
                // Run tests
                match test_runner::run_tests(
                    loader,
                    settings.test_filter.as_deref(),
                    settings.clone(),
                )
                .await
                {
                    Ok(summary) => {
                        // Exit with error code if tests failed
                        if summary.failed > 0 {
                            std::process::exit(1);
                        }
                    }
                    Err(err) => {
                        log::error!("Test execution error: {err}");
                        std::process::exit(1);
                    }
                }
            } else {
                log::debug!("Run application");
                // Run normal workflow
                if let Err(rr) = Runtime::run(loader, dispatch.clone(), settings).await {
                    log::error!("Runtime Error: {rr:?}");
                }
            }
        }
    };

    // passamos a future para o escopo correto de dispatcher
    tracing::dispatcher::with_default(&dispatch, || fut).await;
}
