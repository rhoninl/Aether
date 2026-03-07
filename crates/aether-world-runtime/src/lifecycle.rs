#[derive(Debug, Clone)]
pub enum RuntimeState {
    Stopped,
    Booting,
    Running,
    Draining,
    StoppedError(String),
}

#[derive(Debug, Clone)]
pub struct LifecycleEvent {
    pub world_id: String,
    pub state: RuntimeState,
    pub timestamp_ms: u64,
}

impl RuntimeState {
    pub fn can_advance_to(&self, next: &RuntimeState) -> bool {
        matches!(
            (self, next),
            (RuntimeState::Stopped, RuntimeState::Booting)
                | (RuntimeState::Booting, RuntimeState::Running)
                | (RuntimeState::Running, RuntimeState::Draining)
                | (RuntimeState::Draining, RuntimeState::Stopped)
                | (_, RuntimeState::StoppedError(_))
        )
    }
}

