//! Health monitoring for federated servers.

use std::collections::HashMap;

/// Health status of a federated server.
#[derive(Debug, Clone, PartialEq)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unreachable,
}

/// Health record for a single federated server.
#[derive(Debug, Clone)]
pub struct HealthRecord {
    pub server_id: String,
    pub status: HealthStatus,
    pub last_check_ms: u64,
    pub consecutive_failures: u32,
    pub total_checks: u64,
    pub total_failures: u64,
}

const DEFAULT_DEGRADED_THRESHOLD: u32 = 3;
const DEFAULT_FAILURE_THRESHOLD: u32 = 5;

/// Monitors the health of federated servers based on success/failure reports.
///
/// State transitions:
/// - Healthy -> Degraded: when consecutive_failures >= degraded_threshold
/// - Degraded -> Unreachable: when consecutive_failures >= failure_threshold
/// - Unreachable|Degraded -> Healthy: on any success
#[derive(Debug)]
pub struct HealthMonitor {
    records: HashMap<String, HealthRecord>,
    degraded_threshold: u32,
    failure_threshold: u32,
}

impl HealthMonitor {
    pub fn new() -> Self {
        Self {
            records: HashMap::new(),
            degraded_threshold: DEFAULT_DEGRADED_THRESHOLD,
            failure_threshold: DEFAULT_FAILURE_THRESHOLD,
        }
    }

    pub fn with_thresholds(degraded_threshold: u32, failure_threshold: u32) -> Self {
        Self {
            records: HashMap::new(),
            degraded_threshold,
            failure_threshold,
        }
    }

    /// Record a successful health check for a server.
    /// Resets consecutive failures and sets status to Healthy.
    pub fn record_success(&mut self, server_id: &str, timestamp_ms: u64) {
        let record = self.get_or_create(server_id);
        record.consecutive_failures = 0;
        record.status = HealthStatus::Healthy;
        record.last_check_ms = timestamp_ms;
        record.total_checks += 1;
    }

    /// Record a failed health check for a server.
    /// Increments consecutive failures and transitions status based on thresholds.
    pub fn record_failure(&mut self, server_id: &str, timestamp_ms: u64) {
        let failure_threshold = self.failure_threshold;
        let degraded_threshold = self.degraded_threshold;
        let record = self.get_or_create(server_id);
        record.consecutive_failures += 1;
        record.total_checks += 1;
        record.total_failures += 1;
        record.last_check_ms = timestamp_ms;

        if record.consecutive_failures >= failure_threshold {
            record.status = HealthStatus::Unreachable;
        } else if record.consecutive_failures >= degraded_threshold {
            record.status = HealthStatus::Degraded;
        }
    }

    /// Get the health status of a server. Returns None if never checked.
    pub fn get_status(&self, server_id: &str) -> Option<&HealthRecord> {
        self.records.get(server_id)
    }

    /// Get all health records.
    pub fn get_all(&self) -> Vec<&HealthRecord> {
        self.records.values().collect()
    }

    /// Get all servers with a specific status.
    pub fn get_by_status(&self, status: &HealthStatus) -> Vec<&HealthRecord> {
        self.records
            .values()
            .filter(|r| &r.status == status)
            .collect()
    }

    /// Remove a server from monitoring.
    pub fn remove(&mut self, server_id: &str) -> Option<HealthRecord> {
        self.records.remove(server_id)
    }

    /// Number of servers being monitored.
    pub fn count(&self) -> usize {
        self.records.len()
    }

    fn get_or_create(&mut self, server_id: &str) -> &mut HealthRecord {
        self.records
            .entry(server_id.to_string())
            .or_insert_with(|| HealthRecord {
                server_id: server_id.to_string(),
                status: HealthStatus::Healthy,
                last_check_ms: 0,
                consecutive_failures: 0,
                total_checks: 0,
                total_failures: 0,
            })
    }
}

