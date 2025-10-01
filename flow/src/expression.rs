use crate::{context::Context, error::FlowError};
use evalexpr::{Context as EvalContext, Expression, Value};
use serde_json::Value as JsonValue;

/// 解析并执行表达式
pub fn evaluate_expression(
    expr_str: &str,
    context: &Context,
) -> Result<bool, FlowError> {
    // 创建表达式上下文
    let mut eval_context = EvalContext::new();
    
    // 注入上下文变量
    inject_context_variables(&mut eval_context, context)?;
    
    // 解析表达式
    let expression = Expression::new(expr_str)?;
    
    // 执行表达式
    let result = expression.evaluate(&eval_context)?;
    
    // 转换为布尔值
    result.as_boolean().ok_or_else(|| {
        FlowError::ExpressionError(format!(
            "Expression '{}' did not return a boolean value",
            expr_str
        ))
    }).map(|b| b)
}

/// 将flow上下文变量注入到表达式引擎中
fn inject_context_variables(
    eval_context: &mut EvalContext,
    context: &Context,
) -> Result<(), FlowError> {
    // 注入params
    if let Some(params) = context.get_main() {
        inject_json_value(eval_context, "params", params)?;
    }
    
    // 注入payload
    if let Some(payload) = context.get_payload() {
        inject_json_value(eval_context, "payload", payload)?;
    }
    
    // 注入input
    if let Some(input) = context.get_input() {
        inject_json_value(eval_context, "input", input)?;
    }
    
    Ok(())
}

/// 将JSON值注入到表达式上下文
fn inject_json_value(
    eval_context: &mut EvalContext,
    name: &str,
    value: &JsonValue,
) -> Result<(), FlowError> {
    let eval_value = convert_json_to_eval_value(value)?;
    eval_context.set_value(name.to_string(), eval_value)?;
    Ok(())
}

/// 转换serde_json::Value到evalexpr::Value
fn convert_json_to_eval_value(
    json: &JsonValue,
) -> Result<Value, FlowError> {
    match json {
        JsonValue::Null => Ok(Value::Null),
        JsonValue::Bool(b) => Ok(Value::Boolean(*b)),
        JsonValue::Number(n) => {
            // 尝试转换为整数或浮点数
            if let Some(i) = n.as_i64() {
                Ok(Value::Int(i))
            } else {
                n.as_f64()
                    .map(Value::Float)
                    .ok_or_else(|| FlowError::ExpressionError("Invalid number format".into()))
            }
        }
        JsonValue::String(s) => Ok(Value::String(s.clone())),
        JsonValue::Array(arr) => {
            let mut elements = Vec::new();
            for elem in arr {
                elements.push(convert_json_to_eval_value(elem)?);
            }
            Ok(Value::Array(elements))
        }
        JsonValue::Object(obj) => {
            let mut map = std::collections::HashMap::new();
            for (key, value) in obj {
                map.insert(key.clone(), convert_json_to_eval_value(value)?);
            }
            Ok(Value::Map(map))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_evaluate_expression() {
        let context = Context::from_main(json!({
            "age": 20,
            "income": 6000.0,
            "name": "Alice"
        }));
        
        // 测试简单比较
        assert!(evaluate_expression("params.age >= 18", &context).unwrap());
        assert!(evaluate_expression("params.income > 5000", &context).unwrap());
        
        // 测试逻辑运算
        assert!(evaluate_expression(
            "params.age >= 18 && params.income > 5000",
            &context
        ).unwrap());
        
        // 测试字符串比较
        assert!(evaluate_expression(
            "params.name == 'Alice'",
            &context
        ).unwrap());
    }
}
