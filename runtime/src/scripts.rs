use crate::runtime::Runtime;
use crate::settings::Settings;
use crate::loader::{Loader, ModuleSetup, Package};
use crossbeam::channel;
use log::{debug, error};
use flow::Context;
use serde_json::Value as JsonValue;
use tokio;

pub fn run_script(path: &str, setup: ModuleSetup, settings: &Settings) {
    debug!("Running script at path: {}", path);
    let dispatch = setup.dispatch.clone();

    tracing::dispatcher::with_default(&dispatch, || {
        let _guard = flow::otel::init_tracing_subscriber(setup.app_data.clone());
        flow::use_log!();

        if let Ok(rt) = tokio::runtime::Runtime::new() {
            rt.block_on(async move {
                let loader = Loader::load(&path, settings.print_yaml).await.unwrap();
                let (tx_main_package, rx_main_package) = channel::unbounded::<Package>();
                let app_data = loader.app_data.clone();
                let dispatch = setup.dispatch.clone();
                let dispatch_for_runtime = dispatch.clone();
                let settings_cloned = settings.clone();

                // 创建runtime任务
                let tx_for_runtime = tx_main_package.clone();
                let context = Context::from_main(setup.with.clone());

                let runtime_handle = tokio::task::spawn(async move {
                    Runtime::run_script(
                        tx_for_runtime,
                        rx_main_package,
                        loader,
                        dispatch_for_runtime,
                        settings_cloned,
                        context,
                    )
                    .await
                });

                let rx = module_channel!(setup);

                debug!("Script module loaded, starting main loop");

                for package in rx {
                    debug!("Received package: {:?}", package);

                    let span = tracing::span!(
                        tracing::Level::INFO,
                        "auto_start_steps",
                        otel.name = app_data.name.clone().unwrap_or("unknown".to_string()),
                    );

                    // 创建响应通道
                    let (response_tx, response_rx) = tokio::sync::oneshot::channel::<JsonValue>();

                    let runtime_package = Package {
                        response: Some(response_tx),
                        request_data: package.input(),
                        origin: 0,
                        span: Some(span),
                        dispatch: Some(dispatch.clone()),
                    };

                    debug!("Sending package to main loop: {:?}", runtime_package);

                    if let Err(err) = tx_main_package.send(runtime_package) {
                        error!("Failed to send package: {:?}", err);
                        continue;
                    }

                    debug!("Package sent to main loop, waiting for response");

                    let response = match response_rx.await {
                        Ok(result) if result.is_null() => ModuleResponse::from_success(
                            package.payload().unwrap_or(JsonValue::Null),
                        ),
                        Ok(result) => ModuleResponse::from_success(result),
                        Err(err) => ModuleResponse::from_error(format!("Runtime error: {}", err)),
                    };

                    if let Err(err) = package.sender.send(response) {
                        error!("Failed to send response back to module: {:?}", err);
                    }

                    debug!("Response sent back to module");
                }

                debug!("Script module no listeners, waiting for runtime to finish");

                runtime_handle
                    .await
                    .unwrap_or_else(|err| {
                        error!("Runtime task error: {:?}", err);
                        std::process::exit(1);
                    })
                    .unwrap_or_else(|err| {
                        error!("Runtime error: {:?}", err);
                        std::process::exit(1);
                    });
            });
        } else {
            tracing::error!("Error creating runtime");
            return;
        }
    });
}

// 模块响应结构
#[derive(Debug)]
pub enum ModuleResponse {
    Success(JsonValue),
    Error(String),
}

impl ModuleResponse {
    pub fn from_success(value: JsonValue) -> Self {
        ModuleResponse::Success(value)
    }
    
    pub fn from_error(message: String) -> Self {
        ModuleResponse::Error(message)
    }
}

// 模块通道宏
#[macro_export]
macro_rules! module_channel {
    ($setup:expr) => {{
        use crossbeam::channel;
        let (tx, rx) = channel::unbounded::<ModuleRequest>();
        let _ = $setup.setup_sender.send(Some(tx));
        rx
    }};
}

#[derive(Debug)]
pub struct ModuleRequest {
    input_data: Option<JsonValue>,
    payload_data: Option<JsonValue>,
    sender: channel::Sender<ModuleResponse>,
}

impl ModuleRequest {
    pub fn new(input: Option<JsonValue>, payload: Option<JsonValue>, sender: channel::Sender<ModuleResponse>) -> Self {
        Self {
            input_data: input,
            payload_data: payload,
            sender,
        }
    }
    
    pub fn input(&self) -> Option<&JsonValue> {
        self.input_data.as_ref()
    }
    
    pub fn payload(&self) -> Option<&JsonValue> {
        self.payload_data.as_ref()
    }
}
    