#[derive(Debug, Clone)]
pub enum WasmStaticRule {
    BannedImport,
    NetworkAccess,
    FileSystemAccess,
}

#[derive(Debug)]
pub struct WasmViolation {
    pub artifact_id: String,
    pub rule: WasmStaticRule,
    pub line: u32,
}

#[derive(Debug)]
pub struct WasmWarden {
    pub strict_mode: bool,
}

