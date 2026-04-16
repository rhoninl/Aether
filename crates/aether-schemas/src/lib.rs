//! # aether-schemas
//!
//! Canonical, declarative, content-addressed schema layer for every artifact
//! the Aether engine consumes.
//!
//! ## Design tenets
//!
//! 1. **Declarative is the source of truth.** Every artifact must be expressible
//!    in the canonical on-disk form (YAML for humans, CBOR for machines). Rust
//!    in-memory types are a mirror, not an authority.
//! 2. **Deterministic bytes.** Binary serialization uses CBOR with lexicographically
//!    sorted map keys. Given the same struct, two processes on two machines
//!    produce byte-identical output.
//! 3. **Content addressing.** Every top-level artifact implements
//!    [`content_address::ContentAddress`]. The content ID (CID) is the SHA-256
//!    of the deterministic CBOR encoding, prefixed with the schema version.
//! 4. **Agent-first errors.** Parse errors carry a JSON pointer to the offending
//!    field and a human-actionable `suggested_fix`, because the primary author
//!    of these documents is an AI agent.
//! 5. **Schema versioning is explicit.** Every document carries a
//!    `schema_version` field, and [`migration::Migrator`] codifies the upgrade
//!    path from any past version.
//!
//! ## Module map
//!
//! | Module | Purpose |
//! | --- | --- |
//! | [`world_manifest`] | Top-level world document (task 67). |
//! | [`entity`] | Entities, props, components (task 68). |
//! | [`chunk`] | Streaming unit aligned with `aether-world-runtime` (task 69). |
//! | [`script`] | DSL source + WASM + signature + capabilities (task 70). |
//! | [`quest`] | MMORPG primitives behind the `mmorpg` feature (task 71). |
//! | [`content_address`] | `Cid`, `ContentAddress`, SHA-256 hasher (task 72). |
//! | [`migration`] | `Migrator` trait + v0→v1 migration (task 73). |
//! | [`error`] | `SchemaError` with JSON pointer + suggested fix. |
//! | [`emit`] | JSON Schema emission for agent training (task 75). |
//!
//! ## Deprecation policy
//!
//! A `schema_version` is never reused. When a field is removed, it enters a
//! *deprecated* phase where readers accept it but writers omit it; after one
//! minor release cycle it is removed from the Rust type and the migration
//! becomes lossy. The `Migrator` trait guarantees forward migrations; backward
//! migrations are opt-in and may be lossy.
//!
//! ## Stability
//!
//! `schema_version: 1` is the first stable wire. v0 documents are accepted via
//! [`migration::MigratorV0ToV1`] as a best-effort bridge.

pub mod chunk;
pub mod content_address;
pub mod emit;
pub mod entity;
pub mod error;
pub mod migration;
#[cfg(feature = "mmorpg")]
pub mod quest;
pub mod script;
pub mod world_manifest;

pub use chunk::{ChunkCoord, ChunkManifest, LodLevel};
pub use content_address::{Cid, ContentAddress, SchemaVersioned};
pub use emit::{emit_all_schemas, schema_for};
pub use entity::{Component, ComponentValue, Entity, EntityKind, Prop, Transform};
pub use error::{SchemaError, SchemaResult};
pub use migration::{Migrator, MigratorV0ToV1, SchemaVersion};
pub use script::{CapabilityDeclaration, ScriptArtifact, ScriptSignature};
pub use world_manifest::{
    LightingSettings, RuntimeSettings, SpawnPoint, WorldManifest, WorldManifestMeta,
};

/// Current canonical schema version for all top-level artifacts.
pub const CURRENT_SCHEMA_VERSION: SchemaVersion = SchemaVersion::V1;

/// Canonical binary serialization (CBOR with sorted map keys).
///
/// Given any `T: serde::Serialize` whose serialization surface is stable
/// (structs with `#[serde(deny_unknown_fields)]`, no float NaNs, canonical
/// numeric types), this produces a byte-for-byte reproducible encoding.
///
/// This is the single entry point every artifact must use for on-disk binary
/// form and for content-address hashing.
pub fn to_canonical_bytes<T: serde::Serialize + ?Sized>(value: &T) -> SchemaResult<Vec<u8>> {
    // ciborium's `into_writer` emits CBOR map keys in insertion order; we
    // therefore serialize first via serde_json::Value (which we then sort) then
    // re-encode as CBOR. This is cheap compared to the dominant I/O cost and
    // avoids us rolling a custom CBOR encoder.
    let json = serde_json::to_value(value).map_err(|e| SchemaError::Serialize {
        pointer: String::from("/"),
        message: e.to_string(),
        suggested_fix: String::from(
            "ensure the value implements serde::Serialize and contains no non-finite floats",
        ),
    })?;
    let sorted = sort_json_value(json);
    let mut out = Vec::with_capacity(256);
    ciborium::ser::into_writer(&sorted, &mut out).map_err(|e| SchemaError::Serialize {
        pointer: String::from("/"),
        message: e.to_string(),
        suggested_fix: String::from("check that the value has no unsupported CBOR types"),
    })?;
    Ok(out)
}

