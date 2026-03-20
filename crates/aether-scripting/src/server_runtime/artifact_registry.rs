//! Artifact registry with SHA-256 manifest verification.
//!
//! Stores and retrieves AOT-compiled artifacts indexed by `(script_id, version)`.
//! Each entry includes a manifest with integrity hashes that are verified
//! before an artifact is considered valid for loading.

use std::collections::HashMap;

use super::aot::{sha256, AotArtifact, AotTarget};

/// Maximum number of versions to retain per script before evicting old ones.
const DEFAULT_MAX_VERSIONS_PER_SCRIPT: usize = 8;

/// A manifest entry describing a registered artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactManifest {
    /// Unique script identifier.
    pub script_id: u64,
    /// Version number (monotonically increasing per script).
    pub version: u32,
    /// SHA-256 of the original WASM source bytes.
    pub source_hash: [u8; 32],
    /// SHA-256 of the compiled native artifact bytes.
    pub artifact_hash: [u8; 32],
    /// The target platform the artifact was compiled for.
    pub target: AotTarget,
    /// Size of the native artifact in bytes.
    pub artifact_size: usize,
}

/// Errors that can occur in the artifact registry.
#[derive(Debug)]
pub enum RegistryError {
    /// The artifact's hash does not match the expected manifest hash.
    HashMismatch {
        field: &'static str,
        expected: [u8; 32],
        actual: [u8; 32],
    },
    /// The requested script/version was not found.
    NotFound { script_id: u64, version: u32 },
    /// A version conflict: the version already exists for this script.
    VersionConflict { script_id: u64, version: u32 },
    /// The registry has reached its maximum capacity.
    RegistryFull { max_scripts: usize },
}

impl std::fmt::Display for RegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::HashMismatch {
                field,
                expected,
                actual,
            } => write!(
                f,
                "{field} hash mismatch: expected {}, got {}",
                hex::encode(expected),
                hex::encode(actual),
            ),
            Self::NotFound { script_id, version } => {
                write!(
                    f,
                    "artifact not found: script={script_id} version={version}"
                )
            }
            Self::VersionConflict { script_id, version } => {
                write!(
                    f,
                    "version conflict: script={script_id} version={version} already exists"
                )
            }
            Self::RegistryFull { max_scripts } => {
                write!(f, "registry full: max {max_scripts} scripts")
            }
        }
    }
}

impl std::error::Error for RegistryError {}

/// Internal storage for a single script's versioned artifacts.
#[derive(Debug)]
struct ScriptEntry {
    /// Map of version -> (manifest, native bytes).
    versions: HashMap<u32, (ArtifactManifest, Vec<u8>)>,
    /// The latest registered version number.
    latest_version: u32,
}

impl ScriptEntry {
    fn new() -> Self {
        Self {
            versions: HashMap::new(),
            latest_version: 0,
        }
    }
}

/// In-memory registry for AOT-compiled artifacts.
///
/// Artifacts are indexed by `(script_id, version)` and verified against
/// SHA-256 manifests on registration and retrieval.
#[derive(Debug)]
pub struct ArtifactRegistry {
    scripts: HashMap<u64, ScriptEntry>,
    max_scripts: usize,
    max_versions_per_script: usize,
}

impl ArtifactRegistry {
    /// Creates a new registry with the specified maximum script capacity.
    pub fn new(max_scripts: usize) -> Self {
        Self {
            scripts: HashMap::new(),
            max_scripts,
            max_versions_per_script: DEFAULT_MAX_VERSIONS_PER_SCRIPT,
        }
    }

    /// Creates a registry with custom version retention limit.
    pub fn with_version_limit(max_scripts: usize, max_versions_per_script: usize) -> Self {
        Self {
            scripts: HashMap::new(),
            max_scripts,
            max_versions_per_script,
        }
    }

    /// Returns the number of registered scripts.
    pub fn script_count(&self) -> usize {
        self.scripts.len()
    }

