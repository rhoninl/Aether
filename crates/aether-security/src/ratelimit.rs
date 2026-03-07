#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionKey {
    Move,
    Chat,
    Trade,
    ScriptRpc,
    VoiceFrame,
    InventoryAction,
}

#[derive(Debug, Clone)]
pub struct RateLimit {
    pub action: ActionKey,
    pub per_user_per_minute: u32,
    pub burst: u32,
}

#[derive(Debug, Clone)]
pub struct RateLimitBucket {
    pub user_id: u64,
    pub action: ActionKey,
    pub window_start_ms: u64,
    pub allowance: i32,
}

