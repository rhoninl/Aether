//! Retention schedule management for pseudonymized data.
//!
//! Implements time-based retention policies (e.g., 7-year financial data
//! retention) with legal hold override support.

use serde::{Deserialize, Serialize};

/// Default retention period in years for financial/ledger data.
const DEFAULT_RETENTION_YEARS: u16 = 7;

/// Milliseconds per year (approximate: 365.25 days).
const MS_PER_YEAR: u64 = 365 * 24 * 60 * 60 * 1000 + 6 * 60 * 60 * 1000;

/// The lifecycle state of a retained record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RetentionState {
    /// Record is within its retention period.
    Active,
    /// Record retention is frozen due to a legal hold.
    Frozen,
    /// Record has exceeded its retention period and can be purged.
    Expired,
}

/// Configuration for a retention schedule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionWindow {
    /// How many years to retain records.
    pub years: u16,
    /// Whether legal holds override expiry.
    pub keep_legal_holds: bool,
    /// Whether to log audit events for retention actions.
    pub audit_retained: bool,
}

impl RetentionWindow {
    /// Create a default 7-year retention window.
    pub fn default_financial() -> Self {
        Self {
            years: DEFAULT_RETENTION_YEARS,
            keep_legal_holds: true,
            audit_retained: true,
        }
    }

    /// Create a custom retention window.
    pub fn new(years: u16, keep_legal_holds: bool) -> Self {
        Self {
            years,
            keep_legal_holds,
            audit_retained: true,
        }
    }

    /// Compute the expiry timestamp given a creation timestamp.
    pub fn compute_expiry_ms(&self, created_ms: u64) -> u64 {
        created_ms + (self.years as u64) * MS_PER_YEAR
    }
}

/// A record being tracked for retention.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionRecord {
    /// The table or collection this record belongs to.
    pub table_name: String,
    /// The unique row identifier.
    pub row_id: String,
    /// The pseudonym (if pseudonymized) or user_id reference.
    pub pseudonym: String,
    /// When this record was created (ms since epoch).
    pub created_ms: u64,
    /// When this record expires (ms since epoch).
    pub until_ms: u64,
    /// Current lifecycle state.
    pub state: RetentionState,
    /// Whether this record has a legal hold override.
    pub has_legal_hold: bool,
}

/// Manages retention schedules and record lifecycle.
#[derive(Debug)]
pub struct RetentionSchedule {
    window: RetentionWindow,
    records: Vec<RetentionRecord>,
}

impl RetentionSchedule {
    /// Create a new retention schedule with the given window.
    pub fn new(window: RetentionWindow) -> Self {
        Self {
            window,
            records: Vec::new(),
        }
    }

    /// Create a schedule with the default 7-year financial retention.
    pub fn default_financial() -> Self {
        Self::new(RetentionWindow::default_financial())
    }

    /// Add a record to the retention schedule.
    pub fn add_record(
        &mut self,
        table_name: String,
        row_id: String,
        pseudonym: String,
        created_ms: u64,
    ) {
        let until_ms = self.window.compute_expiry_ms(created_ms);
        self.records.push(RetentionRecord {
            table_name,
            row_id,
            pseudonym,
            created_ms,
            until_ms,
            state: RetentionState::Active,
            has_legal_hold: false,
        });
    }

    /// Place a legal hold on a record, freezing its retention.
    pub fn freeze_record(&mut self, row_id: &str) -> bool {
        if let Some(record) = self.records.iter_mut().find(|r| r.row_id == row_id) {
            record.state = RetentionState::Frozen;
            record.has_legal_hold = true;
            true
        } else {
            false
        }
    }

    /// Release a legal hold on a record, returning it to active state.
    pub fn unfreeze_record(&mut self, row_id: &str) -> bool {
        if let Some(record) = self.records.iter_mut().find(|r| r.row_id == row_id) {
            record.has_legal_hold = false;
            record.state = RetentionState::Active;
            true
        } else {
            false
        }
    }

