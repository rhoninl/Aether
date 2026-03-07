#[derive(Debug)]
pub enum ModerationResult {
    Passed,
    Rejected,
    Pending,
}

#[derive(Debug)]
pub struct CentralServiceGate {
    pub require_aec_routing: bool,
    pub require_auth_service: bool,
    pub require_registry_moderation: bool,
}

#[derive(Debug)]
pub enum ModifiedSinceApproval {
    Yes,
    No,
}

