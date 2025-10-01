//! A lightweight dynamic rule-based workflow engine.
//! 
//! # Overview
//! Flow is a flexible workflow engine that executes JSON-defined workflows
//! with conditional logic and step-based execution.
//! 
//! # Example
//! ```rust
//! use flow_engine::{Flow, Context};
//! use serde_json::json;
//! 
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // 定义工作流
//!     let workflow = json!({
//!         "steps": [
//!             {
//!                 "label": "Check age",
//!                 "condition": {
//!                     "left": "params.age",
//!                     "right": "18",
//!                     "operator": "greater_than_or_equal"
//!                 },
//!                 "then": {
//!                     "steps": [
//!                         {
//!                             "label": "Check income",
//!                             "condition": {
//!                                 "left": "params.income",
//!                                 "right": "5000.0",
//!                                 "operator": "greater_than_or_equal"
//!                             },
//!                             "then": { "return": "Approved" },
//!                             "else": { "return": "Rejected - Insufficient income" }
//!                         }
//!                     ]
//!                 },
//!                 "else": { "return": "Rejected - Underage" }
//!             }
//!         ]
//!     });
//! 
//!     // 创建flow实例
//!     let flow = Flow::from_json(&workflow)?;
//!     
//!     // 执行工作流
//!     let mut context = Context::from_main(json!({ "age": 20, "income": 6000.0 }));
//!     let result = flow.execute(&mut context).await?;
//!     
//!     println!("Result: {:?}", result);  // 输出: Some("Approved")
//!     Ok(())
//! }
//! ```

pub mod context;
pub mod error;
pub mod expression;
pub mod flow;
pub mod pipeline;
pub mod step;
pub mod transform;

// 导出核心类型
pub use context::Context;
pub use error::FlowError;
pub use flow::Flow;
