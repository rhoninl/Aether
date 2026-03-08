//! Content-addressed hashing using SHA-256.

use sha2::{Digest, Sha256};

/// Computes SHA-256 hex digest for content-addressed storage.
pub struct ContentHasher;

impl ContentHasher {
    /// Compute the SHA-256 hex digest of the given data.
    pub fn hash(data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        let result = hasher.finalize();
        hex_encode(&result)
    }

    /// Verify that data matches an expected SHA-256 hex digest.
    pub fn verify(data: &[u8], expected_hash: &str) -> bool {
        Self::hash(data) == expected_hash
    }
}

/// A piece of asset data paired with its content hash.
#[derive(Debug, Clone)]
pub struct HashedAsset {
    pub data: Vec<u8>,
    pub hash: String,
}

impl HashedAsset {
    /// Create a new HashedAsset by hashing the provided data.
    pub fn new(data: Vec<u8>) -> Self {
        let hash = ContentHasher::hash(&data);
        Self { data, hash }
    }

    /// Verify integrity of the stored data against the stored hash.
    pub fn verify(&self) -> bool {
        ContentHasher::verify(&self.data, &self.hash)
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_empty_data() {
        let hash = ContentHasher::hash(b"");
        // SHA-256 of empty string is well-known
        assert_eq!(
            hash,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn hash_hello_world() {
        let hash = ContentHasher::hash(b"hello world");
        assert_eq!(
            hash,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn hash_deterministic() {
        let data = b"some asset data with bytes";
        let hash1 = ContentHasher::hash(data);
        let hash2 = ContentHasher::hash(data);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn hash_different_data_produces_different_hashes() {
        let hash1 = ContentHasher::hash(b"data_a");
        let hash2 = ContentHasher::hash(b"data_b");
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn verify_correct_hash() {
        let data = b"verify me";
        let hash = ContentHasher::hash(data);
        assert!(ContentHasher::verify(data, &hash));
    }

    #[test]
    fn verify_incorrect_hash() {
        assert!(!ContentHasher::verify(b"data", "0000000000000000"));
    }

    #[test]
    fn hashed_asset_creation() {
        let asset = HashedAsset::new(vec![1, 2, 3, 4]);
        assert!(!asset.hash.is_empty());
        assert_eq!(asset.hash.len(), 64); // SHA-256 hex is 64 chars
    }

    #[test]
    fn hashed_asset_verify_integrity() {
        let asset = HashedAsset::new(vec![10, 20, 30]);
        assert!(asset.verify());
    }

    #[test]
    fn hashed_asset_tampered_fails_verify() {
        let mut asset = HashedAsset::new(vec![10, 20, 30]);
        asset.data[0] = 99;
        assert!(!asset.verify());
    }

    #[test]
    fn hash_length_is_64_hex_chars() {
        let hash = ContentHasher::hash(b"any data");
        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn hash_binary_data() {
        let data: Vec<u8> = (0..=255).collect();
        let hash = ContentHasher::hash(&data);
        assert_eq!(hash.len(), 64);
        assert!(ContentHasher::verify(&data, &hash));
    }
}
