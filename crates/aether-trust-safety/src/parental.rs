#[derive(Debug, Clone)]
pub enum ContentFilter {
    Off,
    Mild,
    Strict,
}

#[derive(Debug, Clone)]
pub struct TimeLimit {
    pub minutes_per_day: u32,
    pub hard_stop: bool,
}

#[derive(Debug, Clone)]
pub struct ParentalControl {
    pub enabled: bool,
    pub filter: ContentFilter,
    pub time_limit: Option<TimeLimit>,
    pub social_allowed: bool,
}
