use crate::{context::Context, error::FlowError, step::{NextStep, Step, StepOutput}};

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
