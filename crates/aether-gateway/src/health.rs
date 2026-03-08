//! Downstream service health monitoring.
//!
//! Tracks per-service health using a simple state machine:
//!
//! ```text
//! Healthy --> Degraded --> Unhealthy --> Healthy (on recovery)
//! ```
//!
//! State transitions are driven by consecutive success/failure reports.

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Health state of a single downstream service.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceHealthState {
    /// Service is operating normally.
    Healthy,
    /// Some failures observed; service may be struggling.
    Degraded,
    /// Service is considered down.
    Unhealthy,
}

/// Configuration for health-check thresholds.
#[derive(Debug, Clone)]
pub struct HealthCheckConfig {
    /// Consecutive failures before transitioning from Healthy to Degraded.
    pub degraded_threshold: u32,
    /// Consecutive failures before transitioning from Degraded to Unhealthy.
    pub unhealthy_threshold: u32,
    /// Consecutive successes required to transition back to Healthy from any
    /// non-healthy state.
    pub recovery_threshold: u32,
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            degraded_threshold: 3,
            unhealthy_threshold: 5,
            recovery_threshold: 2,
        }
    }
}

/// Snapshot of a service's current health.
#[derive(Debug, Clone)]
pub struct ServiceHealth {
    pub state: ServiceHealthState,
    pub consecutive_failures: u32,
    pub consecutive_successes: u32,
    pub last_check_ms: u64,
    pub total_checks: u64,
    pub total_failures: u64,
}

// ---------------------------------------------------------------------------
// HealthChecker
// ---------------------------------------------------------------------------

/// Tracks health of multiple downstream services.
pub struct HealthChecker {
    config: HealthCheckConfig,
    services: HashMap<String, ServiceHealth>,
}

impl HealthChecker {
    pub fn new(config: HealthCheckConfig) -> Self {
        Self {
            config,
            services: HashMap::new(),
        }
    }

    /// Record a successful health probe for `service` at time `now_ms`.
    pub fn report_success(&mut self, service: &str, now_ms: u64) {
        let recovery_threshold = self.config.recovery_threshold;
        let entry = self.get_or_insert(service, now_ms);
        entry.consecutive_failures = 0;
        entry.consecutive_successes += 1;
        entry.total_checks += 1;
        entry.last_check_ms = now_ms;

        // Recovery transition.
        if entry.consecutive_successes >= recovery_threshold {
            entry.state = ServiceHealthState::Healthy;
        }
    }

    /// Record a failed health probe for `service` at time `now_ms`.
    pub fn report_failure(&mut self, service: &str, now_ms: u64) {
        let unhealthy_threshold = self.config.unhealthy_threshold;
        let degraded_threshold = self.config.degraded_threshold;
        let entry = self.get_or_insert(service, now_ms);
        entry.consecutive_successes = 0;
        entry.consecutive_failures += 1;
        entry.total_checks += 1;
        entry.total_failures += 1;
        entry.last_check_ms = now_ms;

        // Degradation transitions.
        if entry.consecutive_failures >= unhealthy_threshold {
            entry.state = ServiceHealthState::Unhealthy;
        } else if entry.consecutive_failures >= degraded_threshold {
            entry.state = ServiceHealthState::Degraded;
        }
    }

    /// Return the current health state for `service`.  Returns `Healthy` for
    /// unknown services (optimistic default).
    pub fn status(&self, service: &str) -> ServiceHealthState {
        self.services
            .get(service)
            .map(|h| h.state)
            .unwrap_or(ServiceHealthState::Healthy)
    }

    /// Return the full health snapshot for a service, if tracked.
    pub fn health(&self, service: &str) -> Option<&ServiceHealth> {
        self.services.get(service)
    }

    /// List all tracked service names.
    pub fn tracked_services(&self) -> Vec<&str> {
        self.services.keys().map(|s| s.as_str()).collect()
    }

    // -- internals ----------------------------------------------------------

