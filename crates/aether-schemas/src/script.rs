//! Script artifact (task 70).
//!
//! A script artifact carries:
//! 1. The DSL source blob (produced by U07's DSL, in Bet 3).
//! 2. The compiled WASM blob (the runtime-loadable module).
//! 3. A signature proving who compiled it.
//! 4. An explicit capability declaration restricting what the script can do.
//!
//! Both blobs are stored inline as base64 strings when embedded in a manifest;
//! for larger scripts the manifest references `*_cid` content addresses
//! instead. The two forms are mutually exclusive and enforced by [`ScriptArtifact::validate`].

use std::collections::BTreeSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::{SchemaError, SchemaResult};

/// Top-level script artifact.
///
/// Scripts are referenced from [`crate::world_manifest::WorldManifest::scripts`]
/// by stable `id`. Runtime resolves the id to this artifact, verifies the
/// signature, confirms the capability bundle matches the host policy, then
/// instantiates the WASM module.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ScriptArtifact {
    pub id: String,

    /// Human-friendly label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,

    /// Optional DSL source (inline base64) or a reference to a content-addressed
    /// blob holding the source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_inline_base64: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_cid: Option<String>,

    /// Compiled WASM — same inline-vs-cid split as the source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wasm_inline_base64: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wasm_cid: Option<String>,

    /// Signature over `wasm` bytes (see [`ScriptSignature`]).
    pub signature: ScriptSignature,

    /// Capability declaration — must fit within the host's policy.
    pub capabilities: CapabilityDeclaration,

    /// Free-form tags.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

impl ScriptArtifact {
    pub fn validate(&self, pointer_base: &str) -> SchemaResult<()> {
        if self.id.trim().is_empty() {
            return Err(SchemaError::validation(
                format!("{pointer_base}/id"),
                "script id must be non-empty",
                "assign a stable id such as `combat.boss_phase_1`",
            ));
        }

        // At most one source form, at most one wasm form, and at least one of each.
        match (&self.source_inline_base64, &self.source_cid) {
            (Some(_), Some(_)) => {
                return Err(SchemaError::validation(
                    format!("{pointer_base}/source_cid"),
                    "source_inline_base64 and source_cid are mutually exclusive",
                    "keep only one; prefer source_cid for blobs > 4KB",
                ));
            }
            (None, None) => {
                return Err(SchemaError::validation(
                    format!("{pointer_base}/source_cid"),
                    "missing DSL source: set either source_inline_base64 or source_cid",
                    "inline small scripts via source_inline_base64; CID-reference large ones",
                ));
            }
            _ => {}
        }
        match (&self.wasm_inline_base64, &self.wasm_cid) {
            (Some(_), Some(_)) => {
                return Err(SchemaError::validation(
                    format!("{pointer_base}/wasm_cid"),
                    "wasm_inline_base64 and wasm_cid are mutually exclusive",
                    "keep only one; prefer wasm_cid in production",
                ));
            }
            (None, None) => {
                return Err(SchemaError::validation(
                    format!("{pointer_base}/wasm_cid"),
                    "missing compiled WASM: set either wasm_inline_base64 or wasm_cid",
                    "compile the DSL source via U07 and record the output",
                ));
            }
            _ => {}
        }

        self.signature
            .validate(&format!("{pointer_base}/signature"))?;
        self.capabilities
            .validate(&format!("{pointer_base}/capabilities"))?;
        Ok(())
    }
}

/// Ed25519-style signature record. The cryptographic verification is performed
/// by `aether-security`; this schema layer merely ensures the record is
/// structurally well-formed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ScriptSignature {
    /// Identifier of the signing key (human-friendly).
    pub key_id: String,

    /// Base64-encoded public key.
    pub public_key_base64: String,

    /// Base64-encoded signature over the canonical bytes of the WASM module.
    pub signature_base64: String,

    /// Algorithm identifier, e.g., "ed25519".
    #[serde(default = "ScriptSignature::default_algorithm")]
    pub algorithm: String,
}

impl ScriptSignature {
    fn default_algorithm() -> String {
        "ed25519".to_string()
    }

    fn validate(&self, pointer_base: &str) -> SchemaResult<()> {
        if self.key_id.trim().is_empty() {
            return Err(SchemaError::validation(
                format!("{pointer_base}/key_id"),
                "signature.key_id must be non-empty",
                "record the identity of the signer, e.g. `studio.ci`",
            ));
        }
        if self.signature_base64.trim().is_empty() {
            return Err(SchemaError::validation(
                format!("{pointer_base}/signature_base64"),
                "signature must be non-empty",
                "sign the WASM blob with the declared key and paste the base64 signature",
            ));
        }
        if self.algorithm != "ed25519" {
            return Err(SchemaError::validation(
                format!("{pointer_base}/algorithm"),
                format!("unsupported algorithm: {}", self.algorithm),
                "supported algorithms: ed25519",
            ));
        }
        Ok(())
    }
}

