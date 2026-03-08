//! Health check utilities for persistence backends.
//!
//! Provides a unified health checker that queries all configured backends and
//! reports their status.

use crate::nats::EventBus;
use crate::postgres::DatabaseClient;
use crate::redis_client::CacheClient;

/// Status of a single backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendStatus {
    /// Backend is reachable and responding.
    Healthy,
    /// Backend is unreachable or returned an error.
    Unhealthy,
    /// Backend is not configured / not in use.
    Unconfigured,
}

/// Aggregate health status across all backends.
#[derive(Debug, Clone)]
pub struct HealthReport {
    pub database: BackendStatus,
    pub cache: BackendStatus,
    pub event_bus: BackendStatus,
}

impl HealthReport {
    /// Returns `true` if all configured backends are healthy.
    pub fn is_all_healthy(&self) -> bool {
        let statuses = [self.database, self.cache, self.event_bus];
        statuses
            .iter()
            .all(|s| matches!(s, BackendStatus::Healthy | BackendStatus::Unconfigured))
    }

    /// Returns `true` if any configured backend is unhealthy.
    pub fn has_unhealthy(&self) -> bool {
        let statuses = [self.database, self.cache, self.event_bus];
        statuses
            .iter()
            .any(|s| matches!(s, BackendStatus::Unhealthy))
    }

    /// Returns the number of healthy backends.
    pub fn healthy_count(&self) -> usize {
        [self.database, self.cache, self.event_bus]
            .iter()
            .filter(|s| matches!(s, BackendStatus::Healthy))
            .count()
    }

    /// Returns a report where all backends are unconfigured.
    pub fn all_unconfigured() -> Self {
        Self {
            database: BackendStatus::Unconfigured,
            cache: BackendStatus::Unconfigured,
            event_bus: BackendStatus::Unconfigured,
        }
    }
}

/// Check the health of a database client.
pub async fn check_database(client: &dyn DatabaseClient) -> BackendStatus {
    if client.is_healthy().await {
        BackendStatus::Healthy
    } else {
        BackendStatus::Unhealthy
    }
}

/// Check the health of a cache client.
pub async fn check_cache(client: &dyn CacheClient) -> BackendStatus {
    if client.is_healthy().await {
        BackendStatus::Healthy
    } else {
        BackendStatus::Unhealthy
    }
}

/// Check the health of an event bus.
pub async fn check_event_bus(bus: &dyn EventBus) -> BackendStatus {
    if bus.is_healthy().await {
        BackendStatus::Healthy
    } else {
        BackendStatus::Unhealthy
    }
}