    /// Update record states based on the current time.
    ///
    /// Records past their expiry that are not frozen transition to Expired.
    pub fn update_states(&mut self, now_ms: u64) {
        for record in &mut self.records {
            if record.state == RetentionState::Frozen {
                continue; // Legal holds override expiry
            }
            if now_ms >= record.until_ms {
                record.state = RetentionState::Expired;
            }
        }
    }

    /// Collect all expired records that can be purged.
    pub fn collect_expired(&self) -> Vec<&RetentionRecord> {
        self.records
            .iter()
            .filter(|r| r.state == RetentionState::Expired)
            .collect()
    }

    /// Collect all active (non-expired, non-frozen) records.
    pub fn collect_active(&self) -> Vec<&RetentionRecord> {
        self.records
            .iter()
            .filter(|r| r.state == RetentionState::Active)
            .collect()
    }

    /// Collect all frozen records.
    pub fn collect_frozen(&self) -> Vec<&RetentionRecord> {
        self.records
            .iter()
            .filter(|r| r.state == RetentionState::Frozen)
            .collect()
    }

    /// Remove all expired records and return the count removed.
    pub fn purge_expired(&mut self) -> usize {
        let before = self.records.len();
        self.records.retain(|r| r.state != RetentionState::Expired);
        before - self.records.len()
    }

