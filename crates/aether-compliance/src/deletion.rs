#[derive(Debug, Clone)]
pub enum DeleteScope {
    Profile,
    Social,
    Chat,
    Telemetry,
    All,
}

#[derive(Debug, Clone)]
pub enum LegalHold {
    Active { reason: String, case_id: String },
    Expired,
    None,
}

#[derive(Debug, Clone)]
pub struct ProfileDeletion {
    pub user_id: u64,
    pub scope: Vec<DeleteScope>,
    pub started_ms: u64,
    pub requested_by: u64,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct DeleteRequest {
    pub request_id: String,
    pub user_id: u64,
    pub scope: Vec<DeleteScope>,
    pub legal_hold: LegalHold,
}
