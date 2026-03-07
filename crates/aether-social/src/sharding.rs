#[derive(Debug, Clone)]
pub struct ShardMapPolicy {
    pub shard_prefix_bits: u8,
    pub target_shards: u32,
    pub enable_eventual: bool,
    pub lag_budget_ms: u64,
}

impl ShardMapPolicy {
    pub fn shard_for_user(user_id: u64, shards: u32) -> u32 {
        let effective_shards = if shards == 0 { 1 } else { shards };
        let mask = effective_shards.saturating_sub(1);
        (user_id & u64::from(mask)) as u32
    }
}