    /// The total number of tracked records.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Whether the schedule has no records.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Get the retention window configuration.
    pub fn window(&self) -> &RetentionWindow {
        &self.window
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const YEAR_MS: u64 = MS_PER_YEAR;

    #[test]
    fn default_financial_window_is_7_years() {
        let window = RetentionWindow::default_financial();
        assert_eq!(window.years, 7);
        assert!(window.keep_legal_holds);
        assert!(window.audit_retained);
    }

    #[test]
    fn compute_expiry_adds_years() {
        let window = RetentionWindow::new(7, true);
        let created = 1_000_000;
        let expiry = window.compute_expiry_ms(created);
        assert_eq!(expiry, created + 7 * YEAR_MS);
    }

    #[test]
    fn add_record_sets_correct_expiry() {
        let mut schedule = RetentionSchedule::default_financial();
        schedule.add_record("ledger".into(), "r1".into(), "pseudo1".into(), 1000);
        let records = schedule.collect_active();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].until_ms, 1000 + 7 * YEAR_MS);
        assert_eq!(records[0].state, RetentionState::Active);
    }

    #[test]
    fn record_expires_after_retention_period() {
        let mut schedule = RetentionSchedule::default_financial();
        schedule.add_record("ledger".into(), "r1".into(), "pseudo1".into(), 0);

        // Before expiry
        schedule.update_states(6 * YEAR_MS);
        assert_eq!(schedule.collect_expired().len(), 0);
        assert_eq!(schedule.collect_active().len(), 1);

        // After expiry
        schedule.update_states(8 * YEAR_MS);
        assert_eq!(schedule.collect_expired().len(), 1);
        assert_eq!(schedule.collect_active().len(), 0);
    }

    #[test]
    fn frozen_record_does_not_expire() {
        let mut schedule = RetentionSchedule::default_financial();
        schedule.add_record("ledger".into(), "r1".into(), "pseudo1".into(), 0);
        schedule.freeze_record("r1");

        // Well past expiry
        schedule.update_states(100 * YEAR_MS);
        assert_eq!(schedule.collect_frozen().len(), 1);
        assert_eq!(schedule.collect_expired().len(), 0);
    }

    #[test]
    fn unfreeze_record_returns_to_active() {
        let mut schedule = RetentionSchedule::default_financial();
        schedule.add_record("ledger".into(), "r1".into(), "pseudo1".into(), 0);
        schedule.freeze_record("r1");
        assert_eq!(schedule.collect_frozen().len(), 1);

        schedule.unfreeze_record("r1");
        assert_eq!(schedule.collect_frozen().len(), 0);
        assert_eq!(schedule.collect_active().len(), 1);
    }

    #[test]
    fn unfrozen_record_can_expire() {
        let mut schedule = RetentionSchedule::default_financial();
        schedule.add_record("ledger".into(), "r1".into(), "pseudo1".into(), 0);
        schedule.freeze_record("r1");
        schedule.update_states(100 * YEAR_MS);
        assert_eq!(schedule.collect_expired().len(), 0);

        schedule.unfreeze_record("r1");
        schedule.update_states(100 * YEAR_MS);
        assert_eq!(schedule.collect_expired().len(), 1);
    }

    #[test]
    fn purge_expired_removes_records() {
        let mut schedule = RetentionSchedule::default_financial();
        schedule.add_record("ledger".into(), "r1".into(), "p1".into(), 0);
        schedule.add_record("ledger".into(), "r2".into(), "p2".into(), 0);
        schedule.add_record("ledger".into(), "r3".into(), "p3".into(), 5 * YEAR_MS);

        schedule.update_states(8 * YEAR_MS);
        assert_eq!(schedule.collect_expired().len(), 2);
        assert_eq!(schedule.collect_active().len(), 1);

        let purged = schedule.purge_expired();
        assert_eq!(purged, 2);
        assert_eq!(schedule.len(), 1);
    }

    #[test]
    fn freeze_nonexistent_record_returns_false() {
        let mut schedule = RetentionSchedule::default_financial();
        assert!(!schedule.freeze_record("no-such-row"));
    }

    #[test]
    fn unfreeze_nonexistent_record_returns_false() {
        let mut schedule = RetentionSchedule::default_financial();
        assert!(!schedule.unfreeze_record("no-such-row"));
    }

    #[test]
    fn len_and_is_empty() {
        let mut schedule = RetentionSchedule::default_financial();
        assert!(schedule.is_empty());
        assert_eq!(schedule.len(), 0);

        schedule.add_record("t".into(), "r1".into(), "p".into(), 0);
        assert!(!schedule.is_empty());
        assert_eq!(schedule.len(), 1);
    }

    #[test]
    fn custom_retention_window() {
        let window = RetentionWindow::new(3, false);
        let mut schedule = RetentionSchedule::new(window);
        schedule.add_record("t".into(), "r1".into(), "p".into(), 0);

        // Expires after 3 years, not 7
        schedule.update_states(4 * YEAR_MS);
        assert_eq!(schedule.collect_expired().len(), 1);
    }

    #[test]
    fn window_accessor() {
        let schedule = RetentionSchedule::default_financial();
        assert_eq!(schedule.window().years, 7);
    }

    #[test]
    fn multiple_records_independent_states() {
        let mut schedule = RetentionSchedule::default_financial();
        schedule.add_record("t".into(), "r1".into(), "p1".into(), 0);
        schedule.add_record("t".into(), "r2".into(), "p2".into(), 0);
        schedule.add_record("t".into(), "r3".into(), "p3".into(), 0);

        schedule.freeze_record("r2");
        schedule.update_states(8 * YEAR_MS);

        assert_eq!(schedule.collect_expired().len(), 2); // r1, r3
        assert_eq!(schedule.collect_frozen().len(), 1); // r2
    }

    #[test]
    fn exact_expiry_boundary() {
        let mut schedule = RetentionSchedule::default_financial();
        let created = 1000;
        schedule.add_record("t".into(), "r1".into(), "p".into(), created);
        let expiry = created + 7 * YEAR_MS;

        // One ms before expiry: still active
        schedule.update_states(expiry - 1);
        assert_eq!(schedule.collect_active().len(), 1);

        // Exactly at expiry: expired
        schedule.update_states(expiry);
        assert_eq!(schedule.collect_expired().len(), 1);
    }
}
