#[derive(Debug, Clone)]
pub struct MeshLodSpec {
    pub source_prim_count: u32,
    pub ratios: Vec<f32>,
}

#[derive(Debug, Clone)]
pub struct AutoLodPolicy {
    pub quality: u8,
    pub max_distance: f32,
}

#[derive(Debug, Clone)]
pub struct ProgressionRule {
    pub low_lod_first: bool,
    pub refine_budget_ms: u64,
}

