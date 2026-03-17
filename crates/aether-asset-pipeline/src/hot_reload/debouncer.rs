//! Debouncing logic for file system events.
//!
//! Coalesces rapid file changes within a configurable time window
//! to avoid redundant asset reprocessing.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::hot_reload::events::ChangeKind;

/// Tracks pending file changes and emits them only after the debounce window expires.
pub struct Debouncer {
    /// Debounce window duration.
    window: Duration,
    /// Pending changes: path -> (latest change kind, last event time).
    pending: HashMap<PathBuf, (ChangeKind, Instant)>,
}

impl Debouncer {
    /// Create a new debouncer with the given window duration in milliseconds.
    pub fn new(window_ms: u64) -> Self {
        Self {
            window: Duration::from_millis(window_ms),
            pending: HashMap::new(),
        }
    }

    /// Record a file change event. Resets the debounce timer for that path.
    pub fn record(&mut self, path: PathBuf, kind: ChangeKind) {
        self.pending.insert(path, (kind, Instant::now()));
    }

    /// Record a file change event with a specific timestamp (for testing).
    pub fn record_at(&mut self, path: PathBuf, kind: ChangeKind, at: Instant) {
        self.pending.insert(path, (kind, at));
    }

    /// Drain all changes whose debounce window has expired as of `now`.
    /// Returns the settled changes and removes them from pending.
    pub fn drain_settled(&mut self, now: Instant) -> Vec<(PathBuf, ChangeKind)> {
        let mut settled = Vec::new();
        let mut settled_keys = Vec::new();

        for (path, (kind, last_event)) in &self.pending {
            if now.duration_since(*last_event) >= self.window {
                settled.push((path.clone(), kind.clone()));
                settled_keys.push(path.clone());
            }
        }

        for key in settled_keys {
            self.pending.remove(&key);
        }

        settled.sort_by(|a, b| a.0.cmp(&b.0));
        settled
    }

    /// Returns the number of pending (not yet settled) changes.
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Returns the debounce window duration.
    pub fn window(&self) -> Duration {
        self.window
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_debouncer_has_correct_window() {
        let d = Debouncer::new(300);
        assert_eq!(d.window(), Duration::from_millis(300));
    }

    #[test]
    fn new_debouncer_has_no_pending() {
        let d = Debouncer::new(300);
        assert_eq!(d.pending_count(), 0);
    }

    #[test]
    fn record_adds_pending_event() {
        let mut d = Debouncer::new(300);
        d.record(PathBuf::from("a.png"), ChangeKind::Modified);
        assert_eq!(d.pending_count(), 1);
    }

    #[test]
    fn record_same_path_overwrites() {
        let mut d = Debouncer::new(300);
        d.record(PathBuf::from("a.png"), ChangeKind::Modified);
        d.record(PathBuf::from("a.png"), ChangeKind::Deleted);
        assert_eq!(d.pending_count(), 1);
    }

    #[test]
    fn record_different_paths() {
        let mut d = Debouncer::new(300);
        d.record(PathBuf::from("a.png"), ChangeKind::Modified);
        d.record(PathBuf::from("b.png"), ChangeKind::Created);
        assert_eq!(d.pending_count(), 2);
    }

    #[test]
    fn drain_settled_before_window_returns_empty() {
        let mut d = Debouncer::new(300);
        let now = Instant::now();
        d.record_at(PathBuf::from("a.png"), ChangeKind::Modified, now);
        // Immediately check - not yet settled
        let settled = d.drain_settled(now);
        assert!(settled.is_empty());
        assert_eq!(d.pending_count(), 1);
    }

    #[test]
    fn drain_settled_after_window_returns_event() {
        let mut d = Debouncer::new(300);
        let now = Instant::now();
        d.record_at(PathBuf::from("a.png"), ChangeKind::Modified, now);
        // Check after window expires
        let later = now + Duration::from_millis(301);
        let settled = d.drain_settled(later);
        assert_eq!(settled.len(), 1);
        assert_eq!(settled[0].0, PathBuf::from("a.png"));
        assert_eq!(settled[0].1, ChangeKind::Modified);
        assert_eq!(d.pending_count(), 0);
    }

    #[test]
    fn drain_settled_removes_only_expired() {
        let mut d = Debouncer::new(300);
        let t0 = Instant::now();
        let t1 = t0 + Duration::from_millis(200);

        d.record_at(PathBuf::from("early.png"), ChangeKind::Modified, t0);
        d.record_at(PathBuf::from("late.png"), ChangeKind::Created, t1);

        // Check at t0 + 301ms: early has expired, late has not
        let check = t0 + Duration::from_millis(301);
        let settled = d.drain_settled(check);
        assert_eq!(settled.len(), 1);
        assert_eq!(settled[0].0, PathBuf::from("early.png"));
        assert_eq!(d.pending_count(), 1);
    }

    #[test]
    fn rapid_changes_coalesce_to_latest_kind() {
        let mut d = Debouncer::new(300);
        let t0 = Instant::now();

        d.record_at(PathBuf::from("a.png"), ChangeKind::Created, t0);
        d.record_at(
            PathBuf::from("a.png"),
            ChangeKind::Modified,
            t0 + Duration::from_millis(50),
        );
        d.record_at(
            PathBuf::from("a.png"),
            ChangeKind::Modified,
            t0 + Duration::from_millis(100),
        );

        assert_eq!(d.pending_count(), 1);

        // The latest event was at t0+100, so wait until t0+100+300=t0+400
        let check = t0 + Duration::from_millis(401);
        let settled = d.drain_settled(check);
        assert_eq!(settled.len(), 1);
        assert_eq!(settled[0].1, ChangeKind::Modified);
    }

    #[test]
    fn drain_settled_is_idempotent_after_drain() {
        let mut d = Debouncer::new(100);
        let t0 = Instant::now();
        d.record_at(PathBuf::from("a.png"), ChangeKind::Modified, t0);

        let check = t0 + Duration::from_millis(200);
        let first = d.drain_settled(check);
        assert_eq!(first.len(), 1);

        let second = d.drain_settled(check);
        assert!(second.is_empty());
    }

    #[test]
    fn zero_window_settles_immediately() {
        let mut d = Debouncer::new(0);
        let now = Instant::now();
        d.record_at(PathBuf::from("a.png"), ChangeKind::Modified, now);
        let settled = d.drain_settled(now);
        assert_eq!(settled.len(), 1);
    }

    #[test]
    fn multiple_paths_settle_independently() {
        let mut d = Debouncer::new(100);
        let t0 = Instant::now();

        d.record_at(PathBuf::from("a.png"), ChangeKind::Modified, t0);
        d.record_at(
            PathBuf::from("b.glb"),
            ChangeKind::Created,
            t0 + Duration::from_millis(50),
        );
        d.record_at(
            PathBuf::from("c.wav"),
            ChangeKind::Deleted,
            t0 + Duration::from_millis(200),
        );

        // At t0+150: only a.png settled (100ms window from t0)
        let check1 = t0 + Duration::from_millis(151);
        let s1 = d.drain_settled(check1);
        assert_eq!(s1.len(), 2); // a.png (t0+100) and b.glb (t0+150)

        // At t0+350: c.wav settled
        let check2 = t0 + Duration::from_millis(351);
        let s2 = d.drain_settled(check2);
        assert_eq!(s2.len(), 1);
        assert_eq!(s2[0].0, PathBuf::from("c.wav"));
    }
}
