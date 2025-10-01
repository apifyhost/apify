use crate::{
    context::Context,
    error::FlowError,
    step::{Step, StepOutput},
};
use serde_json::Value as JsonValue;

/// 流程错误
#[derive(Debug)]
pub enum PipelineError {
    StepError(FlowError),
    StepNotFound(usize),
}

impl std::fmt::Display for PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PipelineError::StepError(err) => write!(f, "Step error: {}", err),
            PipelineError::StepNotFound(step_id) => write!(f, "Step {} not found", step_id),
        }
    }
}

impl std::error::Error for PipelineError {}

impl From<FlowError> for PipelineError {
    fn from(err: FlowError) -> Self {
        PipelineError::StepError(err)
    }
}

/// 流程结构，包含多个步骤
#[derive(Debug, Clone)]
pub struct Pipeline {
    id: usize,
    steps: Vec<Step>,
}

impl Pipeline {
    /// 创建新流程
    pub fn new(id: usize, steps: Vec<Step>) -> Self {
        Self { id, steps }
    }
    
    /// 获取流程ID
    pub fn get_id(&self) -> usize {
        self.id
    }
    
    /// 获取步骤数量
    pub fn step_count(&self) -> usize {
        self.steps.len()
    }
    
    /// 执行流程，从指定步骤开始
    pub async fn execute(
        &self,
        context: &mut Context,
        start_step: usize,
    ) -> Result<Option<StepOutput>, PipelineError> {
        if start_step >= self.steps.len() {
            return Err(PipelineError::StepNotFound(start_step));
        }
        
        for (step_idx, step) in self.steps.iter().enumerate().skip(start_step) {
            let step_output = step.execute(context).await?;
            
            // 记录步骤输出
            context.add_step_id_output(step.get_id().to_string(), step_output.output.clone().unwrap_or(JsonValue::Null));
            
            match step_output.next_step {
                // 如果需要停止或跳转到其他流程/步骤，返回控制
                crate::step::NextStep::Stop | 
                crate::step::NextStep::Pipeline(_) |
                crate::step::NextStep::Step { .. } => {
                    return Ok(Some(step_output));
                }
                
                // 继续执行下一步
                crate::step::NextStep::Next => {
                    // 如果是最后一步，返回完成
                    if step_idx == self.steps.len() - 1 {
                        return Ok(Some(step_output));
                    }
                }
            }
        }
        
        Ok(None)
    }
}
