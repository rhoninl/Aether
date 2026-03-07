#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorMode {
    Desktop,
    InVr,
}

#[derive(Debug, Clone)]
pub enum EditorEvent {
    Save { world_id: String },
    OpenWorld { world_id: String },
    CloseWorld,
    Undo,
    Redo,
}

#[derive(Debug, Clone)]
pub struct StudioManifestDraft {
    pub world_id: String,
    pub dirty: bool,
    pub modified_at_ms: u64,
    pub source: EditorMode,
}

#[derive(Debug)]
pub struct ErrorReport {
    pub code: u32,
    pub message: String,
    pub recoverable: bool,
}

