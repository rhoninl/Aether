//! Pseudonymization functions for GDPR-compliant data anonymization.
//!
//! Uses SHA-256 hash of user_id + salt to produce irreversible pseudonyms
//! that replace personal identifiers in ledger and audit records.

use sha2::{Digest, Sha256};

/// Pseudonymize a user ID by hashing it with a deletion salt.
///
/// Produces a hex-encoded SHA-256 hash of `user_id || salt`.
/// This is a one-way operation; the original user_id cannot be recovered
/// without the salt.
pub fn pseudonymize_id(user_id: u64, salt: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(user_id.to_le_bytes());
    hasher.update(salt);
    let result = hasher.finalize();
    hex_encode(&result)
}

/// Generate a cryptographically random 32-byte deletion salt.
pub fn generate_salt() -> Vec<u8> {
    use rand::RngCore;
    let mut salt = vec![0u8; 32];
    rand::thread_rng().fill_bytes(&mut salt);
    salt
}

/// Encode raw bytes as a hex string.
fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// A pseudonymized ledger row, replacing the original user_id.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PseudonymizedRow {
    /// The pseudonym that replaced the original user_id.
    pub pseudonym: String,
    /// The original table/collection this row belongs to.
    pub table_name: String,
    /// The original row identifier.
    pub row_id: String,
    /// Timestamp (ms) when pseudonymization was applied.
    pub pseudonymized_at_ms: u64,
}

/// Pseudonymize a batch of ledger rows for a given user.
pub fn pseudonymize_rows(
    user_id: u64,
    salt: &[u8],
    rows: &[(String, String)],
    now_ms: u64,
) -> Vec<PseudonymizedRow> {
    let pseudonym = pseudonymize_id(user_id, salt);
    rows.iter()
        .map(|(table, row_id)| PseudonymizedRow {
            pseudonym: pseudonym.clone(),
            table_name: table.clone(),
            row_id: row_id.clone(),
            pseudonymized_at_ms: now_ms,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pseudonymize_is_deterministic() {
        let salt = b"test-salt-32-bytes-long-exactly!";
        let result1 = pseudonymize_id(42, salt);
        let result2 = pseudonymize_id(42, salt);
        assert_eq!(result1, result2);
    }

    #[test]
    fn pseudonymize_produces_hex_string() {
        let salt = b"some-salt";
        let result = pseudonymize_id(1, salt);
        // SHA-256 produces 32 bytes = 64 hex chars
        assert_eq!(result.len(), 64);
        assert!(result.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn different_user_ids_produce_different_pseudonyms() {
        let salt = b"shared-salt";
        let a = pseudonymize_id(1, salt);
        let b = pseudonymize_id(2, salt);
        assert_ne!(a, b);
    }

    #[test]
    fn different_salts_produce_different_pseudonyms() {
        let a = pseudonymize_id(42, b"salt-a");
        let b = pseudonymize_id(42, b"salt-b");
        assert_ne!(a, b);
    }

    #[test]
    fn generate_salt_produces_32_bytes() {
        let salt = generate_salt();
        assert_eq!(salt.len(), 32);
    }

    #[test]
    fn generate_salt_is_random() {
        let a = generate_salt();
        let b = generate_salt();
        assert_ne!(a, b);
    }

    #[test]
    fn pseudonymize_rows_applies_same_pseudonym() {
        let salt = b"row-salt";
        let rows = vec![
            ("ledger".to_string(), "row-1".to_string()),
            ("ledger".to_string(), "row-2".to_string()),
            ("audit".to_string(), "row-3".to_string()),
        ];
        let result = pseudonymize_rows(100, salt, &rows, 1000);
        assert_eq!(result.len(), 3);

        let expected_pseudonym = pseudonymize_id(100, salt);
        for row in &result {
            assert_eq!(row.pseudonym, expected_pseudonym);
            assert_eq!(row.pseudonymized_at_ms, 1000);
        }
    }

    #[test]
    fn pseudonymize_rows_preserves_table_and_row_ids() {
        let salt = b"preserve-salt";
        let rows = vec![("payments".to_string(), "pay-001".to_string())];
        let result = pseudonymize_rows(5, salt, &rows, 500);
        assert_eq!(result[0].table_name, "payments");
        assert_eq!(result[0].row_id, "pay-001");
    }

    #[test]
    fn pseudonymize_empty_rows_returns_empty() {
        let salt = b"empty-salt";
        let result = pseudonymize_rows(1, salt, &[], 0);
        assert!(result.is_empty());
    }
}
