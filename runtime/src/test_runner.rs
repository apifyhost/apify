use crate::loader::{load_module, Loader};
use crate::settings::Settings;
use crossbeam::channel;
use log::{debug, error};
// 修正引用路径
use flow::script::{build_engine, Script};
use flow::{Context, Flow};  // 引用flow项目的正确结构体
use sdk::otel::init_tracing_subscriber;
use sdk::prelude::json;
use sdk::structs::{ModulePackage, ModuleSetup, Modules};
use sdk::valu3::prelude::*;
use sdk::valu3::value::Value;
use std::collections::HashMap;
use std::fmt::Write;
use std::sync::Arc;
use tokio::sync::oneshot;

#[derive(Debug)]
pub struct TestResult {
    pub index: usize,
    pub passed: bool,
    pub message: String,
    pub describe: Option<String>,
}

#[derive(Debug)]
pub struct TestSummary {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub results: Vec<TestResult>,
}

pub async fn run_tests(
    loader: Loader,
    test_filter: Option<&str>,
    settings: Settings,
) -> Result<TestSummary, String> {
    debug!("run_tests");
    let tests = loader
        .tests
        .as_ref()
        .ok_or("No tests found in the runtime file")?;
    let steps = &loader.steps;

    if !tests.is_array() {
        return Err(format!("Tests must be an array, got: {:?}", tests));
    }

    let test_cases = tests.as_array().unwrap();

    let filtered_tests: Vec<_> = if let Some(filter) = test_filter {
        test_cases
            .values
            .iter()
            .enumerate()
            .filter(|(_, test_case)| {
                if let Some(description) = test_case.get("describe") {
                    let desc_str = description.as_string();
                    return desc_str.contains(filter);
                }
                false
            })
            .collect()
    } else {
        test_cases.values.iter().enumerate().collect()
    };

    debug!("filtered_tests");

    let total = filtered_tests.len();

    if total == 0 {
        if let Some(filter) = test_filter {
            println!("⚠️  No tests match filter: '{}'", filter);
        } else {
            println!("⚠️  No tests to run");
        }

        return Ok(TestSummary {
            total: 0,
            passed: 0,
            failed: 0,
            results: Vec::new(),
        });
    }

    if let Some(filter) = test_filter {
        println!(
            "🧪 Running {} test(s) matching '{}' (out of {} total)...",
            total,
            filter,
            test_cases.len()
        );
    } else {
        println!("🧪 Running {} test(s)...", total);
    }
    println!();

    let modules = load_modules_like_runtime(&loader, settings)
        .await
        .map_err(|e| format!("Failed to load modules for tests: {}", e))?;

    let workflow = json!({
        "steps": steps
    });

    // 使用正确的Flow结构体
    let flow = Flow::try_from_value(&workflow, Some(modules))
        .map_err(|e| format!("Failed to create flow: {}", e))?;

    let mut results = Vec::new();
    let mut passed = 0;

    for (run_index, (_, test_case)) in filtered_tests.iter().enumerate() {
        let test_index = run_index + 1;

        let test_description = test_case.get("describe").map(|v| v.as_string());

        if let Some(ref desc) = test_description {
            print!("Test {}: {} - ", test_index, desc);
        } else {
            print!("Test {}: ", test_index);
        }

        let result = run_single_test(test_case, &flow).await;

        match result {
            Ok(msg) => {
                println!("✅ PASSED");
                passed += 1;
                results.push(TestResult {
                    index: test_index,
                    passed: true,
                    message: msg,
                    describe: test_description.clone(),
                });
            }
            Err(msg) => {
                println!("❌ FAILED - {}", msg);
                results.push(TestResult {
                    index: test_index,
                    passed: false,
                    message: msg,
                    describe: test_description.clone(),
                });
            }
        }
    }

    let failed = total - passed;
    println!();
    println!("📊 Test Results:");
    println!("   Total: {}", total);
    println!("   Passed: {} ✅", passed);
    println!("   Failed: {} ❌", failed);

    if failed > 0 {
        println!();
        println!("❌ Some tests failed!");
    } else {
        println!();
        println!("🎉 All tests passed!");
    }

    Ok(TestSummary {
        total,
        passed,
        failed,
        results,
    })
}

