//! Branch model.
//!
//! A [`Branch`] is a named, mutable pointer to a diff CID — the
//! *head* of the branch. Branches have optional parentage so that
//! `branch("feature", parent = Some("main"))` records the lineage.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::diff::{AgentRef, Cid};
use crate::error::{Result, VcsError};

/// The built-in default branch name. Mirrors CLAUDE.md's "main, not
/// master" rule.
pub const DEFAULT_BRANCH: &str = "main";

/// A branch: name + head + parentage + author.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Branch {
    /// Branch name — unique per store.
    pub name: String,
    /// Current head CID of the branch.
    pub head: Cid,
    /// Optional parent branch (for tracking fork points).
    pub parent_branch: Option<String>,
    /// Who created this branch.
    pub created_by: AgentRef,
}

/// A branch store: branch metadata + the ancestry graph of diffs.
///
/// Implementations are free to persist state however they like. This
/// crate ships an in-memory implementation for tests and demos.
pub trait BranchStore {
    /// Create a new branch forked from `parent`. If `parent` is
    /// `None`, the new branch starts at the zero CID.
    fn branch(&mut self, parent: Option<&str>, name: &str, created_by: AgentRef)
        -> Result<Branch>;

    /// Look up the current head CID of a branch.
    fn head(&self, branch: &str) -> Result<Cid>;

    /// Move a branch's head. Used by `merge`, `rollback`, etc.
    fn set_head(&mut self, branch: &str, head: Cid) -> Result<()>;

    /// List every branch name in the store.
    fn list_branches(&self) -> Vec<String>;

    /// Record that `child` has `parent` as its immediate predecessor
    /// in the diff DAG. Used by `merge` to walk ancestry.
    fn record_ancestry(&mut self, child: Cid, parent: Cid);

    /// Return the set of ancestor CIDs of `cid`, inclusive.
    fn ancestors(&self, cid: Cid) -> Vec<Cid>;

    /// Return the full [`Branch`] struct for a branch name.
    fn get_branch(&self, name: &str) -> Result<Branch>;
}

/// Thread-unsafe in-memory [`BranchStore`]. Good enough for tests
/// and single-threaded embedding; a `SyncMemoryBranchStore` wrapper
/// can be layered on by callers.
#[derive(Debug, Default)]
pub struct MemoryBranchStore {
    branches: BTreeMap<String, Branch>,
    /// Reverse-direction ancestry: child -> its immediate parents.
    parents: BTreeMap<Cid, Vec<Cid>>,
}

impl MemoryBranchStore {
    /// Empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a store with a `main` branch seeded at the zero CID.
    pub fn with_default_main(created_by: AgentRef) -> Self {
        let mut s = Self::new();
        s.branches.insert(
            DEFAULT_BRANCH.to_string(),
            Branch {
                name: DEFAULT_BRANCH.to_string(),
                head: [0u8; 32],
                parent_branch: None,
                created_by,
            },
        );
        s
    }
}

impl BranchStore for MemoryBranchStore {
    fn branch(
        &mut self,
        parent: Option<&str>,
        name: &str,
        created_by: AgentRef,
    ) -> Result<Branch> {
        if self.branches.contains_key(name) {
            return Err(VcsError::BranchExists(name.to_string()));
        }
        let head = match parent {
            Some(p) => {
                self.branches
                    .get(p)
                    .ok_or_else(|| VcsError::UnknownBranch(p.to_string()))?
                    .head
            }
            None => [0u8; 32],
        };
        let branch = Branch {
            name: name.to_string(),
            head,
            parent_branch: parent.map(str::to_string),
            created_by,
        };
        self.branches.insert(name.to_string(), branch.clone());
        Ok(branch)
    }

    fn head(&self, branch: &str) -> Result<Cid> {
        self.branches
            .get(branch)
            .map(|b| b.head)
            .ok_or_else(|| VcsError::UnknownBranch(branch.to_string()))
    }

    fn set_head(&mut self, branch: &str, head: Cid) -> Result<()> {
        let b = self
            .branches
            .get_mut(branch)
            .ok_or_else(|| VcsError::UnknownBranch(branch.to_string()))?;
        b.head = head;
        Ok(())
    }

    fn list_branches(&self) -> Vec<String> {
        self.branches.keys().cloned().collect()
    }

    fn record_ancestry(&mut self, child: Cid, parent: Cid) {
        let entry = self.parents.entry(child).or_default();
        if !entry.contains(&parent) {
            entry.push(parent);
        }
    }

    fn ancestors(&self, cid: Cid) -> Vec<Cid> {
        let mut out = Vec::new();
        let mut stack = vec![cid];
        while let Some(c) = stack.pop() {
            if out.contains(&c) {
                continue;
            }
            out.push(c);
            if let Some(parents) = self.parents.get(&c) {
                for p in parents {
                    stack.push(*p);
                }
            }
        }
        out
    }

    fn get_branch(&self, name: &str) -> Result<Branch> {
        self.branches
            .get(name)
            .cloned()
            .ok_or_else(|| VcsError::UnknownBranch(name.to_string()))
    }
}
