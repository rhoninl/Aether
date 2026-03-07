#[derive(Debug)]
pub struct Token {
    pub user_id: u64,
    pub token_id: String,
    pub expires_ms: u64,
}

#[derive(Debug)]
pub struct AuthValidationPolicy {
    pub require_expiry_check: bool,
    pub require_signature: bool,
    pub accepted_issuers: Vec<String>,
}

#[derive(Debug)]
pub enum AuthzResult {
    Allowed,
    Denied(String),
    Expired,
}

