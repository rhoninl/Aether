#[derive(Debug, Clone)]
pub enum KeyPurpose {
    DeletionSalt,
    RetentionToken,
    LegalHold,
}

#[derive(Debug, Clone)]
pub struct KeystoreEntry {
    pub key_id: String,
    pub purpose: KeyPurpose,
    pub encrypted_value: String,
    pub approver_ids: Vec<u64>,
    pub created_ms: u64,
}

#[derive(Debug)]
pub struct ComplianceKeystore {
    pub keys: Vec<KeystoreEntry>,
}
