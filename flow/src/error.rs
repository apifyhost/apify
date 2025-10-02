use thiserror::Error;

#[derive(Error, Debug)]
pub enum FlowError {
    #[error("JSON parse error: {0}")]
    JsonParseError(#[from] serde_json::Error),

    #[error("Expression error: {0}")]
    ExpressionError(String),

    #[error("Condition error: {0}")]
    ConditionError(String),

    #[error("Step error: {0}")]
    StepError(String),

    #[error("Pipeline error: {0}")]
    PipelineError(String),

    #[error("Transform error: {0}")]
    TransformError(String),

    #[error("Context error: {0}")]
    ContextError(String),

    #[error("Pipeline not found: {0}")]
    PipelineNotFound(usize),

    #[error("Step not found: {0} (Pipeline {1})")]
    StepNotFound(usize, usize),
}
