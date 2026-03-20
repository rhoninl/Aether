#[derive(Debug)]
pub struct RateLimitPolicy {
    pub max_requests_per_minute: u32,
    pub burst: u32,
    pub ban_threshold_per_minute: u32,
}

#[derive(Debug)]
pub struct RateLimitStatus {
    pub remaining: u32,
    pub reset_in_ms: u64,
    pub allowed: bool,
}
