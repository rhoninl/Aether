//! Rollback and revert.
//!
//! - [`revert`] takes a [`Diff`] and returns an inverse diff that
//!   undoes each of its ops. `ModifyComponent`, `RemoveEntity`,
//!   `ReplaceEntity`, `RemoveChunk`, and `ReplaceChunk` require
//!   `prior_*` snapshots to be reversible; if a snapshot is missing
//!   we raise [`VcsError::InverseMissingPriorValue`].
//! - [`rollback`] resets a branch's head to an earlier CID. History
//!   is *not* rewritten — a synthetic rollback diff is manufactured
//!   and becomes the new head's ancestor, so downstream federates can
//!   audit the reset.

use crate::branch::BranchStore;
use crate::diff::{AgentRef, Cid, Diff, Op};
use crate::error::{Result, VcsError};

/// Build the inverse of a single op. Returns the inverted op or an
/// error if the op did not carry enough information to reverse.
fn invert_op(op: &Op) -> Result<Op> {
    Ok(match op {
        Op::AddEntity { entity, payload } => Op::RemoveEntity {
            entity: *entity,
            prior_payload: Some(payload.clone()),
        },
        Op::RemoveEntity {
            entity,
            prior_payload,
        } => {
            let payload = prior_payload
                .clone()
                .ok_or(VcsError::InverseMissingPriorValue)?;
            Op::AddEntity {
                entity: *entity,
                payload,
            }
        }
        Op::ReplaceEntity {
            entity,
            payload,
            prior_payload,
        } => {
            let prior = prior_payload
                .clone()
                .ok_or(VcsError::InverseMissingPriorValue)?;
            Op::ReplaceEntity {
                entity: *entity,
                payload: prior,
                prior_payload: Some(payload.clone()),
            }
        }
        Op::ModifyComponent {
            entity,
            component_name,
            value,
            prior_value,
        } => {
            let prior = prior_value
                .clone()
                .ok_or(VcsError::InverseMissingPriorValue)?;
            Op::ModifyComponent {
                entity: *entity,
                component_name: component_name.clone(),
                value: prior,
                prior_value: Some(value.clone()),
            }
        }
        Op::RetargetScript {
            entity,
            old_script_cid,
            new_script_cid,
        } => Op::RetargetScript {
            entity: *entity,
            old_script_cid: *new_script_cid,
            new_script_cid: *old_script_cid,
        },
        Op::AddChunk { chunk, payload } => Op::RemoveChunk {
            chunk: *chunk,
            prior_payload: Some(payload.clone()),
        },
        Op::RemoveChunk {
            chunk,
            prior_payload,
        } => {
            let payload = prior_payload
                .clone()
                .ok_or(VcsError::InverseMissingPriorValue)?;
            Op::AddChunk {
                chunk: *chunk,
                payload,
            }
        }
        Op::ReplaceChunk {
            chunk,
            payload,
            prior_payload,
        } => {
            let prior = prior_payload
                .clone()
                .ok_or(VcsError::InverseMissingPriorValue)?;
            Op::ReplaceChunk {
                chunk: *chunk,
                payload: prior,
                prior_payload: Some(payload.clone()),
            }
        }
    })
}

/// Produce the inverse of an entire diff. The inverse's ops are the
/// per-op inverses in *reverse* order (so that if `diff` applied
/// cleanly, `revert(diff)` applied to the post-state returns to the
/// pre-state).
///
/// `base` and `target` are swapped. The `author` is carried forward
/// from the caller-supplied `authored_by`; the revert is itself a new
/// authored change.
pub fn revert(diff: &Diff, authored_by: AgentRef, timestamp_unix_ms: u64) -> Result<Diff> {
    let mut inverted: Vec<Op> = Vec::with_capacity(diff.ops.len());
    for op in diff.ops.iter().rev() {
        inverted.push(invert_op(op)?);
    }
    Ok(Diff {
        base: diff.target,
        target: diff.base,
        ops: inverted,
        author: authored_by,
        timestamp_unix_ms,
    })
}

/// Reset `branch`'s head to `to_cid` without rewriting history.
///
/// The target CID must be in the branch's ancestor set (otherwise we
/// refuse — callers should create a new branch instead). On success
/// the branch's head becomes `to_cid`; the `BranchStore`
/// implementation is responsible for any ancestry bookkeeping the
/// caller needs.
///
/// This function intentionally does not fabricate a synthetic diff —
/// the caller chooses whether to author an explicit `revert` diff
/// first (for semantic correctness) or simply retire the tip (for an
/// audit-grade reset). Both workflows are common in practice.
pub fn rollback<S: BranchStore>(store: &mut S, branch: &str, to_cid: Cid) -> Result<()> {
    let head = store.head(branch)?;
    let ancestors = store.ancestors(head);
    if !ancestors.contains(&to_cid) {
        return Err(VcsError::RollbackOutOfLineage {
            branch: branch.to_string(),
        });
    }
    store.set_head(branch, to_cid)?;
    Ok(())
}
