#[derive(Debug)]
pub enum RegistrationState {
    Draft,
    Submitted,
    ModerationPending,
    Approved,
    Rejected,
    NeedsReview,
}

#[derive(Debug, Clone)]
pub struct SelfHostedWorld {
    pub world_id: String,
    pub owner_id: u64,
    pub endpoint: String,
    pub state: RegistrationState,
    pub discovered: bool,
    pub aot_artifact_id: Option<String>,
}
