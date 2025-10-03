use crate::{
    context::Context,
    pipeline::{Pipeline, PipelineError},
    step_worker::NextStep,
    transform::{TransformError, value_to_pipelines},
};
use phs::build_engine;
use sdk::prelude::{log::error, *};
use std::{collections::HashMap, fmt::Display, sync::Arc};

#[derive(Debug)]
pub enum FlowError {
    TransformError(TransformError),
    PipelineError(PipelineError),
    PipelineNotFound,
    ParentError,
}

impl Display for FlowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FlowError::TransformError(err) => write!(f, "Transform error: {err}"),
            FlowError::PipelineError(err) => write!(f, "Pipeline error: {err}"),
            FlowError::PipelineNotFound => write!(f, "Pipeline not found"),
            FlowError::ParentError => write!(f, "Parent error"),
        }
    }
}

impl std::error::Error for FlowError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            FlowError::TransformError(err) => Some(err),
            FlowError::PipelineError(err) => Some(err),
            FlowError::PipelineNotFound => None,
            FlowError::ParentError => None,
        }
    }
}

pub type PipelineMap = HashMap<usize, Pipeline>;

#[derive(Debug, Default)]
pub struct Flow {
    pipelines: PipelineMap,
}

impl Flow {
    pub fn try_from_value(value: &Value, modules: Option<Arc<Modules>>) -> Result<Self, FlowError> {
        let engine = match &modules {
            Some(modules) => {
                let repositories = modules.extract_repositories();
                build_engine(Some(repositories))
            }
            None => build_engine(None),
        };

        let modules = if let Some(modules) = modules {
            modules
        } else {
            Arc::new(Modules::default())
        };

        let pipelines =
            value_to_pipelines(engine, modules, value).map_err(FlowError::TransformError)?;

        Ok(Self { pipelines })
    }

    pub async fn execute(&self, context: &mut Context) -> Result<Option<Value>, FlowError> {
        if self.pipelines.is_empty() {
            return Ok(None);
        }

        let mut current_pipeline = self.pipelines.len() - 1;
        let mut current_step = 0;

        loop {
            log::debug!("Executing pipeline {current_pipeline} step {current_step}");
            let pipeline = self
                .pipelines
                .get(&current_pipeline)
                .ok_or(FlowError::PipelineNotFound)?;

            match pipeline.execute(context, current_step).await {
                Ok(step_output) => match step_output {
                    Some(step_output) => {
                        log::debug!(
                            "Next step decision: {:?}, payload: {:?}",
                            step_output.next_step,
                            step_output.output
                        );
                        match step_output.next_step {
                            NextStep::Stop => {
                                log::debug!("NextStep::Stop - terminating execution");
                                return Ok(step_output.output);
                            }
                            NextStep::Next => {
                                log::debug!(
                                    "NextStep::Next - checking if sub-pipeline needs to return to parent"
                                );
                                // Check if this is the main pipeline (highest index)
                                let main_pipeline = self.pipelines.len() - 1;
                                if current_pipeline == main_pipeline {
                                    log::debug!(
                                        "NextStep::Next - terminating execution (main pipeline completed)"
                                    );
                                    return Ok(step_output.output);
                                } else {
                                    log::debug!(
                                        "NextStep::Next - sub-pipeline completed, checking for parent return"
                                    );
                                    // This is a sub-pipeline that completed - we should return to parent
                                    // For now, terminate execution but this needs proper parent tracking
                                    return Ok(step_output.output);
                                }
                            }
                            NextStep::Pipeline(id) => {
                                log::debug!("NextStep::Pipeline({id}) - jumping to pipeline");
                                current_pipeline = id;
                                current_step = 0;
                            }
                            NextStep::GoToStep(to) => {
                                log::debug!("NextStep::GoToStep({to:?}) - jumping to step");
                                current_pipeline = to.pipeline;
                                current_step = to.step;
                            }
                        }
                    }
                    None => {
                        return Ok(None);
                    }
                },
                Err(err) => {
                    error!("Error executing step: {err:?}");
                    return Err(FlowError::PipelineError(err));
                }
            }
        }
    }
}
