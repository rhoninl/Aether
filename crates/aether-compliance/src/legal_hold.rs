//! Legal hold management for GDPR compliance.
//!
//! Legal holds defer deletion of user data during active investigations.
//! A user may have multiple concurrent holds from different cases.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A single legal hold placed on a user's data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hold {
    /// Unique case identifier for this hold.
    pub case_id: String,
    /// Human-readable reason for the hold.
    pub reason: String,
    /// The user whose data is held.
    pub user_id: u64,
    /// When the hold was placed.
    pub placed_at: DateTime<Utc>,
    /// When the hold was released, if ever.
    pub released_at: Option<DateTime<Utc>>,
}

impl Hold {
    /// Whether this hold is currently active (not released).
    pub fn is_active(&self) -> bool {
        self.released_at.is_none()
    }
}

/// Manages legal holds across all users.
#[derive(Debug, Default)]
pub struct HoldManager {
    holds: Vec<Hold>,
}

impl HoldManager {
    /// Create a new empty hold manager.
    pub fn new() -> Self {
        Self { holds: Vec::new() }
    }

    /// Place a legal hold on a user's data.
    ///
    /// Returns an error if a hold with the same case_id already exists
    /// and is active.
    pub fn place_hold(
        &mut self,
        case_id: String,
        reason: String,
        user_id: u64,
        now: DateTime<Utc>,
    ) -> Result<(), HoldError> {
        let duplicate = self
            .holds
            .iter()
            .any(|h| h.case_id == case_id && h.is_active());
        if duplicate {
            return Err(HoldError::DuplicateCaseId(case_id));
        }

        self.holds.push(Hold {
            case_id,
            reason,
            user_id,
            placed_at: now,
            released_at: None,
        });
        Ok(())
    }

    /// Release a legal hold by case_id.
    ///
    /// Returns an error if the case_id is not found or already released.
    pub fn release_hold(
        &mut self,
        case_id: &str,
        now: DateTime<Utc>,
    ) -> Result<(), HoldError> {
        let hold = self
            .holds
            .iter_mut()
            .find(|h| h.case_id == case_id && h.is_active());
        match hold {
            Some(h) => {
                h.released_at = Some(now);
                Ok(())
            }
            None => Err(HoldError::NotFound(case_id.to_string())),
        }
    }

    /// Check if a user has any active legal holds.
    pub fn has_active_holds(&self, user_id: u64) -> bool {
        self.holds
            .iter()
            .any(|h| h.user_id == user_id && h.is_active())
    }

    /// Get all active holds for a user.
    pub fn active_holds_for_user(&self, user_id: u64) -> Vec<&Hold> {
        self.holds
            .iter()
            .filter(|h| h.user_id == user_id && h.is_active())
            .collect()
    }

    /// Get all holds (active and released) for a user.
    pub fn all_holds_for_user(&self, user_id: u64) -> Vec<&Hold> {
        self.holds.iter().filter(|h| h.user_id == user_id).collect()
    }
}

/// Errors that can occur during hold management.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HoldError {
    /// A hold with this case_id is already active.
    DuplicateCaseId(String),
    /// No active hold found with this case_id.
    NotFound(String),
}

impl std::fmt::Display for HoldError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HoldError::DuplicateCaseId(id) => {
                write!(f, "active hold already exists for case_id: {id}")
            }
            HoldError::NotFound(id) => {
                write!(f, "no active hold found for case_id: {id}")
            }
        }
    }
}

