#[derive(Debug)]
pub enum MuteAction {
    UntilMs(u64),
    Permanent,
}

#[derive(Debug)]
pub struct ModerationTool {
    pub world_id: String,
    pub actor_id: u64,
    pub target_id: u64,
    pub reason: String,
}

#[derive(Debug)]
pub struct WorldOwnerToolset {
    pub can_mute: bool,
    pub can_kick: bool,
    pub can_ban: bool,
}

#[derive(Debug)]
pub enum KickAction {
    Evict,
    Warn,
}

