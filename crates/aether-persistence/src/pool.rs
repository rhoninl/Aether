//! Connection configuration parsed from environment variables.
//!
//! All configurable values come from environment variables with sensible defaults.

use std::time::Duration;

/// Default PostgreSQL connection string.
pub const DEFAULT_DATABASE_URL: &str = "postgres://localhost/aether";

/// Default Redis connection string.
pub const DEFAULT_REDIS_URL: &str = "redis://localhost:6379";

/// Default NATS server URL.
pub const DEFAULT_NATS_URL: &str = "nats://localhost:4222";

/// Default connection pool size.
pub const DEFAULT_POOL_SIZE: u32 = 10;

/// Default connection timeout in seconds.
pub const DEFAULT_CONNECT_TIMEOUT_SECS: u64 = 5;

/// Environment variable name for the PostgreSQL URL.
pub const ENV_DATABASE_URL: &str = "DATABASE_URL";

/// Environment variable name for the Redis URL.
pub const ENV_REDIS_URL: &str = "REDIS_URL";

/// Environment variable name for the NATS URL.
pub const ENV_NATS_URL: &str = "NATS_URL";

/// Environment variable name for connection pool size.
pub const ENV_DB_POOL_SIZE: &str = "DB_POOL_SIZE";

/// Environment variable name for connection timeout.
pub const ENV_DB_CONNECT_TIMEOUT_SECS: &str = "DB_CONNECT_TIMEOUT_SECS";

/// Configuration for connecting to all persistence backends.
#[derive(Debug, Clone)]
pub struct ConnectionConfig {
    pub database_url: String,
    pub redis_url: String,
    pub nats_url: String,
    pub pool_size: u32,
    pub connect_timeout: Duration,
}

impl ConnectionConfig {
    /// Build configuration from environment variables, falling back to defaults.
    pub fn from_env() -> Self {
        let database_url =
            std::env::var(ENV_DATABASE_URL).unwrap_or_else(|_| DEFAULT_DATABASE_URL.to_string());
        let redis_url =
            std::env::var(ENV_REDIS_URL).unwrap_or_else(|_| DEFAULT_REDIS_URL.to_string());
        let nats_url = std::env::var(ENV_NATS_URL).unwrap_or_else(|_| DEFAULT_NATS_URL.to_string());
        let pool_size = std::env::var(ENV_DB_POOL_SIZE)
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(DEFAULT_POOL_SIZE);
        let connect_timeout_secs = std::env::var(ENV_DB_CONNECT_TIMEOUT_SECS)
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(DEFAULT_CONNECT_TIMEOUT_SECS);

        Self {
            database_url,
            redis_url,
            nats_url,
            pool_size,
            connect_timeout: Duration::from_secs(connect_timeout_secs),
        }
    }

    /// Build configuration from explicit values (useful in tests).
    pub fn new(
        database_url: impl Into<String>,
        redis_url: impl Into<String>,
        nats_url: impl Into<String>,
        pool_size: u32,
        connect_timeout: Duration,
    ) -> Self {
        Self {
            database_url: database_url.into(),
            redis_url: redis_url.into(),
            nats_url: nats_url.into(),
            pool_size,
            connect_timeout,
        }
    }
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            database_url: DEFAULT_DATABASE_URL.to_string(),
            redis_url: DEFAULT_REDIS_URL.to_string(),
            nats_url: DEFAULT_NATS_URL.to_string(),
            pool_size: DEFAULT_POOL_SIZE,
            connect_timeout: Duration::from_secs(DEFAULT_CONNECT_TIMEOUT_SECS),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_expected_values() {
        let cfg = ConnectionConfig::default();
        assert_eq!(cfg.database_url, DEFAULT_DATABASE_URL);
        assert_eq!(cfg.redis_url, DEFAULT_REDIS_URL);
        assert_eq!(cfg.nats_url, DEFAULT_NATS_URL);
        assert_eq!(cfg.pool_size, DEFAULT_POOL_SIZE);
        assert_eq!(
            cfg.connect_timeout,
            Duration::from_secs(DEFAULT_CONNECT_TIMEOUT_SECS)
        );
    }

    #[test]
    fn new_overrides_all_fields() {
        let cfg = ConnectionConfig::new(
            "postgres://custom/db",
            "redis://custom:1234",
            "nats://custom:5678",
            20,
            Duration::from_secs(30),
        );
        assert_eq!(cfg.database_url, "postgres://custom/db");
        assert_eq!(cfg.redis_url, "redis://custom:1234");
        assert_eq!(cfg.nats_url, "nats://custom:5678");
        assert_eq!(cfg.pool_size, 20);
        assert_eq!(cfg.connect_timeout, Duration::from_secs(30));
    }

    #[test]
    fn from_env_uses_defaults_when_vars_unset() {
        // Clear any env vars that might be set in CI
        std::env::remove_var(ENV_DATABASE_URL);
        std::env::remove_var(ENV_REDIS_URL);
        std::env::remove_var(ENV_NATS_URL);
        std::env::remove_var(ENV_DB_POOL_SIZE);
        std::env::remove_var(ENV_DB_CONNECT_TIMEOUT_SECS);

        let cfg = ConnectionConfig::from_env();
        assert_eq!(cfg.database_url, DEFAULT_DATABASE_URL);
        assert_eq!(cfg.redis_url, DEFAULT_REDIS_URL);
        assert_eq!(cfg.nats_url, DEFAULT_NATS_URL);
        assert_eq!(cfg.pool_size, DEFAULT_POOL_SIZE);
    }

    #[test]
    fn pool_size_ignores_non_numeric_env() {
        std::env::set_var(ENV_DB_POOL_SIZE, "not_a_number");
        let cfg = ConnectionConfig::from_env();
        assert_eq!(cfg.pool_size, DEFAULT_POOL_SIZE);
        std::env::remove_var(ENV_DB_POOL_SIZE);
    }

    #[test]
    fn connect_timeout_ignores_non_numeric_env() {
        std::env::set_var(ENV_DB_CONNECT_TIMEOUT_SECS, "abc");
        let cfg = ConnectionConfig::from_env();
        assert_eq!(
            cfg.connect_timeout,
            Duration::from_secs(DEFAULT_CONNECT_TIMEOUT_SECS)
        );
        std::env::remove_var(ENV_DB_CONNECT_TIMEOUT_SECS);
    }

    #[test]
    fn config_is_clone_and_debug() {
        let cfg = ConnectionConfig::default();
        let cloned = cfg.clone();
        assert_eq!(cloned.database_url, cfg.database_url);
        let debug = format!("{cfg:?}");
        assert!(debug.contains("ConnectionConfig"));
    }
}
