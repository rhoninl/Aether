//! Content-addressed signed manifests for asset distribution.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::validation::FileType;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub asset_id: Uuid,
    pub version: u32,
    pub content_hash: String,
    pub size_bytes: u64,
    pub file_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedManifest {
    pub manifest_id: Uuid,
    pub entries: Vec<ManifestEntry>,
    pub digest: String,
}

impl SignedManifest {
    /// Verify that the digest matches the entries.
    pub fn verify(&self) -> bool {
        let computed = compute_digest(&self.entries);
        computed == self.digest
    }
}

/// Builder for constructing signed manifests.
#[derive(Debug, Default)]
pub struct ManifestBuilder {
    entries: Vec<ManifestEntry>,
}

impl ManifestBuilder {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn add_entry(
        mut self,
        asset_id: Uuid,
        version: u32,
        content_hash: String,
        size_bytes: u64,
        file_type: &FileType,
    ) -> Self {
        self.entries.push(ManifestEntry {
            asset_id,
            version,
            content_hash,
            size_bytes,
            file_type: format!("{file_type:?}"),
        });
        self
    }

    pub fn build(self) -> SignedManifest {
        let digest = compute_digest(&self.entries);
        SignedManifest {
            manifest_id: Uuid::new_v4(),
            entries: self.entries,
            digest,
        }
    }
}

fn compute_digest(entries: &[ManifestEntry]) -> String {
    let mut hasher = Sha256::new();
    for entry in entries {
        hasher.update(entry.content_hash.as_bytes());
        hasher.update(entry.version.to_le_bytes());
        hasher.update(entry.size_bytes.to_le_bytes());
    }
    format!("{:x}", hasher.finalize())
}

/// Compute SHA-256 hash of raw data bytes.
pub fn compute_content_hash(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_entry_manifest() {
        let asset_id = Uuid::new_v4();
        let manifest = ManifestBuilder::new()
            .add_entry(asset_id, 1, "abc123".into(), 1024, &FileType::Glb)
            .build();
        assert_eq!(manifest.entries.len(), 1);
        assert!(!manifest.digest.is_empty());
        assert!(manifest.verify());
    }

    #[test]
    fn multi_entry_manifest() {
        let manifest = ManifestBuilder::new()
            .add_entry(Uuid::new_v4(), 1, "hash1".into(), 100, &FileType::Png)
            .add_entry(Uuid::new_v4(), 2, "hash2".into(), 200, &FileType::Wav)
            .add_entry(Uuid::new_v4(), 1, "hash3".into(), 300, &FileType::Lua)
            .build();
        assert_eq!(manifest.entries.len(), 3);
        assert!(manifest.verify());
    }

    #[test]
    fn empty_manifest() {
        let manifest = ManifestBuilder::new().build();
        assert!(manifest.entries.is_empty());
        assert!(manifest.verify());
    }

    #[test]
    fn digest_changes_with_different_entries() {
        let id = Uuid::new_v4();
        let m1 = ManifestBuilder::new()
            .add_entry(id, 1, "aaa".into(), 100, &FileType::Glb)
            .build();
        let m2 = ManifestBuilder::new()
            .add_entry(id, 1, "bbb".into(), 100, &FileType::Glb)
            .build();
        assert_ne!(m1.digest, m2.digest);
    }

    #[test]
    fn digest_deterministic() {
        let id = Uuid::new_v4();
        let m1 = ManifestBuilder::new()
            .add_entry(id, 1, "same".into(), 100, &FileType::Glb)
            .build();
        let m2 = ManifestBuilder::new()
            .add_entry(id, 1, "same".into(), 100, &FileType::Glb)
            .build();
        assert_eq!(m1.digest, m2.digest);
    }

    #[test]
    fn tampered_manifest_fails_verify() {
        let manifest = ManifestBuilder::new()
            .add_entry(Uuid::new_v4(), 1, "hash1".into(), 100, &FileType::Png)
            .build();
        let mut tampered = manifest;
        tampered.entries[0].content_hash = "tampered".into();
        assert!(!tampered.verify());
    }

    #[test]
    fn manifest_entry_preserves_file_type() {
        let manifest = ManifestBuilder::new()
            .add_entry(Uuid::new_v4(), 1, "h".into(), 10, &FileType::Wasm)
            .build();
        assert_eq!(manifest.entries[0].file_type, "Wasm");
    }

    #[test]
    fn compute_content_hash_produces_sha256() {
        let hash = compute_content_hash(b"hello");
        // Known SHA-256 of "hello"
        assert_eq!(
            hash,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn compute_content_hash_empty_data() {
        let hash = compute_content_hash(b"");
        // SHA-256 of empty string
        assert_eq!(
            hash,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn different_data_different_hash() {
        let h1 = compute_content_hash(b"data1");
        let h2 = compute_content_hash(b"data2");
        assert_ne!(h1, h2);
    }

    #[test]
    fn manifest_has_unique_id() {
        let m1 = ManifestBuilder::new().build();
        let m2 = ManifestBuilder::new().build();
        assert_ne!(m1.manifest_id, m2.manifest_id);
    }
}
