#[derive(Debug, Clone)]
pub enum AvatarRatingBucket {
    TierS,
    TierA,
    TierB,
    TierC,
}

#[derive(Debug, Clone)]
pub struct AvatarBudget {
    pub max_polygons: u32,
    pub max_material_layers: u8,
    pub max_bone_count: u16,
}

#[derive(Debug, Clone)]
pub struct BudgetConstraint {
    pub bucket: AvatarRatingBucket,
    pub min_bucket: AvatarRatingBucket,
    pub strict_match: bool,
}

#[derive(Debug, Clone)]
pub struct AvatarPerformanceRating {
    pub bucket: AvatarRatingBucket,
    pub computed_score: f32,
    pub budget: AvatarBudget,
}

#[derive(Debug, Clone)]
pub struct PerformanceOverride {
    pub world_min_bucket: AvatarRatingBucket,
    pub enabled: bool,
}
