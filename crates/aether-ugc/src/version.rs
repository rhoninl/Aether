//! Asset versioning with sequential version tracking.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::approval::ApprovalStatus;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetVersion {
    pub id: Uuid,
    pub asset_id: Uuid,
    pub version: u32,
    pub content_hash: String,
    pub size_bytes: u64,
    pub status: ApprovalStatus,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum VersionError {
    ParentMismatch { expected: u32, got: u32 },
    DuplicateVersion(u32),
    EmptyHash,
}

impl std::fmt::Display for VersionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VersionError::ParentMismatch { expected, got } => {
                write!(f, "parent version mismatch: expected {expected}, got {got}")
            }
            VersionError::DuplicateVersion(v) => write!(f, "duplicate version: {v}"),
            VersionError::EmptyHash => write!(f, "content hash must not be empty"),
        }
    }
}

impl std::error::Error for VersionError {}

#[derive(Debug, Clone)]
pub struct VersionHistory {
    asset_id: Uuid,
    versions: Vec<AssetVersion>,
}

impl VersionHistory {
    pub fn new(asset_id: Uuid) -> Self {
        Self {
            asset_id,
            versions: Vec::new(),
        }
    }

    pub fn asset_id(&self) -> Uuid {
        self.asset_id
    }

    pub fn latest_version(&self) -> Option<u32> {
        self.versions.last().map(|v| v.version)
    }

    pub fn get_version(&self, version: u32) -> Option<&AssetVersion> {
        self.versions.iter().find(|v| v.version == version)
    }

    pub fn all_versions(&self) -> &[AssetVersion] {
        &self.versions
    }

    pub fn add_version(
        &mut self,
        content_hash: String,
        size_bytes: u64,
        parent_version: Option<u32>,
    ) -> Result<&AssetVersion, VersionError> {
        if content_hash.is_empty() {
            return Err(VersionError::EmptyHash);
        }

        let next_version = self.latest_version().map_or(1, |v| v + 1);

        if let Some(parent) = parent_version {
            let expected = next_version.saturating_sub(1);
            if parent != expected {
                return Err(VersionError::ParentMismatch {
                    expected,
                    got: parent,
                });
            }
        }

        let version = AssetVersion {
            id: Uuid::new_v4(),
            asset_id: self.asset_id,
            version: next_version,
            content_hash,
            size_bytes,
            status: ApprovalStatus::Pending,
            created_at: Utc::now(),
        };

        self.versions.push(version);
        Ok(self.versions.last().unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_version_is_one() {
        let mut history = VersionHistory::new(Uuid::new_v4());
        let v = history.add_version("abc123".into(), 100, None).unwrap();
        assert_eq!(v.version, 1);
    }

    #[test]
    fn sequential_increment() {
        let mut history = VersionHistory::new(Uuid::new_v4());
        history.add_version("hash1".into(), 100, None).unwrap();
        history.add_version("hash2".into(), 200, None).unwrap();
        let v3 = history.add_version("hash3".into(), 300, None).unwrap();
        assert_eq!(v3.version, 3);
    }

    #[test]
    fn parent_version_validated() {
        let mut history = VersionHistory::new(Uuid::new_v4());
        history.add_version("hash1".into(), 100, None).unwrap();
        // parent=1 is correct for version 2
        let v2 = history.add_version("hash2".into(), 200, Some(1)).unwrap();
        assert_eq!(v2.version, 2);
    }

    #[test]
    fn parent_version_mismatch_rejected() {
        let mut history = VersionHistory::new(Uuid::new_v4());
        history.add_version("hash1".into(), 100, None).unwrap();
        // parent=5 is wrong for version 2 (expected parent=1)
        let err = history
            .add_version("hash2".into(), 200, Some(5))
            .unwrap_err();
        assert_eq!(
            err,
            VersionError::ParentMismatch {
                expected: 1,
                got: 5
            }
        );
    }

    #[test]
    fn empty_hash_rejected() {
        let mut history = VersionHistory::new(Uuid::new_v4());
        let err = history.add_version("".into(), 100, None).unwrap_err();
        assert_eq!(err, VersionError::EmptyHash);
    }

    #[test]
    fn get_version_returns_correct() {
        let mut history = VersionHistory::new(Uuid::new_v4());
        history.add_version("hash1".into(), 100, None).unwrap();
        history.add_version("hash2".into(), 200, None).unwrap();
        let v1 = history.get_version(1).unwrap();
        assert_eq!(v1.content_hash, "hash1");
        let v2 = history.get_version(2).unwrap();
        assert_eq!(v2.content_hash, "hash2");
    }

    #[test]
    fn get_nonexistent_version_returns_none() {
        let history = VersionHistory::new(Uuid::new_v4());
        assert!(history.get_version(1).is_none());
    }

    #[test]
    fn latest_version_empty_history() {
        let history = VersionHistory::new(Uuid::new_v4());
        assert!(history.latest_version().is_none());
    }

    #[test]
    fn all_versions_returns_all() {
        let mut history = VersionHistory::new(Uuid::new_v4());
        history.add_version("h1".into(), 10, None).unwrap();
        history.add_version("h2".into(), 20, None).unwrap();
        assert_eq!(history.all_versions().len(), 2);
    }

    #[test]
    fn new_version_status_is_pending() {
        let mut history = VersionHistory::new(Uuid::new_v4());
        let v = history.add_version("hash".into(), 100, None).unwrap();
        assert_eq!(v.status, ApprovalStatus::Pending);
    }

    #[test]
    fn version_has_unique_id() {
        let mut history = VersionHistory::new(Uuid::new_v4());
        let v1 = history.add_version("h1".into(), 10, None).unwrap().id;
        let v2 = history.add_version("h2".into(), 20, None).unwrap().id;
        assert_ne!(v1, v2);
    }

    #[test]
    fn asset_id_preserved() {
        let asset_id = Uuid::new_v4();
        let mut history = VersionHistory::new(asset_id);
        let v = history.add_version("h".into(), 10, None).unwrap();
        assert_eq!(v.asset_id, asset_id);
        assert_eq!(history.asset_id(), asset_id);
    }

    #[test]
    fn parent_version_zero_for_first_add() {
        let mut history = VersionHistory::new(Uuid::new_v4());
        // For first version, parent should be 0 (latest_version is None, so expected = 1-1 = 0)
        let v = history.add_version("h1".into(), 10, Some(0)).unwrap();
        assert_eq!(v.version, 1);
    }
}
