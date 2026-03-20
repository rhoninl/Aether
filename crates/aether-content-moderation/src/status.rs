//! Moderation status state machine with validated transitions.

/// All possible moderation states for a content item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModerationStatus {
    /// Content submitted, awaiting scanning.
    Pending,
    /// Content is being scanned by automated systems.
    Scanning,
    /// Content automatically approved by decision engine.
    AutoApproved,
    /// Content flagged and awaiting human review.
    InReview,
    /// Content approved by a human moderator.
    Approved,
    /// Content rejected (by automation or human moderator).
    Rejected,
    /// Content rejection has been appealed.
    Appealed,
}

/// Error returned when an invalid state transition is attempted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidTransition {
    pub from: ModerationStatus,
    pub to: ModerationStatus,
}

impl std::fmt::Display for InvalidTransition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "invalid moderation status transition: {:?} -> {:?}",
            self.from, self.to
        )
    }
}

impl std::error::Error for InvalidTransition {}

impl ModerationStatus {
    /// Returns the set of states that are reachable from the current state.
    pub fn valid_next_states(&self) -> &'static [ModerationStatus] {
        match self {
            ModerationStatus::Pending => &[ModerationStatus::Scanning],
            ModerationStatus::Scanning => &[
                ModerationStatus::AutoApproved,
                ModerationStatus::InReview,
                ModerationStatus::Rejected,
            ],
            ModerationStatus::AutoApproved => &[],
            ModerationStatus::InReview => &[ModerationStatus::Approved, ModerationStatus::Rejected],
            ModerationStatus::Approved => &[],
            ModerationStatus::Rejected => &[ModerationStatus::Appealed],
            ModerationStatus::Appealed => &[ModerationStatus::InReview],
        }
    }

    /// Attempt to transition to a new state. Returns the new state on success,
    /// or an `InvalidTransition` error if the transition is not allowed.
    pub fn transition(self, to: ModerationStatus) -> Result<ModerationStatus, InvalidTransition> {
        if self.valid_next_states().contains(&to) {
            Ok(to)
        } else {
            Err(InvalidTransition { from: self, to })
        }
    }

    /// Returns true if this is a terminal state (no further transitions).
    pub fn is_terminal(&self) -> bool {
        self.valid_next_states().is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pending_to_scanning() {
        let status = ModerationStatus::Pending;
        let result = status.transition(ModerationStatus::Scanning);
        assert_eq!(result, Ok(ModerationStatus::Scanning));
    }

    #[test]
    fn test_pending_cannot_skip_to_approved() {
        let status = ModerationStatus::Pending;
        let result = status.transition(ModerationStatus::Approved);
        assert_eq!(
            result,
            Err(InvalidTransition {
                from: ModerationStatus::Pending,
                to: ModerationStatus::Approved,
            })
        );
    }

    #[test]
    fn test_scanning_to_auto_approved() {
        let status = ModerationStatus::Scanning;
        assert_eq!(
            status.transition(ModerationStatus::AutoApproved),
            Ok(ModerationStatus::AutoApproved)
        );
    }

    #[test]
    fn test_scanning_to_in_review() {
        let status = ModerationStatus::Scanning;
        assert_eq!(
            status.transition(ModerationStatus::InReview),
            Ok(ModerationStatus::InReview)
        );
    }

    #[test]
    fn test_scanning_to_rejected() {
        let status = ModerationStatus::Scanning;
        assert_eq!(
            status.transition(ModerationStatus::Rejected),
            Ok(ModerationStatus::Rejected)
        );
    }

    #[test]
    fn test_in_review_to_approved() {
        let status = ModerationStatus::InReview;
        assert_eq!(
            status.transition(ModerationStatus::Approved),
            Ok(ModerationStatus::Approved)
        );
    }

    #[test]
    fn test_in_review_to_rejected() {
        let status = ModerationStatus::InReview;
        assert_eq!(
            status.transition(ModerationStatus::Rejected),
            Ok(ModerationStatus::Rejected)
        );
    }

    #[test]
    fn test_rejected_to_appealed() {
        let status = ModerationStatus::Rejected;
        assert_eq!(
            status.transition(ModerationStatus::Appealed),
            Ok(ModerationStatus::Appealed)
        );
    }

    #[test]
    fn test_appealed_to_in_review() {
        let status = ModerationStatus::Appealed;
        assert_eq!(
            status.transition(ModerationStatus::InReview),
            Ok(ModerationStatus::InReview)
        );
    }

    #[test]
    fn test_auto_approved_is_terminal() {
        assert!(ModerationStatus::AutoApproved.is_terminal());
    }

    #[test]
    fn test_approved_is_terminal() {
        assert!(ModerationStatus::Approved.is_terminal());
    }

    #[test]
    fn test_pending_is_not_terminal() {
        assert!(!ModerationStatus::Pending.is_terminal());
    }

    #[test]
    fn test_rejected_is_not_terminal() {
        // Rejected can be appealed
        assert!(!ModerationStatus::Rejected.is_terminal());
    }

    #[test]
    fn test_invalid_transition_display() {
        let err = InvalidTransition {
            from: ModerationStatus::Pending,
            to: ModerationStatus::Approved,
        };
        let msg = format!("{}", err);
        assert!(msg.contains("Pending"));
        assert!(msg.contains("Approved"));
    }

    #[test]
    fn test_full_happy_path() {
        let status = ModerationStatus::Pending;
        let status = status.transition(ModerationStatus::Scanning).unwrap();
        let status = status.transition(ModerationStatus::InReview).unwrap();
        let status = status.transition(ModerationStatus::Approved).unwrap();
        assert_eq!(status, ModerationStatus::Approved);
        assert!(status.is_terminal());
    }

    #[test]
    fn test_full_rejection_appeal_path() {
        let status = ModerationStatus::Pending;
        let status = status.transition(ModerationStatus::Scanning).unwrap();
        let status = status.transition(ModerationStatus::Rejected).unwrap();
        let status = status.transition(ModerationStatus::Appealed).unwrap();
        let status = status.transition(ModerationStatus::InReview).unwrap();
        let status = status.transition(ModerationStatus::Approved).unwrap();
        assert_eq!(status, ModerationStatus::Approved);
    }

    #[test]
    fn test_auto_approve_path() {
        let status = ModerationStatus::Pending;
        let status = status.transition(ModerationStatus::Scanning).unwrap();
        let status = status.transition(ModerationStatus::AutoApproved).unwrap();
        assert_eq!(status, ModerationStatus::AutoApproved);
        assert!(status.is_terminal());
    }

    #[test]
    fn test_cannot_transition_from_auto_approved() {
        let status = ModerationStatus::AutoApproved;
        assert!(status.transition(ModerationStatus::InReview).is_err());
        assert!(status.transition(ModerationStatus::Rejected).is_err());
        assert!(status.transition(ModerationStatus::Pending).is_err());
    }

    #[test]
    fn test_cannot_transition_from_approved() {
        let status = ModerationStatus::Approved;
        assert!(status.transition(ModerationStatus::InReview).is_err());
        assert!(status.transition(ModerationStatus::Rejected).is_err());
    }
}
