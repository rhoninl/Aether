#[derive(Debug, Clone)]
pub enum HapticWave {
    Sine {
        freq_hz: f32,
        amplitude: f32,
    },
    Pulse {
        amplitude: f32,
        duration_ms: u16,
    },
    Ramp {
        from_amplitude: f32,
        to_amplitude: f32,
        duration_ms: u16,
    },
}

#[derive(Debug, Clone)]
pub enum HapticChannel {
    Left,
    Right,
    Combined,
}

#[derive(Debug, Clone)]
pub enum HapticEffect {
    Click,
    Impact,
    Buzz,
    Custom(HapticWave),
}

#[derive(Debug, Clone)]
pub struct HapticRequest {
    pub player_id: u64,
    pub channel: HapticChannel,
    pub effect: HapticEffect,
    pub cooldown_ms: u32,
    pub looped: bool,
}
