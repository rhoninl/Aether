//! Schema versioning + migration policy (task 73).
//!
//! Every top-level artifact carries a `schema_version` field whose value is
//! one of the variants of [`SchemaVersion`]. The [`Migrator`] trait codifies
//! the upgrade path from a known past version to a future version.
//!
//! ## Deprecation policy
//!
//! 1. Field *additions* are backward-compatible; readers treat missing fields
//!    as `Default`.
//! 2. Field *removals* follow a two-step dance:
//!     - Step 1 (one release): field marked `#[serde(default)]` and ignored at
//!       runtime; writers still emit it.
//!     - Step 2 (next release): field removed from the Rust type. The
//!       `Migrator` drops the field and records it under `migration_notes`.
//! 3. Field *renames* follow the same two-step dance using `#[serde(alias)]`.
//! 4. Type *changes* require a new `SchemaVersion` variant and a `Migrator`.
//!
//! The migration graph is linear: `V0 -> V1 -> V2 -> ...`. Each step is
//! implemented as a dedicated `Migrator` and the chain is composed at load
//! time.

use serde::{Deserialize, Serialize};

use crate::error::{SchemaError, SchemaResult};

/// Wire-level schema version. Enum variants map to stable integers via
/// `#[serde(into = "u32", try_from = "u32")]` to avoid surprising readers that
/// serialize the type to integers directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SchemaVersion {
    /// Pre-stable; only accepted for migration purposes.
    V0,
    /// First stable wire.
    V1,
}

impl SchemaVersion {
    pub const fn as_u32(self) -> u32 {
        match self {
            SchemaVersion::V0 => 0,
            SchemaVersion::V1 => 1,
        }
    }

    pub fn from_u32(value: u32) -> SchemaResult<Self> {
        match value {
            0 => Ok(SchemaVersion::V0),
            1 => Ok(SchemaVersion::V1),
            other => Err(SchemaError::UnsupportedVersion {
                found: other,
                expected: vec![0, 1],
                suggested_fix: format!(
                    "set `schema_version` to 1 (the current stable wire); support for v0 is migration-only"
                ),
            }),
        }
    }
}

impl Serialize for SchemaVersion {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        self.as_u32().serialize(ser)
    }
}

impl<'de> Deserialize<'de> for SchemaVersion {
    fn deserialize<D: serde::Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        let v = u32::deserialize(de)?;
        SchemaVersion::from_u32(v).map_err(serde::de::Error::custom)
    }
}

impl schemars::JsonSchema for SchemaVersion {
    fn schema_name() -> String {
        "SchemaVersion".to_string()
    }

    fn json_schema(_gen: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        use schemars::schema::{InstanceType, Metadata, Schema, SchemaObject, SingleOrVec};
        let mut schema = SchemaObject {
            instance_type: Some(SingleOrVec::Single(Box::new(InstanceType::Integer))),
            ..Default::default()
        };
        schema.metadata = Some(Box::new(Metadata {
            description: Some(
                "Canonical wire schema version. Currently 1 is the stable release; 0 is accepted for migration only."
                    .to_string(),
            ),
            ..Default::default()
        }));
        schema.enum_values = Some(vec![
            serde_json::Value::Number(0u32.into()),
            serde_json::Value::Number(1u32.into()),
        ]);
        Schema::Object(schema)
    }
}

/// In-place migrator from one version to the next.
///
/// Implementations operate on a generic `serde_json::Value` so that they can
/// be applied uniformly to any artifact kind. `from`/`to` must differ by
/// exactly 1 step.
pub trait Migrator {
    fn from_version(&self) -> SchemaVersion;
    fn to_version(&self) -> SchemaVersion;
    fn migrate(&self, value: serde_json::Value) -> SchemaResult<serde_json::Value>;
}

/// Best-effort bridge from v0 (pre-stable) to v1 (current stable).
///
/// v0 documents differ from v1 in two ways:
/// 1. They lack a `schema_version` field entirely (treated as 0).
/// 2. Some optional settings (spawn_points, runtime) were free-form maps;
///    v1 enforces structured objects. This migrator leaves unknown keys under
///    `migration_notes` for human review rather than dropping them silently.
pub struct MigratorV0ToV1;

impl Migrator for MigratorV0ToV1 {
    fn from_version(&self) -> SchemaVersion {
        SchemaVersion::V0
    }

    fn to_version(&self) -> SchemaVersion {
        SchemaVersion::V1
    }

