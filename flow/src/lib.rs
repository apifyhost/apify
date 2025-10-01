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

#[cfg(test)]
mod integration_tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_complete_workflow() {
        // 定义一个完整的工作流
        let workflow_json = json!({
            "steps": [
                {
                    "label": "check_age",
                    "condition": {
                        "left": "params.age",
                        "operator": "greater_than_or_equal",
                        "right": 18
                    },
                    "then": {
                        "steps": [
                            {
                                "label": "adult_processing",
                                "return": {"status": "adult", "message": "Processing adult user"}
                            }
                        ]
                    },
                    "else": {
                        "steps": [
                            {
                                "label": "minor_processing", 
                                "return": {"status": "minor", "message": "Processing minor user"}
                            }
                        ]
                    }
                }
            ]
        });

        // 测试成人分支
        let flow = Flow::from_json(&workflow_json).unwrap();
        let mut ctx = Context::from_main(json!({"age": 25}));
        let result = flow.execute(&mut ctx).await.unwrap();
        
        assert_eq!(result, Some(json!({
            "status": "adult",
            "message": "Processing adult user"
        })));

        // 测试未成年人分支
        let flow = Flow::from_json(&workflow_json).unwrap();
        let mut ctx = Context::from_main(json!({"age": 16}));
        let result = flow.execute(&mut ctx).await.unwrap();
        
        assert_eq!(result, Some(json!({
            "status": "minor", 
            "message": "Processing minor user"
        })));
    }

    #[tokio::test]
    async fn test_sequential_steps() {
        let workflow_json = json!({
            "steps": [
                {
                    "label": "step1"
                },
                {
                    "label": "step2"
                },
                {
                    "label": "final_step",
                    "return": {"sequence": "complete", "step": 3}
                }
            ]
        });

        let flow = Flow::from_json(&workflow_json).unwrap();
        let mut ctx = Context::new();
        let result = flow.execute(&mut ctx).await.unwrap();

        assert_eq!(result, Some(json!({
            "sequence": "complete",
            "step": 3
        })));
    }

    #[tokio::test]
    async fn test_pipeline_jumping() {
        let workflow_json = json!({
            "steps": [
                {
                    "label": "redirect",
                    "to": {
                        "pipeline": 1,
                        "step": 0
                    }
                }
            ]
        });

        let flow = Flow::from_json(&workflow_json).unwrap();
        let mut ctx = Context::new();
        let result = flow.execute(&mut ctx).await;

        // 应该出错，因为pipeline 1不存在
        assert!(result.is_err());
    }

    #[test]
    fn test_error_types() {
        // 测试各种错误类型
        let transform_error = FlowError::TransformError("test".into());
        assert!(format!("{}", transform_error).contains("转换错误"));

        let pipeline_error = FlowError::PipelineNotFound(42);
        assert!(format!("{}", pipeline_error).contains("流程未找到"));

        let step_error = FlowError::StepNotFound(1, 2);
        assert!(format!("{}", step_error).contains("步骤未找到"));
    }
}
