/// 生成步骤唯一ID
pub fn generate_step_id(pipeline_id: usize, step_idx: usize) -> String {
    format!("p{}_s{}", pipeline_id, step_idx)
}
