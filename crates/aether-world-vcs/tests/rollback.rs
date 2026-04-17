//! Revert / rollback behaviour.

use aether_world_vcs::{
    cid_of, revert, rollback, AgentRef, BranchStore, Cid, Diff, MemoryBranchStore, Op,
    DEFAULT_BRANCH,
};

fn agent(s: &str) -> AgentRef {
    AgentRef::Agent {
        service_account: s.into(),
    }
}

fn make_diff() -> Diff {
    Diff {
        base: [0u8; 32],
        target: {
            let mut t = [0u8; 32];
            t[0] = 1;
            t
        },
        ops: vec![
            Op::AddEntity {
                entity: 10,
                payload: b"payload".to_vec(),
            },
            Op::ModifyComponent {
                entity: 10,
                component_name: "Hp".into(),
                value: b"100".to_vec(),
                prior_value: Some(b"50".to_vec()),
            },
            Op::AddChunk {
                chunk: 3,
                payload: b"chunk".to_vec(),
            },
        ],
        author: agent("agent.alpha"),
        timestamp_unix_ms: 1,
    }
}

#[test]
fn revert_swaps_base_and_target_and_inverts_ops() {
    let d = make_diff();
    let inv = revert(&d, agent("agent.alpha"), 2).expect("revert ok");
    assert_eq!(inv.base, d.target);
    assert_eq!(inv.target, d.base);
    assert_eq!(inv.ops.len(), d.ops.len());
    // First op in the inverse is the last op's inverse: AddChunk -> RemoveChunk.
    assert!(matches!(inv.ops[0], Op::RemoveChunk { chunk: 3, .. }));
    // Last op in the inverse is the first op's inverse: AddEntity -> RemoveEntity.
    assert!(matches!(
        inv.ops[inv.ops.len() - 1],
        Op::RemoveEntity { entity: 10, .. }
    ));
}

#[test]
fn revert_modify_component_uses_prior_value() {
    let d = make_diff();
    let inv = revert(&d, agent("agent.alpha"), 2).expect("revert ok");
    let middle = &inv.ops[1];
    match middle {
        Op::ModifyComponent {
            entity,
            component_name,
            value,
            prior_value,
        } => {
            assert_eq!(*entity, 10);
            assert_eq!(component_name, "Hp");
            assert_eq!(value.as_slice(), b"50");
            assert_eq!(prior_value.as_deref(), Some(b"100".as_slice()));
        }
        other => panic!("unexpected op: {other:?}"),
    }
}

#[test]
fn revert_fails_without_prior_value() {
    // An AddEntity op is always reversible. A bare ModifyComponent
    // without prior_value is not.
    let d = Diff {
        base: [0u8; 32],
        target: [0u8; 32],
        ops: vec![Op::ModifyComponent {
            entity: 1,
            component_name: "X".into(),
            value: b"a".to_vec(),
            prior_value: None,
        }],
        author: agent("agent.alpha"),
        timestamp_unix_ms: 1,
    };
    assert!(revert(&d, agent("agent.alpha"), 2).is_err());
}

#[test]
fn rollback_rejects_out_of_lineage_target() {
    let mut store = MemoryBranchStore::with_default_main(agent("agent.alpha"));
    let cid_a: Cid = cid_of(&make_diff()).unwrap();
    store.set_head(DEFAULT_BRANCH, cid_a).unwrap();
    // cid_b is not in the branch's ancestry.
    let mut d2 = make_diff();
    d2.timestamp_unix_ms = 999;
    let cid_b: Cid = cid_of(&d2).unwrap();
    let err = rollback(&mut store, DEFAULT_BRANCH, cid_b).unwrap_err();
    assert!(matches!(
        err,
        aether_world_vcs::VcsError::RollbackOutOfLineage { .. }
    ));
}

#[test]
fn rollback_moves_head_to_valid_ancestor() {
    let mut store = MemoryBranchStore::with_default_main(agent("agent.alpha"));
    let zero: Cid = [0u8; 32];
    let cid_a: Cid = cid_of(&make_diff()).unwrap();
    // Record ancestry: cid_a's parent is zero (main's initial head).
    store.record_ancestry(cid_a, zero);
    store.set_head(DEFAULT_BRANCH, cid_a).unwrap();
    // Roll back from cid_a to zero.
    rollback(&mut store, DEFAULT_BRANCH, zero).expect("rollback ok");
    assert_eq!(store.head(DEFAULT_BRANCH).unwrap(), zero);
}
