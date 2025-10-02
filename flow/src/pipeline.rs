use crate::{
    context::Context,
    error::FlowError,
    step::{NextStep, Step, StepOutput},
};

#[derive(Debug, Clone)]
pub struct Pipeline {
    pub id: usize,
    pub steps: Vec<Step>,
}

impl Pipeline {
    pub fn new(id: usize, steps: Vec<Step>) -> Self {
        Self { id, steps }
    }

    pub async fn execute(
        &self,
        context: &Context,
        start_step: usize,
    ) -> Result<StepOutput, FlowError> {
        if start_step >= self.steps.len() {
            return Err(FlowError::StepNotFound(start_step, self.id));
        }

        for (step_idx, step) in self.steps.iter().enumerate().skip(start_step) {
            let step_output = step.execute(context).await?;

            if let Some(output) = &step_output.output {
                let mut ctx_clone = context.clone();
                ctx_clone.add_step_output(step.id.clone(), output.clone());
            }

            match step_output.next_step {
                NextStep::Stop | NextStep::Pipeline(_) | NextStep::Step { .. } => {
                    return Ok(step_output);
                }
                NextStep::Next => {
                    if step_idx == self.steps.len() - 1 {
                        return Ok(StepOutput {
                            next_step: NextStep::Stop,
                            output: step_output.output,
                        });
                    }
                }
            }
        }

        Ok(StepOutput {
            next_step: NextStep::Stop,
            output: context.get_payload(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::Context;
    use crate::step::{NextStep, Step};
    use serde_json::json;

    fn create_test_step(id: &str, next_step: NextStep) -> Step {
        Step {
            id: id.to_string(),
            label: None,
            condition: None,
            return_val: match next_step {
                NextStep::Stop => Some(json!({"result": id})),
                _ => None,
            },
            then_pipeline: None,
            else_pipeline: None,
            to_step: match next_step {
                NextStep::Step { pipeline, step } => Some((pipeline, step)),
                _ => None,
            },
        }
    }

    #[tokio::test]
    async fn test_pipeline_execute_sequential() {
        let steps = vec![
            create_test_step("step1", NextStep::Next),
            create_test_step("step2", NextStep::Next),
            create_test_step("step3", NextStep::Stop),
        ];

        let pipeline = Pipeline::new(0, steps);
        let ctx = Context::new();

        let output = pipeline.execute(&ctx, 0).await.unwrap();

        assert_eq!(output.next_step, NextStep::Stop);
        assert_eq!(output.output, Some(json!({"result": "step3"})));
    }

    #[tokio::test]
    async fn test_pipeline_execute_from_middle() {
        let steps = vec![
            create_test_step("step1", NextStep::Next),
            create_test_step("step2", NextStep::Next),
            create_test_step("step3", NextStep::Stop),
        ];

        let pipeline = Pipeline::new(0, steps);
        let ctx = Context::new();

        let output = pipeline.execute(&ctx, 1).await.unwrap();

        assert_eq!(output.next_step, NextStep::Stop);
        assert_eq!(output.output, Some(json!({"result": "step3"})));
    }

    #[tokio::test]
    async fn test_pipeline_execute_with_jump() {
        let steps = vec![
            create_test_step(
                "step1",
                NextStep::Step {
                    pipeline: 1,
                    step: 0,
                },
            ),
            create_test_step("step2", NextStep::Stop),
        ];

        let pipeline = Pipeline::new(0, steps);
        let ctx = Context::new();

        let output = pipeline.execute(&ctx, 0).await.unwrap();

        assert_eq!(
            output.next_step,
            NextStep::Step {
                pipeline: 1,
                step: 0
            }
        );
        assert!(output.output.is_none());
    }

    #[tokio::test]
    async fn test_pipeline_execute_error_step_not_found() {
        let steps = vec![create_test_step("step1", NextStep::Next)];

        let pipeline = Pipeline::new(0, steps);
        let ctx = Context::new();

        let result = pipeline.execute(&ctx, 5).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            FlowError::StepNotFound(step_idx, pipeline_id) => {
                assert_eq!(step_idx, 5);
                assert_eq!(pipeline_id, 0);
            }
            _ => panic!("Expected StepNotFound error"),
        }
    }

    #[tokio::test]
    async fn test_pipeline_execute_empty() {
        let pipeline = Pipeline::new(0, vec![]);
        let ctx = Context::new();

        let result = pipeline.execute(&ctx, 0).await;
        assert!(result.is_err());

        match result.unwrap_err() {
            FlowError::StepNotFound(step_idx, pipeline_id) => {
                assert_eq!(step_idx, 0);
                assert_eq!(pipeline_id, 0);
            }
            _ => panic!("Expected StepNotFound error"),
        }
    }

    #[test]
    fn test_pipeline_creation() {
        let steps = vec![
            create_test_step("step1", NextStep::Next),
            create_test_step("step2", NextStep::Stop),
        ];

        let pipeline = Pipeline::new(42, steps.clone());

        assert_eq!(pipeline.id, 42);
        assert_eq!(pipeline.steps.len(), 2);
        assert_eq!(pipeline.steps[0].id, "step1");
        assert_eq!(pipeline.steps[1].id, "step2");
    }
}
