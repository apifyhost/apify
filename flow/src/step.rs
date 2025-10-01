use crate::{condition::Condition, context::Context, error::FlowError, utils::generate_step_id};
use serde_json::Value as JsonValue;

#[derive(Debug, Clone, PartialEq)]
pub enum NextStep {
    Stop,
    Next,
    Pipeline(usize),
    Step {
        pipeline: usize,
        step: usize,
    },
}

#[derive(Debug, Clone)]
pub struct StepOutput {
    pub next_step: NextStep,
    pub output: Option<JsonValue>,
}

#[derive(Debug, Clone)]
pub struct Step {
    pub id: String,
    pub label: Option<String>,
    pub condition: Option<Condition>,
    pub return_val: Option<JsonValue>,
    pub then_pipeline: Option<usize>,
    pub else_pipeline: Option<usize>,
    pub to_step: Option<(usize, usize)>,
}

impl Step {
    pub fn from_json(
        pipeline_id: usize,
        step_idx: usize,
        value: &JsonValue,
    ) -> Result<Self, FlowError> {
        Ok(Self {
            id: generate_step_id(pipeline_id, step_idx),
            label: value["label"].as_str().map(|s| s.to_string()),
            condition: value.get("condition").map(Condition::from_json).transpose()?,
            return_val: value.get("return").cloned(),
            then_pipeline: value["then"]["pipeline_id"].as_u64().map(|x| x as usize),
            else_pipeline: value["else"]["pipeline_id"].as_u64().map(|x| x as usize),
            to_step: if let Some(to) = value.get("to") {
                Some((
                    to["pipeline"].as_u64().ok_or_else(|| FlowError::StepError("缺少 'to.pipeline'".into()))? as usize,
                    to["step"].as_u64().ok_or_else(|| FlowError::StepError("缺少 'to.step'".into()))? as usize,
                ))
            } else {
                None
            },
        })
    }

    pub async fn execute(&self, context: &Context) -> Result<StepOutput, FlowError> {
        // 直接返回
        if let Some(return_val) = &self.return_val {
            return Ok(StepOutput {
                next_step: NextStep::Stop,
                output: Some(return_val.clone()),
            });
        }

        // 直接跳转
        if let Some((pipeline, step)) = self.to_step {
            return Ok(StepOutput {
                next_step: NextStep::Step { pipeline, step },
                output: None,
            });
        }

        // 条件判断
        if let Some(condition) = &self.condition {
            let next_step = if condition.evaluate(context)? {
                self.then_pipeline.clone().map(NextStep::Pipeline).unwrap_or(NextStep::Next)
            } else {
                self.else_pipeline.clone().map(NextStep::Pipeline).unwrap_or(NextStep::Next)
            };
            return Ok(StepOutput { next_step, output: None });
        }

        // 默认下一步
        Ok(StepOutput {
            next_step: NextStep::Next,
            output: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::Context;
    use serde_json::json;

    #[test]
    fn test_step_from_json_basic() {
        let json_step = json!({
            "label": "test_step",
            "return": {"result": "success"}
        });
        
        let step = Step::from_json(0, 0, &json_step).unwrap();
        
        assert_eq!(step.id, "p0_s0");
        assert_eq!(step.label, Some("test_step".to_string()));
        assert_eq!(step.return_val, Some(json!({"result": "success"})));
        assert!(step.condition.is_none());
        assert!(step.then_pipeline.is_none());
        assert!(step.else_pipeline.is_none());
        assert!(step.to_step.is_none());
    }

    #[test]
    fn test_step_from_json_with_condition() {
        let json_step = json!({
            "condition": {
                "left": "params.value",
                "operator": "greater_than",
                "right": 10
            },
            "then": {
                "pipeline_id": 1
            },
            "else": {
                "pipeline_id": 2
            }
        });
        
        let step = Step::from_json(0, 0, &json_step).unwrap();
        
        assert!(step.condition.is_some());
        assert_eq!(step.then_pipeline, Some(1));
        assert_eq!(step.else_pipeline, Some(2));
    }

    #[test]
    fn test_step_from_json_with_to_step() {
        let json_step = json!({
            "to": {
                "pipeline": 1,
                "step": 2
            }
        });
        
        let step = Step::from_json(0, 0, &json_step).unwrap();
        
        assert_eq!(step.to_step, Some((1, 2)));
    }

    #[tokio::test]
    async fn test_step_execute_return() {
        let ctx = Context::new();
        let step = Step {
            id: "test".to_string(),
            label: None,
            condition: None,
            return_val: Some(json!({"result": "immediate"})),
            then_pipeline: None,
            else_pipeline: None,
            to_step: None,
        };
        
        let output = step.execute(&ctx).await.unwrap();
        
        assert_eq!(output.next_step, NextStep::Stop);
        assert_eq!(output.output, Some(json!({"result": "immediate"})));
    }

    #[tokio::test]
    async fn test_step_execute_to_step() {
        let ctx = Context::new();
        let step = Step {
            id: "test".to_string(),
            label: None,
            condition: None,
            return_val: None,
            then_pipeline: None,
            else_pipeline: None,
            to_step: Some((1, 2)),
        };
        
        let output = step.execute(&ctx).await.unwrap();
        
        assert_eq!(output.next_step, NextStep::Step { pipeline: 1, step: 2 });
        assert!(output.output.is_none());
    }

    #[tokio::test]
    async fn test_step_execute_condition_true() {
        let ctx = Context::from_main(json!({"value": 15}));
        let condition = Condition::from_json(&json!({
            "left": "params.value",
            "operator": "greater_than",
            "right": 10
        })).unwrap();
        
        let step = Step {
            id: "test".to_string(),
            label: None,
            condition: Some(condition),
            return_val: None,
            then_pipeline: Some(1),
            else_pipeline: Some(2),
            to_step: None,
        };
        
        let output = step.execute(&ctx).await.unwrap();
        
        assert_eq!(output.next_step, NextStep::Pipeline(1));
        assert!(output.output.is_none());
    }

    #[tokio::test]
    async fn test_step_execute_condition_false() {
        let ctx = Context::from_main(json!({"value": 5}));
        let condition = Condition::from_json(&json!({
            "left": "params.value",
            "operator": "greater_than",
            "right": 10
        })).unwrap();
        
        let step = Step {
            id: "test".to_string(),
            label: None,
            condition: Some(condition),
            return_val: None,
            then_pipeline: Some(1),
            else_pipeline: Some(2),
            to_step: None,
        };
        
        let output = step.execute(&ctx).await.unwrap();
        
        assert_eq!(output.next_step, NextStep::Pipeline(2));
        assert!(output.output.is_none());
    }

    #[tokio::test]
    async fn test_step_execute_default_next() {
        let ctx = Context::new();
        let step = Step {
            id: "test".to_string(),
            label: None,
            condition: None,
            return_val: None,
            then_pipeline: None,
            else_pipeline: None,
            to_step: None,
        };
        
        let output = step.execute(&ctx).await.unwrap();
        
        assert_eq!(output.next_step, NextStep::Next);
        assert!(output.output.is_none());
    }
}