impl Default for HealthMonitor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_server_starts_healthy_on_success() {
        let mut monitor = HealthMonitor::new();
        monitor.record_success("s1", 1000);
        let record = monitor.get_status("s1").unwrap();
        assert_eq!(record.status, HealthStatus::Healthy);
        assert_eq!(record.consecutive_failures, 0);
        assert_eq!(record.total_checks, 1);
    }

    #[test]
    fn single_failure_stays_healthy() {
        let mut monitor = HealthMonitor::new();
        monitor.record_success("s1", 1000);
        monitor.record_failure("s1", 2000);
        let record = monitor.get_status("s1").unwrap();
        assert_eq!(record.status, HealthStatus::Healthy);
        assert_eq!(record.consecutive_failures, 1);
    }

    #[test]
    fn transitions_to_degraded_at_threshold() {
        let mut monitor = HealthMonitor::with_thresholds(3, 5);
        monitor.record_failure("s1", 1000);
        monitor.record_failure("s1", 2000);
        assert_eq!(
            monitor.get_status("s1").unwrap().status,
            HealthStatus::Healthy
        );

        monitor.record_failure("s1", 3000);
        assert_eq!(
            monitor.get_status("s1").unwrap().status,
            HealthStatus::Degraded
        );
    }

    #[test]
    fn transitions_to_unreachable_at_threshold() {
        let mut monitor = HealthMonitor::with_thresholds(2, 4);
        for i in 0..4 {
            monitor.record_failure("s1", (i + 1) * 1000);
        }
        assert_eq!(
            monitor.get_status("s1").unwrap().status,
            HealthStatus::Unreachable
        );
    }

    #[test]
    fn success_resets_to_healthy_from_degraded() {
        let mut monitor = HealthMonitor::with_thresholds(2, 5);
        monitor.record_failure("s1", 1000);
        monitor.record_failure("s1", 2000);
        assert_eq!(
            monitor.get_status("s1").unwrap().status,
            HealthStatus::Degraded
        );

        monitor.record_success("s1", 3000);
        let record = monitor.get_status("s1").unwrap();
        assert_eq!(record.status, HealthStatus::Healthy);
        assert_eq!(record.consecutive_failures, 0);
    }

    #[test]
    fn success_resets_to_healthy_from_unreachable() {
        let mut monitor = HealthMonitor::with_thresholds(2, 3);
        for i in 0..3 {
            monitor.record_failure("s1", (i + 1) * 1000);
        }
        assert_eq!(
            monitor.get_status("s1").unwrap().status,
            HealthStatus::Unreachable
        );

        monitor.record_success("s1", 5000);
        assert_eq!(
            monitor.get_status("s1").unwrap().status,
            HealthStatus::Healthy
        );
    }

    #[test]
    fn total_failures_accumulate_across_resets() {
        let mut monitor = HealthMonitor::new();
        monitor.record_failure("s1", 1000);
        monitor.record_failure("s1", 2000);
        monitor.record_success("s1", 3000);
        monitor.record_failure("s1", 4000);

        let record = monitor.get_status("s1").unwrap();
        assert_eq!(record.total_failures, 3);
        assert_eq!(record.total_checks, 4);
        assert_eq!(record.consecutive_failures, 1);
    }

    #[test]
    fn get_status_returns_none_for_unknown() {
        let monitor = HealthMonitor::new();
        assert!(monitor.get_status("unknown").is_none());
    }

    #[test]
    fn get_all_returns_all_records() {
        let mut monitor = HealthMonitor::new();
        monitor.record_success("s1", 1000);
        monitor.record_success("s2", 1000);
        monitor.record_success("s3", 1000);
        assert_eq!(monitor.get_all().len(), 3);
    }

    #[test]
    fn get_by_status_filters() {
        let mut monitor = HealthMonitor::with_thresholds(1, 3);
        monitor.record_success("s1", 1000);
        monitor.record_failure("s2", 1000); // Degraded (threshold=1)
        monitor.record_failure("s3", 1000);
        monitor.record_failure("s3", 2000);
        monitor.record_failure("s3", 3000); // Unreachable (threshold=3)

        assert_eq!(monitor.get_by_status(&HealthStatus::Healthy).len(), 1);
        assert_eq!(monitor.get_by_status(&HealthStatus::Degraded).len(), 1);
        assert_eq!(monitor.get_by_status(&HealthStatus::Unreachable).len(), 1);
    }

    #[test]
    fn remove_server() {
        let mut monitor = HealthMonitor::new();
        monitor.record_success("s1", 1000);
        assert_eq!(monitor.count(), 1);
        let removed = monitor.remove("s1").unwrap();
        assert_eq!(removed.server_id, "s1");
        assert_eq!(monitor.count(), 0);
    }

    #[test]
    fn remove_missing_returns_none() {
        let mut monitor = HealthMonitor::new();
        assert!(monitor.remove("nope").is_none());
    }

    #[test]
    fn last_check_ms_is_updated() {
        let mut monitor = HealthMonitor::new();
        monitor.record_success("s1", 1000);
        assert_eq!(monitor.get_status("s1").unwrap().last_check_ms, 1000);
        monitor.record_failure("s1", 5000);
        assert_eq!(monitor.get_status("s1").unwrap().last_check_ms, 5000);
    }

    #[test]
    fn default_creates_empty_monitor() {
        let monitor = HealthMonitor::default();
        assert_eq!(monitor.count(), 0);
    }

    #[test]
    fn first_failure_on_unknown_server_creates_record() {
        let mut monitor = HealthMonitor::new();
        monitor.record_failure("s1", 1000);
        let record = monitor.get_status("s1").unwrap();
        assert_eq!(record.consecutive_failures, 1);
        assert_eq!(record.total_checks, 1);
    }
}
