#[derive(Debug, Clone)]
pub struct ZoneSplitPolicy {
    pub max_zone_population: u32,
    pub min_zone_population: u32,
    pub split_min_size: u32,
    pub merge_check_window_ms: u64,
}

#[derive(Debug, Clone)]
pub struct ZoneSpec {
    pub world_id: String,
    pub shard_key: String,
    pub zone_index: String,
}

#[derive(Debug, Clone)]
pub struct MergeThreshold {
    pub merge_player_threshold: u32,
    pub merge_hold_ms: u64,
}

#[derive(Debug, Clone)]
pub struct LoadMetrics {
    pub zone_id: String,
    pub players: u32,
    pub sample_ms: u64,
    pub cpu_pct: f32,
}

#[derive(Debug, Clone)]
pub enum AxisChoice {
    X,
    Y,
    Z,
}

#[derive(Debug, Clone)]
pub enum SplitResult {
    SplitOk { left: ZoneSpec, right: ZoneSpec, axis: AxisChoice },
    TooFewPlayers,
    Unchanged,
}

#[derive(Debug, Clone)]
pub struct SplitPolicy {
    pub preferred_axes: Vec<AxisChoice>,
    pub max_depth: u8,
}

impl Default for SplitPolicy {
    fn default() -> Self {
        Self {
            preferred_axes: vec![AxisChoice::X, AxisChoice::Z],
            max_depth: 3,
        }
    }
}

