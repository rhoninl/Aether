#[derive(Debug)]
pub struct PatroniConfig {
    pub cluster_name: String,
    pub dcs: Vec<String>,
    pub grace_period_ms: u64,
    pub target_recovery_seconds: u64,
}

#[derive(Debug)]
pub struct DatabaseFailoverPolicy {
    pub patroni: PatroniConfig,
    pub health_check_ms: u64,
    pub switchover_urgency: u8,
}
