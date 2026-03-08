//! Asset bundle packaging in the .aether format.
//!
//! Bundle format (binary):
//! ```text
//! [4 bytes: magic "AETH"]
//! [4 bytes: version u32 LE]
//! [4 bytes: manifest_length u32 LE]
//! [manifest_length bytes: JSON manifest]
//! [entry data concatenated...]
//! ```

use serde::{Deserialize, Serialize};

use crate::hash::ContentHasher;

const BUNDLE_MAGIC: &[u8; 4] = b"AETH";
const BUNDLE_VERSION: u32 = 1;

/// A manifest describing all entries in an asset bundle.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AssetBundleManifest {
    pub version: u32,
    pub content_hash: String,
    pub entries: Vec<ManifestEntry>,
}

/// A single entry in the bundle manifest.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ManifestEntry {
    pub name: String,
    pub offset: u64,
    pub size: u64,
    pub content_hash: String,
}

/// An entry to be written into a bundle.
#[derive(Debug, Clone)]
pub struct BundleEntry {
    pub name: String,
    pub data: Vec<u8>,
}

/// A fully assembled asset bundle.
#[derive(Debug, Clone)]
pub struct AssetBundle {
    pub manifest: AssetBundleManifest,
    pub entries: Vec<BundleEntry>,
}

/// The serialized bundle bytes ready for writing to disk.
#[derive(Debug, Clone)]
pub struct WrittenBundle {
    pub bytes: Vec<u8>,
    pub manifest: AssetBundleManifest,
}

/// Errors during bundle operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BundleWriteError {
    EmptyBundle,
    SerializationFailed(String),
    InvalidBundle(String),
}

impl std::fmt::Display for BundleWriteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BundleWriteError::EmptyBundle => write!(f, "cannot create empty bundle"),
            BundleWriteError::SerializationFailed(msg) => {
                write!(f, "manifest serialization failed: {}", msg)
            }
            BundleWriteError::InvalidBundle(msg) => {
                write!(f, "invalid bundle: {}", msg)
            }
        }
    }
}

impl std::error::Error for BundleWriteError {}

/// Builds and writes asset bundles.
pub struct BundleWriter;

impl BundleWriter {
    /// Build an asset bundle from a list of entries.
    pub fn build(entries: Vec<BundleEntry>) -> Result<AssetBundle, BundleWriteError> {
        if entries.is_empty() {
            return Err(BundleWriteError::EmptyBundle);
        }

        let mut manifest_entries = Vec::new();
        let mut offset: u64 = 0;

        for entry in &entries {
            let content_hash = ContentHasher::hash(&entry.data);
            let size = entry.data.len() as u64;
            manifest_entries.push(ManifestEntry {
                name: entry.name.clone(),
                offset,
                size,
                content_hash,
            });
            offset += size;
        }

        // Compute overall content hash from all entry hashes concatenated
        let all_hashes: String = manifest_entries
            .iter()
            .map(|e| e.content_hash.as_str())
            .collect::<Vec<_>>()
            .join("");
        let content_hash = ContentHasher::hash(all_hashes.as_bytes());

        let manifest = AssetBundleManifest {
            version: BUNDLE_VERSION,
            content_hash,
            entries: manifest_entries,
        };

        Ok(AssetBundle { manifest, entries })
    }

    /// Serialize an asset bundle to bytes in the .aether format.
    pub fn write(bundle: &AssetBundle) -> Result<WrittenBundle, BundleWriteError> {
        let manifest_json = serde_json::to_vec(&bundle.manifest)
            .map_err(|e| BundleWriteError::SerializationFailed(e.to_string()))?;

        let manifest_len = manifest_json.len() as u32;

        let data_size: usize = bundle.entries.iter().map(|e| e.data.len()).sum();
        let total_size = 4 + 4 + 4 + manifest_json.len() + data_size;

        let mut bytes = Vec::with_capacity(total_size);

        // Header
        bytes.extend_from_slice(BUNDLE_MAGIC);
        bytes.extend_from_slice(&BUNDLE_VERSION.to_le_bytes());
        bytes.extend_from_slice(&manifest_len.to_le_bytes());

        // Manifest
        bytes.extend_from_slice(&manifest_json);

        // Entry data
        for entry in &bundle.entries {
            bytes.extend_from_slice(&entry.data);
        }

        Ok(WrittenBundle {
            bytes,
            manifest: bundle.manifest.clone(),
        })
    }

