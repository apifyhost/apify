use crate::{
    context::Context,
    error::FlowError,
    expression::evaluate_expression,
};
use serde::Serialize;
use serde_json::Value as JsonValue;
use std::fmt;

/// 步骤执行后的下一步指令
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum NextStep {
    /// 终止工作流
    Stop,
    /// 执行当前流程的下一步
    Next,
    /// 跳转到指定流程
    Pipeline(usize),
    /// 跳转到指定流程的指定步骤
    Step { pipeline: usize, step: usize },
}

/// 步骤执行结果
#[derive(Debug, Clone)]
pub struct StepOutput {
    pub next_step: NextStep,
    pub output: Option<JsonValue>,
}

/// 条件判断结构
#[derive(Debug, Clone)]
pub struct Condition {
    left: String,
    right: String,
    operator: String,
}

impl Condition {
    /// 从JSON创建条件
    pub fn from_json(value: &JsonValue) -> Result<Self, FlowError> {
        let left = value["left"]
            .as_str()
            .ok_or_else(|| FlowError::StepError("Missing 'left' in condition".into()))?
            .to_string();
            
        let right = value["right"]
            .as_str()
            .ok_or_else(|| FlowError::StepError("Missing 'right' in condition".into()))?
            .to_string();
            
        let operator = value["operator"]
            .as_str()
            .ok_or_else(|| FlowError::StepError("Missing 'operator' in condition".into()))?
            .to_string();
            
        Ok(Self { left, right, operator })
    }
    
    /// 评估条件是否成立
    pub fn evaluate(&self, context: &Context) -> Result<bool, FlowError> {
        // 构建表达式字符串
        let expr = match self.operator.as_str() {
            "equal" => format!("{} == {}", self.left, self.right),
            "not_equal" => format!("{} != {}", self.left, self.right),
            "greater_than" => format!("{} > {}", self.left, self.right),
            "less_than" => format!("{} < {}", self.left, self.right),
            "greater_than_or_equal" => format!("{} >= {}", self.left, self.right),
            "less_than_or_equal" => format!("{} <= {}", self.left, self.right),
            "contains" => format!("{} contains {}", self.left, self.right),
            "not_contains" => format!("!({} contains {})", self.left, self.right),
            "and" => format!("{} && {}", self.left, self.right),
            "or" => format!("{} || {}", self.left, self.right),
            _ => return Err(FlowError::StepError(format!(
                "Unsupported operator: {}", self.operator
            ))),
        };
        
        evaluate_expression(&expr, context)
    }
}

/// 工作流步骤
#[derive(Debug, Clone)]
pub struct Step {
    id: String,
    label: Option<String>,
    condition: Option<Condition>,
    then_pipeline: Option<usize>,
    else_pipeline: Option<usize>,
    return_value: Option<JsonValue>,
    to_step: Option<(usize, usize)>, // (pipeline, step)
}

impl Step {
    /// 从JSON创建步骤
    pub fn from_json(id: String, value: &JsonValue) -> Result<Self, FlowError> {
        let label = value["label"].as_str().map(|s| s.to_string());
        
        // 解析条件
        let condition = if let Some(cond_val) = value.get("condition") {
            Some(Condition::from_json(cond_val)?)
        } else {
            None
        };
        
        // 解析then分支
        let then_pipeline = value["then"].as_u64().map(|v| v as usize);
        
        // 解析else分支
        let else_pipeline = value["else"].as_u64().map(|v| v as usize);
        
        // 解析return值
        let return_value = value.get("return").cloned();
        
        // 解析跳转步骤
        let to_step = if let Some(to) = value.get("to") {
            if let (Some(pipeline), Some(step)) = (
                to["pipeline"].as_u64(),
                to["step"].as_u64()
            ) {
                Some((pipeline as usize, step as usize))
            } else {
                None
            }
        } else {
            None
        };
        
        Ok(Self {
            id,
            label,
            condition,
            then_pipeline,
            else_pipeline,
            return_value,
            to_step,
        })
    }
    
    /// 执行步骤
    pub async fn execute(&self, context: &Context) -> Result<StepOutput, FlowError> {
        // 如果有return值，直接返回并终止
        if let Some(return_val) = &self.return_value {
            return Ok(StepOutput {
                next_step: NextStep::Stop,
                output: Some(return_val.clone()),
            });
        }
        
        // 如果有明确的跳转目标，直接跳转
        if let Some((pipeline, step)) = self.to_step {
            return Ok(StepOutput {
                next_step: NextStep::Step { pipeline, step },
                output: None,
            });
        }
        
        // 处理条件判断
        let condition_met = if let Some(condition) = &self.condition {
            condition.evaluate(context)?
        } else {
            // 没有条件时默认执行下一步
            true
        };
        
        // 根据条件结果决定下一步
        let next_step = if condition_met {
            if let Some(pipeline_id) = self.then_pipeline {
                NextStep::Pipeline(pipeline_id)
            } else {
                NextStep::Next
            }
        } else {
            if let Some(pipeline_id) = self.else_pipeline {
                NextStep::Pipeline(pipeline_id)
            } else {
                NextStep::Next
            }
        };
        
        Ok(StepOutput {
            next_step,
            output: context.get_payload().cloned(),
        })
    }
    
    pub fn get_id(&self) -> &str {
        &self.id
    }
    
    pub fn get_label(&self) -> Option<&str> {
        self.label.as_deref()
    }
}

impl fmt::Display for Step {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Step {}: {}", self.id, self.label.as_deref().unwrap_or("Unlabeled"))
    }
}