/// Run health checks against all provided backends.
///
/// Pass `None` for any backend that is not configured.
pub async fn check_all(
    database: Option<&dyn DatabaseClient>,
    cache: Option<&dyn CacheClient>,
    event_bus: Option<&dyn EventBus>,
) -> HealthReport {
    let database_status = match database {
        Some(db) => check_database(db).await,
        None => BackendStatus::Unconfigured,
    };

    let cache_status = match cache {
        Some(c) => check_cache(c).await,
        None => BackendStatus::Unconfigured,
    };

    let event_bus_status = match event_bus {
        Some(bus) => check_event_bus(bus).await,
        None => BackendStatus::Unconfigured,
    };

    HealthReport {
        database: database_status,
        cache: cache_status,
        event_bus: event_bus_status,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nats::MockEventBus;
    use crate::postgres::MockDatabaseClient;
    use crate::redis_client::MockCacheClient;

    #[tokio::test]
    async fn all_healthy_reports_true() {
        let db = MockDatabaseClient::healthy();
        let cache = MockCacheClient::healthy();
        let bus = MockEventBus::healthy();

        let report = check_all(
            Some(&db as &dyn DatabaseClient),
            Some(&cache as &dyn CacheClient),
            Some(&bus as &dyn EventBus),
        )
        .await;

        assert_eq!(report.database, BackendStatus::Healthy);
        assert_eq!(report.cache, BackendStatus::Healthy);
        assert_eq!(report.event_bus, BackendStatus::Healthy);
        assert!(report.is_all_healthy());
        assert!(!report.has_unhealthy());
        assert_eq!(report.healthy_count(), 3);
    }

    #[tokio::test]
    async fn one_unhealthy_detected() {
        let db = MockDatabaseClient::healthy();
        let cache = MockCacheClient::unhealthy();
        let bus = MockEventBus::healthy();

        let report = check_all(
            Some(&db as &dyn DatabaseClient),
            Some(&cache as &dyn CacheClient),
            Some(&bus as &dyn EventBus),
        )
        .await;

        assert_eq!(report.database, BackendStatus::Healthy);
        assert_eq!(report.cache, BackendStatus::Unhealthy);
        assert_eq!(report.event_bus, BackendStatus::Healthy);
        assert!(!report.is_all_healthy());
        assert!(report.has_unhealthy());
        assert_eq!(report.healthy_count(), 2);
    }

    #[tokio::test]
    async fn all_unhealthy() {
        let db = MockDatabaseClient::unhealthy();
        let cache = MockCacheClient::unhealthy();
        let bus = MockEventBus::unhealthy();

        let report = check_all(
            Some(&db as &dyn DatabaseClient),
            Some(&cache as &dyn CacheClient),
            Some(&bus as &dyn EventBus),
        )
        .await;

        assert!(!report.is_all_healthy());
        assert!(report.has_unhealthy());
        assert_eq!(report.healthy_count(), 0);
    }

    #[tokio::test]
    async fn unconfigured_backends_treated_as_ok() {
        let report = check_all(None, None, None).await;

        assert_eq!(report.database, BackendStatus::Unconfigured);
        assert_eq!(report.cache, BackendStatus::Unconfigured);
        assert_eq!(report.event_bus, BackendStatus::Unconfigured);
        assert!(report.is_all_healthy());
        assert!(!report.has_unhealthy());
        assert_eq!(report.healthy_count(), 0);
    }

    #[tokio::test]
    async fn mixed_configured_and_unconfigured() {
        let db = MockDatabaseClient::healthy();

        let report = check_all(
            Some(&db as &dyn DatabaseClient),
            None,
            None,
        )
        .await;

        assert_eq!(report.database, BackendStatus::Healthy);
        assert_eq!(report.cache, BackendStatus::Unconfigured);
        assert_eq!(report.event_bus, BackendStatus::Unconfigured);
        assert!(report.is_all_healthy());
        assert_eq!(report.healthy_count(), 1);
    }

    #[tokio::test]
    async fn check_database_healthy() {
        let db = MockDatabaseClient::healthy();
        let status = check_database(&db).await;
        assert_eq!(status, BackendStatus::Healthy);
    }

    #[tokio::test]
    async fn check_database_unhealthy() {
        let db = MockDatabaseClient::unhealthy();
        let status = check_database(&db).await;
        assert_eq!(status, BackendStatus::Unhealthy);
    }

    #[tokio::test]
    async fn check_cache_healthy() {
        let cache = MockCacheClient::healthy();
        let status = check_cache(&cache).await;
        assert_eq!(status, BackendStatus::Healthy);
    }

    #[tokio::test]
    async fn check_cache_unhealthy() {
        let cache = MockCacheClient::unhealthy();
        let status = check_cache(&cache).await;
        assert_eq!(status, BackendStatus::Unhealthy);
    }

    #[tokio::test]
    async fn check_event_bus_healthy() {
        let bus = MockEventBus::healthy();
        let status = check_event_bus(&bus).await;
        assert_eq!(status, BackendStatus::Healthy);
    }

    #[tokio::test]
    async fn check_event_bus_unhealthy() {
        let bus = MockEventBus::unhealthy();
        let status = check_event_bus(&bus).await;
        assert_eq!(status, BackendStatus::Unhealthy);
    }

    #[test]
    fn all_unconfigured_report() {
        let report = HealthReport::all_unconfigured();
        assert_eq!(report.database, BackendStatus::Unconfigured);
        assert_eq!(report.cache, BackendStatus::Unconfigured);
        assert_eq!(report.event_bus, BackendStatus::Unconfigured);
        assert!(report.is_all_healthy());
    }

    #[test]
    fn health_report_is_debug_and_clone() {
        let report = HealthReport {
            database: BackendStatus::Healthy,
            cache: BackendStatus::Unhealthy,
            event_bus: BackendStatus::Unconfigured,
        };
        let cloned = report.clone();
        assert_eq!(cloned.database, report.database);
        let debug = format!("{report:?}");
        assert!(debug.contains("HealthReport"));
    }

    #[test]
    fn backend_status_equality() {
        assert_eq!(BackendStatus::Healthy, BackendStatus::Healthy);
        assert_ne!(BackendStatus::Healthy, BackendStatus::Unhealthy);
        assert_ne!(BackendStatus::Healthy, BackendStatus::Unconfigured);
        assert_ne!(BackendStatus::Unhealthy, BackendStatus::Unconfigured);
    }
}
