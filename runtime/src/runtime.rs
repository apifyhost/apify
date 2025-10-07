use crate::loader::{Loader, load_module};
#[cfg(target_env = "gnu")]
use crate::memory::force_memory_release;
use crate::settings::Settings;
use crossbeam::channel;
use flow::phs::{Script, ScriptError, build_engine};
use flow::{Context, Flow};
use futures::future::join_all;
use log::{debug, error, info};
use sdk::structs::Plugin;
use sdk::tokio;
use sdk::{
    prelude::Value,
    structs::{ModulePackage, ModuleSetup, Modules},
    tracing::{self, Dispatch, dispatcher},
};
use std::fmt::Display;
use std::sync::Arc;
#[cfg(target_env = "gnu")]
use std::thread;
use tokio::sync::oneshot;

#[derive(Debug)]
#[allow(clippy::enum_variant_names)]
pub enum RuntimeError {
    ModuleWithError(ScriptError),
    ModuleRegisterError,
    FlowExecutionError(String),
}

impl Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RuntimeError::ModuleRegisterError => write!(f, "Module register error"),
            RuntimeError::FlowExecutionError(err) => write!(f, "Flow execution error: {err}"),
            RuntimeError::ModuleWithError(err) => write!(f, "Module with error: {err}"),
        }
    }
}

pub struct Runtime {}

impl Runtime {
    async fn load_modules(
        loader: Loader,
        dispatch: Dispatch,
        settings: Settings,
        tx_main_plugin: channel::Sender<Plugin>,
    ) -> Result<Modules, RuntimeError> {
        let mut modules = Modules::default();
        let engine = build_engine(None);
        // -------------------------
        // Load the modules
        // -------------------------
        let app_data = loader.app_data.clone();
        let loader_main_id = loader.main;

        for (id, module) in loader.modules.into_iter().enumerate() {
            let (setup_sender, setup_receive) =
                oneshot::channel::<Option<channel::Sender<ModulePackage>>>();

            // Se --var-main foi especificado, não permitir que módulos principais sejam executados
            let main_sender = if loader_main_id == id as i32 && settings.var_main.is_none() {
                Some(tx_main_plugin.clone())
            } else {
                None
            };

            let with = {
                let script = match Script::try_build(engine.clone(), &module.with) {
                    Ok(payload) => payload,
                    Err(err) => return Err(RuntimeError::ModuleWithError(err)),
                };

                script
                    .evaluate_without_context()
                    .map_err(RuntimeError::ModuleWithError)?
            };

            let setup = ModuleSetup {
                id,
                setup_sender,
                main_sender,
                with,
                dispatch: dispatch.clone(),
                app_data: app_data.clone(),
                is_test_mode: false,
            };

            let module_target = module.module.clone();
            let module_version = module.version.clone();
            let local_path = module.local_path.clone();
            let settings = settings.clone();

            std::thread::spawn(move || {
                let result =
                    load_module(setup, &module_target, &module_version, local_path, settings);

                if let Err(err) = result {
                    error!("Runtime Error Load Module: {err:?}")
                }
            });

            debug!(
                "Module {} loaded with name \"{}\" and version \"{}\"",
                module.module, module.name, module.version
            );

            match setup_receive.await {
                Ok(Some(sender)) => {
                    debug!("Module {} registered", module.name);
                    modules.register(module, sender);
                }
                Ok(None) => {
                    debug!("Module {} did not register", module.name);
                }
                Err(_) => {
                    return Err(RuntimeError::ModuleRegisterError);
                }
            }
        }

        Ok(modules)
    }

