#[derive(Debug, Clone)]
pub enum VisemeCurve {
    A,
    E,
    I,
    O,
    U,
    F,
    Rest,
}

#[derive(Debug, Clone)]
pub struct LipSyncFrame {
    pub timestamp_ms: u64,
    pub viseme: VisemeCurve,
    pub amplitude: f32,
    pub phoneme_id: u32,
}

#[derive(Debug, Clone)]
pub struct LipSyncConfig {
    pub frame_ms: u64,
    pub low_cutoff_hz: f32,
    pub mouth_open_threshold: f32,
    pub jitter_filter_ms: u64,
}

