use crate::{context::Context, error::FlowError, pipeline::Pipeline, step::NextStep, transform::json_to_pipelines};
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

        Ok(Self { pipelines, main_pipeline_id })
    }

    pub async fn execute(&self, context: &mut Context) -> Result<Option<JsonValue>, FlowError> {
        let mut current_pipeline_id = self.main_pipeline_id;
        let mut current_step_idx = 0;

        loop {
            let pipeline = self.pipelines.get(&current_pipeline_id)
                .ok_or_else(|| FlowError::PipelineNotFound(current_pipeline_id))?;

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
                
                NextStep::Step { pipeline: target_id, step: target_step } => {
                    current_pipeline_id = target_id;
                    current_step_idx = target_step;
                }
            }
        }
    }
}