    async fn listener(
        rx_main_plugin: channel::Receiver<Plugin>,
        steps: Value,
        modules: Modules,
        settings: Settings,
        default_context: Option<Context>,
    ) -> Result<(), RuntimeError> {
        let flow = Arc::new({
            match Flow::try_from_value(&steps, Some(Arc::new(modules))) {
                Ok(flow) => flow,
                Err(err) => return Err(RuntimeError::FlowExecutionError(err.to_string())),
            }
        });

        drop(steps);

        let mut handles = Vec::new();
        let default_context = default_context.clone();

        for _i in 0..settings.plugin_consumer_count {
            let rx_main_plugin = rx_main_plugin.clone();
            let flow = flow.clone();
            let default_context = default_context.clone();

            let handle = tokio::task::spawn_blocking(move || {
                for mut main_plugin in rx_main_plugin {
                    let flow = flow.clone();
                    let parent = match main_plugin.span.clone() {
                        Some(span) => span,
                        None => {
                            error!("Span not found in main module");
                            continue;
                        }
                    };
                    let dispatch = match main_plugin.dispatch.clone() {
                        Some(dispatch) => dispatch,
                        None => {
                            error!("Dispatch not found in main module");
                            continue;
                        }
                    };

                    let mut context = {
                        let data = main_plugin.get_data().cloned().unwrap_or(Value::Null);
                        if let Some(mut context) = default_context.clone() {
                            context.set_main(data);
                            context
                        } else {
                            Context::from_main(data)
                        }
                    };

                    tokio::task::block_in_place(move || {
                        dispatcher::with_default(&dispatch, || {
                            let _enter = parent.enter();
                            let rt = tokio::runtime::Handle::current();

                            rt.block_on(async {
                                match flow.execute(&mut context).await {
                                    Ok(result) => {
                                        let result_value = result.unwrap_or(Value::Undefined);
                                        main_plugin.send(result_value);
                                    }
                                    Err(err) => {
                                        error!("Runtime Error Execute Steps: {err:?}");
                                    }
                                }
                            });
                        });
                    });
                }
            });

            handles.push(handle);
        }

        join_all(handles).await;

        Ok(())
    }

    pub async fn run(
        loader: Loader,
        dispatch: Dispatch,
        settings: Settings,
    ) -> Result<(), RuntimeError> {
        // -------------------------
        // Create the channels
        // -------------------------
        let (tx_main_plugin, rx_main_plugin) = channel::unbounded::<Plugin>();

        let no_main = loader.main == -1 || settings.var_main.is_some();
        let steps = loader.get_steps();
        let modules = Self::load_modules(
            loader,
            dispatch.clone(),
            settings.clone(),
            tx_main_plugin.clone(),
        )
        .await?;

        // Se não há main definido ou --var-main foi especificado, forçar o início dos steps
        if no_main {
            // Criar um span padrão para o início dos steps
            let span = tracing::span!(
                tracing::Level::INFO,
                "auto_start_steps",
                otel.name = "apify auto start"
            );

            // Se --var-main foi especificado, processar o valor usando valu3
            let request_data = if let Some(var_main_str) = &settings.var_main {
                // Usar valu3 para processar o valor da mesma forma que outros valores
                match Value::json_to_value(var_main_str) {
                    Ok(value) => Some(value),
                    Err(err) => {
                        error!("Failed to parse --var-main value '{var_main_str}': {err:?}");
                        return Err(RuntimeError::FlowExecutionError(format!(
                            "Failed to parse --var-main value: {err:?}"
                        )));
                    }
                }
            } else {
                None
            };

            // Enviar um pacote com os dados do --var-main para iniciar os steps
            let plugin = Plugin {
                response: None,
                request_data,
                origin: 0,
                span: Some(span),
                dispatch: Some(dispatch.clone()),
            };

            if let Err(err) = tx_main_plugin.send(plugin) {
                error!("Failed to send plugin: {err:?}");
                return Err(RuntimeError::FlowExecutionError(
                    "Failed to send plugin".to_string(),
                ));
            }

            if settings.var_main.is_some() {
                info!("Using --var-main to simulate main plugin output");
            }
        }

        drop(tx_main_plugin);

        #[cfg(target_env = "gnu")]
        if settings.garbage_collection {
            thread::spawn(move || {
                loop {
                    thread::sleep(std::time::Duration::from_secs(
                        settings.garbage_collection_interval,
                    ));
                    force_memory_release(settings.min_allocated_memory);
                }
            });
        }

        info!("Flow!");

        // -------------------------
        // Create the flow
        // -------------------------
        Self::listener(rx_main_plugin, steps, modules, settings, None)
            .await
            .map_err(|err| {
                error!("Runtime Error: {err:?}");
                err
            })?;

        Ok(())
    }

    pub async fn run_script(
        tx_main_plugin: channel::Sender<Plugin>,
        rx_main_plugin: channel::Receiver<Plugin>,
        loader: Loader,
        dispatch: Dispatch,
        settings: Settings,
        context: Context,
    ) -> Result<(), RuntimeError> {
        let steps = loader.get_steps();

        let modules = Self::load_modules(
            loader,
            dispatch.clone(),
            settings.clone(),
            tx_main_plugin.clone(),
        )
        .await?;

        Self::listener(rx_main_plugin, steps, modules, settings, Some(context))
            .await
            .map_err(|err| {
                error!("Runtime Error: {err:?}");
                err
            })?;

        Ok(())
    }
}
