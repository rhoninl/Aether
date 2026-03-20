//! Compliance keystore for encrypted salt and key storage.
//!
//! Stores deletion salts and other compliance-critical keys with
//! dual-approval requirements and audit metadata.

use serde::{Deserialize, Serialize};

/// The minimum number of approvers required for a keystore entry.
const MIN_APPROVERS: usize = 2;

/// The purpose of a stored key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeyPurpose {
    /// Salt used to pseudonymize a deleted user's ledger data.
    DeletionSalt,
    /// Token for retention schedule management.
    RetentionToken,
    /// Key related to a legal hold.
    LegalHold,
}

/// A single entry in the compliance keystore.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeystoreEntry {
    /// Unique identifier for this key.
    pub key_id: String,
    /// What this key is used for.
    pub purpose: KeyPurpose,
    /// The encrypted value (XOR-encrypted with master key for demo).
    pub encrypted_value: Vec<u8>,
    /// IDs of personnel who approved this key's storage.
    pub approver_ids: Vec<u64>,
    /// Timestamp (ms) when this entry was created.
    pub created_ms: u64,
}

/// The compliance keystore manages encrypted keys with access controls.
#[derive(Debug)]
pub struct ComplianceKeystore {
    entries: Vec<KeystoreEntry>,
    master_key: Vec<u8>,
}

impl ComplianceKeystore {
    /// Create a new keystore with the given master encryption key.
    pub fn new(master_key: Vec<u8>) -> Self {
        Self {
            entries: Vec::new(),
            master_key,
        }
    }

    /// Store an entry in the keystore.
    ///
    /// The plaintext value is encrypted before storage.
    /// Requires at least 2 approver IDs (dual-approval).
    pub fn store(
        &mut self,
        key_id: String,
        purpose: KeyPurpose,
        plaintext_value: &[u8],
        approver_ids: Vec<u64>,
        created_ms: u64,
    ) -> Result<(), KeystoreError> {
        if approver_ids.len() < MIN_APPROVERS {
            return Err(KeystoreError::InsufficientApprovers {
                required: MIN_APPROVERS,
                provided: approver_ids.len(),
            });
        }

        if self.entries.iter().any(|e| e.key_id == key_id) {
            return Err(KeystoreError::DuplicateKeyId(key_id));
        }

        let encrypted_value = xor_encrypt(plaintext_value, &self.master_key);

        self.entries.push(KeystoreEntry {
            key_id,
            purpose,
            encrypted_value,
            approver_ids,
            created_ms,
        });

        Ok(())
    }

    /// Look up an entry by key_id and return the decrypted value.
    pub fn lookup(&self, key_id: &str) -> Option<Vec<u8>> {
        self.entries
            .iter()
            .find(|e| e.key_id == key_id)
            .map(|e| xor_encrypt(&e.encrypted_value, &self.master_key))
    }

    /// Look up an entry's metadata (without decrypting the value).
    pub fn lookup_entry(&self, key_id: &str) -> Option<&KeystoreEntry> {
        self.entries.iter().find(|e| e.key_id == key_id)
    }

    /// List all entries with a given purpose.
    pub fn entries_by_purpose(&self, purpose: &KeyPurpose) -> Vec<&KeystoreEntry> {
        self.entries
            .iter()
            .filter(|e| e.purpose == *purpose)
            .collect()
    }

    /// The total number of entries in the keystore.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the keystore is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// XOR-based encryption/decryption (symmetric, for demo purposes).
///
/// In production, this would use AES-256-GCM or similar.
fn xor_encrypt(data: &[u8], key: &[u8]) -> Vec<u8> {
    data.iter()
        .enumerate()
        .map(|(i, byte)| byte ^ key[i % key.len()])
        .collect()
}

/// Errors from keystore operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeystoreError {
    /// Not enough approvers for this operation.
    InsufficientApprovers { required: usize, provided: usize },
    /// A key with this ID already exists.
    DuplicateKeyId(String),
}

impl std::fmt::Display for KeystoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KeystoreError::InsufficientApprovers { required, provided } => {
                write!(
                    f,
                    "insufficient approvers: {provided} provided, \
                     {required} required"
                )
            }
            KeystoreError::DuplicateKeyId(id) => {
                write!(f, "key_id already exists: {id}")
            }
        }
    }
}

