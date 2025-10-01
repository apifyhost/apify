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
