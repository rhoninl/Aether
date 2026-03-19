//! Version management for world projects.
//!
//! Tracks published versions, supports semantic versioning bumps, and
//! serializes/deserializes the version history in TOML format.

use serde::{Deserialize, Serialize};

use crate::error::VersionError;

/// A published version record.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VersionRecord {
    pub version: String,
    pub published_at: String,
    pub changelog: String,
    pub checksum: String,
}

/// Version history stored in `.aether/versions.toml`.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct VersionHistory {
    pub versions: Vec<VersionRecord>,
}

/// Version bump type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BumpKind {
    Major,
    Minor,
    Patch,
}

impl VersionHistory {
    /// Create a new empty version history.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the most recently published version, if any.
    pub fn latest(&self) -> Option<&VersionRecord> {
        self.versions.first()
    }

    /// Find a version record by its version string.
    pub fn find(&self, version: &str) -> Option<&VersionRecord> {
        self.versions.iter().find(|v| v.version == version)
    }

    /// Publish a new version record. Duplicates are rejected.
    pub fn publish(&mut self, record: VersionRecord) -> Result<(), VersionError> {
        if self.versions.iter().any(|v| v.version == record.version) {
            return Err(VersionError::DuplicateVersion(record.version));
        }
        // Insert at front so `latest()` returns the newest.
        self.versions.insert(0, record);
        Ok(())
    }

    /// Returns true if the given version exists in the history and can be
    /// rolled back to (i.e., it is not the latest version).
    pub fn can_rollback_to(&self, version: &str) -> bool {
        match self.latest() {
            Some(latest) if latest.version == version => false,
            _ => self.versions.iter().any(|v| v.version == version),
        }
    }
}

/// Parse a semver string "MAJOR.MINOR.PATCH" and bump the specified component.
pub fn bump_version(current: &str, kind: BumpKind) -> Result<String, VersionError> {
    let parts: Vec<&str> = current.split('.').collect();
    if parts.len() != 3 {
        return Err(VersionError::InvalidSemver(current.to_string()));
    }

    let major: u64 = parts[0]
        .parse()
        .map_err(|_| VersionError::InvalidSemver(current.to_string()))?;
    let minor: u64 = parts[1]
        .parse()
        .map_err(|_| VersionError::InvalidSemver(current.to_string()))?;
    let patch: u64 = parts[2]
        .parse()
        .map_err(|_| VersionError::InvalidSemver(current.to_string()))?;

    let (new_major, new_minor, new_patch) = match kind {
        BumpKind::Major => (major + 1, 0, 0),
        BumpKind::Minor => (major, minor + 1, 0),
        BumpKind::Patch => (major, minor, patch + 1),
    };

    Ok(format!("{new_major}.{new_minor}.{new_patch}"))
}

/// Serialize a `VersionHistory` to TOML.
pub fn serialize_version_history(history: &VersionHistory) -> Result<String, VersionError> {
    toml::to_string_pretty(history).map_err(|e| VersionError::Serialization(e.to_string()))
}

