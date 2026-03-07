#[derive(Debug, Clone)]
pub enum GeometryRule {
    WeaponLike,
    ExtremeAspectRatio,
    DisallowedTopology,
}

#[derive(Debug)]
pub struct MeshFinding {
    pub artifact_id: String,
    pub score: f32,
    pub rule: GeometryRule,
}

#[derive(Debug)]
pub struct MeshScanner {
    pub enabled: bool,
    pub min_triangle: u32,
}