    fn get_or_insert(&mut self, service: &str, now_ms: u64) -> &mut ServiceHealth {
        self.services
            .entry(service.to_string())
            .or_insert_with(|| ServiceHealth {
                state: ServiceHealthState::Healthy,
                consecutive_failures: 0,
                consecutive_successes: 0,
                last_check_ms: now_ms,
                total_checks: 0,
                total_failures: 0,
            })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn default_checker() -> HealthChecker {
        HealthChecker::new(HealthCheckConfig::default())
    }

    #[test]
    fn unknown_service_is_healthy() {
        let checker = default_checker();
        assert_eq!(
            checker.status("nonexistent"),
            ServiceHealthState::Healthy
        );
    }

    #[test]
    fn healthy_after_successes() {
        let mut checker = default_checker();
        checker.report_success("svc_a", 1000);
        checker.report_success("svc_a", 2000);
        assert_eq!(checker.status("svc_a"), ServiceHealthState::Healthy);
    }

    #[test]
    fn transition_to_degraded() {
        let mut checker = default_checker(); // degraded_threshold = 3
        checker.report_failure("svc_a", 1000);
        checker.report_failure("svc_a", 2000);
        assert_eq!(checker.status("svc_a"), ServiceHealthState::Healthy);
        checker.report_failure("svc_a", 3000);
        assert_eq!(checker.status("svc_a"), ServiceHealthState::Degraded);
    }

    #[test]
    fn transition_to_unhealthy() {
        let mut checker = default_checker(); // unhealthy_threshold = 5
        for t in 0..5 {
            checker.report_failure("svc_a", 1000 + t * 1000);
        }
        assert_eq!(checker.status("svc_a"), ServiceHealthState::Unhealthy);
    }

    #[test]
    fn recovery_from_unhealthy() {
        let mut checker = default_checker(); // recovery_threshold = 2
        // Drive to unhealthy.
        for t in 0..5 {
            checker.report_failure("svc_a", 1000 + t * 1000);
        }
        assert_eq!(checker.status("svc_a"), ServiceHealthState::Unhealthy);

        // One success is not enough.
        checker.report_success("svc_a", 10_000);
        assert_eq!(checker.status("svc_a"), ServiceHealthState::Unhealthy);

        // Second success triggers recovery.
        checker.report_success("svc_a", 11_000);
        assert_eq!(checker.status("svc_a"), ServiceHealthState::Healthy);
    }

    #[test]
    fn recovery_from_degraded() {
        let mut checker = default_checker();
        // Drive to degraded.
        for t in 0..3 {
            checker.report_failure("svc_a", 1000 + t * 1000);
        }
        assert_eq!(checker.status("svc_a"), ServiceHealthState::Degraded);

        // Recover.
        checker.report_success("svc_a", 10_000);
        checker.report_success("svc_a", 11_000);
        assert_eq!(checker.status("svc_a"), ServiceHealthState::Healthy);
    }

    #[test]
    fn failure_resets_success_counter() {
        let mut checker = default_checker();
        // One success then failure; consecutive_successes should reset.
        checker.report_success("svc_a", 1000);
        checker.report_failure("svc_a", 2000);
        // Need 3 failures for degraded; only 1 so far.
        assert_eq!(checker.status("svc_a"), ServiceHealthState::Healthy);
    }

    #[test]
    fn success_resets_failure_counter() {
        let mut checker = default_checker();
        // Two failures, then a success, then two more failures.
        checker.report_failure("svc_a", 1000);
        checker.report_failure("svc_a", 2000);
        checker.report_success("svc_a", 3000);
        checker.report_failure("svc_a", 4000);
        checker.report_failure("svc_a", 5000);
        // Only 2 consecutive failures (not 3), so still Healthy.
        assert_eq!(checker.status("svc_a"), ServiceHealthState::Healthy);
    }

    #[test]
    fn custom_thresholds() {
        let config = HealthCheckConfig {
            degraded_threshold: 1,
            unhealthy_threshold: 2,
            recovery_threshold: 1,
        };
        let mut checker = HealthChecker::new(config);

        checker.report_failure("svc_a", 1000);
        assert_eq!(checker.status("svc_a"), ServiceHealthState::Degraded);

        checker.report_failure("svc_a", 2000);
        assert_eq!(checker.status("svc_a"), ServiceHealthState::Unhealthy);

        checker.report_success("svc_a", 3000);
        assert_eq!(checker.status("svc_a"), ServiceHealthState::Healthy);
    }

    #[test]
    fn multiple_services_independent() {
        let mut checker = default_checker();
        for t in 0..5 {
            checker.report_failure("svc_a", 1000 + t * 1000);
        }
        checker.report_success("svc_b", 1000);

        assert_eq!(checker.status("svc_a"), ServiceHealthState::Unhealthy);
        assert_eq!(checker.status("svc_b"), ServiceHealthState::Healthy);
    }

    #[test]
    fn health_snapshot() {
        let mut checker = default_checker();
        checker.report_success("svc_a", 1000);
        checker.report_failure("svc_a", 2000);
        checker.report_failure("svc_a", 3000);

        let h = checker.health("svc_a").unwrap();
        assert_eq!(h.total_checks, 3);
        assert_eq!(h.total_failures, 2);
        assert_eq!(h.consecutive_failures, 2);
        assert_eq!(h.consecutive_successes, 0);
        assert_eq!(h.last_check_ms, 3000);
    }

    #[test]
    fn tracked_services_list() {
        let mut checker = default_checker();
        checker.report_success("alpha", 1000);
        checker.report_success("beta", 2000);

        let mut tracked = checker.tracked_services();
        tracked.sort();
        assert_eq!(tracked, vec!["alpha", "beta"]);
    }

    #[test]
    fn default_config() {
        let config = HealthCheckConfig::default();
        assert_eq!(config.degraded_threshold, 3);
        assert_eq!(config.unhealthy_threshold, 5);
        assert_eq!(config.recovery_threshold, 2);
    }
}
