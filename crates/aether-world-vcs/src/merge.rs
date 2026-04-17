//! Three-way merge + conflict detection.
//!
//! Given two diffs `a` and `b` that share a common ancestor, this
//! module merges their ops. The algorithm is deliberately simple:
//!
//! 1. For every op in `a` and `b`, compute its [`Op::subject_key`].
//! 2. Group ops by subject key.
//! 3. If a subject key is touched by exactly one side, the op
//!    passes through.
//! 4. If both sides touched the same subject key with byte-identical
//!    ops, we collapse them (one op survives).
//! 5. Otherwise the pair is a [`Conflict`].
//!
//! A [`MergeOutcome::Clean`] carries the merged op list. A
//! [`MergeOutcome::Conflicted`] carries the partially-merged op list
//! alongside a [`ConflictReport`] listing every clash. Resolution is
//! the caller's job — typically surfaced through the review workflow.
//!
//! The merge is symmetric: `merge(a, b)` and `merge(b, a)` produce the
//! same conflict pairs (possibly with swapped `a`/`b` sides).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::diff::{Diff, Op};

/// A single clashing pair of ops that both touched the same subject.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Conflict {
    /// The shared subject key; see [`Op::subject_key`].
    pub subject: String,
    /// Ops from the `a` side that touched this subject.
    pub a_ops: Vec<Op>,
    /// Ops from the `b` side that touched this subject.
    pub b_ops: Vec<Op>,
    /// Human-readable hint describing how to resolve.
    pub hint: String,
}

/// Aggregate of all conflicts from a merge.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ConflictReport {
    /// One entry per clashing subject key.
    pub conflicts: Vec<Conflict>,
}

impl ConflictReport {
    /// Whether the report contains any conflicts.
    pub fn is_empty(&self) -> bool {
        self.conflicts.is_empty()
    }

    /// Number of conflicts.
    pub fn len(&self) -> usize {
        self.conflicts.len()
    }
}

/// The output of a merge attempt.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum MergeOutcome {
    /// Merge succeeded; the ops are a valid combined diff body.
    Clean {
        /// Merged op list. Order: all `a`-only ops first (in `a`
        /// order), then `b`-only ops (in `b` order), then
        /// byte-identical collapsed ops.
        merged_ops: Vec<Op>,
    },
    /// Merge has at least one conflict. The caller must resolve
    /// conflicts before proceeding (typically via review).
    Conflicted {
        /// Ops from subjects that merged cleanly.
        clean_ops: Vec<Op>,
        /// Full conflict report.
        report: ConflictReport,
    },
}

impl MergeOutcome {
    /// Convenience: is this a clean merge?
    pub fn is_clean(&self) -> bool {
        matches!(self, MergeOutcome::Clean { .. })
    }
}

/// Resolve the hint message for a pair of conflicting op groups.
fn resolution_hint(a: &[Op], b: &[Op]) -> String {
    match (a.first(), b.first()) {
        (Some(Op::AddEntity { .. }), Some(Op::RemoveEntity { .. }))
        | (Some(Op::RemoveEntity { .. }), Some(Op::AddEntity { .. })) => {
            "add/remove conflict: one side created the entity, the other removed it; pick one".into()
        }
        (Some(Op::ModifyComponent { .. }), Some(Op::ModifyComponent { .. })) => {
            "component modified on both sides; pick one value or write a merge value".into()
        }
        (Some(Op::RetargetScript { .. }), Some(Op::RetargetScript { .. })) => {
            "script retargeted on both sides; pick one script CID".into()
        }
        _ => "both sides touched the same subject; pick one side's op".into(),
    }
}

/// Three-way merge. `_ancestor` is currently unused — this
/// implementation compares the two diff bodies directly — but is part
/// of the signature so that a future optimization (skip ops already
/// in the ancestor) can plug in without a breaking change.
pub fn merge(_ancestor: &Diff, a: &Diff, b: &Diff) -> MergeOutcome {
    // Group ops by subject key, preserving original per-side ordering.
    let mut a_by_subject: BTreeMap<String, Vec<Op>> = BTreeMap::new();
    let mut b_by_subject: BTreeMap<String, Vec<Op>> = BTreeMap::new();
    let mut a_order: Vec<String> = Vec::new();
    let mut b_order: Vec<String> = Vec::new();
    for op in &a.ops {
        let key = op.subject_key();
        if !a_by_subject.contains_key(&key) {
            a_order.push(key.clone());
        }
        a_by_subject.entry(key).or_default().push(op.clone());
    }
    for op in &b.ops {
        let key = op.subject_key();
        if !b_by_subject.contains_key(&key) {
            b_order.push(key.clone());
        }
        b_by_subject.entry(key).or_default().push(op.clone());
    }

    let mut clean_ops: Vec<Op> = Vec::new();
    let mut conflicts: Vec<Conflict> = Vec::new();

    // Pass 1: a-only subjects (preserve a_order) + intersections.
    for subject in &a_order {
        match b_by_subject.get(subject) {
            None => {
                // a-only
                if let Some(ops) = a_by_subject.get(subject) {
                    clean_ops.extend(ops.iter().cloned());
                }
            }
            Some(b_ops) => {
                let a_ops = a_by_subject.get(subject).cloned().unwrap_or_default();
                if a_ops == *b_ops {
                    // Byte-identical — collapse.
                    clean_ops.extend(a_ops);
                } else {
                    conflicts.push(Conflict {
                        subject: subject.clone(),
                        hint: resolution_hint(&a_ops, b_ops),
                        a_ops,
                        b_ops: b_ops.clone(),
                    });
                }
            }
        }
    }

    // Pass 2: b-only subjects.
    for subject in &b_order {
        if !a_by_subject.contains_key(subject) {
            if let Some(ops) = b_by_subject.get(subject) {
                clean_ops.extend(ops.iter().cloned());
            }
        }
    }

    if conflicts.is_empty() {
        MergeOutcome::Clean {
            merged_ops: clean_ops,
        }
    } else {
        MergeOutcome::Conflicted {
            clean_ops,
            report: ConflictReport { conflicts },
        }
    }
}
