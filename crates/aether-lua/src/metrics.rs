use std::sync::atomic::{AtomicU64, Ordering};

/// Simple atomic metrics counters for the Lua runtime.
/// No external dependency -- just atomic counters.
pub struct LuaMetrics {
    pub scripts_active: AtomicU64,
    pub errors_total: AtomicU64,
    pub reloads_total: AtomicU64,
    pub cpu_exceeded_total: AtomicU64,
    pub rate_limit_rejected_total: AtomicU64,
}

/// Point-in-time snapshot of all metrics values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetricsSnapshot {
    pub scripts_active: u64,
    pub errors_total: u64,
    pub reloads_total: u64,
    pub cpu_exceeded_total: u64,
    pub rate_limit_rejected_total: u64,
}

impl LuaMetrics {
    pub fn new() -> Self {
        Self {
            scripts_active: AtomicU64::new(0),
            errors_total: AtomicU64::new(0),
            reloads_total: AtomicU64::new(0),
            cpu_exceeded_total: AtomicU64::new(0),
            rate_limit_rejected_total: AtomicU64::new(0),
        }
    }

    pub fn inc_errors(&self) {
        self.errors_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_reloads(&self) {
        self.reloads_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_cpu_exceeded(&self) {
        self.cpu_exceeded_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_rate_limit_rejected(&self) {
        self.rate_limit_rejected_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn set_scripts_active(&self, n: u64) {
        self.scripts_active.store(n, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            scripts_active: self.scripts_active.load(Ordering::Relaxed),
            errors_total: self.errors_total.load(Ordering::Relaxed),
            reloads_total: self.reloads_total.load(Ordering::Relaxed),
            cpu_exceeded_total: self.cpu_exceeded_total.load(Ordering::Relaxed),
            rate_limit_rejected_total: self.rate_limit_rejected_total.load(Ordering::Relaxed),
        }
    }
}

impl Default for LuaMetrics {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_values_zero() {
        let m = LuaMetrics::new();
        let snap = m.snapshot();
        assert_eq!(snap.scripts_active, 0);
        assert_eq!(snap.errors_total, 0);
        assert_eq!(snap.reloads_total, 0);
        assert_eq!(snap.cpu_exceeded_total, 0);
        assert_eq!(snap.rate_limit_rejected_total, 0);
    }

    #[test]
    fn test_inc_errors() {
        let m = LuaMetrics::new();
        m.inc_errors();
        m.inc_errors();
        m.inc_errors();
        assert_eq!(m.snapshot().errors_total, 3);
    }

    #[test]
    fn test_inc_reloads() {
        let m = LuaMetrics::new();
        m.inc_reloads();
        m.inc_reloads();
        assert_eq!(m.snapshot().reloads_total, 2);
    }

    #[test]
    fn test_inc_cpu_exceeded() {
        let m = LuaMetrics::new();
        m.inc_cpu_exceeded();
        assert_eq!(m.snapshot().cpu_exceeded_total, 1);
    }

    #[test]
    fn test_inc_rate_limit_rejected() {
        let m = LuaMetrics::new();
        m.inc_rate_limit_rejected();
        m.inc_rate_limit_rejected();
        assert_eq!(m.snapshot().rate_limit_rejected_total, 2);
    }

    #[test]
    fn test_set_scripts_active() {
        let m = LuaMetrics::new();
        m.set_scripts_active(5);
        assert_eq!(m.snapshot().scripts_active, 5);
        m.set_scripts_active(3);
        assert_eq!(m.snapshot().scripts_active, 3);
    }

    #[test]
    fn test_snapshot_captures_all() {
        let m = LuaMetrics::new();
        m.set_scripts_active(10);
        m.inc_errors();
        m.inc_errors();
        m.inc_reloads();
        m.inc_cpu_exceeded();
        m.inc_cpu_exceeded();
        m.inc_cpu_exceeded();
        m.inc_rate_limit_rejected();

        let snap = m.snapshot();
        assert_eq!(snap.scripts_active, 10);
        assert_eq!(snap.errors_total, 2);
        assert_eq!(snap.reloads_total, 1);
        assert_eq!(snap.cpu_exceeded_total, 3);
        assert_eq!(snap.rate_limit_rejected_total, 1);
    }
}
