//! Save/serialization contract for ECS snapshots.
//!
//! This module is only available when the `serde` cargo feature is enabled.
//! It defines a stable [`WorldSnapshot`] format, a [`SaveLoad`] trait that any
//! type can implement for JSON round-tripping, and two helpers
//! ([`snapshot_world_manual`] / [`restore_world_manual`]) that games can use
//! to build snapshots from whatever components they care about.
//!
//! The core `aether-ecs` crate stays serde-free; everything here lives behind
//! the feature flag so downstream users opt in explicitly.
//!
//! # Example: round-trip a toy component
//!
//! ```
//! use aether_ecs::save::{
//!     restore_world_manual, snapshot_world_manual, SaveLoad, SaveError, SNAPSHOT_VERSION,
//! };
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Debug, Serialize, Deserialize, PartialEq)]
//! struct Position { x: f32, y: f32 }
//!
//! // Build a snapshot from a hand-assembled list of (type_name, json) pairs.
//! let positions = vec![Position { x: 1.0, y: 2.0 }, Position { x: 3.0, y: 4.0 }];
//! let entries = vec![(
//!     std::any::type_name::<Position>().to_string(),
//!     serde_json::to_value(&positions).unwrap(),
//! )];
//! let snapshot = snapshot_world_manual(SNAPSHOT_VERSION, entries);
//!
//! // Round-trip through JSON.
//! let json = snapshot.to_json().unwrap();
//! let restored = aether_ecs::save::WorldSnapshot::from_json(&json).unwrap();
//!
//! // Drain components back into game state via restore_world_manual.
//! let mut out: Vec<Position> = Vec::new();
//! restore_world_manual(&restored, |type_name, value| {
//!     if type_name == std::any::type_name::<Position>() {
//!         out = serde_json::from_value(value.clone())
//!             .map_err(|e| SaveError::Json(e.to_string()))?;
//!     }
//!     Ok(())
//! }).unwrap();
//!
//! assert_eq!(out, positions);
//! ```

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Current snapshot format version. Bump on incompatible format changes.
pub const SNAPSHOT_VERSION: u32 = 1;

const ERR_JSON_PARSE: &str = "failed to parse snapshot JSON";
const ERR_JSON_EMIT: &str = "failed to serialize snapshot to JSON";

/// Errors produced by [`SaveLoad`] implementations and the snapshot helpers.
#[derive(Debug)]
pub enum SaveError {
    /// A JSON encode/decode failed. The inner string is a human-readable
    /// description suitable for logging.
    Json(String),
    /// A component type name appeared in the snapshot that the restorer did
    /// not recognize.
    UnknownComponent(String),
    /// The snapshot's format version does not match what the caller expected.
    VersionMismatch { expected: u32, found: u32 },
}

impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SaveError::Json(msg) => write!(f, "save error: json: {msg}"),
            SaveError::UnknownComponent(name) => {
                write!(f, "save error: unknown component type `{name}`")
            }
            SaveError::VersionMismatch { expected, found } => write!(
                f,
                "save error: snapshot version mismatch (expected {expected}, found {found})"
            ),
        }
    }
}

impl std::error::Error for SaveError {}

/// Round-trip trait for anything that can be serialized to and from JSON.
///
/// Implemented for [`WorldSnapshot`] out of the box; games are encouraged to
/// implement it for their own save payloads too.
pub trait SaveLoad: Sized {
    /// Encode `self` to a JSON string.
    fn to_json(&self) -> Result<String, SaveError>;
    /// Decode `self` from a JSON string.
    fn from_json(s: &str) -> Result<Self, SaveError>;
}

/// Stable on-disk representation of a world snapshot.
///
/// A snapshot groups component values by their Rust type name (as returned by
/// [`std::any::type_name`]). Each value is an arbitrary
/// [`serde_json::Value`] — typically an array of per-entity component values.
/// This is deliberately loose so games can store whatever layout fits them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorldSnapshot {
    /// Snapshot format version. See [`SNAPSHOT_VERSION`].
    pub version: u32,
    /// Map from component type name to its serialized value.
    pub components: BTreeMap<String, serde_json::Value>,
}

impl WorldSnapshot {
    /// Create an empty snapshot at the current [`SNAPSHOT_VERSION`].
    pub fn new() -> Self {
        Self {
            version: SNAPSHOT_VERSION,
            components: BTreeMap::new(),
        }
    }

    /// Check that the snapshot's version equals `expected`, otherwise return
    /// [`SaveError::VersionMismatch`].
    pub fn ensure_version(&self, expected: u32) -> Result<(), SaveError> {
        if self.version == expected {
            Ok(())
        } else {
            Err(SaveError::VersionMismatch {
                expected,
                found: self.version,
            })
        }
    }
}

impl Default for WorldSnapshot {
    fn default() -> Self {
        Self::new()
    }
}

impl SaveLoad for WorldSnapshot {
    fn to_json(&self) -> Result<String, SaveError> {
        serde_json::to_string(self).map_err(|e| SaveError::Json(format!("{ERR_JSON_EMIT}: {e}")))
    }

    fn from_json(s: &str) -> Result<Self, SaveError> {
        serde_json::from_str(s).map_err(|e| SaveError::Json(format!("{ERR_JSON_PARSE}: {e}")))
    }
}