impl std::error::Error for KeystoreError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_keystore() -> ComplianceKeystore {
        ComplianceKeystore::new(b"test-master-key-32bytes-long!!!!".to_vec())
    }

    #[test]
    fn store_and_lookup_roundtrip() {
        let mut ks = make_keystore();
        let plaintext = b"my-secret-salt";
        ks.store(
            "key-001".into(),
            KeyPurpose::DeletionSalt,
            plaintext,
            vec![1, 2],
            1000,
        )
        .unwrap();

        let decrypted = ks.lookup("key-001").unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn lookup_missing_key_returns_none() {
        let ks = make_keystore();
        assert!(ks.lookup("nonexistent").is_none());
    }

    #[test]
    fn insufficient_approvers_rejected() {
        let mut ks = make_keystore();
        let result = ks.store(
            "key-001".into(),
            KeyPurpose::DeletionSalt,
            b"salt",
            vec![1], // only 1 approver
            1000,
        );
        assert_eq!(
            result,
            Err(KeystoreError::InsufficientApprovers {
                required: 2,
                provided: 1,
            })
        );
    }

    #[test]
    fn zero_approvers_rejected() {
        let mut ks = make_keystore();
        let result = ks.store(
            "key-001".into(),
            KeyPurpose::DeletionSalt,
            b"salt",
            vec![],
            1000,
        );
        assert_eq!(
            result,
            Err(KeystoreError::InsufficientApprovers {
                required: 2,
                provided: 0,
            })
        );
    }

    #[test]
    fn duplicate_key_id_rejected() {
        let mut ks = make_keystore();
        ks.store(
            "key-001".into(),
            KeyPurpose::DeletionSalt,
            b"salt1",
            vec![1, 2],
            1000,
        )
        .unwrap();

        let result = ks.store(
            "key-001".into(),
            KeyPurpose::RetentionToken,
            b"salt2",
            vec![3, 4],
            2000,
        );
        assert_eq!(result, Err(KeystoreError::DuplicateKeyId("key-001".into())));
    }

    #[test]
    fn encrypted_value_differs_from_plaintext() {
        let mut ks = make_keystore();
        let plaintext = b"sensitive-data-here";
        ks.store(
            "key-001".into(),
            KeyPurpose::DeletionSalt,
            plaintext,
            vec![1, 2],
            1000,
        )
        .unwrap();

        let entry = ks.lookup_entry("key-001").unwrap();
        assert_ne!(entry.encrypted_value, plaintext);
    }

    #[test]
    fn entries_by_purpose_filters_correctly() {
        let mut ks = make_keystore();
        ks.store(
            "k1".into(),
            KeyPurpose::DeletionSalt,
            b"s1",
            vec![1, 2],
            100,
        )
        .unwrap();
        ks.store(
            "k2".into(),
            KeyPurpose::RetentionToken,
            b"s2",
            vec![1, 2],
            200,
        )
        .unwrap();
        ks.store(
            "k3".into(),
            KeyPurpose::DeletionSalt,
            b"s3",
            vec![1, 2],
            300,
        )
        .unwrap();

        let deletion = ks.entries_by_purpose(&KeyPurpose::DeletionSalt);
        assert_eq!(deletion.len(), 2);

        let retention = ks.entries_by_purpose(&KeyPurpose::RetentionToken);
        assert_eq!(retention.len(), 1);

        let hold = ks.entries_by_purpose(&KeyPurpose::LegalHold);
        assert_eq!(hold.len(), 0);
    }

    #[test]
    fn len_and_is_empty() {
        let mut ks = make_keystore();
        assert!(ks.is_empty());
        assert_eq!(ks.len(), 0);

        ks.store(
            "k1".into(),
            KeyPurpose::DeletionSalt,
            b"s1",
            vec![1, 2],
            100,
        )
        .unwrap();
        assert!(!ks.is_empty());
        assert_eq!(ks.len(), 1);
    }

    #[test]
    fn lookup_entry_returns_metadata() {
        let mut ks = make_keystore();
        ks.store(
            "key-001".into(),
            KeyPurpose::LegalHold,
            b"data",
            vec![10, 20],
            5000,
        )
        .unwrap();

        let entry = ks.lookup_entry("key-001").unwrap();
        assert_eq!(entry.key_id, "key-001");
        assert_eq!(entry.purpose, KeyPurpose::LegalHold);
        assert_eq!(entry.approver_ids, vec![10, 20]);
        assert_eq!(entry.created_ms, 5000);
    }

    #[test]
    fn lookup_entry_missing_returns_none() {
        let ks = make_keystore();
        assert!(ks.lookup_entry("nope").is_none());
    }

    #[test]
    fn xor_encryption_is_symmetric() {
        let key = b"key123";
        let data = b"hello world";
        let encrypted = xor_encrypt(data, key);
        let decrypted = xor_encrypt(&encrypted, key);
        assert_eq!(decrypted, data);
    }

    #[test]
    fn two_approvers_is_minimum() {
        let mut ks = make_keystore();
        let result = ks.store("k".into(), KeyPurpose::DeletionSalt, b"v", vec![1, 2], 0);
        assert!(result.is_ok());
    }

    #[test]
    fn three_approvers_allowed() {
        let mut ks = make_keystore();
        let result = ks.store("k".into(), KeyPurpose::DeletionSalt, b"v", vec![1, 2, 3], 0);
        assert!(result.is_ok());
    }
}
