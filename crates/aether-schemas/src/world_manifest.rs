//! Top-level WorldManifest document (task 67).
//!
//! The `WorldManifest` is the single declarative artifact an agent or human
//! author produces to describe a world. It names the chunks, props, entities,
//! spawn points, runtime settings, lighting, and script references.
//!
//! This crate does not execute manifests; it only defines their structure,
//! canonical encoding, content-addressed CID, and migrations. Execution lives
//! in `aether-world-runtime`.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::chunk::ChunkManifest;
use crate::content_address::{ContentAddress, SchemaVersioned};
use crate::entity::{Entity, Prop};
use crate::error::{SchemaError, SchemaResult};
use crate::migration::SchemaVersion;
use crate::script::ScriptArtifact;

/// The canonical WorldManifest.
///
/// Structural invariants enforced by [`WorldManifest::validate`]:
/// - `schema_version` equals [`crate::CURRENT_SCHEMA_VERSION`].
/// - `world_id` is non-empty.
/// - Chunk coords are unique per LOD.
/// - Entity ids are unique.
/// - Prop ids are unique.
/// - Spawn point ids are unique.
/// - Script ids are unique.
/// - At most one spawn point has `is_default == true`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorldManifest {
    pub schema_version: SchemaVersion,
    pub world_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,

    /// Authoring metadata — purely informational, never consumed by the runtime.
    #[serde(default)]
    pub meta: WorldManifestMeta,

    /// Runtime settings (gravity, tick rate, etc.).
    #[serde(default)]
    pub runtime_settings: RuntimeSettings,

    /// Lighting + skybox settings.
    #[serde(default)]
    pub lighting: LightingSettings,

    /// Streaming chunks.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub chunks: Vec<ChunkManifest>,

    /// Reusable prop templates.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub props: Vec<Prop>,

    /// Declarative entities spawned at world boot.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entities: Vec<Entity>,

    /// Spawn points.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub spawn_points: Vec<SpawnPoint>,

    /// Script references. Inline scripts are fine for small worlds; in
    /// production we expect these to be `source_cid`/`wasm_cid` references.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scripts: Vec<ScriptArtifact>,

    /// Migration breadcrumbs left by the `Migrator` chain. Readers should log
    /// and ideally surface these to authors.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub migration_notes: BTreeMap<String, serde_json::Value>,
}

impl WorldManifest {
    /// A minimal example used for tests and for schema documentation fixtures.
    pub fn minimal_example() -> Self {
        WorldManifest {
            schema_version: SchemaVersion::V1,
            world_id: "hello.aether".into(),
            display_name: Some("Hello, Aether".into()),
            meta: WorldManifestMeta {
                author: Some("agent:opus".into()),
                created_at_unix: Some(1_700_000_000),
                description: Some(
                    "Minimal agent-authored world used as a canonical fixture.".into(),
                ),
            },
            runtime_settings: RuntimeSettings::default(),
            lighting: LightingSettings::default(),
            chunks: vec![],
            props: vec![],
            entities: vec![],
            spawn_points: vec![SpawnPoint {
                id: "origin".into(),
                position: [0.0, 0.0, 0.0],
                yaw_deg: 0.0,
                is_default: true,
            }],
            scripts: vec![],
            migration_notes: BTreeMap::new(),
        }
    }

