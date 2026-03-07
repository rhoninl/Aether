#[derive(Debug)]
pub enum RuntimeSettingsError {
    TickRateTooLow,
    TooManyPlayers,
    InvalidSpawnPoints,
    GravityCritical,
}

#[derive(Debug, Clone)]
pub struct RuntimeSettings {
    pub gravity: f32,
    pub tick_rate_hz: u32,
    pub max_players: u32,
    pub max_npcs: u32,
}

#[derive(Debug)]
pub enum WorldLifecycle {
    Booting,
    Running,
    ShuttingDown,
    Shutdown,
}

#[derive(Debug, Clone)]
pub enum WorldBootError {
    InvalidManifest(String),
    StreamingUnavailable,
    ResourceExhausted,
}

#[derive(Debug, Clone)]
pub enum WorldLifecycleEvent {
    BootRequested(String),
    BootComplete(String),
    ShutdownRequested(String),
    ShutdownComplete(String),
}

