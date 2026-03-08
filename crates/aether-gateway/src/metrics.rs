//! Request metrics collection.
//!
//! Pure in-memory counters and latency tracking per route. No external
//! dependencies; designed to be periodically scraped or snapshotted by an
//! observability layer.

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Immutable snapshot of metrics for a single route.
#[derive(Debug, Clone)]
pub struct RouteMetricsSnapshot {
    /// Total requests observed.
    pub total_requests: u64,
    /// Total successful requests.
    pub success_count: u64,
    /// Total failed requests.
    pub error_count: u64,
    /// Minimum observed latency in milliseconds.
    pub latency_min_ms: u64,
    /// Maximum observed latency in milliseconds.
    pub latency_max_ms: u64,
    /// Sum of all latencies (divide by `total_requests` for average).
    pub latency_sum_ms: u64,
}

impl RouteMetricsSnapshot {
    /// Average latency in milliseconds, or 0 if no requests recorded.
    pub fn latency_avg_ms(&self) -> u64 {
        if self.total_requests == 0 {
            0
        } else {
            self.latency_sum_ms / self.total_requests
        }
    }

    /// Error rate as a fraction in `[0.0, 1.0]`.
    pub fn error_rate(&self) -> f64 {
        if self.total_requests == 0 {
            0.0
        } else {
            self.error_count as f64 / self.total_requests as f64
        }
    }
}

// ---------------------------------------------------------------------------
// Internal mutable state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct RouteMetrics {
    total_requests: u64,
    success_count: u64,
    error_count: u64,
    latency_min_ms: u64,
    latency_max_ms: u64,
    latency_sum_ms: u64,
}

impl RouteMetrics {
    fn new() -> Self {
        Self {
            total_requests: 0,
            success_count: 0,
            error_count: 0,
            latency_min_ms: u64::MAX,
            latency_max_ms: 0,
            latency_sum_ms: 0,
        }
    }

    fn record(&mut self, latency_ms: u64, success: bool) {
        self.total_requests += 1;
        if success {
            self.success_count += 1;
        } else {
            self.error_count += 1;
        }
        self.latency_sum_ms += latency_ms;
        if latency_ms < self.latency_min_ms {
            self.latency_min_ms = latency_ms;
        }
        if latency_ms > self.latency_max_ms {
            self.latency_max_ms = latency_ms;
        }
    }

    fn snapshot(&self) -> RouteMetricsSnapshot {
        RouteMetricsSnapshot {
            total_requests: self.total_requests,
            success_count: self.success_count,
            error_count: self.error_count,
            latency_min_ms: if self.total_requests == 0 {
                0
            } else {
                self.latency_min_ms
            },
            latency_max_ms: self.latency_max_ms,
            latency_sum_ms: self.latency_sum_ms,
        }
    }
}

// ---------------------------------------------------------------------------
// RequestMetrics
// ---------------------------------------------------------------------------

/// Collects per-route request metrics.
pub struct RequestMetrics {
    routes: HashMap<String, RouteMetrics>,
    /// Global counter of currently in-flight requests (manual inc/dec).
    active_requests: u64,
}

impl RequestMetrics {
    pub fn new() -> Self {
        Self {
            routes: HashMap::new(),
            active_requests: 0,
        }
    }

    /// Record that a request to `route` completed with the given latency and
    /// success/failure status.
    pub fn record(&mut self, route: &str, latency_ms: u64, success: bool) {
        self.routes
            .entry(route.to_string())
            .or_insert_with(RouteMetrics::new)
            .record(latency_ms, success);
    }

    /// Return a snapshot of metrics for `route`, if any requests have been
    /// recorded.
    pub fn snapshot(&self, route: &str) -> Option<RouteMetricsSnapshot> {
        self.routes.get(route).map(|m| m.snapshot())
    }

    /// Return snapshots for all tracked routes.
    pub fn all_snapshots(&self) -> HashMap<String, RouteMetricsSnapshot> {
        self.routes
            .iter()
            .map(|(k, v)| (k.clone(), v.snapshot()))
            .collect()
    }

    /// Increment the active-request gauge.
    pub fn inc_active(&mut self) {
        self.active_requests += 1;
    }

    /// Decrement the active-request gauge.
    pub fn dec_active(&mut self) {
        self.active_requests = self.active_requests.saturating_sub(1);
    }

    /// Current number of in-flight requests.
    pub fn active_requests(&self) -> u64 {
        self.active_requests
    }

    /// Number of routes being tracked.
    pub fn tracked_route_count(&self) -> usize {
        self.routes.len()
    }

