//! 轻量级工作流引擎，支持JSON配置和条件分支

pub mod condition;
pub mod context;
pub mod error;
pub mod expression;
pub mod flow;
pub mod pipeline;
pub mod step;
pub mod transform;
pub mod utils;

// 导出公共类型
pub use context::Context;
pub use error::FlowError;
pub use flow::Flow;
pub use pipeline::Pipeline;
pub use step::{NextStep, Step, StepOutput};
