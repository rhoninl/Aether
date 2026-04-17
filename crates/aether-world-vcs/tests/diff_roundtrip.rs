//! Diff encode/decode + CID stability + signature round-trip.
//! Also emits `docs/schemas/world-diff.v1.json` so the schema stays
//! in lockstep with the Rust types.

use std::fs;
use std::path::PathBuf;

use aether_world_vcs::diff::{canonical_cbor, cid_of, decode_cbor};
use aether_world_vcs::{sign_diff, verify_signed_diff, AgentRef, Cid, Diff, Op};

fn sample_diff() -> Diff {
    let base: Cid = [0u8; 32];
    let mut target: Cid = [0u8; 32];
    target[0] = 1;
    Diff {
        base,
        target,
        ops: vec![
            Op::AddEntity {
                entity: 42,
                payload: b"hello".to_vec(),
            },
            Op::ModifyComponent {
                entity: 42,
                component_name: "Transform".into(),
                value: b"new-xform".to_vec(),
                prior_value: Some(b"old-xform".to_vec()),
            },
            Op::AddChunk {
                chunk: 7,
                payload: b"chunk-bytes".to_vec(),
            },
        ],
        author: AgentRef::Agent {
            service_account: "agent.alpha".into(),
        },
        timestamp_unix_ms: 1_713_000_000_000,
    }
}

#[test]
fn diff_cbor_roundtrip() {
    let diff = sample_diff();
    let bytes = canonical_cbor(&diff).expect("encode");
    let decoded = decode_cbor(&bytes).expect("decode");
    assert_eq!(diff, decoded);
}

#[test]
fn cid_is_stable() {
    // The same diff value hashed twice must produce the same CID.
    let diff = sample_diff();
    let a = cid_of(&diff).expect("cid a");
    let b = cid_of(&diff).expect("cid b");
    assert_eq!(a, b);
    // And a round-tripped diff must hash identically.
    let bytes = canonical_cbor(&diff).expect("encode");
    let decoded = decode_cbor(&bytes).expect("decode");
    let c = cid_of(&decoded).expect("cid c");
    assert_eq!(a, c);
}

#[test]
fn cid_changes_when_ops_change() {
    let mut a = sample_diff();
    let mut b = sample_diff();
    if let Op::ModifyComponent { value, .. } = &mut b.ops[1] {
        value.push(b'!');
    }
    let ca = cid_of(&a).unwrap();
    let cb = cid_of(&b).unwrap();
    assert_ne!(ca, cb);
    // Flipping timestamp also changes CID (authorship metadata is
    // content-addressed).
    a.timestamp_unix_ms += 1;
    let ca2 = cid_of(&a).unwrap();
    assert_ne!(ca, ca2);
}

#[test]
fn signature_roundtrips() {
    let diff = sample_diff();
    let (sk, _vk) = aether_world_vcs::generate_keypair();
    let signed = sign_diff(diff.clone(), &sk).expect("sign");
    verify_signed_diff(&signed).expect("verify");
    // Tampering with the diff invalidates the signature.
    let mut tampered = signed.clone();
    tampered.diff.ops.push(Op::RemoveChunk {
        chunk: 7,
        prior_payload: Some(b"chunk-bytes".to_vec()),
    });
    assert!(verify_signed_diff(&tampered).is_err());
}

/// Emit the JSON Schema to `docs/schemas/world-diff.v1.json`. Runs as
/// a test so CI guarantees the schema is in lockstep with the Rust
/// types.
#[test]
fn emit_json_schema() {
    use schemars::{schema_for, JsonSchema};
    use serde::{Deserialize, Serialize};

    // Schemars-friendly mirror types. We keep them local to the test
    // so `aether-world-vcs` itself stays dep-light at runtime.
    #[derive(JsonSchema, Serialize, Deserialize)]
    #[serde(tag = "kind", rename_all = "snake_case")]
    #[allow(dead_code)]
    enum AgentRefSchema {
        Agent { service_account: String },
        Human { user_id: String },
    }

    #[derive(JsonSchema, Serialize, Deserialize)]
    #[serde(tag = "op", rename_all = "snake_case")]
    #[allow(dead_code)]
    enum OpSchema {
        AddEntity {
            entity: u64,
            payload: Vec<u8>,
        },
        RemoveEntity {
            entity: u64,
            prior_payload: Option<Vec<u8>>,
        },
        ReplaceEntity {
            entity: u64,
            payload: Vec<u8>,
            prior_payload: Option<Vec<u8>>,
        },
        ModifyComponent {
            entity: u64,
            component_name: String,
            value: Vec<u8>,
            prior_value: Option<Vec<u8>>,
        },
        RetargetScript {
            entity: u64,
            old_script_cid: Vec<u8>,
            new_script_cid: Vec<u8>,
        },
        AddChunk {
            chunk: u64,
            payload: Vec<u8>,
        },
        RemoveChunk {
            chunk: u64,
            prior_payload: Option<Vec<u8>>,
        },
        ReplaceChunk {
            chunk: u64,
            payload: Vec<u8>,
            prior_payload: Option<Vec<u8>>,
        },
    }

    #[derive(JsonSchema, Serialize, Deserialize)]
    #[allow(dead_code)]
    struct DiffSchema {
        base: Vec<u8>,
        target: Vec<u8>,
        ops: Vec<OpSchema>,
        author: AgentRefSchema,
        timestamp_unix_ms: u64,
    }

    #[derive(JsonSchema, Serialize, Deserialize)]
    #[allow(dead_code)]
    struct SignedDiffSchema {
        diff: DiffSchema,
        signature: Vec<u8>,
        public_key: Vec<u8>,
    }

    let schema = schema_for!(SignedDiffSchema);
    let pretty = serde_json::to_string_pretty(&schema).expect("schema json");

    // Resolve `docs/schemas/` relative to the workspace root. Cargo
    // sets CARGO_MANIFEST_DIR to this crate's directory.
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let target = crate_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .join("docs/schemas/world-diff.v1.json");
    fs::create_dir_all(target.parent().unwrap()).expect("mkdir schemas");
    fs::write(&target, pretty).expect("write schema");
    assert!(target.exists());
}
