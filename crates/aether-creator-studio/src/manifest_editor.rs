//! World manifest creation, validation, and patching.

use serde::{Deserialize, Serialize};

const MAX_NAME_LENGTH: usize = 100;
const MAX_PLAYERS_LIMIT: u32 = 1000;
const GRAVITY_MIN: f32 = -100.0;
const GRAVITY_MAX: f32 = 100.0;
const DEFAULT_GRAVITY: f32 = -9.81;
const DEFAULT_TICK_RATE: u32 = 60;
const DEFAULT_MAX_PLAYERS: u32 = 50;

/// A complete world manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldManifest {
    pub world_id: String,
    pub name: String,
    pub description: String,
    pub physics: PhysicsSettings,
    pub spawn_points: Vec<SpawnPoint>,
    pub max_players: u32,
}

/// Physics configuration for the world.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhysicsSettings {
    pub gravity: f32,
    pub tick_rate: u32,
}

/// A spawn point in the world.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnPoint {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub yaw_deg: f32,
}

impl SpawnPoint {
    pub fn new(x: f32, y: f32, z: f32, yaw_deg: f32) -> Self {
        Self { x, y, z, yaw_deg }
    }
}

/// A validation error on a manifest field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestValidationError {
    pub field: String,
    pub message: String,
}

impl ManifestValidationError {
    pub fn new(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
        }
    }
}

/// A patch to apply to an existing manifest.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ManifestPatch {
    pub name: Option<String>,
    pub description: Option<String>,
    pub gravity: Option<f32>,
    pub tick_rate: Option<u32>,
    pub max_players: Option<u32>,
    pub add_spawn_points: Vec<SpawnPoint>,
    pub remove_spawn_point_indices: Vec<usize>,
}

/// Create a default manifest with sensible defaults.
pub fn create_default_manifest(
    world_id: impl Into<String>,
    name: impl Into<String>,
) -> WorldManifest {
    WorldManifest {
        world_id: world_id.into(),
        name: name.into(),
        description: String::new(),
        physics: PhysicsSettings {
            gravity: DEFAULT_GRAVITY,
            tick_rate: DEFAULT_TICK_RATE,
        },
        spawn_points: vec![SpawnPoint::new(0.0, 0.0, 0.0, 0.0)],
        max_players: DEFAULT_MAX_PLAYERS,
    }
}