impl std::error::Error for HoldError {}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn time(year: i32, month: u32, day: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(year, month, day, 0, 0, 0).unwrap()
    }

    #[test]
    fn place_hold_succeeds() {
        let mut mgr = HoldManager::new();
        let result = mgr.place_hold(
            "CASE-001".into(),
            "fraud investigation".into(),
            42,
            time(2026, 1, 1),
        );
        assert!(result.is_ok());
        assert!(mgr.has_active_holds(42));
    }

    #[test]
    fn duplicate_case_id_is_rejected() {
        let mut mgr = HoldManager::new();
        mgr.place_hold("CASE-001".into(), "reason".into(), 42, time(2026, 1, 1))
            .unwrap();
        let result =
            mgr.place_hold("CASE-001".into(), "other".into(), 42, time(2026, 1, 2));
        assert_eq!(
            result,
            Err(HoldError::DuplicateCaseId("CASE-001".into()))
        );
    }

    #[test]
    fn release_hold_succeeds() {
        let mut mgr = HoldManager::new();
        mgr.place_hold("CASE-001".into(), "reason".into(), 42, time(2026, 1, 1))
            .unwrap();
        let result = mgr.release_hold("CASE-001", time(2026, 3, 1));
        assert!(result.is_ok());
        assert!(!mgr.has_active_holds(42));
    }

    #[test]
    fn release_nonexistent_hold_fails() {
        let mut mgr = HoldManager::new();
        let result = mgr.release_hold("CASE-999", time(2026, 1, 1));
        assert_eq!(result, Err(HoldError::NotFound("CASE-999".into())));
    }

    #[test]
    fn release_already_released_hold_fails() {
        let mut mgr = HoldManager::new();
        mgr.place_hold("CASE-001".into(), "reason".into(), 42, time(2026, 1, 1))
            .unwrap();
        mgr.release_hold("CASE-001", time(2026, 2, 1)).unwrap();
        let result = mgr.release_hold("CASE-001", time(2026, 3, 1));
        assert_eq!(result, Err(HoldError::NotFound("CASE-001".into())));
    }

    #[test]
    fn multiple_holds_per_user() {
        let mut mgr = HoldManager::new();
        mgr.place_hold("CASE-A".into(), "fraud".into(), 42, time(2026, 1, 1))
            .unwrap();
        mgr.place_hold("CASE-B".into(), "tax".into(), 42, time(2026, 1, 2))
            .unwrap();
        assert!(mgr.has_active_holds(42));
        assert_eq!(mgr.active_holds_for_user(42).len(), 2);

        // Release one hold; user still has active holds
        mgr.release_hold("CASE-A", time(2026, 2, 1)).unwrap();
        assert!(mgr.has_active_holds(42));
        assert_eq!(mgr.active_holds_for_user(42).len(), 1);

        // Release second hold; no more active holds
        mgr.release_hold("CASE-B", time(2026, 3, 1)).unwrap();
        assert!(!mgr.has_active_holds(42));
    }

    #[test]
    fn holds_are_per_user() {
        let mut mgr = HoldManager::new();
        mgr.place_hold("CASE-001".into(), "reason".into(), 42, time(2026, 1, 1))
            .unwrap();
        assert!(mgr.has_active_holds(42));
        assert!(!mgr.has_active_holds(99));
    }

    #[test]
    fn all_holds_includes_released() {
        let mut mgr = HoldManager::new();
        mgr.place_hold("CASE-A".into(), "reason".into(), 42, time(2026, 1, 1))
            .unwrap();
        mgr.release_hold("CASE-A", time(2026, 2, 1)).unwrap();
        mgr.place_hold("CASE-B".into(), "reason".into(), 42, time(2026, 3, 1))
            .unwrap();

        assert_eq!(mgr.all_holds_for_user(42).len(), 2);
        assert_eq!(mgr.active_holds_for_user(42).len(), 1);
    }

    #[test]
    fn released_case_id_can_be_reused() {
        let mut mgr = HoldManager::new();
        mgr.place_hold("CASE-001".into(), "v1".into(), 42, time(2026, 1, 1))
            .unwrap();
        mgr.release_hold("CASE-001", time(2026, 2, 1)).unwrap();
        // Same case_id can be reused after release
        let result =
            mgr.place_hold("CASE-001".into(), "v2".into(), 42, time(2026, 3, 1));
        assert!(result.is_ok());
    }

    #[test]
    fn hold_is_active_check() {
        let hold = Hold {
            case_id: "C1".into(),
            reason: "test".into(),
            user_id: 1,
            placed_at: time(2026, 1, 1),
            released_at: None,
        };
        assert!(hold.is_active());

        let released_hold = Hold {
            case_id: "C2".into(),
            reason: "test".into(),
            user_id: 1,
            placed_at: time(2026, 1, 1),
            released_at: Some(time(2026, 2, 1)),
        };
        assert!(!released_hold.is_active());
    }
}
