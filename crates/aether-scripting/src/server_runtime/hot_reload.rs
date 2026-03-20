//! Hot-reload support for server-side WASM modules.
//!
//! Manages versioned module slots and supports atomic version swaps
//! without requiring a world restart. Tracks module lifecycle state
//! (active, draining, unloaded) to ensure safe transitions.

use std::collections::HashMap;

use super::aot::AotTarget;

/// Maximum number of in-flight (draining) versions allowed per script.
const MAX_DRAINING_VERSIONS: usize = 2;

/// State of a module version in the hot-reload lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleLifecycleState {
    /// The version is actively serving new requests.
    Active,
    /// The version is draining: no new requests, but existing executions
    /// are allowed to complete.
    Draining,
    /// The version has been fully unloaded.
    Unloaded,
}

/// A specific version of a module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleVersion {
    /// Version number.
    pub version: u32,
    /// SHA-256 hash of the artifact for this version.
    pub artifact_hash: [u8; 32],
    /// Target platform.
    pub target: AotTarget,
    /// Current lifecycle state.
    pub state: ModuleLifecycleState,
    /// Number of in-flight executions on this version.
    pub in_flight_count: u32,
}

/// Outcome of a hot-reload operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReloadOutcome {
    /// The new version was successfully swapped in.
    Swapped {
        script_id: u64,
        old_version: u32,
        new_version: u32,
    },
    /// The new version was loaded as the first version (no swap needed).
    FirstLoad { script_id: u64, version: u32 },
    /// The reload was rejected.
    Rejected { script_id: u64, reason: String },
}

/// Errors that can occur during hot-reload operations.
#[derive(Debug)]
pub enum HotReloadError {
    /// The script is not registered in the reload manager.
    ScriptNotFound(u64),
    /// The version number is invalid (e.g., not monotonically increasing).
    InvalidVersion {
        script_id: u64,
        current: u32,
        requested: u32,
    },
    /// Too many versions are currently draining.
    TooManyDrainingVersions { script_id: u64, count: usize },
    /// Cannot unload: there are still in-flight executions.
    InFlightExecutions {
        script_id: u64,
        version: u32,
        count: u32,
    },
}

impl std::fmt::Display for HotReloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ScriptNotFound(id) => write!(f, "script {id} not found in reload manager"),
            Self::InvalidVersion {
                script_id,
                current,
                requested,
            } => write!(
                f,
                "invalid version for script {script_id}: current={current}, requested={requested}"
            ),
            Self::TooManyDrainingVersions { script_id, count } => write!(
                f,
                "script {script_id} has {count} draining versions (max {MAX_DRAINING_VERSIONS})"
            ),
            Self::InFlightExecutions {
                script_id,
                version,
                count,
            } => write!(
                f,
                "script {script_id} version {version} has {count} in-flight executions"
            ),
        }
    }
}

impl std::error::Error for HotReloadError {}

/// A slot holding the current and previous versions of a module.
#[derive(Debug)]
struct ModuleSlot {
    /// The currently active version (if any).
    active: Option<ModuleVersion>,
    /// Versions that are draining (no new requests, waiting for in-flight to finish).
    draining: Vec<ModuleVersion>,
}

impl ModuleSlot {
    fn new() -> Self {
        Self {
            active: None,
            draining: Vec::new(),
        }
    }

    fn active_version(&self) -> Option<u32> {
        self.active.as_ref().map(|v| v.version)
    }
}

/// Manages hot-reload for all scripts in a world.
///
/// Each script has a `ModuleSlot` that tracks the active version and
/// any draining versions. The manager supports atomic version swaps
/// and lifecycle tracking.
#[derive(Debug)]
pub struct HotReloadManager {
    slots: HashMap<u64, ModuleSlot>,
}

impl Default for HotReloadManager {
    fn default() -> Self {
        Self::new()
    }
}

impl HotReloadManager {
    /// Creates a new hot-reload manager.
    pub fn new() -> Self {
        Self {
            slots: HashMap::new(),
        }
    }

    /// Returns the number of managed scripts.
    pub fn script_count(&self) -> usize {
        self.slots.len()
    }

    /// Returns the active version for a script, if any.
    pub fn active_version(&self, script_id: u64) -> Option<u32> {
        self.slots.get(&script_id).and_then(|s| s.active_version())
    }

    /// Returns the lifecycle state of a specific version.
    pub fn version_state(&self, script_id: u64, version: u32) -> Option<ModuleLifecycleState> {
        let slot = self.slots.get(&script_id)?;
        if let Some(active) = &slot.active {
            if active.version == version {
                return Some(active.state);
            }
        }
        slot.draining
            .iter()
            .find(|v| v.version == version)
            .map(|v| v.state)
    }