/// Validate a manifest, returning any errors found.
pub fn validate_manifest(manifest: &WorldManifest) -> Result<(), Vec<ManifestValidationError>> {
    let mut errors = Vec::new();

    if manifest.world_id.is_empty() {
        errors.push(ManifestValidationError::new(
            "world_id",
            "world_id must not be empty",
        ));
    }

    if manifest.name.is_empty() {
        errors.push(ManifestValidationError::new(
            "name",
            "name must not be empty",
        ));
    } else if manifest.name.len() > MAX_NAME_LENGTH {
        errors.push(ManifestValidationError::new(
            "name",
            format!("name must be at most {MAX_NAME_LENGTH} characters"),
        ));
    }

    if manifest.max_players == 0 || manifest.max_players > MAX_PLAYERS_LIMIT {
        errors.push(ManifestValidationError::new(
            "max_players",
            format!("max_players must be between 1 and {MAX_PLAYERS_LIMIT}"),
        ));
    }

    if manifest.physics.gravity < GRAVITY_MIN || manifest.physics.gravity > GRAVITY_MAX {
        errors.push(ManifestValidationError::new(
            "physics.gravity",
            format!("gravity must be between {GRAVITY_MIN} and {GRAVITY_MAX}"),
        ));
    }

    if manifest.spawn_points.is_empty() {
        errors.push(ManifestValidationError::new(
            "spawn_points",
            "at least one spawn point is required",
        ));
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Apply a patch to a manifest, returning the updated manifest.
/// Validates the result before returning.
pub fn apply_patch(
    manifest: &WorldManifest,
    patch: &ManifestPatch,
) -> Result<WorldManifest, Vec<ManifestValidationError>> {
    let mut result = manifest.clone();

    if let Some(ref name) = patch.name {
        result.name = name.clone();
    }
    if let Some(ref desc) = patch.description {
        result.description = desc.clone();
    }
    if let Some(gravity) = patch.gravity {
        result.physics.gravity = gravity;
    }
    if let Some(tick_rate) = patch.tick_rate {
        result.physics.tick_rate = tick_rate;
    }
    if let Some(max_players) = patch.max_players {
        result.max_players = max_players;
    }

    // Remove spawn points by index (in reverse order to preserve indices)
    let mut indices_to_remove = patch.remove_spawn_point_indices.clone();
    indices_to_remove.sort_unstable();
    indices_to_remove.dedup();
    for &idx in indices_to_remove.iter().rev() {
        if idx < result.spawn_points.len() {
            result.spawn_points.remove(idx);
        }
    }

    // Add new spawn points
    for sp in &patch.add_spawn_points {
        result.spawn_points.push(sp.clone());
    }

    validate_manifest(&result)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_default_manifest() {
        let m = create_default_manifest("world-1", "My World");
        assert_eq!(m.world_id, "world-1");
        assert_eq!(m.name, "My World");
        assert!(m.description.is_empty());
        assert_eq!(m.physics.gravity, DEFAULT_GRAVITY);
        assert_eq!(m.physics.tick_rate, DEFAULT_TICK_RATE);
        assert_eq!(m.max_players, DEFAULT_MAX_PLAYERS);
        assert_eq!(m.spawn_points.len(), 1);
    }

    #[test]
    fn test_validate_valid_manifest() {
        let m = create_default_manifest("w1", "Test World");
        assert!(validate_manifest(&m).is_ok());
    }

    #[test]
    fn test_validate_empty_world_id() {
        let mut m = create_default_manifest("", "Test");
        m.world_id = String::new();
        let errs = validate_manifest(&m).unwrap_err();
        assert!(errs.iter().any(|e| e.field == "world_id"));
    }

    #[test]
    fn test_validate_empty_name() {
        let mut m = create_default_manifest("w1", "");
        m.name = String::new();
        let errs = validate_manifest(&m).unwrap_err();
        assert!(errs.iter().any(|e| e.field == "name"));
    }

    #[test]
    fn test_validate_long_name() {
        let long_name = "x".repeat(101);
        let mut m = create_default_manifest("w1", "ok");
        m.name = long_name;
        let errs = validate_manifest(&m).unwrap_err();
        assert!(errs.iter().any(|e| e.field == "name"));
    }

    #[test]
    fn test_validate_max_name_length() {
        let name = "x".repeat(100);
        let mut m = create_default_manifest("w1", "ok");
        m.name = name;
        assert!(validate_manifest(&m).is_ok());
    }

    #[test]
    fn test_validate_max_players_zero() {
        let mut m = create_default_manifest("w1", "Test");
        m.max_players = 0;
        let errs = validate_manifest(&m).unwrap_err();
        assert!(errs.iter().any(|e| e.field == "max_players"));
    }

    #[test]
    fn test_validate_max_players_too_high() {
        let mut m = create_default_manifest("w1", "Test");
        m.max_players = 1001;
        let errs = validate_manifest(&m).unwrap_err();
        assert!(errs.iter().any(|e| e.field == "max_players"));
    }

    #[test]
    fn test_validate_max_players_boundary() {
        let mut m = create_default_manifest("w1", "Test");
        m.max_players = 1000;
        assert!(validate_manifest(&m).is_ok());

        m.max_players = 1;
        assert!(validate_manifest(&m).is_ok());
    }

    #[test]
    fn test_validate_gravity_out_of_range() {
        let mut m = create_default_manifest("w1", "Test");
        m.physics.gravity = -200.0;
        let errs = validate_manifest(&m).unwrap_err();
        assert!(errs.iter().any(|e| e.field == "physics.gravity"));

        m.physics.gravity = 200.0;
        let errs = validate_manifest(&m).unwrap_err();
        assert!(errs.iter().any(|e| e.field == "physics.gravity"));
    }

    #[test]
    fn test_validate_gravity_boundary() {
        let mut m = create_default_manifest("w1", "Test");
        m.physics.gravity = -100.0;
        assert!(validate_manifest(&m).is_ok());

        m.physics.gravity = 100.0;
        assert!(validate_manifest(&m).is_ok());
    }

    #[test]
    fn test_validate_no_spawn_points() {
        let mut m = create_default_manifest("w1", "Test");
        m.spawn_points.clear();
        let errs = validate_manifest(&m).unwrap_err();
        assert!(errs.iter().any(|e| e.field == "spawn_points"));
    }

    #[test]
    fn test_validate_multiple_errors() {
        let mut m = create_default_manifest("", "");
        m.world_id = String::new();
        m.name = String::new();
        m.max_players = 0;
        m.spawn_points.clear();

        let errs = validate_manifest(&m).unwrap_err();
        assert!(errs.len() >= 4);
    }

    // Patch tests
    #[test]
    fn test_apply_patch_name() {
        let m = create_default_manifest("w1", "Old Name");
        let patch = ManifestPatch {
            name: Some("New Name".into()),
            ..Default::default()
        };
        let result = apply_patch(&m, &patch).unwrap();
        assert_eq!(result.name, "New Name");
    }

    #[test]
    fn test_apply_patch_description() {
        let m = create_default_manifest("w1", "Test");
        let patch = ManifestPatch {
            description: Some("A cool world".into()),
            ..Default::default()
        };
        let result = apply_patch(&m, &patch).unwrap();
        assert_eq!(result.description, "A cool world");
    }

    #[test]
    fn test_apply_patch_gravity() {
        let m = create_default_manifest("w1", "Test");
        let patch = ManifestPatch {
            gravity: Some(-5.0),
            ..Default::default()
        };
        let result = apply_patch(&m, &patch).unwrap();
        assert_eq!(result.physics.gravity, -5.0);
    }

    #[test]
    fn test_apply_patch_tick_rate() {
        let m = create_default_manifest("w1", "Test");
        let patch = ManifestPatch {
            tick_rate: Some(30),
            ..Default::default()
        };
        let result = apply_patch(&m, &patch).unwrap();
        assert_eq!(result.physics.tick_rate, 30);
    }

    #[test]
    fn test_apply_patch_max_players() {
        let m = create_default_manifest("w1", "Test");
        let patch = ManifestPatch {
            max_players: Some(100),
            ..Default::default()
        };
        let result = apply_patch(&m, &patch).unwrap();
        assert_eq!(result.max_players, 100);
    }

    #[test]
    fn test_apply_patch_add_spawn_point() {
        let m = create_default_manifest("w1", "Test");
        let patch = ManifestPatch {
            add_spawn_points: vec![SpawnPoint::new(10.0, 0.0, 10.0, 90.0)],
            ..Default::default()
        };
        let result = apply_patch(&m, &patch).unwrap();
        assert_eq!(result.spawn_points.len(), 2);
    }

    #[test]
    fn test_apply_patch_remove_spawn_point() {
        let m = create_default_manifest("w1", "Test");
        let patch = ManifestPatch {
            remove_spawn_point_indices: vec![0],
            add_spawn_points: vec![SpawnPoint::new(5.0, 0.0, 5.0, 0.0)],
            ..Default::default()
        };
        let result = apply_patch(&m, &patch).unwrap();
        assert_eq!(result.spawn_points.len(), 1);
        assert_eq!(result.spawn_points[0].x, 5.0);
    }

    #[test]
    fn test_apply_patch_invalid_result() {
        let m = create_default_manifest("w1", "Test");
        let patch = ManifestPatch {
            max_players: Some(0),
            ..Default::default()
        };
        let result = apply_patch(&m, &patch);
        assert!(result.is_err());
    }

    #[test]
    fn test_apply_patch_remove_all_spawns_invalid() {
        let m = create_default_manifest("w1", "Test");
        let patch = ManifestPatch {
            remove_spawn_point_indices: vec![0],
            ..Default::default()
        };
        let result = apply_patch(&m, &patch);
        assert!(result.is_err());
    }

    #[test]
    fn test_apply_patch_no_changes() {
        let m = create_default_manifest("w1", "Test");
        let patch = ManifestPatch::default();
        let result = apply_patch(&m, &patch).unwrap();
        assert_eq!(result.name, "Test");
        assert_eq!(result.world_id, "w1");
    }

    #[test]
    fn test_apply_patch_multiple_fields() {
        let m = create_default_manifest("w1", "Old");
        let patch = ManifestPatch {
            name: Some("New".into()),
            description: Some("Desc".into()),
            gravity: Some(-5.0),
            max_players: Some(200),
            ..Default::default()
        };
        let result = apply_patch(&m, &patch).unwrap();
        assert_eq!(result.name, "New");
        assert_eq!(result.description, "Desc");
        assert_eq!(result.physics.gravity, -5.0);
        assert_eq!(result.max_players, 200);
    }

    #[test]
    fn test_remove_out_of_range_index_ignored() {
        let m = create_default_manifest("w1", "Test");
        let patch = ManifestPatch {
            remove_spawn_point_indices: vec![99],
            ..Default::default()
        };
        let result = apply_patch(&m, &patch).unwrap();
        assert_eq!(result.spawn_points.len(), 1);
    }

    #[test]
    fn test_spawn_point_constructor() {
        let sp = SpawnPoint::new(1.0, 2.0, 3.0, 45.0);
        assert_eq!(sp.x, 1.0);
        assert_eq!(sp.y, 2.0);
        assert_eq!(sp.z, 3.0);
        assert_eq!(sp.yaw_deg, 45.0);
    }

    #[test]
    fn test_validation_error_constructor() {
        let e = ManifestValidationError::new("field", "msg");
        assert_eq!(e.field, "field");
        assert_eq!(e.message, "msg");
    }
}
