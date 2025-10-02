pub fn generate_step_id(pipeline_id: usize, step_idx: usize) -> String {
    format!("p{pipeline_id}_s{step_idx}")
}
