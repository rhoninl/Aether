#[derive(Debug, Clone)]
pub struct WasmSandboxCapability {
    pub max_memory_pages: u32,
    pub max_cpu_ms: u64,
    pub max_file_reads: u32,
    pub max_net_calls_per_sec: u32,
    pub allowed_api: Vec<String>,
}

#[derive(Debug)]
pub enum WasmSurfaceError {
    ApiViolation,
    ResourceQuotaExceeded,
    ModuleTampered,
}

