use serde_json::Value as JsonValue;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Context {
    main: JsonValue,                          // 主输入参数
    payload: Option<JsonValue>,               // 中间结果
    step_outputs: HashMap<String, JsonValue>, // 步骤输出
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}

impl Context {
    pub fn new() -> Self {
        Self {
            main: JsonValue::Null,
            payload: None,
            step_outputs: HashMap::new(),
        }
    }

    pub fn from_main(main: JsonValue) -> Self {
        Self {
            main,
            ..Self::new()
        }
    }

    pub fn get_main(&self) -> &JsonValue {
        &self.main
    }

    pub fn set_main(&mut self, main: JsonValue) {
        self.main = main;
    }

    pub fn get_payload(&self) -> Option<JsonValue> {
        self.payload.clone()
    }

    pub fn set_payload(&mut self, payload: JsonValue) {
        self.payload = Some(payload);
    }

    pub fn add_step_output(&mut self, step_id: String, output: JsonValue) {
        self.step_outputs.insert(step_id, output);
    }

    pub fn get_step_output(&self, step_id: &str) -> Option<&JsonValue> {
        self.step_outputs.get(step_id)
    }

    /// 获取变量值（支持 params.xxx, payload.xxx, steps.xxx）
    pub fn get_variable(&self, path: &str) -> Option<JsonValue> {
        let parts: Vec<&str> = path.split('.').collect();
        if parts.is_empty() {
            return None;
        }

        match parts[0] {
            "params" => Self::get_path(&self.main, &parts[1..]),
            "payload" => self
                .payload
                .as_ref()
                .and_then(|p| Self::get_path(p, &parts[1..])),
            "steps" => parts
                .get(1)
                .and_then(|id| self.step_outputs.get(*id))
                .cloned(),
            _ => None,
        }
    }

    /// 按路径获取JSON值
    fn get_path(value: &JsonValue, parts: &[&str]) -> Option<JsonValue> {
        if parts.is_empty() {
            return Some(value.clone());
        }

        match value {
            JsonValue::Object(map) => map
                .get(parts[0])
                .and_then(|v| Self::get_path(v, &parts[1..])),
            JsonValue::Array(arr) => parts[0]
                .parse::<usize>()
                .ok()
                .and_then(|i| arr.get(i))
                .and_then(|v| Self::get_path(v, &parts[1..])),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_context_creation() {
        let ctx = Context::new();
        assert!(ctx.get_main().is_null());
        assert!(ctx.get_payload().is_none());
    }

    #[test]
    fn test_context_from_main() {
        let main_data = json!({"name": "test", "value": 42});
        let ctx = Context::from_main(main_data.clone());

        assert_eq!(ctx.get_main(), &main_data);
        assert!(ctx.get_payload().is_none());
    }

    #[test]
    fn test_set_payload() {
        let mut ctx = Context::new();
        let payload = json!({"result": "success"});

        ctx.set_payload(payload.clone());
        assert_eq!(ctx.get_payload(), Some(payload));
    }

    #[test]
    fn test_step_outputs() {
        let mut ctx = Context::new();
        let step_id = "p0_s1".to_string();
        let output = json!({"data": "processed"});

        ctx.add_step_output(step_id.clone(), output.clone());
        assert_eq!(ctx.get_step_output(&step_id), Some(&output));
        assert!(ctx.get_step_output("nonexistent").is_none());
    }

    #[test]
    fn test_get_variable_params() {
        let main_data = json!({
            "user": {
                "name": "Alice",
                "age": 25
            },
            "scores": [85, 90, 78]
        });
        let ctx = Context::from_main(main_data);

        assert_eq!(ctx.get_variable("params.user.name"), Some(json!("Alice")));
        assert_eq!(ctx.get_variable("params.user.age"), Some(json!(25)));
        assert_eq!(ctx.get_variable("params.scores.0"), Some(json!(85)));
        assert_eq!(ctx.get_variable("params.nonexistent"), None);
    }

    #[test]
    fn test_get_variable_payload() {
        let mut ctx = Context::new();
        let payload = json!({
            "result": {
                "status": "success",
                "data": [1, 2, 3]
            }
        });
        ctx.set_payload(payload);

        assert_eq!(
            ctx.get_variable("payload.result.status"),
            Some(json!("success"))
        );
        assert_eq!(ctx.get_variable("payload.result.data.1"), Some(json!(2)));
    }

    #[test]
    fn test_get_variable_steps() {
        let mut ctx = Context::new();
        let step_output = json!({"output": "step1_result"});

        ctx.add_step_output("p0_s0".to_string(), step_output.clone());
        assert_eq!(ctx.get_variable("steps.p0_s0"), Some(step_output));
    }

    #[test]
    fn test_clone_context() {
        let mut ctx1 = Context::from_main(json!({"data": "original"}));
        ctx1.set_payload(json!({"result": "test"}));
        ctx1.add_step_output("step1".to_string(), json!({"output": "data"}));

        let ctx2 = ctx1.clone();

        assert_eq!(ctx1.get_main(), ctx2.get_main());
        assert_eq!(ctx1.get_payload(), ctx2.get_payload());
        assert_eq!(ctx1.get_step_output("step1"), ctx2.get_step_output("step1"));
    }
}
