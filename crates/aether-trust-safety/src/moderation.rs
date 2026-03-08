#[derive(Debug, Clone)]
pub enum MuteAction {
    UntilMs(u64),
    Permanent,
}

#[derive(Debug, Clone)]
pub struct ModerationTool {
    pub world_id: String,
    pub actor_id: u64,
    pub target_id: u64,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct WorldOwnerToolset {
    pub can_mute: bool,
    pub can_kick: bool,
    pub can_ban: bool,
}

#[derive(Debug, Clone)]
pub enum KickAction {
    Evict,
    Warn,
}