    /// Run every structural check. Errors include JSON pointers and a
    /// suggested fix.
    pub fn validate(&self) -> SchemaResult<()> {
        if self.schema_version != crate::CURRENT_SCHEMA_VERSION {
            return Err(SchemaError::UnsupportedVersion {
                found: self.schema_version.as_u32(),
                expected: vec![crate::CURRENT_SCHEMA_VERSION.as_u32()],
                suggested_fix: "run the v0→v1 migrator, or update the document".into(),
            });
        }
        if self.world_id.trim().is_empty() {
            return Err(SchemaError::validation(
                "/world_id",
                "world_id must be non-empty",
                "set `world_id` to a stable dotted path such as `starter.meadow`",
            ));
        }

        self.runtime_settings.validate("/runtime_settings")?;
        self.lighting.validate("/lighting")?;

        // Uniqueness checks.
        assert_unique(
            self.chunks.iter().map(|c| (c.coord, c.lod)),
            "/chunks",
            "chunk (coord, lod) must be unique",
            "remove duplicate chunk manifests or differentiate by LOD",
        )?;
        for (i, c) in self.chunks.iter().enumerate() {
            c.validate(&format!("/chunks/{i}"))?;
        }

        assert_unique(
            self.props.iter().map(|p| p.id.as_str()),
            "/props",
            "prop id must be unique",
            "rename duplicate prop ids",
        )?;
        for (i, p) in self.props.iter().enumerate() {
            p.validate(&format!("/props/{i}"))?;
        }

        assert_unique(
            self.entities.iter().map(|e| e.id.as_str()),
            "/entities",
            "entity id must be unique",
            "rename duplicate entity ids",
        )?;
        for (i, e) in self.entities.iter().enumerate() {
            e.validate(&format!("/entities/{i}"))?;
        }

        assert_unique(
            self.spawn_points.iter().map(|s| s.id.as_str()),
            "/spawn_points",
            "spawn point id must be unique",
            "rename duplicate spawn point ids",
        )?;
        for (i, s) in self.spawn_points.iter().enumerate() {
            s.validate(&format!("/spawn_points/{i}"))?;
        }
        let default_count = self
            .spawn_points
            .iter()
            .filter(|s| s.is_default)
            .count();
        if default_count > 1 {
            return Err(SchemaError::validation(
                "/spawn_points",
                "at most one spawn_point may be marked is_default",
                "leave is_default: true on exactly one spawn point",
            ));
        }

        assert_unique(
            self.scripts.iter().map(|s| s.id.as_str()),
            "/scripts",
            "script id must be unique",
            "rename duplicate script ids",
        )?;
        for (i, s) in self.scripts.iter().enumerate() {
            s.validate(&format!("/scripts/{i}"))?;
        }
        Ok(())
    }
}

impl SchemaVersioned for WorldManifest {
    fn schema_version(&self) -> SchemaVersion {
        self.schema_version
    }
}

impl ContentAddress for WorldManifest {
    fn schema_version(&self) -> SchemaVersion {
        self.schema_version
    }

    fn canonical_bytes(&self) -> SchemaResult<Vec<u8>> {
        crate::to_canonical_bytes(self)
    }
}

/// Informational manifest metadata.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorldManifestMeta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at_unix: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Runtime settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RuntimeSettings {
    /// Gravity in m/s^2. Must be in `[-100, 100]`.
    #[serde(default = "RuntimeSettings::default_gravity")]
    pub gravity: f32,
    /// Simulation tick rate. Must be in `(0, 240]`.
    #[serde(default = "RuntimeSettings::default_tick_rate")]
    pub tick_rate_hz: u32,
    /// Maximum concurrent players in this world shard.
    #[serde(default = "RuntimeSettings::default_max_players")]
    pub max_players: u32,
}

impl Default for RuntimeSettings {
    fn default() -> Self {
        RuntimeSettings {
            gravity: Self::default_gravity(),
            tick_rate_hz: Self::default_tick_rate(),
            max_players: Self::default_max_players(),
        }
    }
}

impl RuntimeSettings {
    fn default_gravity() -> f32 {
        -9.81
    }
    fn default_tick_rate() -> u32 {
        60
    }
    fn default_max_players() -> u32 {
        32
    }

    pub fn validate(&self, pointer_base: &str) -> SchemaResult<()> {
        if !self.gravity.is_finite() || !(-100.0..=100.0).contains(&self.gravity) {
            return Err(SchemaError::validation(
                format!("{pointer_base}/gravity"),
                "gravity must be finite and in [-100, 100]",
                "use -9.81 for Earth-like gravity",
            ));
        }
        if self.tick_rate_hz == 0 || self.tick_rate_hz > 240 {
            return Err(SchemaError::validation(
                format!("{pointer_base}/tick_rate_hz"),
                "tick_rate_hz must be in (0, 240]",
                "use 60 for standard simulations",
            ));
        }
        if self.max_players == 0 {
            return Err(SchemaError::validation(
                format!("{pointer_base}/max_players"),
                "max_players must be > 0",
                "use 32 for a small shard",
            ));
        }
        Ok(())
    }
}

/// Lighting + skybox.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LightingSettings {
    #[serde(default = "LightingSettings::default_sun")]
    pub sun_intensity: f32,
    #[serde(default = "LightingSettings::default_ambient")]
    pub ambient_intensity: f32,
    /// Optional skybox asset CID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skybox_asset_cid: Option<String>,
}

impl Default for LightingSettings {
    fn default() -> Self {
        LightingSettings {
            sun_intensity: Self::default_sun(),
            ambient_intensity: Self::default_ambient(),
            skybox_asset_cid: None,
        }
    }
}

