#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WasmExecutionMode {
    ClientJit,
    ServerAot,
    ServerAotOnly,
}

#[derive(Debug)]
pub struct WasmProfile {
    pub platform: String,
    pub mode: WasmExecutionMode,
    pub script_budget_ms: u32,
}
