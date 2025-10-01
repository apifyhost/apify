use thiserror::Error;

#[derive(Error, Debug)]
pub enum FlowError {
    #[error("JSON解析错误: {0}")]
    JsonParseError(#[from] serde_json::Error),

    #[error("表达式错误: {0}")]
    ExpressionError(String),

    #[error("条件错误: {0}")]
    ConditionError(String),

    #[error("步骤错误: {0}")]
    StepError(String),

    #[error("流程错误: {0}")]
    PipelineError(String),

    #[error("转换错误: {0}")]
    TransformError(String),

    #[error("上下文错误: {0}")]
    ContextError(String),

    #[error("流程未找到: {0}")]
    PipelineNotFound(usize),

    #[error("步骤未找到: {0} (流程 {1})")]
    StepNotFound(usize, usize),
}
