use serde_json::{Map, Value as JsonValue};
use std::collections::HashMap;

/// 工作流执行上下文，存储所有执行过程中的数据
#[derive(Debug, Clone, Default)]
pub struct Context {
    main: Option<JsonValue>,      // 主输入数据
    payload: Option<JsonValue>,   // 流程执行中的中间结果
    input: Option<JsonValue>,     // 当前步骤的输入
    steps: Vec<JsonValue>,        // 所有步骤的输出记录
    step_outputs: HashMap<String, JsonValue>, // 按步骤ID存储的输出
}

impl Context {
    /// 创建新的空上下文
    pub fn new() -> Self {
        Self::default()
    }

    /// 从主输入数据创建上下文
    pub fn from_main(main: JsonValue) -> Self {
        Self {
            main: Some(main),
            ..Self::default()
        }
    }

    // Getters
    pub fn get_main(&self) -> Option<&JsonValue> {
        self.main.as_ref()
    }

    pub fn get_payload(&self) -> Option<&JsonValue> {
        self.payload.as_ref()
    }

    pub fn get_input(&self) -> Option<&JsonValue> {
        self.input.as_ref()
    }

    pub fn get_steps(&self) -> &Vec<JsonValue> {
        &self.steps
    }

    pub fn get_step_output(&self, step_id: &str) -> Option<&JsonValue> {
        self.step_outputs.get(step_id)
    }

    // Setters
    pub fn set_main(&mut self, main: JsonValue) {
        self.main = Some(main);
    }

    pub fn set_payload(&mut self, payload: JsonValue) {
        self.payload = Some(payload);
    }

    pub fn set_input(&mut self, input: JsonValue) {
        self.input = Some(input);
    }

    /// 添加步骤输出到历史记录
    pub fn add_step_payload(&mut self, payload: Option<JsonValue>) {
        if let Some(payload) = payload {
            self.steps.push(payload.clone());
            self.payload = Some(payload);
        }
    }

    /// 按步骤ID记录输出
    pub fn add_step_id_output(&mut self, step_id: String, payload: JsonValue) {
        self.step_outputs.insert(step_id, payload.clone());
        self.payload = Some(payload);
    }

    /// 合并另一个上下文的数据
    pub fn merge(&mut self, other: &Context) {
        if self.main.is_none() {
            self.main = other.main.clone();
        }
        
        if let Some(payload) = &other.payload {
            self.payload = Some(payload.clone());
        }
        
        self.steps.extend(other.steps.iter().cloned());
        
        for (k, v) in &other.step_outputs {
            self.step_outputs.insert(k.clone(), v.clone());
        }
    }
}
