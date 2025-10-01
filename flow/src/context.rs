use serde_json::Value as JsonValue;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Context {
    main: JsonValue,          // 主输入参数
    payload: Option<JsonValue>, // 中间结果
    step_outputs: HashMap<String, JsonValue>, // 步骤输出
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
        Self { main, ..Self::new() }
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
            "params" => self.get_path(&self.main, &parts[1..]),
            "payload" => self.payload.as_ref().and_then(|p| self.get_path(p, &parts[1..])),
            "steps" => parts.get(1).and_then(|id| self.step_outputs.get(*id)).cloned(),
            _ => None,
        }
    }

    /// 按路径获取JSON值
    fn get_path(&self, value: &JsonValue, parts: &[&str]) -> Option<JsonValue> {
        if parts.is_empty() {
            return Some(value.clone());
        }

        match value {
            JsonValue::Object(map) => map.get(parts[0]).and_then(|v| self.get_path(v, &parts[1..])),
            JsonValue::Array(arr) => parts[0]
                .parse::<usize>()
                .ok()
                .and_then(|i| arr.get(i))
                .and_then(|v| self.get_path(v, &parts[1..])),
            _ => None,
        }
    }
}
