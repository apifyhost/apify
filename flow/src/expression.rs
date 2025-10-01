use crate::{context::Context, error::FlowError};
use evalexpr::{eval_boolean_with_context, HashMapContext, ContextWithMutableVariables};
use serde_json::Value as JsonValue;

/// 解析并执行表达式（使用 evalexpr）
pub fn evaluate_expression(expr_str: &str, context: &Context) -> Result<bool, FlowError> {
    let mut eval_context = HashMapContext::new();
    
    // 设置变量到上下文中
    set_context_variables(&mut eval_context, context);
    
    eval_boolean_with_context(expr_str, &eval_context)
        .map_err(|e| FlowError::ExpressionError(format!("表达式执行失败: {}", e)))
}

/// 将自定义上下文中的变量设置到 evalexpr 上下文中
fn set_context_variables(eval_context: &mut HashMapContext, context: &Context) {
    // 设置 params 相关的变量
    if let JsonValue::Object(params) = context.get_main() {
        for (key, value) in params {
            if let Some(eval_value) = convert_json_value(value.clone()) {
                // 使用 params_ 前缀来避免命名冲突
                eval_context.set_value(format!("params_{}", key), eval_value).ok();
            }
        }
    }

    // 设置 payload 相关的变量
    if let Some(payload) = context.get_payload() {
        if let JsonValue::Object(payload_obj) = payload {
            for (key, value) in payload_obj {
                if let Some(eval_value) = convert_json_value(value.clone()) {
                    eval_context.set_value(format!("payload_{}", key), eval_value).ok();
                }
            }
        }
    }
}

/// 类型转换
fn convert_json_value(json_val: JsonValue) -> Option<evalexpr::Value> {
    match json_val {
        JsonValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                Some(evalexpr::Value::Int(i))
            } else {
                n.as_f64().map(evalexpr::Value::Float)
            }
        }
        JsonValue::Bool(b) => Some(evalexpr::Value::Boolean(b)),
        JsonValue::String(s) => Some(evalexpr::Value::String(s)),
        JsonValue::Array(arr) => {
            if !arr.is_empty() {
                convert_json_value(arr[0].clone())
            } else {
                Some(evalexpr::Value::Boolean(false))
            }
        }
        JsonValue::Object(_) => Some(evalexpr::Value::Boolean(true)),
        JsonValue::Null => Some(evalexpr::Value::Boolean(false)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::Context;
    use serde_json::json;

    #[test]
    fn test_basic_expressions() {
        let ctx = Context::from_main(json!({ 
            "age": 25, 
            "income": 6000.0, 
            "is_student": false,
            "name": "Alice"
        }));
        
        // 测试基本比较
        assert!(evaluate_expression("params_age >= 18", &ctx).unwrap());
        assert!(evaluate_expression("params_income > 5000", &ctx).unwrap());
        assert!(evaluate_expression("!params_is_student", &ctx).unwrap());
    }

    #[test]
    fn test_complex_expressions() {
        let ctx = Context::from_main(json!({ 
            "age": 25, 
            "income": 6000.0, 
            "is_student": false
        }));
        
        // 测试复杂表达式
        assert!(evaluate_expression("params_age >= 18 && params_income > 5000", &ctx).unwrap());
        assert!(evaluate_expression("params_age < 30 || params_is_student", &ctx).unwrap());
    }

    #[test]
    fn test_string_comparison() {
        let ctx = Context::from_main(json!({ 
            "name": "Alice"
        }));
        
        // 测试字符串比较
        assert!(evaluate_expression("params_name == \"Alice\"", &ctx).unwrap());
        assert!(evaluate_expression("params_name != \"Bob\"", &ctx).unwrap());
    }
}
