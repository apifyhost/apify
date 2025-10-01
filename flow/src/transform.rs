use crate::{error::FlowError, pipeline::Pipeline, step::Step};
use serde_json::Value as JsonValue;
use std::collections::HashMap;

pub fn json_to_pipelines(json: &JsonValue) -> Result<HashMap<usize, Pipeline>, FlowError> {
    let mut pipelines = HashMap::new();
    let mut pipeline_counter = 0;

    let main_steps = json.get("steps")
        .and_then(|v| v.as_array())
        .ok_or_else(|| FlowError::TransformError("根节点缺少 'steps' 数组".into()))?;

    process_pipeline(
        pipeline_counter,
        main_steps,
        json,
        &mut pipelines,
        &mut pipeline_counter,
    )?;

    Ok(pipelines)
}

fn process_pipeline(
    pipeline_id: usize,
    steps_json: &[JsonValue],
    _parent_json: &JsonValue,
    pipelines: &mut HashMap<usize, Pipeline>,
    counter: &mut usize,
) -> Result<(), FlowError> {
    let mut steps = Vec::with_capacity(steps_json.len());

    for (step_idx, step_json) in steps_json.iter().enumerate() {
        let mut step_json_clone = step_json.clone();

        // 处理then分支
        if let Some(then_json) = step_json.get("then") {
            *counter += 1;
            let then_pipeline_id = *counter;
            let then_steps = then_json.get("steps")
                .and_then(|v| v.as_array())
                .ok_or_else(|| FlowError::TransformError("'then' 缺少 'steps' 数组".into()))?;
            process_pipeline(then_pipeline_id, then_steps, then_json, pipelines, counter)?;
            step_json_clone["then"]["pipeline_id"] = JsonValue::Number(then_pipeline_id.into());
        }

        // 处理else分支
        if let Some(else_json) = step_json.get("else") {
            *counter += 1;
            let else_pipeline_id = *counter;
            let else_steps = else_json.get("steps")
                .and_then(|v| v.as_array())
                .ok_or_else(|| FlowError::TransformError("'else' 缺少 'steps' 数组".into()))?;
            process_pipeline(else_pipeline_id, else_steps, else_json, pipelines, counter)?;
            step_json_clone["else"]["pipeline_id"] = JsonValue::Number(else_pipeline_id.into());
        }

        let step = Step::from_json(pipeline_id, step_idx, &step_json_clone)?;
        steps.push(step);
    }

    pipelines.insert(pipeline_id, Pipeline::new(pipeline_id, steps));
    Ok(())
}
