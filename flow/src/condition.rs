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
                JsonValue::Bool(b) => {
                    if *b {
                        "true".to_string()
                    } else {
                        "false".to_string()
                    }
                },
                JsonValue::String(s) => format!("\"{}\"", s.escape_default()),
                _ => return Err(FlowError::ConditionError("不支持的 'right' 类型".into())),
            }
        };

        let operator = value["operator"]
            .as_str()
            .ok_or_else(|| FlowError::ConditionError("缺少 'operator' 字段".into()))?;

        // 将变量名从 params.x 转换为 params_x 格式
        let left = left.replace('.', "_");

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::Context;
    use serde_json::json;

    #[test]
    fn test_condition_from_json() {
        let json_condition = json!({
            "left": "params.age",
            "operator": "greater_than",
            "right": 18
        });
        
        let condition = Condition::from_json(&json_condition).unwrap();
        assert_eq!(condition.expr_str, "params_age > 18");
    }

    #[test]
    fn test_all_operators() {
        let test_cases = vec![
            ("equal", "=="),
            ("not_equal", "!="),
            ("greater_than", ">"),
            ("less_than", "<"),
            ("greater_than_or_equal", ">="),
            ("less_than_or_equal", "<="),
            ("and", "&&"),
            ("or", "||"),
        ];
        
        for (operator, expected_op) in test_cases {
            let json_condition = json!({
                "left": "params.value",
                "operator": operator,
                "right": 10
            });
            
            let condition = Condition::from_json(&json_condition).unwrap();
            assert!(condition.expr_str.contains(expected_op));
        }
    }

    #[test]
    fn test_condition_evaluation() {
        let ctx = Context::from_main(json!({ 
            "age": 25,
            "score": 85,
            "active": true
        }));
        
        let condition1 = Condition::from_json(&json!({
            "left": "params.age",
            "operator": "greater_than",
            "right": 18
        })).unwrap();
        
        let condition2 = Condition::from_json(&json!({
            "left": "params.score", 
            "operator": "less_than",
            "right": 90
        })).unwrap();
        
        let condition3 = Condition::from_json(&json!({
            "left": "params.active",
            "operator": "equal", 
            "right": true
        })).unwrap();
        
        // Add debug output
        println!("Condition 1 expr: {}", condition1.expr_str);
        println!("Condition 2 expr: {}", condition2.expr_str);
        println!("Condition 3 expr: {}", condition3.expr_str);
        
        // Test each condition individually
        match condition3.evaluate(&ctx) {
            Ok(result) => {
                println!("Condition 3 result: {}", result);
                assert!(result, "Condition 3 should evaluate to true");
            }
            Err(e) => {
                panic!("Condition 3 evaluation failed: {}", e);
            }
        }
        
        assert!(condition1.evaluate(&ctx).unwrap());
        assert!(condition2.evaluate(&ctx).unwrap());
        assert!(condition3.evaluate(&ctx).unwrap());
    }

    #[test]
    fn test_condition_errors() {
        // 缺少必需字段
        assert!(Condition::from_json(&json!({})).is_err());
        assert!(Condition::from_json(&json!({"left": "value"})).is_err());
        
        // 不支持的运算符
        assert!(Condition::from_json(&json!({
            "left": "params.value",
            "operator": "unknown",
            "right": 10
        })).is_err());
    }

    #[test]
    fn test_condition_with_different_right_types() {
        let test_cases = vec![
            json!(42),           // 数字
            json!(true),         // 布尔值
            json!("string"),     // 字符串
        ];
        
        for right_value in test_cases {
            let json_condition = json!({
                "left": "params.test",
                "operator": "equal",
                "right": right_value
            });
            
            assert!(Condition::from_json(&json_condition).is_ok());
        }
    }
}