    /// Loads a new version of a module, performing an atomic swap if
    /// a previous version exists.
    ///
    /// - If no previous version exists, this is a first load.
    /// - If a previous version exists, it is moved to draining state
    ///   and the new version becomes active.
    pub fn load_version(
        &mut self,
        script_id: u64,
        version: u32,
        artifact_hash: [u8; 32],
        target: AotTarget,
    ) -> Result<ReloadOutcome, HotReloadError> {
        let slot = self.slots.entry(script_id).or_insert_with(ModuleSlot::new);

        // Validate version is newer than current
        if let Some(active) = &slot.active {
            if version <= active.version {
                return Err(HotReloadError::InvalidVersion {
                    script_id,
                    current: active.version,
                    requested: version,
                });
            }

            // Check draining limit
            if slot.draining.len() >= MAX_DRAINING_VERSIONS {
                return Err(HotReloadError::TooManyDrainingVersions {
                    script_id,
                    count: slot.draining.len(),
                });
            }
        }

        let new_version = ModuleVersion {
            version,
            artifact_hash,
            target,
            state: ModuleLifecycleState::Active,
            in_flight_count: 0,
        };

        let outcome = if let Some(mut old_active) = slot.active.take() {
            let old_version = old_active.version;
            old_active.state = ModuleLifecycleState::Draining;
            slot.draining.push(old_active);
            slot.active = Some(new_version);
            ReloadOutcome::Swapped {
                script_id,
                old_version,
                new_version: version,
            }
        } else {
            slot.active = Some(new_version);
            ReloadOutcome::FirstLoad { script_id, version }
        };

        Ok(outcome)
    }

    /// Increments the in-flight execution count for the active version.
    pub fn acquire_execution(&mut self, script_id: u64) -> Result<u32, HotReloadError> {
        let slot = self
            .slots
            .get_mut(&script_id)
            .ok_or(HotReloadError::ScriptNotFound(script_id))?;

        let active = slot
            .active
            .as_mut()
            .ok_or(HotReloadError::ScriptNotFound(script_id))?;

        active.in_flight_count += 1;
        Ok(active.version)
    }

    /// Decrements the in-flight execution count for a specific version.
    pub fn release_execution(
        &mut self,
        script_id: u64,
        version: u32,
    ) -> Result<(), HotReloadError> {
        let slot = self
            .slots
            .get_mut(&script_id)
            .ok_or(HotReloadError::ScriptNotFound(script_id))?;

        // Check active version first
        if let Some(active) = &mut slot.active {
            if active.version == version {
                active.in_flight_count = active.in_flight_count.saturating_sub(1);
                return Ok(());
            }
        }

        // Check draining versions
        if let Some(draining) = slot.draining.iter_mut().find(|v| v.version == version) {
            draining.in_flight_count = draining.in_flight_count.saturating_sub(1);
            return Ok(());
        }

        Err(HotReloadError::ScriptNotFound(script_id))
    }

    /// Attempts to finalize draining versions that have no in-flight executions.
    ///
    /// Returns the list of versions that were fully unloaded.
    pub fn finalize_drained(&mut self, script_id: u64) -> Vec<u32> {
        let mut unloaded = Vec::new();

        if let Some(slot) = self.slots.get_mut(&script_id) {
            slot.draining.retain(|v| {
                if v.in_flight_count == 0 {
                    unloaded.push(v.version);
                    false // remove from draining list
                } else {
                    true // keep in draining list
                }
            });
        }

        unloaded
    }

    /// Returns the number of draining versions for a script.
    pub fn draining_count(&self, script_id: u64) -> usize {
        self.slots
            .get(&script_id)
            .map(|s| s.draining.len())
            .unwrap_or(0)
    }