    /// Parse a written bundle back from bytes, verifying the header.
    pub fn read(bytes: &[u8]) -> Result<AssetBundle, BundleWriteError> {
        if bytes.len() < 12 {
            return Err(BundleWriteError::InvalidBundle(
                "too small for header".to_string(),
            ));
        }

        // Check magic
        if &bytes[0..4] != BUNDLE_MAGIC {
            return Err(BundleWriteError::InvalidBundle(
                "invalid magic bytes".to_string(),
            ));
        }

        // Read version
        let version = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        if version != BUNDLE_VERSION {
            return Err(BundleWriteError::InvalidBundle(format!(
                "unsupported version: {}",
                version
            )));
        }

        // Read manifest length
        let manifest_len =
            u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize;

        if bytes.len() < 12 + manifest_len {
            return Err(BundleWriteError::InvalidBundle(
                "truncated manifest".to_string(),
            ));
        }

        let manifest_bytes = &bytes[12..12 + manifest_len];
        let manifest: AssetBundleManifest = serde_json::from_slice(manifest_bytes)
            .map_err(|e| BundleWriteError::InvalidBundle(format!("manifest parse error: {}", e)))?;

        // Read entries
        let mut data_offset = 12 + manifest_len;
        let mut entries = Vec::new();

        for entry_meta in &manifest.entries {
            let size = entry_meta.size as usize;
            if data_offset + size > bytes.len() {
                return Err(BundleWriteError::InvalidBundle(
                    "truncated entry data".to_string(),
                ));
            }
            let data = bytes[data_offset..data_offset + size].to_vec();
            entries.push(BundleEntry {
                name: entry_meta.name.clone(),
                data,
            });
            data_offset += size;
        }

        Ok(AssetBundle { manifest, entries })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entries() -> Vec<BundleEntry> {
        vec![
            BundleEntry {
                name: "mesh.bin".to_string(),
                data: vec![1, 2, 3, 4, 5],
            },
            BundleEntry {
                name: "texture.bin".to_string(),
                data: vec![10, 20, 30],
            },
        ]
    }

    #[test]
    fn build_bundle_from_entries() {
        let entries = make_entries();
        let bundle = BundleWriter::build(entries).unwrap();
        assert_eq!(bundle.manifest.version, 1);
        assert_eq!(bundle.manifest.entries.len(), 2);
        assert_eq!(bundle.entries.len(), 2);
    }

    #[test]
    fn build_empty_bundle_fails() {
        let result = BundleWriter::build(vec![]);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), BundleWriteError::EmptyBundle);
    }

    #[test]
    fn manifest_entry_offsets() {
        let entries = make_entries();
        let bundle = BundleWriter::build(entries).unwrap();
        assert_eq!(bundle.manifest.entries[0].offset, 0);
        assert_eq!(bundle.manifest.entries[0].size, 5);
        assert_eq!(bundle.manifest.entries[1].offset, 5);
        assert_eq!(bundle.manifest.entries[1].size, 3);
    }

    #[test]
    fn manifest_content_hashes_populated() {
        let entries = make_entries();
        let bundle = BundleWriter::build(entries).unwrap();
        for entry in &bundle.manifest.entries {
            assert!(!entry.content_hash.is_empty());
            assert_eq!(entry.content_hash.len(), 64);
        }
        assert!(!bundle.manifest.content_hash.is_empty());
        assert_eq!(bundle.manifest.content_hash.len(), 64);
    }

    #[test]
    fn manifest_entry_names() {
        let entries = make_entries();
        let bundle = BundleWriter::build(entries).unwrap();
        assert_eq!(bundle.manifest.entries[0].name, "mesh.bin");
        assert_eq!(bundle.manifest.entries[1].name, "texture.bin");
    }

    #[test]
    fn write_bundle_magic_bytes() {
        let entries = make_entries();
        let bundle = BundleWriter::build(entries).unwrap();
        let written = BundleWriter::write(&bundle).unwrap();
        assert_eq!(&written.bytes[0..4], b"AETH");
    }

    #[test]
    fn write_bundle_version() {
        let entries = make_entries();
        let bundle = BundleWriter::build(entries).unwrap();
        let written = BundleWriter::write(&bundle).unwrap();
        let version = u32::from_le_bytes([
            written.bytes[4],
            written.bytes[5],
            written.bytes[6],
            written.bytes[7],
        ]);
        assert_eq!(version, 1);
    }

    #[test]
    fn write_bundle_manifest_length() {
        let entries = make_entries();
        let bundle = BundleWriter::build(entries).unwrap();
        let written = BundleWriter::write(&bundle).unwrap();
        let manifest_len = u32::from_le_bytes([
            written.bytes[8],
            written.bytes[9],
            written.bytes[10],
            written.bytes[11],
        ]) as usize;
        assert!(manifest_len > 0);
        // Verify the manifest JSON is valid
        let manifest_slice = &written.bytes[12..12 + manifest_len];
        let parsed: AssetBundleManifest = serde_json::from_slice(manifest_slice).unwrap();
        assert_eq!(parsed, bundle.manifest);
    }

    #[test]
    fn roundtrip_write_read() {
        let entries = make_entries();
        let bundle = BundleWriter::build(entries).unwrap();
        let written = BundleWriter::write(&bundle).unwrap();
        let read_back = BundleWriter::read(&written.bytes).unwrap();

        assert_eq!(read_back.manifest, bundle.manifest);
        assert_eq!(read_back.entries.len(), bundle.entries.len());
        for (a, b) in read_back.entries.iter().zip(bundle.entries.iter()) {
            assert_eq!(a.name, b.name);
            assert_eq!(a.data, b.data);
        }
    }

    #[test]
    fn read_too_small_fails() {
        let result = BundleWriter::read(&[0, 1, 2]);
        assert!(result.is_err());
    }

    #[test]
    fn read_bad_magic_fails() {
        let mut bytes = vec![0u8; 20];
        bytes[0..4].copy_from_slice(b"XXXX");
        let result = BundleWriter::read(&bytes);
        assert!(result.is_err());
        match result {
            Err(BundleWriteError::InvalidBundle(msg)) => {
                assert!(msg.contains("magic"));
            }
            _ => panic!("expected InvalidBundle error"),
        }
    }

    #[test]
    fn read_wrong_version_fails() {
        let mut bytes = vec![0u8; 20];
        bytes[0..4].copy_from_slice(b"AETH");
        bytes[4..8].copy_from_slice(&99u32.to_le_bytes());
        let result = BundleWriter::read(&bytes);
        assert!(result.is_err());
        match result {
            Err(BundleWriteError::InvalidBundle(msg)) => {
                assert!(msg.contains("version"));
            }
            _ => panic!("expected InvalidBundle error"),
        }
    }

    #[test]
    fn read_truncated_manifest_fails() {
        let mut bytes = vec![0u8; 12];
        bytes[0..4].copy_from_slice(b"AETH");
        bytes[4..8].copy_from_slice(&1u32.to_le_bytes());
        bytes[8..12].copy_from_slice(&1000u32.to_le_bytes()); // claims 1000 bytes but we only have 12
        let result = BundleWriter::read(&bytes);
        assert!(result.is_err());
    }

    #[test]
    fn single_entry_bundle() {
        let entries = vec![BundleEntry {
            name: "single.bin".to_string(),
            data: vec![42],
        }];
        let bundle = BundleWriter::build(entries).unwrap();
        assert_eq!(bundle.manifest.entries.len(), 1);
        assert_eq!(bundle.manifest.entries[0].name, "single.bin");
        assert_eq!(bundle.manifest.entries[0].size, 1);
        assert_eq!(bundle.manifest.entries[0].offset, 0);

        let written = BundleWriter::write(&bundle).unwrap();
        let read_back = BundleWriter::read(&written.bytes).unwrap();
        assert_eq!(read_back.entries[0].data, vec![42]);
    }

    #[test]
    fn large_entry_data() {
        let data: Vec<u8> = (0..10_000).map(|i| (i % 256) as u8).collect();
        let entries = vec![BundleEntry {
            name: "large.bin".to_string(),
            data: data.clone(),
        }];
        let bundle = BundleWriter::build(entries).unwrap();
        let written = BundleWriter::write(&bundle).unwrap();
        let read_back = BundleWriter::read(&written.bytes).unwrap();
        assert_eq!(read_back.entries[0].data, data);
    }

    #[test]
    fn bundle_deterministic() {
        let entries1 = make_entries();
        let entries2 = make_entries();
        let bundle1 = BundleWriter::build(entries1).unwrap();
        let bundle2 = BundleWriter::build(entries2).unwrap();
        let w1 = BundleWriter::write(&bundle1).unwrap();
        let w2 = BundleWriter::write(&bundle2).unwrap();
        assert_eq!(w1.bytes, w2.bytes);
    }

    #[test]
    fn different_data_different_hashes() {
        let bundle1 = BundleWriter::build(vec![BundleEntry {
            name: "a".to_string(),
            data: vec![1],
        }])
        .unwrap();
        let bundle2 = BundleWriter::build(vec![BundleEntry {
            name: "a".to_string(),
            data: vec![2],
        }])
        .unwrap();
        assert_ne!(
            bundle1.manifest.content_hash,
            bundle2.manifest.content_hash
        );
    }

    #[test]
    fn bundle_write_error_display() {
        let err = BundleWriteError::EmptyBundle;
        assert!(format!("{}", err).contains("empty"));

        let err = BundleWriteError::SerializationFailed("json error".to_string());
        assert!(format!("{}", err).contains("json error"));

        let err = BundleWriteError::InvalidBundle("bad header".to_string());
        assert!(format!("{}", err).contains("bad header"));
    }

    #[test]
    fn manifest_serialization_roundtrip() {
        let manifest = AssetBundleManifest {
            version: 1,
            content_hash: "abc123".to_string(),
            entries: vec![ManifestEntry {
                name: "test".to_string(),
                offset: 0,
                size: 10,
                content_hash: "def456".to_string(),
            }],
        };
        let json = serde_json::to_string(&manifest).unwrap();
        let parsed: AssetBundleManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, manifest);
    }
}