impl LightingSettings {
    fn default_sun() -> f32 {
        1.0
    }
    fn default_ambient() -> f32 {
        0.1
    }

    pub fn validate(&self, pointer_base: &str) -> SchemaResult<()> {
        if !self.sun_intensity.is_finite() || self.sun_intensity < 0.0 {
            return Err(SchemaError::validation(
                format!("{pointer_base}/sun_intensity"),
                "sun_intensity must be finite and non-negative",
                "use 1.0 for daylight",
            ));
        }
        if !self.ambient_intensity.is_finite() || self.ambient_intensity < 0.0 {
            return Err(SchemaError::validation(
                format!("{pointer_base}/ambient_intensity"),
                "ambient_intensity must be finite and non-negative",
                "use 0.1 for outdoor daytime",
            ));
        }
        Ok(())
    }
}

/// Spawn point. Mirrors `aether_world_runtime::SpawnPoint`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SpawnPoint {
    pub id: String,
    pub position: [f32; 3],
    #[serde(default)]
    pub yaw_deg: f32,
    #[serde(default)]
    pub is_default: bool,
}

impl SpawnPoint {
    fn validate(&self, pointer_base: &str) -> SchemaResult<()> {
        if self.id.trim().is_empty() {
            return Err(SchemaError::validation(
                format!("{pointer_base}/id"),
                "spawn_point id must be non-empty",
                "set a stable id such as `town_square`",
            ));
        }
        for (i, v) in self.position.iter().enumerate() {
            if !v.is_finite() {
                return Err(SchemaError::validation(
                    format!("{pointer_base}/position/{i}"),
                    "position must be finite",
                    "replace NaN/Inf with a finite value",
                ));
            }
        }
        if !self.yaw_deg.is_finite() {
            return Err(SchemaError::validation(
                format!("{pointer_base}/yaw_deg"),
                "yaw_deg must be finite",
                "use a value in degrees, e.g. 0.0",
            ));
        }
        Ok(())
    }
}

fn assert_unique<I, K>(
    items: I,
    pointer: &str,
    message: &str,
    suggested_fix: &str,
) -> SchemaResult<()>
where
    I: IntoIterator<Item = K>,
    K: Eq + std::hash::Hash,
{
    let mut seen = std::collections::HashSet::new();
    for item in items {
        if !seen.insert(item) {
            return Err(SchemaError::validation(pointer, message, suggested_fix));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::{ChunkCoord, ChunkKind, LodLevel};

    #[test]
    fn minimal_example_validates() {
        let m = WorldManifest::minimal_example();
        m.validate().unwrap();
    }

    #[test]
    fn yaml_roundtrip() {
        let m = WorldManifest::minimal_example();
        let y = serde_yaml::to_string(&m).unwrap();
        let back: WorldManifest = serde_yaml::from_str(&y).unwrap();
        assert_eq!(m, back);
    }

    #[test]
    fn duplicate_chunk_coord_rejected() {
        let mut m = WorldManifest::minimal_example();
        let c = ChunkManifest {
            coord: ChunkCoord::new(0, 0, 0),
            kind: ChunkKind::Terrain,
            lod: LodLevel(0),
            asset_cid: "cid:v1:abc".into(),
            size_bytes: 1,
            prefetch_distance_meters: 0.0,
            tags: vec![],
        };
        m.chunks = vec![c.clone(), c];
        let err = m.validate().unwrap_err();
        assert_eq!(err.pointer(), "/chunks");
    }

    #[test]
    fn multiple_defaults_rejected() {
        let mut m = WorldManifest::minimal_example();
        m.spawn_points.push(SpawnPoint {
            id: "other".into(),
            position: [1.0, 0.0, 0.0],
            yaw_deg: 0.0,
            is_default: true,
        });
        let err = m.validate().unwrap_err();
        assert_eq!(err.pointer(), "/spawn_points");
    }

    #[test]
    fn runtime_settings_reject_zero_tick_rate() {
        let mut m = WorldManifest::minimal_example();
        m.runtime_settings.tick_rate_hz = 0;
        let err = m.validate().unwrap_err();
        assert_eq!(err.pointer(), "/runtime_settings/tick_rate_hz");
    }

    #[test]
    fn content_address_is_stable() {
        let m = WorldManifest::minimal_example();
        let cid1 = m.cid().unwrap();
        let cid2 = m.cid().unwrap();
        assert_eq!(cid1, cid2);
    }
}