    /// Returns the number of versions registered for a given script.
    pub fn version_count(&self, script_id: u64) -> usize {
        self.scripts
            .get(&script_id)
            .map(|e| e.versions.len())
            .unwrap_or(0)
    }

    /// Returns the latest version number for a script, or `None` if not registered.
    pub fn latest_version(&self, script_id: u64) -> Option<u32> {
        self.scripts
            .get(&script_id)
            .filter(|e| !e.versions.is_empty())
            .map(|e| e.latest_version)
    }

    /// Registers an AOT artifact for a script at a specific version.
    ///
    /// Verifies that the artifact's hashes match the computed values before storing.
    pub fn register(
        &mut self,
        script_id: u64,
        version: u32,
        artifact: AotArtifact,
    ) -> Result<ArtifactManifest, RegistryError> {
        // Verify artifact integrity
        let computed_artifact_hash = sha256(&artifact.native_bytes);
        if computed_artifact_hash != artifact.artifact_hash {
            return Err(RegistryError::HashMismatch {
                field: "artifact",
                expected: artifact.artifact_hash,
                actual: computed_artifact_hash,
            });
        }

        // Check capacity for new scripts
        if !self.scripts.contains_key(&script_id) && self.scripts.len() >= self.max_scripts {
            return Err(RegistryError::RegistryFull {
                max_scripts: self.max_scripts,
            });
        }

        let entry = self
            .scripts
            .entry(script_id)
            .or_insert_with(ScriptEntry::new);

        // Check version conflict
        if entry.versions.contains_key(&version) {
            return Err(RegistryError::VersionConflict { script_id, version });
        }

        // Evict oldest versions if over limit
        while entry.versions.len() >= self.max_versions_per_script {
            if let Some(&oldest_version) = entry.versions.keys().min() {
                entry.versions.remove(&oldest_version);
            }
        }

        let manifest = ArtifactManifest {
            script_id,
            version,
            source_hash: artifact.source_hash,
            artifact_hash: artifact.artifact_hash,
            target: artifact.target,
            artifact_size: artifact.native_bytes.len(),
        };

        if version > entry.latest_version {
            entry.latest_version = version;
        }

        entry
            .versions
            .insert(version, (manifest.clone(), artifact.native_bytes));

        Ok(manifest)
    }

    /// Looks up the manifest for a script at a specific version.
    pub fn get_manifest(
        &self,
        script_id: u64,
        version: u32,
    ) -> Result<&ArtifactManifest, RegistryError> {
        let entry = self
            .scripts
            .get(&script_id)
            .ok_or(RegistryError::NotFound { script_id, version })?;

        entry
            .versions
            .get(&version)
            .map(|(manifest, _)| manifest)
            .ok_or(RegistryError::NotFound { script_id, version })
    }

    /// Retrieves the native artifact bytes for a script at a specific version.
    ///
    /// Verifies the artifact hash before returning.
    pub fn get_artifact_bytes(&self, script_id: u64, version: u32) -> Result<&[u8], RegistryError> {
        let entry = self
            .scripts
            .get(&script_id)
            .ok_or(RegistryError::NotFound { script_id, version })?;

        let (manifest, bytes) = entry
            .versions
            .get(&version)
            .ok_or(RegistryError::NotFound { script_id, version })?;

        // Re-verify integrity on retrieval
        let computed_hash = sha256(bytes);
        if computed_hash != manifest.artifact_hash {
            return Err(RegistryError::HashMismatch {
                field: "artifact",
                expected: manifest.artifact_hash,
                actual: computed_hash,
            });
        }

        Ok(bytes)
    }

    /// Removes all versions of a script from the registry.
    ///
    /// Returns `true` if the script was found and removed.
    pub fn remove_script(&mut self, script_id: u64) -> bool {
        self.scripts.remove(&script_id).is_some()
    }

