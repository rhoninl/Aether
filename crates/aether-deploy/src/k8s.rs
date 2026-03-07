#[derive(Debug, Clone)]
pub struct HpaProfile {
    pub min_replicas: u16,
    pub max_replicas: u16,
    pub target_cpu_percent: u8,
    pub target_latency_ms: u64,
}

#[derive(Debug, Clone)]
pub struct WorldServerAutoscaler {
    pub profile: HpaProfile,
    pub scale_to_zero_enabled: bool,
    pub cooldown_seconds: u64,
}

#[derive(Debug, Clone)]
pub struct WorldServerRuntime {
    pub container_image: String,
    pub memory_limit_mib: u32,
    pub cpu_request_millis: u32,
    pub port: u16,
}

#[derive(Debug)]
pub struct AutoscalePolicy {
    pub server: WorldServerRuntime,
    pub hpa: Option<WorldServerAutoscaler>,
}

