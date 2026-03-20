//! SHA-256 integrity verification for WASM modules.
//!
//! Before a module is compiled or loaded from cache, its raw bytes are
//! hashed and compared against a set of approved hashes. Only modules
//! whose hash appears in the approved set are allowed to execute.

use std::collections::HashSet;
use std::fmt;

use sha2::{Digest, Sha256};

/// Computes the SHA-256 hash of raw WASM bytes.
pub fn sha256_hash(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result);
    hash
}

/// Error returned when integrity verification fails.
#[derive(Debug, Clone)]
pub struct IntegrityError {
    pub expected_any_of: usize,
    pub actual_hash: [u8; 32],
}

impl fmt::Display for IntegrityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "integrity check failed: hash {} not in approved set ({} approved hashes)",
            hex::encode(self.actual_hash),
            self.expected_any_of,
        )
    }
}

impl std::error::Error for IntegrityError {}

/// Verifies WASM module integrity by checking SHA-256 hashes against
/// an approved manifest.
#[derive(Debug, Clone)]
pub struct IntegrityVerifier {
    approved_hashes: HashSet<[u8; 32]>,
}

impl IntegrityVerifier {
    /// Creates a new verifier with no approved hashes.
    pub fn new() -> Self {
        Self {
            approved_hashes: HashSet::new(),
        }
    }

    /// Creates a verifier pre-populated with approved hashes.
    pub fn with_approved(hashes: impl IntoIterator<Item = [u8; 32]>) -> Self {
        Self {
            approved_hashes: hashes.into_iter().collect(),
        }
    }

    /// Adds a hash to the approved set.
    pub fn approve(&mut self, hash: [u8; 32]) {
        self.approved_hashes.insert(hash);
    }

    /// Removes a hash from the approved set.
    pub fn revoke(&mut self, hash: &[u8; 32]) {
        self.approved_hashes.remove(hash);
    }

    /// Returns the number of approved hashes.
    pub fn approved_count(&self) -> usize {
        self.approved_hashes.len()
    }

    /// Verifies the given WASM bytes against the approved hash set.
    ///
    /// Returns the computed hash on success, or an `IntegrityError` if
    /// the hash is not in the approved set.
    pub fn verify(&self, wasm_bytes: &[u8]) -> Result<[u8; 32], IntegrityError> {
        let hash = sha256_hash(wasm_bytes);
        if self.approved_hashes.contains(&hash) {
            Ok(hash)
        } else {
            Err(IntegrityError {
                expected_any_of: self.approved_hashes.len(),
                actual_hash: hash,
            })
        }
    }
}

impl Default for IntegrityVerifier {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_wat() -> Vec<u8> {
        wat::parse_str(r#"(module (func (export "run") (nop)))"#).expect("valid WAT")
    }

    #[test]
    fn sha256_hash_is_deterministic() {
        let data = b"hello wasm";
        let h1 = sha256_hash(data);
        let h2 = sha256_hash(data);
        assert_eq!(h1, h2);
    }

    #[test]
    fn sha256_hash_differs_for_different_data() {
        let h1 = sha256_hash(b"aaa");
        let h2 = sha256_hash(b"bbb");
        assert_ne!(h1, h2);
    }

    #[test]
    fn verify_approved_hash_succeeds() {
        let wasm = sample_wat();
        let hash = sha256_hash(&wasm);
        let verifier = IntegrityVerifier::with_approved(vec![hash]);
        let result = verifier.verify(&wasm);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), hash);
    }

    #[test]
    fn verify_unapproved_hash_fails() {
        let wasm = sample_wat();
        let verifier = IntegrityVerifier::new();
        let result = verifier.verify(&wasm);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.expected_any_of, 0);
        assert_eq!(err.actual_hash, sha256_hash(&wasm));
    }

    #[test]
    fn verify_wrong_hash_in_set_fails() {
        let wasm = sample_wat();
        let wrong_hash = sha256_hash(b"not the real wasm");
        let verifier = IntegrityVerifier::with_approved(vec![wrong_hash]);
        let result = verifier.verify(&wasm);
        assert!(result.is_err());
    }

    #[test]
    fn approve_and_revoke() {
        let mut verifier = IntegrityVerifier::new();
        let hash = sha256_hash(b"module bytes");
        assert_eq!(verifier.approved_count(), 0);

        verifier.approve(hash);
        assert_eq!(verifier.approved_count(), 1);

        verifier.revoke(&hash);
        assert_eq!(verifier.approved_count(), 0);
    }

    #[test]
    fn multiple_approved_hashes() {
        let wasm_a = wat::parse_str(r#"(module (func (export "a") (nop)))"#).unwrap();
        let wasm_b = wat::parse_str(r#"(module (func (export "b") (nop)))"#).unwrap();

        let hash_a = sha256_hash(&wasm_a);
        let hash_b = sha256_hash(&wasm_b);
        let verifier = IntegrityVerifier::with_approved(vec![hash_a, hash_b]);

        assert!(verifier.verify(&wasm_a).is_ok());
        assert!(verifier.verify(&wasm_b).is_ok());
    }

    #[test]
    fn integrity_error_display() {
        let err = IntegrityError {
            expected_any_of: 3,
            actual_hash: [0xAB; 32],
        };
        let msg = format!("{err}");
        assert!(msg.contains("integrity check failed"));
        assert!(msg.contains("3 approved hashes"));
    }
}
