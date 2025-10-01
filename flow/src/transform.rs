use crate::{
    error::FlowError,
    pipeline::Pipeline,
    step::Step,
};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use uuid::Uuid;

/// 将JSON配置转换为流程映射
pub fn json_to_pipelines(json: &JsonValue) -> Result<HashMap<usize, Pipeline>, FlowError> {
    let mut pipelines = HashMap::new();
    let mut pipeline_counter = 0;
    
    // 处理主流程
    process_pipeline(json, pipeline_counter, &mut pipelines, &mut pipeline_counter)?;
    
    Ok(pipelines)
}

/// 递归处理流程配置
fn process_pipeline(
    json: &JsonValue,
    pipeline_id: usize,
    pipelines: &mut HashMap<usize, Pipeline>,
    counter: &mut usize,
) -> Result<(), FlowError> {
    // 提取步骤数组
    let steps_json = json.get("steps")
        .and_then(|v| v.as_array())
        .ok_or_else(|| FlowError::ParseError("Missing or invalid 'steps' array".into()))?;
    
    let mut steps = Vec::with_capacity(steps_json.len());
    
    // 处理每个步骤
    for step_json in steps_json {
        // 生成唯一步骤ID
        let step_id = Uuid::new_v4().to_string();
        let step = Step::from_json(step_id, step_json)?;
        
        // 递归处理嵌套的then流程
        if let Some(then_json) = step_json.get("then") {
            *counter += 1;
            let then_pipeline_id = *counter;
            
            // 记录then分支对应的流程ID
            // (在Step::from_json中已经处理)
            
            // 递归处理子流程
            process_pipeline(then_json, then_pipeline_id, pipelines, counter)?;
        }
        
        // 递归处理嵌套的else流程
        if let Some(else_json) = step_json.get("else") {
            *counter += 1;
            let else_pipeline_id = *counter;
            
            // 递归处理子流程
            process_pipeline(else_json, else_pipeline_id, pipelines, counter)?;
        }
        
        steps.push(step);
    }
    
    // 创建并存储流程
    pipelines.insert(pipeline_id, Pipeline::new(pipeline_id, steps));
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_json_to_pipelines() {
        let workflow = json!({
            "steps": [
                {
                    "label": "Check age",
                    "condition": {
                        "left": "params.age",
                        "right": "18",
                        "operator": "greater_than_or_equal"
                    },
                    "then": {
                        "steps": [
                            {
                                "label": "Check income",
                                "condition": {
                                    "left": "params.income",
                                    "right": "5000.0",
                                    "operator": "greater_than_or_equal"
                                },
                                "then": { "return": "Approved" },
                                "else": { "return": "Rejected - Insufficient income" }
                            }
                        ]
                    },
                    "else": { "return": "Rejected - Underage" }
                }
            ]
        });
        
        let pipelines = json_to_pipelines(&workflow).unwrap();
        
        // 主流程 + then流程 + else流程 = 3个流程
        assert_eq!(pipelines.len(), 3);
        
        // 检查主流程步骤数
        assert_eq!(pipelines.get(&0).unwrap().step_count(), 1);
        
        // 检查then流程步骤数
        assert_eq!(pipelines.get(&1).unwrap().step_count(), 1);
    }
}
