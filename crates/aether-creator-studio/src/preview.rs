#[derive(Debug, Clone)]
pub enum HotReloadAction {
    Apply,
    Revert,
    Commit,
    Cancel,
}

#[derive(Debug, Clone)]
pub struct PreviewFrame {
    pub world_id: String,
    pub changed_entities: u32,
    pub applied: bool,
    pub action: HotReloadAction,
    pub timestamp_ms: u64,
}

#[derive(Debug)]
pub enum LivePreviewError {
    ScriptCompileError,
    AssetMissing,
    NetworkTimeout,
    SnapshotStale,
}

