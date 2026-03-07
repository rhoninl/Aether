#[derive(Debug)]
pub enum AuthCheckMode {
    CentralToken,
    LocalFallback,
    Disabled,
}

#[derive(Debug)]
pub struct FederationAuthRequest {
    pub world_id: String,
    pub player_id: u64,
    pub session_token: String,
    pub mode: AuthCheckMode,
}

#[derive(Debug)]
pub struct FederationAuthResult {
    pub allowed: bool,
    pub reason: Option<String>,
    pub central_verified: bool,
}

