#[derive(Debug, Clone)]
pub enum PresenceKind {
    Offline,
    Online,
    InWorld,
}

#[derive(Debug, Clone)]
pub enum PresenceVisibility {
    Visible,
    Hidden,
    Busy,
    Away,
}

#[derive(Debug, Clone)]
pub struct InWorldLocation {
    pub world_id: String,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub zone: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PresenceState {
    pub user_id: u64,
    pub kind: PresenceKind,
    pub visibility: PresenceVisibility,
    pub in_world: Option<InWorldLocation>,
    pub updated_ms: u64,
}