/// Inverse of [`to_canonical_bytes`].
pub fn from_canonical_bytes<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> SchemaResult<T> {
    let value: serde_json::Value =
        ciborium::de::from_reader(bytes).map_err(|e| SchemaError::Parse {
            pointer: String::from("/"),
            message: e.to_string(),
            suggested_fix: String::from(
                "the byte stream is not valid CBOR; verify it was produced by to_canonical_bytes",
            ),
        })?;
    serde_json::from_value(value).map_err(|e| SchemaError::Parse {
        pointer: String::from("/"),
        message: e.to_string(),
        suggested_fix: String::from(
            "decoded CBOR did not match the requested Rust type; check schema_version",
        ),
    })
}

/// Recursively sort the keys of every JSON object in `value` so that CBOR
/// encoding is deterministic.
fn sort_json_value(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let mut sorted: Vec<(String, serde_json::Value)> = map.into_iter().collect();
            sorted.sort_by(|a, b| a.0.cmp(&b.0));
            let mut out = serde_json::Map::with_capacity(sorted.len());
            for (k, v) in sorted {
                out.insert(k, sort_json_value(v));
            }
            serde_json::Value::Object(out)
        }
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.into_iter().map(sort_json_value).collect())
        }
        other => other,
    }
}

/// Parse a canonical YAML document into any schema type.
pub fn from_yaml_str<T: serde::de::DeserializeOwned>(yaml: &str) -> SchemaResult<T> {
    serde_yaml::from_str(yaml).map_err(|e| SchemaError::Parse {
        pointer: yaml_error_pointer(&e),
        message: e.to_string(),
        suggested_fix: String::from(
            "fix the YAML syntax or field types; see the `schema_version` field and consult the JSON Schema under docs/schemas/",
        ),
    })
}

/// Emit any schema type as canonical YAML.
pub fn to_yaml_string<T: serde::Serialize + ?Sized>(value: &T) -> SchemaResult<String> {
    serde_yaml::to_string(value).map_err(|e| SchemaError::Serialize {
        pointer: String::from("/"),
        message: e.to_string(),
        suggested_fix: String::from("values must be serde::Serialize-able to YAML"),
    })
}

fn yaml_error_pointer(err: &serde_yaml::Error) -> String {
    match err.location() {
        Some(loc) => format!("/yaml/line:{}/col:{}", loc.line(), loc.column()),
        None => String::from("/"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_bytes_are_stable_across_calls() {
        let manifest = WorldManifest::minimal_example();
        let a = to_canonical_bytes(&manifest).unwrap();
        let b = to_canonical_bytes(&manifest).unwrap();
        assert_eq!(a, b, "canonical encoding must be deterministic");
    }

    #[test]
    fn canonical_bytes_roundtrip() {
        let manifest = WorldManifest::minimal_example();
        let bytes = to_canonical_bytes(&manifest).unwrap();
        let back: WorldManifest = from_canonical_bytes(&bytes).unwrap();
        assert_eq!(manifest, back);
    }

    #[test]
    fn yaml_roundtrip_preserves_bytes() {
        let manifest = WorldManifest::minimal_example();
        let yaml = to_yaml_string(&manifest).unwrap();
        let parsed: WorldManifest = from_yaml_str(&yaml).unwrap();
        let bytes_a = to_canonical_bytes(&manifest).unwrap();
        let bytes_b = to_canonical_bytes(&parsed).unwrap();
        assert_eq!(
            bytes_a, bytes_b,
            "YAML round-trip must preserve canonical bytes"
        );
    }

    #[test]
    fn sort_json_value_sorts_nested_maps() {
        let v: serde_json::Value = serde_json::json!({
            "z": {"b": 1, "a": 2},
            "a": [{"y": 1, "x": 2}],
        });
        let sorted = sort_json_value(v);
        let serialized = serde_json::to_string(&sorted).unwrap();
        // "a" appears before "z", and nested keys are also sorted.
        assert!(serialized.starts_with("{\"a\":[{\"x\":2,\"y\":1}]"));
    }
}
