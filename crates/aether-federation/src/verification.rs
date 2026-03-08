//! Content-addressed asset verification using SHA-256.

use sha2::{Digest, Sha256};

/// Result of an asset verification check.
#[derive(Debug, Clone, PartialEq)]
pub enum VerificationResult {
    /// Asset matches expected hash and size.
    Valid,
    /// Hash mismatch between computed and expected.
    HashMismatch {
        expected: String,
        actual: String,
    },
    /// Size mismatch between actual data and expected.
    SizeMismatch {
        expected: u64,
        actual: u64,
    },
    /// Both hash and size mismatch.
    BothMismatch {
        expected_hash: String,
        actual_hash: String,
        expected_size: u64,
        actual_size: u64,
    },
}

/// Metadata for content-addressed asset verification.
#[derive(Debug, Clone)]
pub struct AssetVerification {
    /// Hex-encoded SHA-256 hash of the content.
    pub content_hash: String,
    /// Expected size in bytes.
    pub size_bytes: u64,
    /// Whether this asset has been verified.
    pub verified: bool,
}

/// Compute the SHA-256 hex digest of a byte slice.
pub fn compute_hash(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    hex_encode(&result)
}

/// Verify that data matches the expected hash and size in `expected`.
/// Returns a `VerificationResult` indicating match or mismatch details.
pub fn verify_asset(data: &[u8], expected: &AssetVerification) -> VerificationResult {
    let actual_hash = compute_hash(data);
    let actual_size = data.len() as u64;
    let hash_ok = actual_hash == expected.content_hash;
    let size_ok = actual_size == expected.size_bytes;

    match (hash_ok, size_ok) {
        (true, true) => VerificationResult::Valid,
        (false, true) => VerificationResult::HashMismatch {
            expected: expected.content_hash.clone(),
            actual: actual_hash,
        },
        (true, false) => VerificationResult::SizeMismatch {
            expected: expected.size_bytes,
            actual: actual_size,
        },
        (false, false) => VerificationResult::BothMismatch {
            expected_hash: expected.content_hash.clone(),
            actual_hash,
            expected_size: expected.size_bytes,
            actual_size,
        },
    }
}

/// Create an `AssetVerification` from data, computing its hash and recording its size.
pub fn create_verification(data: &[u8]) -> AssetVerification {
    AssetVerification {
        content_hash: compute_hash(data),
        size_bytes: data.len() as u64,
        verified: true,
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_hash_empty_data() {
        let hash = compute_hash(b"");
        // SHA-256 of empty input is a well-known constant.
        assert_eq!(
            hash,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn compute_hash_hello_world() {
        let hash = compute_hash(b"hello world");
        assert_eq!(
            hash,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn compute_hash_deterministic() {
        let data = b"some binary data \x00\x01\x02";
        let h1 = compute_hash(data);
        let h2 = compute_hash(data);
        assert_eq!(h1, h2);
    }

    #[test]
    fn verify_valid_asset() {
        let data = b"asset content";
        let verification = create_verification(data);
        let result = verify_asset(data, &verification);
        assert_eq!(result, VerificationResult::Valid);
    }

    #[test]
    fn verify_hash_mismatch() {
        let data = b"asset content";
        let expected = AssetVerification {
            content_hash: "0000000000000000000000000000000000000000000000000000000000000000"
                .to_string(),
            size_bytes: data.len() as u64,
            verified: false,
        };
        let result = verify_asset(data, &expected);
        match result {
            VerificationResult::HashMismatch { expected: e, actual: a } => {
                assert_eq!(
                    e,
                    "0000000000000000000000000000000000000000000000000000000000000000"
                );
                assert_eq!(a, compute_hash(data));
            }
            other => panic!("expected HashMismatch, got {:?}", other),
        }
    }

    #[test]
    fn verify_size_mismatch() {
        let data = b"asset content";
        let expected = AssetVerification {
            content_hash: compute_hash(data),
            size_bytes: 9999,
            verified: false,
        };
        let result = verify_asset(data, &expected);
        match result {
            VerificationResult::SizeMismatch {
                expected: e,
                actual: a,
            } => {
                assert_eq!(e, 9999);
                assert_eq!(a, data.len() as u64);
            }
            other => panic!("expected SizeMismatch, got {:?}", other),
        }
    }

    #[test]
    fn verify_both_mismatch() {
        let data = b"asset content";
        let expected = AssetVerification {
            content_hash: "bad_hash".to_string(),
            size_bytes: 9999,
            verified: false,
        };
        let result = verify_asset(data, &expected);
        match result {
            VerificationResult::BothMismatch {
                expected_hash,
                actual_hash,
                expected_size,
                actual_size,
            } => {
                assert_eq!(expected_hash, "bad_hash");
                assert_eq!(actual_hash, compute_hash(data));
                assert_eq!(expected_size, 9999);
                assert_eq!(actual_size, data.len() as u64);
            }
            other => panic!("expected BothMismatch, got {:?}", other),
        }
    }

    #[test]
    fn verify_empty_data() {
        let data: &[u8] = b"";
        let verification = create_verification(data);
        assert_eq!(verification.size_bytes, 0);
        assert!(verification.verified);
        let result = verify_asset(data, &verification);
        assert_eq!(result, VerificationResult::Valid);
    }

    #[test]
    fn create_verification_sets_fields() {
        let data = b"test data for verification";
        let v = create_verification(data);
        assert_eq!(v.content_hash, compute_hash(data));
        assert_eq!(v.size_bytes, data.len() as u64);
        assert!(v.verified);
    }

    #[test]
    fn different_data_produces_different_hashes() {
        let h1 = compute_hash(b"data A");
        let h2 = compute_hash(b"data B");
        assert_ne!(h1, h2);
    }
}