    /// Removes a specific version of a script from the registry.
    ///
    /// Returns `true` if the version was found and removed.
    pub fn remove_version(&mut self, script_id: u64, version: u32) -> bool {
        if let Some(entry) = self.scripts.get_mut(&script_id) {
            let removed = entry.versions.remove(&version).is_some();
            if entry.versions.is_empty() {
                self.scripts.remove(&script_id);
            }
            removed
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server_runtime::aot::{AotCompiler, AotTarget};

    fn compile_test_artifact(wasm: &[u8]) -> AotArtifact {
        let compiler = AotCompiler::default();
        compiler.compile(wasm, AotTarget::LinuxX64).unwrap()
    }

    fn valid_wasm() -> Vec<u8> {
        wat::parse_str(r#"(module (func (export "run") (nop)))"#).unwrap()
    }

    fn valid_wasm_b() -> Vec<u8> {
        wat::parse_str(r#"(module (func (export "tick") (nop)))"#).unwrap()
    }

    #[test]
    fn register_and_lookup_artifact() {
        let mut registry = ArtifactRegistry::new(16);
        let wasm = valid_wasm();
        let artifact = compile_test_artifact(&wasm);
        let source_hash = artifact.source_hash;

        let manifest = registry.register(1, 1, artifact).unwrap();
        assert_eq!(manifest.script_id, 1);
        assert_eq!(manifest.version, 1);
        assert_eq!(manifest.source_hash, source_hash);
        assert!(manifest.artifact_size > 0);

        let looked_up = registry.get_manifest(1, 1).unwrap();
        assert_eq!(looked_up, &manifest);
    }

    #[test]
    fn get_artifact_bytes_verifies_integrity() {
        let mut registry = ArtifactRegistry::new(16);
        let wasm = valid_wasm();
        let artifact = compile_test_artifact(&wasm);

        registry.register(1, 1, artifact).unwrap();
        let bytes = registry.get_artifact_bytes(1, 1).unwrap();
        assert!(!bytes.is_empty());
    }

    #[test]
    fn register_rejects_tampered_artifact() {
        let mut registry = ArtifactRegistry::new(16);
        let wasm = valid_wasm();
        let mut artifact = compile_test_artifact(&wasm);

        // Tamper with the native bytes
        if let Some(byte) = artifact.native_bytes.last_mut() {
            *byte ^= 0xFF;
        }

        let result = registry.register(1, 1, artifact);
        assert!(result.is_err());
        match result.unwrap_err() {
            RegistryError::HashMismatch { field, .. } => {
                assert_eq!(field, "artifact");
            }
            other => panic!("expected HashMismatch, got: {other}"),
        }
    }

    #[test]
    fn register_rejects_version_conflict() {
        let mut registry = ArtifactRegistry::new(16);
        let wasm = valid_wasm();
        let artifact = compile_test_artifact(&wasm);

        registry.register(1, 1, artifact).unwrap();

        let artifact2 = compile_test_artifact(&wasm);
        let result = registry.register(1, 1, artifact2);
        assert!(result.is_err());
        match result.unwrap_err() {
            RegistryError::VersionConflict { script_id, version } => {
                assert_eq!(script_id, 1);
                assert_eq!(version, 1);
            }
            other => panic!("expected VersionConflict, got: {other}"),
        }
    }

    #[test]
    fn register_rejects_when_registry_full() {
        let mut registry = ArtifactRegistry::new(1);
        let wasm = valid_wasm();
        let artifact = compile_test_artifact(&wasm);
        registry.register(1, 1, artifact).unwrap();

        let artifact2 = compile_test_artifact(&wasm);
        let result = registry.register(2, 1, artifact2);
        assert!(result.is_err());
        match result.unwrap_err() {
            RegistryError::RegistryFull { max_scripts } => {
                assert_eq!(max_scripts, 1);
            }
            other => panic!("expected RegistryFull, got: {other}"),
        }
    }

    #[test]
    fn lookup_nonexistent_script_returns_not_found() {
        let registry = ArtifactRegistry::new(16);
        let result = registry.get_manifest(999, 1);
        assert!(result.is_err());
        match result.unwrap_err() {
            RegistryError::NotFound { script_id, version } => {
                assert_eq!(script_id, 999);
                assert_eq!(version, 1);
            }
            other => panic!("expected NotFound, got: {other}"),
        }
    }

    #[test]
    fn lookup_nonexistent_version_returns_not_found() {
        let mut registry = ArtifactRegistry::new(16);
        let wasm = valid_wasm();
        let artifact = compile_test_artifact(&wasm);
        registry.register(1, 1, artifact).unwrap();

        let result = registry.get_manifest(1, 99);
        assert!(result.is_err());
    }

    #[test]
    fn multiple_versions_per_script() {
        let mut registry = ArtifactRegistry::new(16);
        let wasm_a = valid_wasm();
        let wasm_b = valid_wasm_b();

        let artifact_a = compile_test_artifact(&wasm_a);
        let artifact_b = compile_test_artifact(&wasm_b);

        registry.register(1, 1, artifact_a).unwrap();
        registry.register(1, 2, artifact_b).unwrap();

        assert_eq!(registry.version_count(1), 2);
        assert_eq!(registry.latest_version(1), Some(2));
    }

    #[test]
    fn evicts_old_versions_when_over_limit() {
        let mut registry = ArtifactRegistry::with_version_limit(16, 2);
        let wasm = valid_wasm();

        for v in 1..=3 {
            let artifact = compile_test_artifact(&wasm);
            registry.register(1, v, artifact).unwrap();
        }

        // Should have evicted version 1, keeping 2 and 3
        assert_eq!(registry.version_count(1), 2);
        assert!(registry.get_manifest(1, 1).is_err());
        assert!(registry.get_manifest(1, 2).is_ok());
        assert!(registry.get_manifest(1, 3).is_ok());
    }

    #[test]
    fn remove_script_clears_all_versions() {
        let mut registry = ArtifactRegistry::new(16);
        let wasm = valid_wasm();

        let artifact = compile_test_artifact(&wasm);
        registry.register(1, 1, artifact).unwrap();

        assert!(registry.remove_script(1));
        assert_eq!(registry.script_count(), 0);
        assert!(!registry.remove_script(1));
    }

    #[test]
    fn remove_version_keeps_other_versions() {
        let mut registry = ArtifactRegistry::new(16);
        let wasm = valid_wasm();

        let a1 = compile_test_artifact(&wasm);
        let a2 = compile_test_artifact(&wasm);
        registry.register(1, 1, a1).unwrap();
        registry.register(1, 2, a2).unwrap();

        assert!(registry.remove_version(1, 1));
        assert_eq!(registry.version_count(1), 1);
        assert!(registry.get_manifest(1, 2).is_ok());
    }

    #[test]
    fn remove_last_version_removes_script() {
        let mut registry = ArtifactRegistry::new(16);
        let wasm = valid_wasm();
        let artifact = compile_test_artifact(&wasm);
        registry.register(1, 1, artifact).unwrap();

        assert!(registry.remove_version(1, 1));
        assert_eq!(registry.script_count(), 0);
    }

    #[test]
    fn remove_nonexistent_returns_false() {
        let mut registry = ArtifactRegistry::new(16);
        assert!(!registry.remove_version(999, 1));
    }

    #[test]
    fn latest_version_returns_none_for_unregistered() {
        let registry = ArtifactRegistry::new(16);
        assert_eq!(registry.latest_version(999), None);
    }

    #[test]
    fn registry_error_display() {
        let err = RegistryError::NotFound {
            script_id: 42,
            version: 3,
        };
        let msg = format!("{err}");
        assert!(msg.contains("42"));
        assert!(msg.contains("3"));

        let err2 = RegistryError::VersionConflict {
            script_id: 1,
            version: 1,
        };
        let msg2 = format!("{err2}");
        assert!(msg2.contains("version conflict"));
    }
}