/// Deserialize a `VersionHistory` from TOML.
pub fn deserialize_version_history(toml_str: &str) -> Result<VersionHistory, VersionError> {
    toml::from_str(toml_str).map_err(|e| VersionError::Serialization(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bump_patch() {
        assert_eq!(bump_version("1.2.3", BumpKind::Patch).unwrap(), "1.2.4");
    }

    #[test]
    fn bump_minor() {
        assert_eq!(bump_version("1.2.3", BumpKind::Minor).unwrap(), "1.3.0");
    }

    #[test]
    fn bump_major() {
        assert_eq!(bump_version("1.2.3", BumpKind::Major).unwrap(), "2.0.0");
    }

    #[test]
    fn bump_from_zero() {
        assert_eq!(bump_version("0.0.0", BumpKind::Patch).unwrap(), "0.0.1");
        assert_eq!(bump_version("0.0.0", BumpKind::Minor).unwrap(), "0.1.0");
        assert_eq!(bump_version("0.0.0", BumpKind::Major).unwrap(), "1.0.0");
    }

    #[test]
    fn bump_invalid_semver_too_few_parts() {
        assert!(bump_version("1.2", BumpKind::Patch).is_err());
    }

    #[test]
    fn bump_invalid_semver_non_numeric() {
        assert!(bump_version("a.b.c", BumpKind::Patch).is_err());
    }

    #[test]
    fn bump_invalid_semver_empty() {
        assert!(bump_version("", BumpKind::Patch).is_err());
    }

    #[test]
    fn bump_invalid_semver_too_many_parts() {
        assert!(bump_version("1.2.3.4", BumpKind::Patch).is_err());
    }

    #[test]
    fn publish_adds_version() {
        let mut history = VersionHistory::new();
        let record = VersionRecord {
            version: "0.1.0".to_string(),
            published_at: "2026-03-19T10:00:00Z".to_string(),
            changelog: "Initial release".to_string(),
            checksum: "sha256:abc".to_string(),
        };
        assert!(history.publish(record).is_ok());
        assert_eq!(history.versions.len(), 1);
    }

    #[test]
    fn publish_duplicate_rejected() {
        let mut history = VersionHistory::new();
        let record = VersionRecord {
            version: "0.1.0".to_string(),
            published_at: "2026-03-19T10:00:00Z".to_string(),
            changelog: "Initial release".to_string(),
            checksum: "sha256:abc".to_string(),
        };
        history.publish(record.clone()).unwrap();
        let err = history.publish(record).unwrap_err();
        match err {
            VersionError::DuplicateVersion(v) => assert_eq!(v, "0.1.0"),
            _ => panic!("expected DuplicateVersion"),
        }
    }

    #[test]
    fn latest_returns_most_recent() {
        let mut history = VersionHistory::new();
        history
            .publish(VersionRecord {
                version: "0.1.0".to_string(),
                published_at: "2026-03-19T10:00:00Z".to_string(),
                changelog: "First".to_string(),
                checksum: "sha256:abc".to_string(),
            })
            .unwrap();
        history
            .publish(VersionRecord {
                version: "0.2.0".to_string(),
                published_at: "2026-03-19T11:00:00Z".to_string(),
                changelog: "Second".to_string(),
                checksum: "sha256:def".to_string(),
            })
            .unwrap();
        assert_eq!(history.latest().unwrap().version, "0.2.0");
    }

    #[test]
    fn latest_on_empty_returns_none() {
        let history = VersionHistory::new();
        assert!(history.latest().is_none());
    }

    #[test]
    fn find_returns_correct_version() {
        let mut history = VersionHistory::new();
        history
            .publish(VersionRecord {
                version: "0.1.0".to_string(),
                published_at: "2026-03-19T10:00:00Z".to_string(),
                changelog: "First".to_string(),
                checksum: "sha256:abc".to_string(),
            })
            .unwrap();
        history
            .publish(VersionRecord {
                version: "0.2.0".to_string(),
                published_at: "2026-03-19T11:00:00Z".to_string(),
                changelog: "Second".to_string(),
                checksum: "sha256:def".to_string(),
            })
            .unwrap();
        let found = history.find("0.1.0").unwrap();
        assert_eq!(found.changelog, "First");
    }

    #[test]
    fn find_nonexistent_returns_none() {
        let history = VersionHistory::new();
        assert!(history.find("9.9.9").is_none());
    }

    #[test]
    fn can_rollback_to_existing_non_latest() {
        let mut history = VersionHistory::new();
        history
            .publish(VersionRecord {
                version: "0.1.0".to_string(),
                published_at: "2026-03-19T10:00:00Z".to_string(),
                changelog: "First".to_string(),
                checksum: "sha256:abc".to_string(),
            })
            .unwrap();
        history
            .publish(VersionRecord {
                version: "0.2.0".to_string(),
                published_at: "2026-03-19T11:00:00Z".to_string(),
                changelog: "Second".to_string(),
                checksum: "sha256:def".to_string(),
            })
            .unwrap();
        assert!(history.can_rollback_to("0.1.0"));
    }

    #[test]
    fn cannot_rollback_to_latest() {
        let mut history = VersionHistory::new();
        history
            .publish(VersionRecord {
                version: "0.1.0".to_string(),
                published_at: "2026-03-19T10:00:00Z".to_string(),
                changelog: "First".to_string(),
                checksum: "sha256:abc".to_string(),
            })
            .unwrap();
        assert!(!history.can_rollback_to("0.1.0"));
    }

    #[test]
    fn cannot_rollback_to_nonexistent() {
        let history = VersionHistory::new();
        assert!(!history.can_rollback_to("9.9.9"));
    }

    #[test]
    fn serialization_round_trip() {
        let mut history = VersionHistory::new();
        history
            .publish(VersionRecord {
                version: "0.1.0".to_string(),
                published_at: "2026-03-19T10:00:00Z".to_string(),
                changelog: "Initial release".to_string(),
                checksum: "sha256:abc123".to_string(),
            })
            .unwrap();
        history
            .publish(VersionRecord {
                version: "0.2.0".to_string(),
                published_at: "2026-03-19T12:00:00Z".to_string(),
                changelog: "Added dungeon scene".to_string(),
                checksum: "sha256:def456".to_string(),
            })
            .unwrap();

        let serialized = serialize_version_history(&history).unwrap();
        let deserialized = deserialize_version_history(&serialized).unwrap();
        assert_eq!(history, deserialized);
    }

    #[test]
    fn deserialize_empty_history() {
        let toml_str = "versions = []\n";
        let history = deserialize_version_history(toml_str).unwrap();
        assert!(history.versions.is_empty());
    }

    #[test]
    fn deserialize_invalid_toml_returns_error() {
        let bad = "this is not valid toml [[[";
        assert!(deserialize_version_history(bad).is_err());
    }
}
