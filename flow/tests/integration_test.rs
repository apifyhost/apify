use flow::{Context, Flow, FlowError};

#[cfg(test)]
mod integration_tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_complete_workflow() {
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

        let flow = Flow::from_json(&workflow_json).unwrap();
        let mut ctx = Context::from_main(json!({"age": 25}));
        let result = flow.execute(&mut ctx).await.unwrap();
        
        assert_eq!(result, Some(json!({
            "status": "adult",
            "message": "Processing adult user"
        })));

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

        assert!(result.is_err());
    }

    #[test]
    fn test_error_types() {
        let transform_error = FlowError::TransformError("test".into());
        assert!(format!("{}", transform_error).contains("转换错误"));

        let pipeline_error = FlowError::PipelineNotFound(42);
        assert!(format!("{}", pipeline_error).contains("流程未找到"));

        let step_error = FlowError::StepNotFound(1, 2);
        assert!(format!("{}", step_error).contains("步骤未找到"));
    }
}

