#[derive(Debug)]
pub enum ModerationStatus {
    Pending,
    Running,
    Cleared,
    Rejected(String),
}

#[derive(Debug)]
pub struct ModerationStatusUpdate {
    pub artifact_id: String,
    pub status: ModerationStatus,
    pub updated_ms: u64,
}

#[derive(Debug)]
pub enum ModerationSignal {
    TriggerScan,
    ScanComplete { approved: bool, reason: Option<String> },
}

