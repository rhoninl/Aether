//! GDPR Article 20 data export bundle generation.
//!
//! Generates a complete, machine-readable export of all user data
//! with an integrity hash for verification.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Status of an export request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExportStatus {
    Pending,
    Building,
    Ready,
    Failed { error: String },
}

/// A section of exported user data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportSection {
    /// Name of this data category (e.g., "profile", "social", "economy").
    pub name: String,
    /// Serialized data payload as JSON string.
    pub data: String,
    /// Number of records in this section.
    pub record_count: usize,
}

/// A complete data export bundle for a user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportBundle {
    pub request_id: String,
    pub user_id: u64,
    pub sections: Vec<ExportSection>,
    pub manifest_hash: String,
    pub status: ExportStatus,
    pub created_at_ms: u64,
}

/// Builder for constructing a data export bundle.
#[derive(Debug)]
pub struct DataExporter {
    user_id: u64,
    request_id: String,
    sections: Vec<ExportSection>,
    created_at_ms: u64,
}

impl DataExporter {
    /// Create a new data exporter for the given user.
    pub fn new(user_id: u64, request_id: String, now_ms: u64) -> Self {
        Self {
            user_id,
            request_id,
            sections: Vec::new(),
            created_at_ms: now_ms,
        }
    }

    /// Add a data section to the export.
    ///
    /// `name` is the category name, `data` is the JSON-serialized payload,
    /// and `record_count` is the number of records in this section.
    pub fn add_section(
        &mut self,
        name: String,
        data: String,
        record_count: usize,
    ) {
        self.sections.push(ExportSection {
            name,
            data,
            record_count,
        });
    }

    /// Finalize the export and produce the bundle with an integrity hash.
    ///
    /// The manifest hash is a SHA-256 hash of all section data concatenated.
    pub fn finalize(self) -> ExportBundle {
        let manifest_hash = compute_manifest_hash(&self.sections);
        ExportBundle {
            request_id: self.request_id,
            user_id: self.user_id,
            sections: self.sections,
            manifest_hash,
            status: ExportStatus::Ready,
            created_at_ms: self.created_at_ms,
        }
    }

    /// The number of sections added so far.
    pub fn section_count(&self) -> usize {
        self.sections.len()
    }
}

/// Compute a SHA-256 hash of all section data for integrity verification.
fn compute_manifest_hash(sections: &[ExportSection]) -> String {
    let mut hasher = Sha256::new();
    for section in sections {
        hasher.update(section.name.as_bytes());
        hasher.update(section.data.as_bytes());
    }
    let result = hasher.finalize();
    result.iter().map(|b| format!("{b:02x}")).collect()
}

/// Verify that an export bundle's manifest hash matches its content.
pub fn verify_bundle_integrity(bundle: &ExportBundle) -> bool {
    let expected = compute_manifest_hash(&bundle.sections);
    expected == bundle.manifest_hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_export_produces_ready_bundle() {
        let exporter = DataExporter::new(42, "req-001".into(), 1000);
        let bundle = exporter.finalize();
        assert_eq!(bundle.user_id, 42);
        assert_eq!(bundle.request_id, "req-001");
        assert_eq!(bundle.status, ExportStatus::Ready);
        assert!(bundle.sections.is_empty());
        assert_eq!(bundle.created_at_ms, 1000);
    }

    #[test]
    fn add_section_increases_count() {
        let mut exporter = DataExporter::new(1, "req".into(), 0);
        assert_eq!(exporter.section_count(), 0);
        exporter.add_section("profile".into(), "{}".into(), 1);
        assert_eq!(exporter.section_count(), 1);
        exporter.add_section("social".into(), "[]".into(), 5);
        assert_eq!(exporter.section_count(), 2);
    }

    #[test]
    fn finalize_includes_all_sections() {
        let mut exporter = DataExporter::new(1, "req".into(), 0);
        exporter.add_section(
            "profile".into(),
            r#"{"name":"Alice"}"#.into(),
            1,
        );
        exporter.add_section("friends".into(), r#"["Bob"]"#.into(), 1);
        let bundle = exporter.finalize();
        assert_eq!(bundle.sections.len(), 2);
        assert_eq!(bundle.sections[0].name, "profile");
        assert_eq!(bundle.sections[1].name, "friends");
    }

    #[test]
    fn manifest_hash_is_deterministic() {
        let make_bundle = || {
            let mut exporter = DataExporter::new(1, "req".into(), 0);
            exporter.add_section("data".into(), "payload".into(), 1);
            exporter.finalize()
        };
        let a = make_bundle();
        let b = make_bundle();
        assert_eq!(a.manifest_hash, b.manifest_hash);
    }

    #[test]
    fn manifest_hash_is_64_hex_chars() {
        let mut exporter = DataExporter::new(1, "req".into(), 0);
        exporter.add_section("data".into(), "value".into(), 1);
        let bundle = exporter.finalize();
        assert_eq!(bundle.manifest_hash.len(), 64);
        assert!(bundle.manifest_hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn different_data_produces_different_hash() {
        let mut exp_a = DataExporter::new(1, "req".into(), 0);
        exp_a.add_section("data".into(), "aaa".into(), 1);
        let bundle_a = exp_a.finalize();

        let mut exp_b = DataExporter::new(1, "req".into(), 0);
        exp_b.add_section("data".into(), "bbb".into(), 1);
        let bundle_b = exp_b.finalize();

        assert_ne!(bundle_a.manifest_hash, bundle_b.manifest_hash);
    }

    #[test]
    fn verify_bundle_integrity_passes_for_valid_bundle() {
        let mut exporter = DataExporter::new(1, "req".into(), 0);
        exporter.add_section("profile".into(), "data".into(), 1);
        let bundle = exporter.finalize();
        assert!(verify_bundle_integrity(&bundle));
    }

    #[test]
    fn verify_bundle_integrity_fails_for_tampered_bundle() {
        let mut exporter = DataExporter::new(1, "req".into(), 0);
        exporter.add_section("profile".into(), "original".into(), 1);
        let mut bundle = exporter.finalize();
        // Tamper with the data
        bundle.sections[0].data = "tampered".into();
        assert!(!verify_bundle_integrity(&bundle));
    }

    #[test]
    fn verify_empty_bundle_integrity() {
        let exporter = DataExporter::new(1, "req".into(), 0);
        let bundle = exporter.finalize();
        assert!(verify_bundle_integrity(&bundle));
    }

    #[test]
    fn section_preserves_record_count() {
        let mut exporter = DataExporter::new(1, "req".into(), 0);
        exporter.add_section("events".into(), "[1,2,3]".into(), 3);
        let bundle = exporter.finalize();
        assert_eq!(bundle.sections[0].record_count, 3);
    }

    #[test]
    fn export_status_variants() {
        assert_eq!(ExportStatus::Pending, ExportStatus::Pending);
        assert_eq!(ExportStatus::Building, ExportStatus::Building);
        assert_eq!(ExportStatus::Ready, ExportStatus::Ready);
        assert_ne!(ExportStatus::Pending, ExportStatus::Ready);
    }
}