/// Explicit capability declaration. The runtime enforces that the script only
/// calls host functions in this bundle.
///
/// This list is deliberately conservative and small; new capabilities should
/// be added via the versioning/migration policy.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CapabilityDeclaration {
    /// Read state from the world (queries, component reads).
    #[serde(default)]
    pub read_world: bool,
    /// Mutate world state (spawn entities, set components).
    #[serde(default)]
    pub write_world: bool,
    /// Spawn/despawn entities by prop id.
    #[serde(default)]
    pub spawn_entities: bool,
    /// Issue network messages to clients.
    #[serde(default)]
    pub network_broadcast: bool,
    /// Access persistent storage (key/value on behalf of the world).
    #[serde(default)]
    pub persistence_read: bool,
    #[serde(default)]
    pub persistence_write: bool,
    /// Free-form custom capabilities — the runtime must recognize each string.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub custom: BTreeSet<String>,
}

impl CapabilityDeclaration {
    /// Return true if this bundle is "safe" — read-only to the world.
    pub fn is_read_only(&self) -> bool {
        !self.write_world
            && !self.spawn_entities
            && !self.network_broadcast
            && !self.persistence_write
    }

    fn validate(&self, pointer_base: &str) -> SchemaResult<()> {
        // A script that writes world state must also read it; enforce this
        // early so the runtime doesn't have to check later.
        if self.write_world && !self.read_world {
            return Err(SchemaError::validation(
                format!("{pointer_base}/read_world"),
                "write_world requires read_world",
                "set read_world: true",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_signature() -> ScriptSignature {
        ScriptSignature {
            key_id: "studio.ci".into(),
            public_key_base64: "AAAA".into(),
            signature_base64: "BBBB".into(),
            algorithm: "ed25519".into(),
        }
    }

    #[test]
    fn script_requires_source_and_wasm() {
        let s = ScriptArtifact {
            id: "s".into(),
            display_name: None,
            source_inline_base64: None,
            source_cid: None,
            wasm_inline_base64: None,
            wasm_cid: None,
            signature: dummy_signature(),
            capabilities: CapabilityDeclaration::default(),
            tags: vec![],
        };
        assert!(s.validate("/scripts/0").is_err());
    }

    #[test]
    fn script_rejects_both_source_forms() {
        let s = ScriptArtifact {
            id: "s".into(),
            display_name: None,
            source_inline_base64: Some("AA".into()),
            source_cid: Some("cid:v1:abc".into()),
            wasm_inline_base64: Some("AA".into()),
            wasm_cid: None,
            signature: dummy_signature(),
            capabilities: CapabilityDeclaration::default(),
            tags: vec![],
        };
        let err = s.validate("/scripts/0").unwrap_err();
        assert_eq!(err.pointer(), "/scripts/0/source_cid");
    }

    #[test]
    fn script_happy_path() {
        let s = ScriptArtifact {
            id: "combat.phase1".into(),
            display_name: Some("Phase 1".into()),
            source_inline_base64: Some("AA==".into()),
            source_cid: None,
            wasm_inline_base64: Some("AA==".into()),
            wasm_cid: None,
            signature: dummy_signature(),
            capabilities: CapabilityDeclaration {
                read_world: true,
                ..Default::default()
            },
            tags: vec![],
        };
        s.validate("/scripts/0").unwrap();
    }

    #[test]
    fn capability_write_requires_read() {
        let c = CapabilityDeclaration {
            read_world: false,
            write_world: true,
            ..Default::default()
        };
        let err = c.validate("/c").unwrap_err();
        assert_eq!(err.pointer(), "/c/read_world");
    }

    #[test]
    fn capability_is_read_only_reports_truthfully() {
        let read_only = CapabilityDeclaration {
            read_world: true,
            ..Default::default()
        };
        assert!(read_only.is_read_only());

        let writer = CapabilityDeclaration {
            read_world: true,
            write_world: true,
            ..Default::default()
        };
        assert!(!writer.is_read_only());
    }

    #[test]
    fn signature_rejects_unsupported_algorithm() {
        let sig = ScriptSignature {
            algorithm: "rsa-pkcs1v15-sha256".into(),
            ..dummy_signature()
        };
        let err = sig.validate("/sig").unwrap_err();
        assert_eq!(err.pointer(), "/sig/algorithm");
    }
}
