//! JSON Schema emission for agent training (task 75).
//!
//! Every canonical type that an agent needs to author is emitted as a JSON
//! Schema document under `docs/schemas/` at the workspace root. Agents can
//! be conditioned on these schemas to produce structurally-valid documents
//! on the first attempt.
//!
//! The emit routine is invoked from `tests/roundtrip.rs` so that running
//! `cargo test -p aether-schemas` regenerates the fixture files. This means
//! the committed schemas are always in sync with the Rust source.

use std::path::{Path, PathBuf};

use schemars::{schema_for, JsonSchema};

use crate::chunk::ChunkManifest;
use crate::entity::{Entity, Prop};
use crate::error::{SchemaError, SchemaResult};
use crate::script::ScriptArtifact;
use crate::world_manifest::WorldManifest;

/// Return a pretty-printed JSON Schema for type `T`.
pub fn schema_for<T: JsonSchema>() -> SchemaResult<String> {
    let schema = schema_for!(T);
    serde_json::to_string_pretty(&schema).map_err(|e| SchemaError::Serialize {
        pointer: "/".into(),
        message: e.to_string(),
        suggested_fix: "check schemars derive on the type".into(),
    })
}

/// Descriptor for a schema file to emit.
struct SchemaEntry {
    filename: &'static str,
    body: fn() -> SchemaResult<String>,
}

fn world_manifest_schema() -> SchemaResult<String> {
    schema_for::<WorldManifest>()
}
fn entity_schema() -> SchemaResult<String> {
    schema_for::<Entity>()
}
fn prop_schema() -> SchemaResult<String> {
    schema_for::<Prop>()
}
fn chunk_schema() -> SchemaResult<String> {
    schema_for::<ChunkManifest>()
}
fn script_schema() -> SchemaResult<String> {
    schema_for::<ScriptArtifact>()
}

const ENTRIES: &[SchemaEntry] = &[
    SchemaEntry {
        filename: "world-manifest.v1.json",
        body: world_manifest_schema,
    },
    SchemaEntry {
        filename: "entity.v1.json",
        body: entity_schema,
    },
    SchemaEntry {
        filename: "prop.v1.json",
        body: prop_schema,
    },
    SchemaEntry {
        filename: "chunk-manifest.v1.json",
        body: chunk_schema,
    },
    SchemaEntry {
        filename: "script-artifact.v1.json",
        body: script_schema,
    },
];

/// Write every declared JSON Schema into `dir`. Existing files are overwritten.
///
/// Returns the list of emitted file paths in the order they were written.
pub fn emit_all_schemas(dir: &Path) -> SchemaResult<Vec<PathBuf>> {
    std::fs::create_dir_all(dir).map_err(|e| SchemaError::Serialize {
        pointer: "/".into(),
        message: format!("failed to create {}: {}", dir.display(), e),
        suggested_fix: "verify write permissions on the schemas directory".into(),
    })?;
    let mut out = Vec::with_capacity(ENTRIES.len());
    for entry in ENTRIES {
        let path = dir.join(entry.filename);
        let body = (entry.body)()?;
        std::fs::write(&path, body.as_bytes()).map_err(|e| SchemaError::Serialize {
            pointer: "/".into(),
            message: format!("failed to write {}: {}", path.display(), e),
            suggested_fix: "verify write permissions on the target path".into(),
        })?;
        out.push(path);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_entry_emits_valid_json() {
        for entry in ENTRIES {
            let body = (entry.body)().unwrap();
            let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
            assert!(parsed.is_object(), "schema for {} must be an object", entry.filename);
        }
    }

    #[test]
    fn emit_all_schemas_writes_files() {
        let dir = tempdir();
        let paths = emit_all_schemas(&dir).unwrap();
        assert_eq!(paths.len(), ENTRIES.len());
        for p in paths {
            let body = std::fs::read_to_string(&p).unwrap();
            assert!(body.trim_start().starts_with('{'));
        }
    }

    fn tempdir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "aether-schemas-emit-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
}