/// Build a [`WorldSnapshot`] by hand from a list of
/// `(component type name, serialized value)` pairs.
///
/// If the same type name appears twice, the later entry wins. This keeps the
/// helper simple — callers that need merging semantics can pre-aggregate.
pub fn snapshot_world_manual(
    version: u32,
    entries: Vec<(String, serde_json::Value)>,
) -> WorldSnapshot {
    WorldSnapshot {
        version,
        components: entries.into_iter().collect(),
    }
}

/// Iterate over every `(component type name, serialized value)` pair in
/// `snapshot` and hand it to `restore_one`. The closure is expected to
/// deserialize and install the value into the game's world.
///
/// The snapshot version is validated against [`SNAPSHOT_VERSION`] before any
/// entries are visited.
pub fn restore_world_manual<F>(snapshot: &WorldSnapshot, mut restore_one: F) -> Result<(), SaveError>
where
    F: FnMut(&str, &serde_json::Value) -> Result<(), SaveError>,
{
    snapshot.ensure_version(SNAPSHOT_VERSION)?;
    for (name, value) in &snapshot.components {
        restore_one(name.as_str(), value)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TYPE_POSITION: &str = "Position";
    const TYPE_VELOCITY: &str = "Velocity";

    fn sample_snapshot() -> WorldSnapshot {
        let mut components = BTreeMap::new();
        components.insert(
            TYPE_POSITION.to_string(),
            serde_json::json!([{ "x": 1.0, "y": 2.0 }, { "x": 3.0, "y": 4.0 }]),
        );
        components.insert(
            TYPE_VELOCITY.to_string(),
            serde_json::json!([{ "dx": 0.5, "dy": -0.5 }]),
        );
        WorldSnapshot {
            version: SNAPSHOT_VERSION,
            components,
        }
    }

    #[test]
    fn round_trip_preserves_version_and_components() {
        let snapshot = sample_snapshot();
        let json = snapshot.to_json().expect("serialize");
        let decoded = WorldSnapshot::from_json(&json).expect("deserialize");
        assert_eq!(decoded.version, SNAPSHOT_VERSION);
        assert_eq!(decoded, snapshot);
    }

    #[test]
    fn version_mismatch_is_reported() {
        let mut snapshot = sample_snapshot();
        snapshot.version = 99;
        let err = snapshot
            .ensure_version(SNAPSHOT_VERSION)
            .expect_err("should mismatch");
        match err {
            SaveError::VersionMismatch { expected, found } => {
                assert_eq!(expected, SNAPSHOT_VERSION);
                assert_eq!(found, 99);
            }
            other => panic!("unexpected error variant: {other:?}"),
        }
    }

    #[test]
    fn restore_rejects_mismatched_version() {
        let mut snapshot = sample_snapshot();
        snapshot.version = SNAPSHOT_VERSION + 1;
        let err = restore_world_manual(&snapshot, |_, _| Ok(())).expect_err("should mismatch");
        assert!(matches!(err, SaveError::VersionMismatch { .. }));
    }

    #[test]
    fn malformed_json_returns_json_error() {
        let err = WorldSnapshot::from_json("{ not valid json").expect_err("should fail");
        match err {
            SaveError::Json(msg) => assert!(msg.contains(ERR_JSON_PARSE)),
            other => panic!("unexpected error variant: {other:?}"),
        }
    }

    #[test]
    fn empty_snapshot_round_trips() {
        let snapshot = WorldSnapshot::new();
        let json = snapshot.to_json().expect("serialize");
        let decoded = WorldSnapshot::from_json(&json).expect("deserialize");
        assert_eq!(decoded.version, SNAPSHOT_VERSION);
        assert!(decoded.components.is_empty());
    }

    #[test]
    fn snapshot_world_manual_and_restore_round_trip() {
        let entries = vec![
            (
                TYPE_POSITION.to_string(),
                serde_json::json!([{"x": 1.0, "y": 2.0}]),
            ),
            (
                TYPE_VELOCITY.to_string(),
                serde_json::json!([{"dx": 0.1, "dy": 0.2}]),
            ),
        ];
        let snapshot = snapshot_world_manual(SNAPSHOT_VERSION, entries);
        assert_eq!(snapshot.version, SNAPSHOT_VERSION);
        assert_eq!(snapshot.components.len(), 2);

        let mut seen: Vec<String> = Vec::new();
        restore_world_manual(&snapshot, |name, value| {
            assert!(value.is_array());
            seen.push(name.to_string());
            Ok(())
        })
        .expect("restore should succeed");
        assert_eq!(seen, vec![TYPE_POSITION.to_string(), TYPE_VELOCITY.to_string()]);
    }

    #[test]
    fn snapshot_world_manual_dedupes_on_type_name() {
        let entries = vec![
            (TYPE_POSITION.to_string(), serde_json::json!([1])),
            (TYPE_POSITION.to_string(), serde_json::json!([2, 3])),
        ];
        let snapshot = snapshot_world_manual(SNAPSHOT_VERSION, entries);
        assert_eq!(snapshot.components.len(), 1);
        assert_eq!(
            snapshot.components.get(TYPE_POSITION).unwrap(),
            &serde_json::json!([2, 3])
        );
    }

    #[test]
    fn restore_propagates_closure_errors() {
        let snapshot = sample_snapshot();
        let err = restore_world_manual(&snapshot, |name, _| {
            Err(SaveError::UnknownComponent(name.to_string()))
        })
        .expect_err("closure error should bubble");
        assert!(matches!(err, SaveError::UnknownComponent(_)));
    }
}
