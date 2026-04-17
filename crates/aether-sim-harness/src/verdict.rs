//! Verdict, repair-patch, and related types.
//!
//! The simulation contract: every agent mutation either commits with a
//! [`Verdict::Pass`] (optionally with warnings) or returns a minimal
//! [`RepairPatch`] that, when re-run, turns the failing verdict into a pass.
//! If no such patch can be synthesized the [`SimReport`] carries
//! `repair_patch: None` and fail reasons explaining why.

use serde::{Deserialize, Serialize};

/// Content-addressable hash tag for a snapshot or a repair patch.
///
/// In the broader engine this is produced by a CID function; inside the
/// harness we use a hex sha256 digest.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Cid(pub String);

impl Cid {
    pub fn from_bytes(bytes: &[u8]) -> Self {
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        h.update(bytes);
        Self(hex::encode(h.finalize()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Structured reason a scenario failed. Kept machine-readable so the
/// repair synthesizer can reason about it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FailureReason {
    pub code: String,
    pub message: String,
    pub data: serde_json::Value,
}

impl FailureReason {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            data: serde_json::json!({}),
        }
    }

    pub fn with_data(mut self, data: serde_json::Value) -> Self {
        self.data = data;
        self
    }
}

/// Structured warning for `PassWithWarnings`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Warning {
    pub code: String,
    pub message: String,
    pub data: serde_json::Value,
}

impl Warning {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            data: serde_json::json!({}),
        }
    }
}

/// Overall verdict for a scenario run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "verdict", rename_all = "snake_case")]
pub enum Verdict {
    Pass,
    PassWithWarnings { warnings: Vec<Warning> },
    Fail { reasons: Vec<FailureReason> },
}

impl Verdict {
    pub fn is_pass(&self) -> bool {
        matches!(self, Verdict::Pass | Verdict::PassWithWarnings { .. })
    }

    pub fn is_fail(&self) -> bool {
        matches!(self, Verdict::Fail { .. })
    }
}

/// One primitive operation in a repair patch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum RepairOp {
    /// Drop an input at `index` in the scenario's input list.
    DropInput { index: usize },
    /// Clamp a numeric parameter on an input.
    ClampInputField {
        index: usize,
        field: String,
        max: f32,
    },
    /// Insert a synthesized input before `index`.
    InsertInputBefore {
        index: usize,
        input: crate::scenario::Input,
    },
    /// Replace an input at `index`.
    ReplaceInput {
        index: usize,
        input: crate::scenario::Input,
    },
}

/// A minimal (smallest patch size) set of ops that flip a failing scenario
/// into a passing one on re-run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RepairPatch {
    pub target_hash: Cid,
    pub ops: Vec<RepairOp>,
}

impl RepairPatch {
    pub fn new(target_hash: Cid, ops: Vec<RepairOp>) -> Self {
        Self { target_hash, ops }
    }

    pub fn size(&self) -> usize {
        self.ops.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verdict_classification() {
        assert!(Verdict::Pass.is_pass());
        assert!(Verdict::PassWithWarnings { warnings: vec![] }.is_pass());
        assert!(Verdict::Fail { reasons: vec![] }.is_fail());
    }

    #[test]
    fn cid_from_bytes_is_stable() {
        let a = Cid::from_bytes(b"hello");
        let b = Cid::from_bytes(b"hello");
        assert_eq!(a, b);
        assert_eq!(a.as_str().len(), 64);
    }

    #[test]
    fn verdict_serde_shape() {
        let v = Verdict::Pass;
        let s = serde_json::to_string(&v).unwrap();
        assert!(s.contains("\"pass\""));
        let back: Verdict = serde_json::from_str(&s).unwrap();
        assert_eq!(v, back);
    }
}