    fn migrate(&self, value: serde_json::Value) -> SchemaResult<serde_json::Value> {
        let mut obj = match value {
            serde_json::Value::Object(m) => m,
            other => {
                return Err(SchemaError::Migration {
                    from: 0,
                    to: 1,
                    pointer: "/".into(),
                    message: format!("expected an object at root, got {:?}", other),
                    suggested_fix: "wrap the document in `{...}` with a schema_version field"
                        .into(),
                });
            }
        };

        obj.insert(
            "schema_version".to_string(),
            serde_json::Value::Number(1u32.into()),
        );

        // Migrate the top-level `physics` block if present by relocating it to
        // `runtime_settings`; v0 documents used `physics`, v1 uses
        // `runtime_settings`.
        if let Some(physics) = obj.remove("physics") {
            obj.insert("runtime_settings".to_string(), physics);
        }

        // Record anything unknown under `migration_notes` so the operator can
        // reconcile manually without the migrator destroying information.
        let mut migration_notes = serde_json::Map::new();
        let known: &[&str] = &[
            "schema_version",
            "world_id",
            "display_name",
            "chunks",
            "props",
            "spawn_points",
            "lighting",
            "skybox",
            "runtime_settings",
            "scripts",
            "meta",
        ];
        let keys: Vec<String> = obj.keys().cloned().collect();
        for key in keys {
            if !known.contains(&key.as_str()) {
                if let Some(val) = obj.remove(&key) {
                    migration_notes.insert(key, val);
                }
            }
        }
        if !migration_notes.is_empty() {
            obj.insert(
                "migration_notes".to_string(),
                serde_json::Value::Object(migration_notes),
            );
        }

        Ok(serde_json::Value::Object(obj))
    }
}

/// Apply a chain of migrators until the target version is reached.
///
/// Migrators must be provided in ascending order. The chain stops as soon as
/// the document reports the target version.
pub fn apply_chain(
    migrators: &[&dyn Migrator],
    mut value: serde_json::Value,
    target: SchemaVersion,
) -> SchemaResult<serde_json::Value> {
    let mut current = detect_version(&value)?;
    while current != target {
        let m = migrators
            .iter()
            .find(|m| m.from_version() == current)
            .ok_or_else(|| SchemaError::Migration {
                from: current.as_u32(),
                to: target.as_u32(),
                pointer: "/schema_version".into(),
                message: format!(
                    "no registered migrator from v{} to the next version",
                    current.as_u32()
                ),
                suggested_fix: "register a Migrator for this step".into(),
            })?;
        value = m.migrate(value)?;
        current = m.to_version();
    }
    Ok(value)
}

fn detect_version(value: &serde_json::Value) -> SchemaResult<SchemaVersion> {
    match value.get("schema_version") {
        Some(serde_json::Value::Number(n)) => {
            let v = n.as_u64().ok_or_else(|| SchemaError::Parse {
                pointer: "/schema_version".into(),
                message: "schema_version is not an unsigned integer".into(),
                suggested_fix: "use a non-negative integer such as 1".into(),
            })?;
            SchemaVersion::from_u32(v as u32)
        }
        Some(other) => Err(SchemaError::Parse {
            pointer: "/schema_version".into(),
            message: format!("expected integer, got {:?}", other),
            suggested_fix: "set schema_version to an unsigned integer".into(),
        }),
        None => Ok(SchemaVersion::V0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_round_trip() {
        assert_eq!(SchemaVersion::from_u32(1).unwrap(), SchemaVersion::V1);
        assert_eq!(SchemaVersion::V1.as_u32(), 1);
        assert!(SchemaVersion::from_u32(42).is_err());
    }

    #[test]
    fn v0_to_v1_inserts_schema_version() {
        let v0 = serde_json::json!({"world_id": "w"});
        let v1 = MigratorV0ToV1.migrate(v0).unwrap();
        assert_eq!(v1["schema_version"], serde_json::json!(1));
    }

    #[test]
    fn v0_to_v1_preserves_unknowns_under_migration_notes() {
        let v0 = serde_json::json!({
            "world_id": "w",
            "legacy_flag": true,
        });
        let v1 = MigratorV0ToV1.migrate(v0).unwrap();
        assert_eq!(v1["migration_notes"]["legacy_flag"], serde_json::json!(true));
        assert!(v1.get("legacy_flag").is_none());
    }

    #[test]
    fn v0_physics_moves_to_runtime_settings() {
        let v0 = serde_json::json!({
            "physics": {"gravity": -9.8}
        });
        let v1 = MigratorV0ToV1.migrate(v0).unwrap();
        assert_eq!(v1["runtime_settings"]["gravity"], serde_json::json!(-9.8));
        assert!(v1.get("physics").is_none());
    }

    #[test]
    fn apply_chain_stops_at_target() {
        let v0 = serde_json::json!({"world_id": "w"});
        let out =
            apply_chain(&[&MigratorV0ToV1 as &dyn Migrator], v0, SchemaVersion::V1).unwrap();
        assert_eq!(out["schema_version"], serde_json::json!(1));
    }

    #[test]
    fn apply_chain_no_op_when_already_at_target() {
        let v1 = serde_json::json!({"schema_version": 1, "world_id": "w"});
        let out = apply_chain(&[], v1.clone(), SchemaVersion::V1).unwrap();
        assert_eq!(out, v1);
    }
}
