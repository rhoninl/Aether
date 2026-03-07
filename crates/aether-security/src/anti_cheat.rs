#[derive(Debug, Clone)]
pub struct InputPlausibility {
    pub entity_id: u64,
    pub max_speed: f32,
    pub max_jump_height: f32,
    pub max_rotation_deg_per_ms: f32,
    pub sample_ms: u64,
}

#[derive(Debug)]
pub enum CheatSignal {
    SpeedHack,
    TeleportViolation,
    RotationHack,
    InputFlood,
    ScriptViolation,
}

#[derive(Debug, Clone)]
pub struct CheatVerdict {
    pub user_id: u64,
    pub signal: CheatSignal,
    pub severity: u8,
    pub reason: String,
    pub block_seconds: u64,
}