async fn run_single_test(test_case: &Value, flow: &Flow) -> Result<String, String> {
    let main_value = test_case.get("main").cloned().unwrap_or(Value::Undefined);
    let initial_payload = test_case
        .get("payload")
        .cloned()
        .unwrap_or(Value::Undefined);

    debug!(
        "Running test with main: {:?}, payload: {:?}",
        main_value, initial_payload
    );

    let mut context = Context::from_main(main_value);

    // 使用正确的方法名add_step_output并添加step_id参数
    if !initial_payload.is_undefined() {
        context.add_step_output("initial_payload".to_string(), initial_payload);
    }

    let result = {
        let result = flow
            .execute(&mut context)
            .await
            .map_err(|e| format!("Execution failed: {}", e))?;

        result.unwrap_or(Value::Undefined)
    };

    if let Some(assert_eq_value) = test_case.get("assert_eq") {
        if deep_equals(&result, assert_eq_value) {
            Ok(format!("Expected and got: {}", result))
        } else {
            let mut msg = String::new();
            write!(
                &mut msg,
                "Expected \x1b[34m{}\x1b[0m, got \x1b[31m{}\x1b[0m",
                assert_eq_value, result
            )
            .unwrap();
            Err(msg)
        }
    } else if let Some(assert_expr) = test_case.get("assert") {
        let assertion_result = evaluate_assertion(assert_expr, &result)
            .map_err(|e| format!("Assertion error: {}", e))?;

        if assertion_result {
            Ok(format!("Assertion passed: {}", assert_expr))
        } else {
            Err(format!("Assertion failed: {}", assert_expr))
        }
    } else {
        Err("No assertion found (assert or assert_eq required)".to_string())
    }
}

async fn load_modules_like_runtime(
    loader: &Loader,
    settings: Settings,
) -> Result<Arc<Modules>, String> {
    let mut modules = Modules::default();

    let guard = init_tracing_subscriber(loader.app_data.clone());
    let dispatch = guard.dispatch.clone();

    let engine = build_engine(None);

    for (id, module) in loader.modules.iter().enumerate() {
        let (setup_sender, setup_receive) =
            oneshot::channel::<Option<channel::Sender<ModulePackage>>>();

        let main_sender = None;

        let with = {
            let script = Script::try_build(engine.clone(), &module.with)
                .map_err(|e| format!("Failed to build script for module {}: {}", module.name, e))?;

            script.evaluate_without_context().map_err(|e| {
                format!(
                    "Failed to evaluate script for module {}: {}",
                    module.name, e
                )
            })?
        };

        let setup = ModuleSetup {
            id,
            setup_sender,
            main_sender,
            with,
            dispatch: dispatch.clone(),
            app_data: loader.app_data.clone(),
            is_test_mode: true,
        };

        let module_target = module.module.clone();
        let module_version = module.version.clone();
        let local_path = module.local_path.clone();
        let module_name = module.name.clone();
        let settings = settings.clone();

        debug!(
            "Module debug: name={}, is_local_path={:?}, local_path={:?}",
            module_name, module.local_path.is_some(), local_path
        );

        std::thread::spawn(move || {
            let result = load_module(setup, &module_target, &module_version, local_path, settings);

            if let Err(err) = result {
                error!("Test runtime Error Load Module: {:?}", err)
            }
        });

        debug!(
            "Module {} loaded with name \"{}\" and version \"{}\"",
            module.module, module.name, module.version
        );

        match setup_receive.await {
            Ok(Some(sender)) => {
                debug!("Module \"{}\" registered", module.name);
                modules.register(module.clone(), sender);
            }
            Ok(None) => {
                debug!("Module \"{}\" did not register", module.name);
            }
            Err(err) => {
                return Err(format!(
                    "Module \"{}\" registration failed: {}",
                    module.name, err
                ));
            }
        }
    }

    Ok(Arc::new(modules))
}

fn deep_equals(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Null, Value::Null) => true,
        (Value::Boolean(a), Value::Boolean(b)) => a == b,
        (Value::Number(a), Value::Number(b)) => {
            let a_val = a.to_f64().unwrap_or(0.0);
            let b_val = b.to_f64().unwrap_or(0.0);
            (a_val - b_val).abs() < f64::EPSILON
        }
        (Value::String(a), Value::String(b)) => a == b,
        (Value::Array(a), Value::Array(b)) => {
            if a.len() != b.len() {
                return false;
            }
            a.values
                .iter()
                .zip(b.values.iter())
                .all(|(a_val, b_val)| deep_equals(a_val, b_val))
        }
        (Value::Object(a), Value::Object(b)) => {
            if a.len() != b.len() {
                return false;
            }

            for (key, a_val) in a.iter() {
                let key_str = key.to_string();
                match b.get(key_str.as_str()) {
                    Some(b_val) => {
                        if !deep_equals(a_val, b_val) {
                            return false;
                        }
                    }
                    None => return false,
                }
            }

            true
        }
        _ => false,
    }
}

fn evaluate_assertion(assert_expr: &Value, result: &Value) -> Result<bool, String> {
    let engine = build_engine(None);

    let script = Script::try_build(engine, assert_expr)
        .map_err(|e| format!("Failed to build assertion script: {}", e))?;

    let context_map: HashMap<String, Value> = [("payload".to_string(), result.clone())]
        .iter()
        .cloned()
        .collect();

    let assertion_result = script
        .evaluate(&context_map)
        .map_err(|e| format!("Failed to evaluate assertion: {}", e))?;

    match assertion_result {
        Value::Boolean(b) => Ok(b),
        Value::String(s) if s == "true".into() => Ok(true),
        Value::String(s) if s == "false".into() => Ok(false),
        _ => Err(format!(
            "Assertion must return boolean, got: {}",
            assertion_result
        )),
    }
}
    