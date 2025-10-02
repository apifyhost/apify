use crate::{context::Context, error::FlowError};
use evalexpr::{eval_boolean_with_context, ContextWithMutableVariables, HashMapContext};
use serde_json::Value as JsonValue;

pub fn evaluate_expression(expr_str: &str, context: &Context) -> Result<bool, FlowError> {
    let mut eval_context = HashMapContext::new();

    set_context_variables(&mut eval_context, context);

    eval_boolean_with_context(expr_str, &eval_context)
        .map_err(|e| FlowError::ExpressionError(format!("表达式执行失败: {e}")))
}

fn set_context_variables(eval_context: &mut HashMapContext, context: &Context) {
    if let JsonValue::Object(params) = context.get_main() {
        for (key, value) in params {
            if let Some(eval_value) = convert_json_value(value.clone()) {
                eval_context
                    .set_value(format!("params_{key}"), eval_value)
                    .ok();
            }
        }
    }

    if let Some(JsonValue::Object(payload_obj)) = context.get_payload() {
        for (key, value) in payload_obj {
            if let Some(eval_value) = convert_json_value(value.clone()) {
                eval_context
                    .set_value(format!("payload_{key}"), eval_value)
                    .ok();
            }
        }
    }
}

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

        assert!(evaluate_expression("params_age >= 18", &ctx).unwrap());
        assert!(evaluate_expression("params_income > 5000", &ctx).unwrap());
        assert!(evaluate_expression("params_age < 30", &ctx).unwrap());
        assert!(evaluate_expression("params_age <= 25", &ctx).unwrap());
        assert!(evaluate_expression("params_age == 25", &ctx).unwrap());
        assert!(evaluate_expression("params_age != 30", &ctx).unwrap());

        assert!(evaluate_expression("!params_is_student", &ctx).unwrap());
        assert!(evaluate_expression("params_age > 20 && !params_is_student", &ctx).unwrap());
        assert!(evaluate_expression("params_age < 30 || params_income > 1000", &ctx).unwrap());

        assert!(evaluate_expression("params_name == \"Alice\"", &ctx).unwrap());
        assert!(evaluate_expression("params_name != \"Bob\"", &ctx).unwrap());
    }

    #[test]
    fn test_complex_expressions() {
        let ctx = Context::from_main(json!({
            "age": 25,
            "income": 6000.0,
            "is_student": false,
            "has_job": true
        }));

        assert!(evaluate_expression(
            "params_age >= 18 && params_income > 5000 && params_has_job && !params_is_student",
            &ctx
        )
        .unwrap());

        assert!(!evaluate_expression(
            "params_age < 18 || params_is_student || params_income < 1000",
            &ctx
        )
        .unwrap());
    }

    #[test]
    fn test_expression_errors() {
        let ctx = Context::from_main(json!({ "value": 10 }));

        assert!(evaluate_expression("invalid syntax", &ctx).is_err());

        assert!(evaluate_expression("undefined_var > 5", &ctx).is_err());
    }

    #[test]
    fn test_edge_cases() {
        let ctx = Context::from_main(json!({
            "zero": 0,
            "negative": -5,
            "empty_string": "",
            "non_empty_string": "hello",
            "bool_true": true,
            "bool_false": false
        }));

        assert!(evaluate_expression("params_zero == 0", &ctx).unwrap());
        assert!(evaluate_expression("params_negative < 0", &ctx).unwrap());
        assert!(evaluate_expression("params_empty_string == \"\"", &ctx).unwrap());
        assert!(evaluate_expression("params_non_empty_string == \"hello\"", &ctx).unwrap());
        assert!(evaluate_expression("params_bool_true", &ctx).unwrap());
        assert!(!evaluate_expression("params_bool_false", &ctx).unwrap());
    }

    #[test]
    fn test_with_payload_variables() {
        let mut ctx = Context::from_main(json!({ "initial": 10 }));
        ctx.set_payload(json!({
            "processed": true,
            "count": 5,
            "message": "done"
        }));

        assert!(evaluate_expression("payload_processed", &ctx).unwrap());
        assert!(evaluate_expression("payload_count == 5", &ctx).unwrap());
        assert!(evaluate_expression("payload_message == \"done\"", &ctx).unwrap());
        assert!(evaluate_expression("params_initial == 10 && payload_processed", &ctx).unwrap());
    }

    #[test]
    fn test_numeric_conversions() {
        let ctx = Context::from_main(json!({
            "int_zero": 0,
            "int_positive": 42,
            "int_negative": -1,
            "float_zero": 0.0,
            "float_positive": 3.22,
            "float_negative": -2.5
        }));

        assert!(evaluate_expression("params_int_zero == 0", &ctx).unwrap());
        assert!(evaluate_expression("params_int_positive == 42", &ctx).unwrap());
        assert!(evaluate_expression("params_int_negative == -1", &ctx).unwrap());
        assert!(evaluate_expression("params_float_zero == 0.0", &ctx).unwrap());
        assert!(evaluate_expression("params_float_positive == 3.22", &ctx).unwrap());
        assert!(evaluate_expression("params_float_negative == -2.5", &ctx).unwrap());
    }
}