    /// Reset all metrics.
    pub fn reset(&mut self) {
        self.routes.clear();
        self.active_requests = 0;
    }
}

impl Default for RequestMetrics {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_and_snapshot() {
        let mut metrics = RequestMetrics::new();
        metrics.record("/api/v1/health", 10, true);
        metrics.record("/api/v1/health", 20, true);
        metrics.record("/api/v1/health", 50, false);

        let snap = metrics.snapshot("/api/v1/health").unwrap();
        assert_eq!(snap.total_requests, 3);
        assert_eq!(snap.success_count, 2);
        assert_eq!(snap.error_count, 1);
        assert_eq!(snap.latency_min_ms, 10);
        assert_eq!(snap.latency_max_ms, 50);
        assert_eq!(snap.latency_sum_ms, 80);
    }

    #[test]
    fn latency_average() {
        let mut metrics = RequestMetrics::new();
        metrics.record("r", 10, true);
        metrics.record("r", 30, true);

        let snap = metrics.snapshot("r").unwrap();
        assert_eq!(snap.latency_avg_ms(), 20);
    }

    #[test]
    fn latency_avg_zero_when_empty() {
        let snap = RouteMetricsSnapshot {
            total_requests: 0,
            success_count: 0,
            error_count: 0,
            latency_min_ms: 0,
            latency_max_ms: 0,
            latency_sum_ms: 0,
        };
        assert_eq!(snap.latency_avg_ms(), 0);
    }

    #[test]
    fn error_rate_calculation() {
        let mut metrics = RequestMetrics::new();
        metrics.record("r", 5, true);
        metrics.record("r", 5, true);
        metrics.record("r", 5, false);
        metrics.record("r", 5, false);

        let snap = metrics.snapshot("r").unwrap();
        let rate = snap.error_rate();
        assert!((rate - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn error_rate_zero_when_no_requests() {
        let snap = RouteMetricsSnapshot {
            total_requests: 0,
            success_count: 0,
            error_count: 0,
            latency_min_ms: 0,
            latency_max_ms: 0,
            latency_sum_ms: 0,
        };
        assert!((snap.error_rate() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn multiple_routes() {
        let mut metrics = RequestMetrics::new();
        metrics.record("route_a", 10, true);
        metrics.record("route_b", 20, false);

        assert!(metrics.snapshot("route_a").is_some());
        assert!(metrics.snapshot("route_b").is_some());
        assert!(metrics.snapshot("route_c").is_none());
        assert_eq!(metrics.tracked_route_count(), 2);
    }

    #[test]
    fn all_snapshots() {
        let mut metrics = RequestMetrics::new();
        metrics.record("a", 1, true);
        metrics.record("b", 2, false);

        let all = metrics.all_snapshots();
        assert_eq!(all.len(), 2);
        assert!(all.contains_key("a"));
        assert!(all.contains_key("b"));
    }

    #[test]
    fn active_request_gauge() {
        let mut metrics = RequestMetrics::new();
        assert_eq!(metrics.active_requests(), 0);

        metrics.inc_active();
        metrics.inc_active();
        assert_eq!(metrics.active_requests(), 2);

        metrics.dec_active();
        assert_eq!(metrics.active_requests(), 1);
    }

    #[test]
    fn active_gauge_saturates_at_zero() {
        let mut metrics = RequestMetrics::new();
        metrics.dec_active();
        assert_eq!(metrics.active_requests(), 0);
    }

    #[test]
    fn reset_clears_everything() {
        let mut metrics = RequestMetrics::new();
        metrics.record("r", 10, true);
        metrics.inc_active();
        metrics.reset();

        assert_eq!(metrics.tracked_route_count(), 0);
        assert_eq!(metrics.active_requests(), 0);
        assert!(metrics.snapshot("r").is_none());
    }

    #[test]
    fn snapshot_min_latency_zero_when_empty() {
        let metrics = RequestMetrics::new();
        assert!(metrics.snapshot("nonexistent").is_none());
    }

    #[test]
    fn single_request_min_max_equal() {
        let mut metrics = RequestMetrics::new();
        metrics.record("r", 42, true);
        let snap = metrics.snapshot("r").unwrap();
        assert_eq!(snap.latency_min_ms, 42);
        assert_eq!(snap.latency_max_ms, 42);
    }

    #[test]
    fn default_trait() {
        let metrics = RequestMetrics::default();
        assert_eq!(metrics.tracked_route_count(), 0);
    }
}
