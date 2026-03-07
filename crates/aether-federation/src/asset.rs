#[derive(Debug)]
pub enum HashMismatchAction {
    Reject,
    Report,
    Quarantine,
}

#[derive(Debug, Clone)]
pub struct FederationAssetReference {
    pub asset_id: String,
    pub sha256: String,
    pub approved: bool,
}

#[derive(Debug)]
pub struct AssetIntegrityPolicy {
    pub verify_download: bool,
    pub require_signature: bool,
    pub on_mismatch: HashMismatchAction,
}

