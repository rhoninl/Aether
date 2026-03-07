#[derive(Debug)]
pub enum ExportStatus {
    Pending,
    Ready,
    Failed,
}

#[derive(Debug)]
pub struct ExportBundle {
    pub request_id: String,
    pub user_id: u64,
    pub payload_path: String,
    pub manifest_hash: String,
    pub status: ExportStatus,
}

