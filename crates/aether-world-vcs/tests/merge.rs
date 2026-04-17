//! Merge algorithm scenarios.

use aether_world_vcs::{merge, AgentRef, Cid, Diff, MergeOutcome, Op};

fn agent(s: &str) -> AgentRef {
    AgentRef::Agent {
        service_account: s.into(),
    }
}

fn diff_with(ops: Vec<Op>, base: u8, target: u8) -> Diff {
    let mut b: Cid = [0u8; 32];
    let mut t: Cid = [0u8; 32];
    b[0] = base;
    t[0] = target;
    Diff {
        base: b,
        target: t,
        ops,
        author: agent("agent.alpha"),
        timestamp_unix_ms: 0,
    }
}

fn empty_ancestor() -> Diff {
    diff_with(vec![], 0, 0)
}

#[test]
fn clean_merge_non_overlapping_ops() {
    let ancestor = empty_ancestor();
    let a = diff_with(
        vec![Op::AddEntity {
            entity: 1,
            payload: b"left".to_vec(),
        }],
        0,
        1,
    );
    let b = diff_with(
        vec![Op::AddEntity {
            entity: 2,
            payload: b"right".to_vec(),
        }],
        0,
        2,
    );
    let out = merge(&ancestor, &a, &b);
    match out {
        MergeOutcome::Clean { merged_ops } => {
            assert_eq!(merged_ops.len(), 2);
        }
        _ => panic!("expected clean merge, got {out:?}"),
    }
}

#[test]
fn identical_ops_collapse() {
    let ancestor = empty_ancestor();
    let shared = Op::ModifyComponent {
        entity: 5,
        component_name: "Tag".into(),
        value: b"same".to_vec(),
        prior_value: Some(b"old".to_vec()),
    };
    let a = diff_with(vec![shared.clone()], 0, 1);
    let b = diff_with(vec![shared], 0, 2);
    let out = merge(&ancestor, &a, &b);
    match out {
        MergeOutcome::Clean { merged_ops } => {
            assert_eq!(merged_ops.len(), 1, "identical ops must collapse");
        }
        _ => panic!("expected clean merge"),
    }
}

#[test]
fn conflicting_component_modification() {
    let ancestor = empty_ancestor();
    let a = diff_with(
        vec![Op::ModifyComponent {
            entity: 5,
            component_name: "Transform".into(),
            value: b"left".to_vec(),
            prior_value: None,
        }],
        0,
        1,
    );
    let b = diff_with(
        vec![Op::ModifyComponent {
            entity: 5,
            component_name: "Transform".into(),
            value: b"right".to_vec(),
            prior_value: None,
        }],
        0,
        2,
    );
    let out = merge(&ancestor, &a, &b);
    match out {
        MergeOutcome::Conflicted { report, .. } => {
            assert_eq!(report.conflicts.len(), 1);
            assert_eq!(report.conflicts[0].subject, "component:5:Transform");
        }
        _ => panic!("expected conflict, got {out:?}"),
    }
}

#[test]
fn conflicting_retarget_script() {
    let ancestor = empty_ancestor();
    let old_cid: Cid = [9u8; 32];
    let mut new_a: Cid = [0u8; 32];
    new_a[0] = 1;
    let mut new_b: Cid = [0u8; 32];
    new_b[0] = 2;
    let a = diff_with(
        vec![Op::RetargetScript {
            entity: 11,
            old_script_cid: old_cid,
            new_script_cid: new_a,
        }],
        0,
        1,
    );
    let b = diff_with(
        vec![Op::RetargetScript {
            entity: 11,
            old_script_cid: old_cid,
            new_script_cid: new_b,
        }],
        0,
        2,
    );
    let out = merge(&ancestor, &a, &b);
    match out {
        MergeOutcome::Conflicted { report, .. } => {
            assert_eq!(report.conflicts.len(), 1);
            assert_eq!(report.conflicts[0].subject, "script:11");
            assert!(report.conflicts[0].hint.contains("script"));
        }
        _ => panic!("expected conflict"),
    }
}

#[test]
fn conflicting_add_remove() {
    let ancestor = empty_ancestor();
    let a = diff_with(
        vec![Op::AddEntity {
            entity: 99,
            payload: b"fresh".to_vec(),
        }],
        0,
        1,
    );
    let b = diff_with(
        vec![Op::RemoveEntity {
            entity: 99,
            prior_payload: Some(b"old".to_vec()),
        }],
        0,
        2,
    );
    let out = merge(&ancestor, &a, &b);
    match out {
        MergeOutcome::Conflicted { report, .. } => {
            assert_eq!(report.conflicts.len(), 1);
            assert_eq!(report.conflicts[0].subject, "entity:99");
            assert!(report.conflicts[0].hint.contains("add/remove"));
        }
        _ => panic!("expected conflict"),
    }
}

#[test]
fn mixed_clean_and_conflict() {
    let ancestor = empty_ancestor();
    let a = diff_with(
        vec![
            Op::AddEntity {
                entity: 1,
                payload: b"a-entity".to_vec(),
            },
            Op::ModifyComponent {
                entity: 5,
                component_name: "Tag".into(),
                value: b"left".to_vec(),
                prior_value: None,
            },
        ],
        0,
        1,
    );
    let b = diff_with(
        vec![
            Op::AddChunk {
                chunk: 2,
                payload: b"b-chunk".to_vec(),
            },
            Op::ModifyComponent {
                entity: 5,
                component_name: "Tag".into(),
                value: b"right".to_vec(),
                prior_value: None,
            },
        ],
        0,
        2,
    );
    let out = merge(&ancestor, &a, &b);
    match out {
        MergeOutcome::Conflicted { clean_ops, report } => {
            assert_eq!(report.conflicts.len(), 1);
            // AddEntity(1) and AddChunk(2) both land cleanly.
            assert_eq!(clean_ops.len(), 2);
        }
        _ => panic!("expected conflict with clean tail"),
    }
}
