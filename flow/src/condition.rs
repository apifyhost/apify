use crate::{context::Context, error::FlowError, expression::evaluate_expression};
use serde_json::Value as JsonValue;

#[derive(Debug, Clone)]
pub struct Condition {
    expr_str: String,
}

impl Condition {
    pub fn from_json(value: &JsonValue) -> Result<Self, FlowError> {
        let left = value["left"]
            .as_str()
            .ok_or_else(|| FlowError::ConditionError("缺少 'left' 字段".into()))?;

        let right = match value["right"].as_str() {
            Some(s) => s.to_string(),
            None => match &value["right"] {
                JsonValue::Number(n) => n.to_string(),
                JsonValue::Bool(b) => if *b { "1" } else { "0" }.to_string(),
                JsonValue::String(s) => format!("\"{}\"", s.escape_default()),
                _ => return Err(FlowError::ConditionError("不支持的 'right' 类型".into())),
            }
        };

        let operator = value["operator"]
            .as_str()
            .ok_or_else(|| FlowError::ConditionError("缺少 'operator' 字段".into()))?;

        let expr_str = match operator {
            "equal" => format!("{} == {}", left, right),
            "not_equal" => format!("{} != {}", left, right),
            "greater_than" => format!("{} > {}", left, right),
            "less_than" => format!("{} < {}", left, right),
            "greater_than_or_equal" => format!("{} >= {}", left, right),
            "less_than_or_equal" => format!("{} <= {}", left, right),
            "and" => format!("{} && {}", left, right),
            "or" => format!("{} || {}", left, right),
            _ => return Err(FlowError::ConditionError(format!("不支持的运算符: {}", operator))),
        };

        Ok(Self { expr_str })
    }

    pub fn evaluate(&self, context: &Context) -> Result<bool, FlowError> {
        evaluate_expression(&self.expr_str, context)
    }
}
