use crate::{
    context::Context, error::FlowError, pipeline::Pipeline, step::NextStep,
    transform::json_to_pipelines,
};
use serde_json::Value as JsonValue;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Flow {
    pipelines: HashMap<usize, Pipeline>,
    main_pipeline_id: usize,
}

impl Flow {
    pub fn from_json(json: &JsonValue) -> Result<Self, FlowError> {
        let pipelines = json_to_pipelines(json)?;
        if pipelines.is_empty() {
            return Err(FlowError::TransformError("未找到流程定义".into()));
        }

        let main_pipeline_id = 0;
        if !pipelines.contains_key(&main_pipeline_id) {
            return Err(FlowError::PipelineNotFound(main_pipeline_id));
        }

        // 检查主流程是否有步骤
        let main_pipeline = pipelines.get(&main_pipeline_id).unwrap();
        if main_pipeline.steps.is_empty() {
            return Err(FlowError::TransformError("主流程不能为空".into()));
        }

        Ok(Self {
            pipelines,
            main_pipeline_id,
        })
    }

    pub async fn execute(&self, context: &mut Context) -> Result<Option<JsonValue>, FlowError> {
        let mut current_pipeline_id = self.main_pipeline_id;
        let mut current_step_idx = 0;

        loop {
            let pipeline = self
                .pipelines
                .get(&current_pipeline_id)
                .ok_or(FlowError::PipelineNotFound(current_pipeline_id))?;

            let step_output = pipeline.execute(context, current_step_idx).await?;

            match step_output.next_step {
                NextStep::Stop => return Ok(step_output.output),

                NextStep::Next => {
                    current_step_idx += 1;
                    if current_step_idx >= pipeline.steps.len() {
                        return Ok(step_output.output);
                    }
                }

                NextStep::Pipeline(target_id) => {
                    current_pipeline_id = target_id;
                    current_step_idx = 0;
                }

                NextStep::Step {
                    pipeline: target_id,
                    step: target_step,
                } => {
                    current_pipeline_id = target_id;
                    current_step_idx = target_step;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::Context;
    use serde_json::json;

    fn create_test_flow_json() -> JsonValue {
        json!({
            "steps": [
                {
                    "label": "step1",
                    "condition": {
                        "left": "params.value",
                        "operator": "greater_than",
                        "right": 10
                    },
                    "then": {
                        "steps": [
                            {
                                "label": "then_step",
                                "return": {"result": "then_branch"}
                            }
                        ]
                    },
                    "else": {
                        "steps": [
                            {
                                "label": "else_step",
                                "return": {"result": "else_branch"}
                            }
                        ]
                    }
                }
            ]
        })
    }

    #[test]
    fn test_flow_from_json() {
        let flow_json = create_test_flow_json();
        let flow = Flow::from_json(&flow_json).unwrap();

        assert_eq!(flow.main_pipeline_id, 0);
        assert!(flow.pipelines.contains_key(&0));
        // 应该创建了3个pipeline：main(0), then(1), else(2)
        assert!(flow.pipelines.contains_key(&1));
        assert!(flow.pipelines.contains_key(&2));
    }

    #[test]
    fn test_flow_from_json_empty() {
        let empty_json = json!({});
        assert!(Flow::from_json(&empty_json).is_err());

        let no_steps_json = json!({"steps": []});
        assert!(Flow::from_json(&no_steps_json).is_err());
    }

    #[tokio::test]
    async fn test_flow_execute_simple() {
        let flow_json = json!({
            "steps": [
                {
                    "return": {"result": "success"}
                }
            ]
        });

        let flow = Flow::from_json(&flow_json).unwrap();
        let mut ctx = Context::new();

        let result = flow.execute(&mut ctx).await.unwrap();

        assert_eq!(result, Some(json!({"result": "success"})));
    }

    #[tokio::test]
    async fn test_flow_execute_conditional_then() {
        let flow_json = create_test_flow_json();
        let flow = Flow::from_json(&flow_json).unwrap();
        let mut ctx = Context::from_main(json!({"value": 15})); // value > 10

        let result = flow.execute(&mut ctx).await.unwrap();

        assert_eq!(result, Some(json!({"result": "then_branch"})));
    }

    #[tokio::test]
    async fn test_flow_execute_conditional_else() {
        let flow_json = create_test_flow_json();
        let flow = Flow::from_json(&flow_json).unwrap();
        let mut ctx = Context::from_main(json!({"value": 5})); // value <= 10

        let result = flow.execute(&mut ctx).await.unwrap();

        assert_eq!(result, Some(json!({"result": "else_branch"})));
    }

    #[tokio::test]
    async fn test_flow_execute_pipeline_not_found() {
        let flow_json = json!({
            "steps": [
                {
                    "to": {
                        "pipeline": 999, // 不存在的pipeline
                        "step": 0
                    }
                }
            ]
        });

        let flow = Flow::from_json(&flow_json).unwrap();
        let mut ctx = Context::new();

        let result = flow.execute(&mut ctx).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            FlowError::PipelineNotFound(pipeline_id) => {
                assert_eq!(pipeline_id, 999);
            }
            _ => panic!("Expected PipelineNotFound error"),
        }
    }

    #[tokio::test]
    async fn test_flow_execute_multiple_steps() {
        let flow_json = json!({
            "steps": [
                {
                    "label": "step1"
                },
                {
                    "label": "step2",
                    "return": {"result": "final"}
                }
            ]
        });

        let flow = Flow::from_json(&flow_json).unwrap();
        let mut ctx = Context::new();

        let result = flow.execute(&mut ctx).await.unwrap();

        assert_eq!(result, Some(json!({"result": "final"})));
    }

    #[test]
    fn test_flow_clone() {
        let flow_json = create_test_flow_json();
        let flow1 = Flow::from_json(&flow_json).unwrap();
        let flow2 = flow1.clone();

        assert_eq!(flow1.main_pipeline_id, flow2.main_pipeline_id);
        assert_eq!(flow1.pipelines.len(), flow2.pipelines.len());
    }
}
