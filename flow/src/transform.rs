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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_json_to_pipelines_basic() {
        let json_flow = json!({
            "steps": [
                {
                    "label": "step1",
                    "return": {"result": "step1"}
                },
                {
                    "label": "step2", 
                    "return": {"result": "step2"}
                }
            ]
        });
        
        let pipelines = json_to_pipelines(&json_flow).unwrap();
        
        assert!(pipelines.contains_key(&0)); // 主pipeline
        assert_eq!(pipelines.len(), 1);
        
        let main_pipeline = pipelines.get(&0).unwrap();
        assert_eq!(main_pipeline.steps.len(), 2);
        assert_eq!(main_pipeline.steps[0].label, Some("step1".to_string()));
        assert_eq!(main_pipeline.steps[1].label, Some("step2".to_string()));
    }

    #[test]
    fn test_json_to_pipelines_with_branches() {
        let json_flow = json!({
            "steps": [
                {
                    "condition": {
                        "left": "params.value",
                        "operator": "greater_than",
                        "right": 10
                    },
                    "then": {
                        "steps": [
                            {"label": "then_step1"},
                            {"label": "then_step2"}
                        ]
                    },
                    "else": {
                        "steps": [
                            {"label": "else_step1"},
                            {"label": "else_step2"},
                            {"label": "else_step3"}
                        ]
                    }
                }
            ]
        });
        
        let pipelines = json_to_pipelines(&json_flow).unwrap();
        
        // 应该创建3个pipeline：main(0), then(1), else(2)
        assert_eq!(pipelines.len(), 3);
        
        assert!(pipelines.contains_key(&0));
        assert!(pipelines.contains_key(&1));
        assert!(pipelines.contains_key(&2));
        
        let then_pipeline = pipelines.get(&1).unwrap();
        let else_pipeline = pipelines.get(&2).unwrap();
        
        assert_eq!(then_pipeline.steps.len(), 2);
        assert_eq!(else_pipeline.steps.len(), 3);
    }

    #[test]
    fn test_json_to_pipelines_nested_branches() {
        let json_flow = json!({
            "steps": [
                {
                    "condition": {
                        "left": "params.a",
                        "operator": "greater_than",
                        "right": 10
                    },
                    "then": {
                        "steps": [
                            {
                                "condition": {
                                    "left": "params.b", 
                                    "operator": "less_than",
                                    "right": 5
                                },
                                "then": {
                                    "steps": [
                                        {"label": "nested_then"}
                                    ]
                                },
                                "else": {
                                    "steps": [
                                        {"label": "nested_else"}
                                    ]
                                }
                            }
                        ]
                    }
                }
            ]
        });
        
        let pipelines = json_to_pipelines(&json_flow).unwrap();
        
        // 应该创建4个pipeline：main(0), then(1), nested_then(2), nested_else(3)
        assert_eq!(pipelines.len(), 4);
    }

    #[test]
    fn test_json_to_pipelines_errors() {
        // 缺少steps数组
        let invalid_json = json!({});
        assert!(json_to_pipelines(&invalid_json).is_err());
        
        // steps不是数组
        let invalid_json2 = json!({"steps": "not_an_array"});
        assert!(json_to_pipelines(&invalid_json2).is_err());
        
        // then分支缺少steps
        let invalid_json3 = json!({
            "steps": [
                {
                    "then": {"not_steps": "invalid"}
                }
            ]
        });
        assert!(json_to_pipelines(&invalid_json3).is_err());
    }

    #[test]
    fn test_pipeline_id_assignment() {
        let json_flow = json!({
            "steps": [
                {
                    "then": {
                        "steps": [
                            {
                                "else": {
                                    "steps": [
                                        {"label": "deep"}
                                    ]
                                }
                            }
                        ]
                    }
                },
                {
                    "else": {
                        "steps": [
                            {"label": "another"}
                        ]
                    }
                }
            ]
        });
        
        let pipelines = json_to_pipelines(&json_flow).unwrap();
        
        // 应该分配连续的ID：main(0), then(1), else(2), else(3)
        let expected_ids = vec![0, 1, 2, 3];
        for id in expected_ids {
            assert!(pipelines.contains_key(&id), "Missing pipeline with id {}", id);
        }
    }
}
