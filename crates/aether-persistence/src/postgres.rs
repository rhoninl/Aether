//! PostgreSQL database client with trait abstraction.
//!
//! Provides a `DatabaseClient` trait for executing queries against a relational store.
//! The real implementation uses `sqlx::PgPool`; a mock is provided for unit tests.

use async_trait::async_trait;

use crate::error::PersistenceError;

/// Abstraction over a relational database client.
///
/// All operations return `PersistenceError` and implementations must be `Send + Sync`.
#[async_trait]
pub trait DatabaseClient: Send + Sync {
    /// Execute a statement (INSERT, UPDATE, DELETE) and return the number of affected rows.
    async fn execute(&self, query: &str, params: &[&str]) -> Result<u64, PersistenceError>;

    /// Fetch a single optional row as raw bytes (serialized to JSON by the backend).
    async fn fetch_optional(
        &self,
        query: &str,
        params: &[&str],
    ) -> Result<Option<Vec<u8>>, PersistenceError>;

    /// Check if the connection is alive.
    async fn is_healthy(&self) -> bool;
}

// ---------------------------------------------------------------------------
// Real implementation (behind "postgres" feature)
// ---------------------------------------------------------------------------

#[cfg(feature = "postgres")]
mod real {
    use super::*;
    use crate::pool::ConnectionConfig;

    /// PostgreSQL client backed by an `sqlx::PgPool`.
    pub struct PgClient {
        pool: sqlx::PgPool,
    }

    impl PgClient {
        /// Create a new pool from a `ConnectionConfig`.
        pub async fn connect(config: &ConnectionConfig) -> Result<Self, PersistenceError> {
            let pool = sqlx::postgres::PgPoolOptions::new()
                .max_connections(config.pool_size)
                .acquire_timeout(config.connect_timeout)
                .connect(&config.database_url)
                .await
                .map_err(|e| PersistenceError::ConnectionFailed(e.to_string()))?;
            Ok(Self { pool })
        }

        /// Expose the inner pool for advanced usage (migrations, etc.).
        pub fn pool(&self) -> &sqlx::PgPool {
            &self.pool
        }
    }

    #[async_trait]
    impl DatabaseClient for PgClient {
        async fn execute(&self, query: &str, _params: &[&str]) -> Result<u64, PersistenceError> {
            let result = sqlx::query(query)
                .execute(&self.pool)
                .await
                .map_err(|e| PersistenceError::QueryFailed(e.to_string()))?;
            Ok(result.rows_affected())
        }

        async fn fetch_optional(
            &self,
            query: &str,
            _params: &[&str],
        ) -> Result<Option<Vec<u8>>, PersistenceError> {
            use sqlx::Row;
            let row = sqlx::query(query)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| PersistenceError::QueryFailed(e.to_string()))?;
            match row {
                Some(r) => {
                    let bytes: Vec<u8> = r
                        .try_get("data")
                        .unwrap_or_default();
                    Ok(Some(bytes))
                }
                None => Ok(None),
            }
        }

        async fn is_healthy(&self) -> bool {
            sqlx::query("SELECT 1")
                .execute(&self.pool)
                .await
                .is_ok()
        }
    }
}

#[cfg(feature = "postgres")]
pub use real::PgClient;

// ---------------------------------------------------------------------------
// Mock implementation (always available)
// ---------------------------------------------------------------------------

/// Mock database client for unit testing.
///
/// Records calls and returns canned responses.
pub struct MockDatabaseClient {
    healthy: bool,
    execute_result: Result<u64, PersistenceError>,
    fetch_result: Result<Option<Vec<u8>>, PersistenceError>,
}

impl MockDatabaseClient {
    /// Create a mock that reports as healthy and returns success.
    pub fn healthy() -> Self {
        Self {
            healthy: true,
            execute_result: Ok(0),
            fetch_result: Ok(None),
        }
    }

    /// Create a mock that reports as unhealthy.
    pub fn unhealthy() -> Self {
        Self {
            healthy: false,
            execute_result: Err(PersistenceError::NotConnected),
            fetch_result: Err(PersistenceError::NotConnected),
        }
    }

    /// Set the rows-affected value returned by `execute`.
    pub fn with_execute_rows(mut self, rows: u64) -> Self {
        self.execute_result = Ok(rows);
        self
    }

    /// Set the data returned by `fetch_optional`.
    pub fn with_fetch_data(mut self, data: Vec<u8>) -> Self {
        self.fetch_result = Ok(Some(data));
        self
    }
}

#[async_trait]
impl DatabaseClient for MockDatabaseClient {
    async fn execute(&self, _query: &str, _params: &[&str]) -> Result<u64, PersistenceError> {
        match &self.execute_result {
            Ok(rows) => Ok(*rows),
            Err(_) => Err(PersistenceError::NotConnected),
        }
    }

    async fn fetch_optional(
        &self,
        _query: &str,
        _params: &[&str],
    ) -> Result<Option<Vec<u8>>, PersistenceError> {
        match &self.fetch_result {
            Ok(data) => Ok(data.clone()),
            Err(_) => Err(PersistenceError::NotConnected),
        }
    }

    async fn is_healthy(&self) -> bool {
        self.healthy
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_healthy_returns_true() {
        let client = MockDatabaseClient::healthy();
        assert!(client.is_healthy().await);
    }

    #[tokio::test]
    async fn mock_unhealthy_returns_false() {
        let client = MockDatabaseClient::unhealthy();
        assert!(!client.is_healthy().await);
    }

    #[tokio::test]
    async fn mock_execute_returns_configured_rows() {
        let client = MockDatabaseClient::healthy().with_execute_rows(42);
        let rows = client.execute("INSERT INTO test", &[]).await.unwrap();
        assert_eq!(rows, 42);
    }

    #[tokio::test]
    async fn mock_execute_default_returns_zero() {
        let client = MockDatabaseClient::healthy();
        let rows = client.execute("DELETE FROM test", &[]).await.unwrap();
        assert_eq!(rows, 0);
    }

    #[tokio::test]
    async fn mock_fetch_returns_none_by_default() {
        let client = MockDatabaseClient::healthy();
        let result = client.fetch_optional("SELECT 1", &[]).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn mock_fetch_returns_configured_data() {
        let data = b"hello world".to_vec();
        let client = MockDatabaseClient::healthy().with_fetch_data(data.clone());
        let result = client.fetch_optional("SELECT data", &[]).await.unwrap();
        assert_eq!(result, Some(data));
    }

    #[tokio::test]
    async fn mock_unhealthy_execute_returns_error() {
        let client = MockDatabaseClient::unhealthy();
        let result = client.execute("INSERT INTO test", &[]).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn mock_unhealthy_fetch_returns_error() {
        let client = MockDatabaseClient::unhealthy();
        let result = client.fetch_optional("SELECT 1", &[]).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn trait_object_works() {
        let client: Box<dyn DatabaseClient> = Box::new(MockDatabaseClient::healthy());
        assert!(client.is_healthy().await);
    }
}