    /// Removes a script entirely from the reload manager.
    ///
    /// Fails if any version has in-flight executions.
    pub fn remove_script(&mut self, script_id: u64) -> Result<(), HotReloadError> {
        let slot = self
            .slots
            .get(&script_id)
            .ok_or(HotReloadError::ScriptNotFound(script_id))?;

        if let Some(active) = &slot.active {
            if active.in_flight_count > 0 {
                return Err(HotReloadError::InFlightExecutions {
                    script_id,
                    version: active.version,
                    count: active.in_flight_count,
                });
            }
        }

        for draining in &slot.draining {
            if draining.in_flight_count > 0 {
                return Err(HotReloadError::InFlightExecutions {
                    script_id,
                    version: draining.version,
                    count: draining.in_flight_count,
                });
            }
        }

        self.slots.remove(&script_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_hash(seed: u8) -> [u8; 32] {
        [seed; 32]
    }

    #[test]
    fn first_load_creates_active_version() {
        let mut manager = HotReloadManager::new();
        let outcome = manager
            .load_version(1, 1, test_hash(0xAA), AotTarget::LinuxX64)
            .unwrap();

        match outcome {
            ReloadOutcome::FirstLoad { script_id, version } => {
                assert_eq!(script_id, 1);
                assert_eq!(version, 1);
            }
            other => panic!("expected FirstLoad, got: {other:?}"),
        }

        assert_eq!(manager.active_version(1), Some(1));
        assert_eq!(
            manager.version_state(1, 1),
            Some(ModuleLifecycleState::Active)
        );
    }

    #[test]
    fn swap_moves_old_to_draining() {
        let mut manager = HotReloadManager::new();
        manager
            .load_version(1, 1, test_hash(0xAA), AotTarget::LinuxX64)
            .unwrap();

        let outcome = manager
            .load_version(1, 2, test_hash(0xBB), AotTarget::LinuxX64)
            .unwrap();

        match outcome {
            ReloadOutcome::Swapped {
                script_id,
                old_version,
                new_version,
            } => {
                assert_eq!(script_id, 1);
                assert_eq!(old_version, 1);
                assert_eq!(new_version, 2);
            }
            other => panic!("expected Swapped, got: {other:?}"),
        }

        assert_eq!(manager.active_version(1), Some(2));
        assert_eq!(
            manager.version_state(1, 1),
            Some(ModuleLifecycleState::Draining)
        );
        assert_eq!(
            manager.version_state(1, 2),
            Some(ModuleLifecycleState::Active)
        );
        assert_eq!(manager.draining_count(1), 1);
    }

    #[test]
    fn rejects_non_monotonic_version() {
        let mut manager = HotReloadManager::new();
        manager
            .load_version(1, 5, test_hash(0xAA), AotTarget::LinuxX64)
            .unwrap();

        let result = manager.load_version(1, 3, test_hash(0xBB), AotTarget::LinuxX64);
        assert!(result.is_err());
        match result.unwrap_err() {
            HotReloadError::InvalidVersion {
                current, requested, ..
            } => {
                assert_eq!(current, 5);
                assert_eq!(requested, 3);
            }
            other => panic!("expected InvalidVersion, got: {other}"),
        }
    }

    #[test]
    fn rejects_same_version() {
        let mut manager = HotReloadManager::new();
        manager
            .load_version(1, 1, test_hash(0xAA), AotTarget::LinuxX64)
            .unwrap();

        let result = manager.load_version(1, 1, test_hash(0xBB), AotTarget::LinuxX64);
        assert!(result.is_err());
    }

    #[test]
    fn rejects_too_many_draining() {
        let mut manager = HotReloadManager::new();
        manager
            .load_version(1, 1, test_hash(0xAA), AotTarget::LinuxX64)
            .unwrap();
        // Acquire execution on v1 so it stays draining
        manager.acquire_execution(1).unwrap();

        manager
            .load_version(1, 2, test_hash(0xBB), AotTarget::LinuxX64)
            .unwrap();
        // Acquire execution on v2 so it stays draining
        manager.acquire_execution(1).unwrap();

        manager
            .load_version(1, 3, test_hash(0xCC), AotTarget::LinuxX64)
            .unwrap();

        // v1 and v2 are draining, can't add more
        let result = manager.load_version(1, 4, test_hash(0xDD), AotTarget::LinuxX64);
        assert!(result.is_err());
        match result.unwrap_err() {
            HotReloadError::TooManyDrainingVersions { count, .. } => {
                assert_eq!(count, MAX_DRAINING_VERSIONS);
            }
            other => panic!("expected TooManyDrainingVersions, got: {other}"),
        }
    }

    #[test]
    fn acquire_and_release_execution() {
        let mut manager = HotReloadManager::new();
        manager
            .load_version(1, 1, test_hash(0xAA), AotTarget::LinuxX64)
            .unwrap();

        let version = manager.acquire_execution(1).unwrap();
        assert_eq!(version, 1);

        manager.release_execution(1, 1).unwrap();
    }

    #[test]
    fn acquire_on_nonexistent_script_fails() {
        let mut manager = HotReloadManager::new();
        let result = manager.acquire_execution(999);
        assert!(result.is_err());
    }

    #[test]
    fn release_on_draining_version() {
        let mut manager = HotReloadManager::new();
        manager
            .load_version(1, 1, test_hash(0xAA), AotTarget::LinuxX64)
            .unwrap();
        manager.acquire_execution(1).unwrap();

        // Swap to v2, v1 goes to draining
        manager
            .load_version(1, 2, test_hash(0xBB), AotTarget::LinuxX64)
            .unwrap();

        // Release the execution that was on v1
        manager.release_execution(1, 1).unwrap();
    }

    #[test]
    fn finalize_drained_removes_completed_versions() {
        let mut manager = HotReloadManager::new();
        manager
            .load_version(1, 1, test_hash(0xAA), AotTarget::LinuxX64)
            .unwrap();
        manager
            .load_version(1, 2, test_hash(0xBB), AotTarget::LinuxX64)
            .unwrap();

        // v1 should be draining with 0 in-flight
        let unloaded = manager.finalize_drained(1);
        assert_eq!(unloaded, vec![1]);
        assert_eq!(manager.draining_count(1), 0);
    }

    #[test]
    fn finalize_drained_keeps_in_flight_versions() {
        let mut manager = HotReloadManager::new();
        manager
            .load_version(1, 1, test_hash(0xAA), AotTarget::LinuxX64)
            .unwrap();
        manager.acquire_execution(1).unwrap();

        manager
            .load_version(1, 2, test_hash(0xBB), AotTarget::LinuxX64)
            .unwrap();

        // v1 has in-flight, should not be finalized
        let unloaded = manager.finalize_drained(1);
        assert!(unloaded.is_empty());
        assert_eq!(manager.draining_count(1), 1);

        // Release and try again
        manager.release_execution(1, 1).unwrap();
        let unloaded = manager.finalize_drained(1);
        assert_eq!(unloaded, vec![1]);
    }

    #[test]
    fn remove_script_succeeds_when_no_in_flight() {
        let mut manager = HotReloadManager::new();
        manager
            .load_version(1, 1, test_hash(0xAA), AotTarget::LinuxX64)
            .unwrap();

        manager.remove_script(1).unwrap();
        assert_eq!(manager.script_count(), 0);
    }

    #[test]
    fn remove_script_fails_with_in_flight() {
        let mut manager = HotReloadManager::new();
        manager
            .load_version(1, 1, test_hash(0xAA), AotTarget::LinuxX64)
            .unwrap();
        manager.acquire_execution(1).unwrap();

        let result = manager.remove_script(1);
        assert!(result.is_err());
        match result.unwrap_err() {
            HotReloadError::InFlightExecutions { count, .. } => {
                assert_eq!(count, 1);
            }
            other => panic!("expected InFlightExecutions, got: {other}"),
        }
    }

    #[test]
    fn remove_nonexistent_script_fails() {
        let mut manager = HotReloadManager::new();
        let result = manager.remove_script(999);
        assert!(result.is_err());
    }

    #[test]
    fn version_state_returns_none_for_unknown() {
        let manager = HotReloadManager::new();
        assert_eq!(manager.version_state(999, 1), None);
    }

    #[test]
    fn multiple_scripts_independent() {
        let mut manager = HotReloadManager::new();
        manager
            .load_version(1, 1, test_hash(0xAA), AotTarget::LinuxX64)
            .unwrap();
        manager
            .load_version(2, 1, test_hash(0xBB), AotTarget::LinuxX64)
            .unwrap();

        assert_eq!(manager.script_count(), 2);
        assert_eq!(manager.active_version(1), Some(1));
        assert_eq!(manager.active_version(2), Some(1));

        manager
            .load_version(1, 2, test_hash(0xCC), AotTarget::LinuxX64)
            .unwrap();
        assert_eq!(manager.active_version(1), Some(2));
        assert_eq!(manager.active_version(2), Some(1)); // unchanged
    }

    #[test]
    fn hot_reload_error_display() {
        let err = HotReloadError::ScriptNotFound(42);
        assert!(format!("{err}").contains("42"));

        let err2 = HotReloadError::InvalidVersion {
            script_id: 1,
            current: 3,
            requested: 2,
        };
        assert!(format!("{err2}").contains("current=3"));
    }

    #[test]
    fn active_version_returns_none_for_unknown_script() {
        let manager = HotReloadManager::new();
        assert_eq!(manager.active_version(999), None);
    }

    #[test]
    fn draining_count_returns_zero_for_unknown() {
        let manager = HotReloadManager::new();
        assert_eq!(manager.draining_count(999), 0);
    }

    #[test]
    fn finalize_drained_on_unknown_script_returns_empty() {
        let mut manager = HotReloadManager::new();
        let unloaded = manager.finalize_drained(999);
        assert!(unloaded.is_empty());
    }
}
