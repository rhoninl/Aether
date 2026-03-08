//! Approval workflow with state machine transitions.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ApprovalStatus {
    Pending,
    Scanning,
    Approved,
    Rejected { reason: String },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ApprovalError {
    InvalidTransition {
        from: ApprovalStatus,
        to: ApprovalStatus,
    },
}

impl std::fmt::Display for ApprovalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApprovalError::InvalidTransition { from, to } => {
                write!(f, "invalid transition from {from:?} to {to:?}")
            }
        }
    }
}

impl std::error::Error for ApprovalError {}

/// Policy for auto-approval decisions.
#[derive(Debug, Clone)]
pub struct ApprovalPolicy {
    pub auto_approve_below_bytes: u64,
    pub trusted_creator_ids: Vec<uuid::Uuid>,
}

impl Default for ApprovalPolicy {
    fn default() -> Self {
        Self {
            auto_approve_below_bytes: 0,
            trusted_creator_ids: Vec::new(),
        }
    }
}

impl ApprovalPolicy {
    /// Determine whether an upload should be auto-approved.
    pub fn should_auto_approve(&self, creator_id: &uuid::Uuid, size_bytes: u64) -> bool {
        if self.trusted_creator_ids.contains(creator_id) {
            return true;
        }
        if self.auto_approve_below_bytes > 0 && size_bytes < self.auto_approve_below_bytes {
            return true;
        }
        false
    }
}

/// State machine governing approval transitions.
#[derive(Debug)]
pub struct ApprovalWorkflow {
    status: ApprovalStatus,
}

impl ApprovalWorkflow {
    pub fn new() -> Self {
        Self {
            status: ApprovalStatus::Pending,
        }
    }

    pub fn status(&self) -> &ApprovalStatus {
        &self.status
    }

    pub fn transition(&mut self, target: ApprovalStatus) -> Result<&ApprovalStatus, ApprovalError> {
        if !self.is_valid_transition(&target) {
            return Err(ApprovalError::InvalidTransition {
                from: self.status.clone(),
                to: target,
            });
        }
        self.status = target;
        Ok(&self.status)
    }

    fn is_valid_transition(&self, target: &ApprovalStatus) -> bool {
        matches!(
            (&self.status, target),
            (ApprovalStatus::Pending, ApprovalStatus::Scanning)
                | (ApprovalStatus::Pending, ApprovalStatus::Approved) // auto-approve
                | (ApprovalStatus::Scanning, ApprovalStatus::Approved)
                | (ApprovalStatus::Scanning, ApprovalStatus::Rejected { .. })
        )
    }
}

impl Default for ApprovalWorkflow {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_status_is_pending() {
        let wf = ApprovalWorkflow::new();
        assert_eq!(*wf.status(), ApprovalStatus::Pending);
    }

    #[test]
    fn pending_to_scanning_valid() {
        let mut wf = ApprovalWorkflow::new();
        let result = wf.transition(ApprovalStatus::Scanning);
        assert!(result.is_ok());
        assert_eq!(*wf.status(), ApprovalStatus::Scanning);
    }

    #[test]
    fn pending_to_approved_valid_auto_approve() {
        let mut wf = ApprovalWorkflow::new();
        let result = wf.transition(ApprovalStatus::Approved);
        assert!(result.is_ok());
        assert_eq!(*wf.status(), ApprovalStatus::Approved);
    }

    #[test]
    fn scanning_to_approved_valid() {
        let mut wf = ApprovalWorkflow::new();
        wf.transition(ApprovalStatus::Scanning).unwrap();
        let result = wf.transition(ApprovalStatus::Approved);
        assert!(result.is_ok());
        assert_eq!(*wf.status(), ApprovalStatus::Approved);
    }

    #[test]
    fn scanning_to_rejected_valid() {
        let mut wf = ApprovalWorkflow::new();
        wf.transition(ApprovalStatus::Scanning).unwrap();
        let result = wf.transition(ApprovalStatus::Rejected {
            reason: "policy violation".into(),
        });
        assert!(result.is_ok());
    }

    #[test]
    fn pending_to_rejected_invalid() {
        let mut wf = ApprovalWorkflow::new();
        let result = wf.transition(ApprovalStatus::Rejected {
            reason: "nope".into(),
        });
        assert!(result.is_err());
    }

    #[test]
    fn approved_to_scanning_invalid() {
        let mut wf = ApprovalWorkflow::new();
        wf.transition(ApprovalStatus::Approved).unwrap();
        let result = wf.transition(ApprovalStatus::Scanning);
        assert!(result.is_err());
    }

    #[test]
    fn rejected_to_approved_invalid() {
        let mut wf = ApprovalWorkflow::new();
        wf.transition(ApprovalStatus::Scanning).unwrap();
        wf.transition(ApprovalStatus::Rejected {
            reason: "bad".into(),
        })
        .unwrap();
        let result = wf.transition(ApprovalStatus::Approved);
        assert!(result.is_err());
    }

    #[test]
    fn double_scanning_invalid() {
        let mut wf = ApprovalWorkflow::new();
        wf.transition(ApprovalStatus::Scanning).unwrap();
        let result = wf.transition(ApprovalStatus::Scanning);
        assert!(result.is_err());
    }

    #[test]
    fn error_contains_from_and_to() {
        let mut wf = ApprovalWorkflow::new();
        let err = wf
            .transition(ApprovalStatus::Rejected {
                reason: "x".into(),
            })
            .unwrap_err();
        match err {
            ApprovalError::InvalidTransition { from, to } => {
                assert_eq!(from, ApprovalStatus::Pending);
                assert!(matches!(to, ApprovalStatus::Rejected { .. }));
            }
        }
    }

    #[test]
    fn policy_auto_approve_trusted_creator() {
        let creator = uuid::Uuid::new_v4();
        let policy = ApprovalPolicy {
            auto_approve_below_bytes: 0,
            trusted_creator_ids: vec![creator],
        };
        assert!(policy.should_auto_approve(&creator, 999_999_999));
    }

    #[test]
    fn policy_auto_approve_small_file() {
        let creator = uuid::Uuid::new_v4();
        let policy = ApprovalPolicy {
            auto_approve_below_bytes: 1000,
            trusted_creator_ids: Vec::new(),
        };
        assert!(policy.should_auto_approve(&creator, 500));
    }

    #[test]
    fn policy_no_auto_approve_large_untrusted() {
        let creator = uuid::Uuid::new_v4();
        let policy = ApprovalPolicy {
            auto_approve_below_bytes: 1000,
            trusted_creator_ids: Vec::new(),
        };
        assert!(!policy.should_auto_approve(&creator, 2000));
    }

    #[test]
    fn policy_default_no_auto_approve() {
        let creator = uuid::Uuid::new_v4();
        let policy = ApprovalPolicy::default();
        assert!(!policy.should_auto_approve(&creator, 100));
    }
}
