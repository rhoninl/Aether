#[derive(Debug, Clone)]
pub enum RatingCategory {
    General,
    Teens,
    Mature,
    Adult,
}

#[derive(Debug)]
pub struct RatingDecision {
    pub artifact_id: String,
    pub category: RatingCategory,
    pub rationale: String,
}

