use std::fmt;

#[derive(Debug)]
pub enum FlowError {
    // 解析相关错误
    ParseError(String),
    // 表达式相关错误
    ExpressionError(String),
    // 流程相关错误
    PipelineError(String),
    // 步骤相关错误
    StepError(String),
    // JSON处理错误
    JsonError(serde_json::Error),
    // 执行时错误
    ExecutionError(String),
}

impl fmt::Display for FlowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FlowError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            FlowError::ExpressionError(msg) => write!(f, "Expression error: {}", msg),
            FlowError::PipelineError(msg) => write!(f, "Pipeline error: {}", msg),
            FlowError::StepError(msg) => write!(f, "Step error: {}", msg),
            FlowError::JsonError(err) => write!(f, "JSON error: {}", err),
            FlowError::ExecutionError(msg) => write!(f, "Execution error: {}", msg),
        }
    }
}

impl std::error::Error for FlowError {}

// 从其他错误类型转换
impl From<serde_json::Error> for FlowError {
    fn from(err: serde_json::Error) -> Self {
        FlowError::JsonError(err)
    }
}

impl From<evalexpr::EvalexprError> for FlowError {
    fn from(err: evalexpr::EvalexprError) -> Self {
        FlowError::ExpressionError(err.to_string())
    }
}
