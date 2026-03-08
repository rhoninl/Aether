//! Redis cache client with trait abstraction.
//!
//! Provides a `CacheClient` trait for key-value operations with optional TTL.
//! The real implementation uses `redis::aio::ConnectionManager`; a mock is provided for tests.

use std::time::Duration;

use async_trait::async_trait;

use crate::error::PersistenceError;

/// Abstraction over a key-value cache (Redis-like).
#[async_trait]
pub trait CacheClient: Send + Sync {
    /// Get a value by key. Returns `None` if the key does not exist.
    async fn get(&self, key: &str) -> Result<Option<String>, PersistenceError>;

    /// Set a key to a value with an optional TTL.
    async fn set(
        &self,
        key: &str,
        value: &str,
        ttl: Option<Duration>,
    ) -> Result<(), PersistenceError>;

    /// Delete a key. Returns `true` if the key existed.
    async fn del(&self, key: &str) -> Result<bool, PersistenceError>;

    /// Check if the cache connection is alive.
    async fn is_healthy(&self) -> bool;
}

// ---------------------------------------------------------------------------
// Real implementation (behind "redis-backend" feature)
// ---------------------------------------------------------------------------

#[cfg(feature = "redis-backend")]
mod real {
    use super::*;
    use crate::pool::ConnectionConfig;
    use redis::AsyncCommands;

    /// Redis client backed by `redis::aio::ConnectionManager`.
    pub struct RedisClient {
        manager: redis::aio::ConnectionManager,
    }

    impl RedisClient {
        /// Connect to Redis using the URL from `ConnectionConfig`.
        pub async fn connect(config: &ConnectionConfig) -> Result<Self, PersistenceError> {
            let client = redis::Client::open(config.redis_url.as_str())
                .map_err(|e| PersistenceError::ConnectionFailed(e.to_string()))?;
            let manager = redis::aio::ConnectionManager::new(client)
                .await
                .map_err(|e| PersistenceError::ConnectionFailed(e.to_string()))?;
            Ok(Self { manager })
        }
    }

    #[async_trait]
    impl CacheClient for RedisClient {
        async fn get(&self, key: &str) -> Result<Option<String>, PersistenceError> {
            let mut conn = self.manager.clone();
            let value: Option<String> = conn
                .get(key)
                .await
                .map_err(|e| PersistenceError::QueryFailed(e.to_string()))?;
            Ok(value)
        }

        async fn set(
            &self,
            key: &str,
            value: &str,
            ttl: Option<Duration>,
        ) -> Result<(), PersistenceError> {
            let mut conn = self.manager.clone();
            match ttl {
                Some(duration) => {
                    let secs = duration.as_secs().max(1);
                    conn.set_ex(key, value, secs)
                        .await
                        .map_err(|e| PersistenceError::QueryFailed(e.to_string()))?;
                }
                None => {
                    conn.set(key, value)
                        .await
                        .map_err(|e| PersistenceError::QueryFailed(e.to_string()))?;
                }
            }
            Ok(())
        }

        async fn del(&self, key: &str) -> Result<bool, PersistenceError> {
            let mut conn = self.manager.clone();
            let deleted: i64 = conn
                .del(key)
                .await
                .map_err(|e| PersistenceError::QueryFailed(e.to_string()))?;
            Ok(deleted > 0)
        }

        async fn is_healthy(&self) -> bool {
            let mut conn = self.manager.clone();
            redis::cmd("PING")
                .query_async::<String>(&mut conn)
                .await
                .is_ok()
        }
    }
}

#[cfg(feature = "redis-backend")]
pub use real::RedisClient;

// ---------------------------------------------------------------------------
// Mock implementation (always available)
// ---------------------------------------------------------------------------

use std::collections::HashMap;
use std::sync::Mutex;

/// Mock cache client for unit testing.
///
/// Stores values in an in-memory `HashMap` behind a `Mutex`.
pub struct MockCacheClient {
    healthy: bool,
    store: Mutex<HashMap<String, String>>,
}

impl MockCacheClient {
    /// Create a healthy mock with an empty store.
    pub fn healthy() -> Self {
        Self {
            healthy: true,
            store: Mutex::new(HashMap::new()),
        }
    }

    /// Create an unhealthy mock.
    pub fn unhealthy() -> Self {
        Self {
            healthy: false,
            store: Mutex::new(HashMap::new()),
        }
    }

    /// Return the number of keys currently stored.
    pub fn len(&self) -> usize {
        self.store.lock().unwrap().len()
    }

    /// Check if the store is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[async_trait]
impl CacheClient for MockCacheClient {
    async fn get(&self, key: &str) -> Result<Option<String>, PersistenceError> {
        if !self.healthy {
            return Err(PersistenceError::NotConnected);
        }
        let store = self.store.lock().unwrap();
        Ok(store.get(key).cloned())
    }

    async fn set(
        &self,
        key: &str,
        value: &str,
        _ttl: Option<Duration>,
    ) -> Result<(), PersistenceError> {
        if !self.healthy {
            return Err(PersistenceError::NotConnected);
        }
        let mut store = self.store.lock().unwrap();
        store.insert(key.to_string(), value.to_string());
        Ok(())
    }

    async fn del(&self, key: &str) -> Result<bool, PersistenceError> {
        if !self.healthy {
            return Err(PersistenceError::NotConnected);
        }
        let mut store = self.store.lock().unwrap();
        Ok(store.remove(key).is_some())
    }

    async fn is_healthy(&self) -> bool {
        self.healthy
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_healthy_check() {
        let client = MockCacheClient::healthy();
        assert!(client.is_healthy().await);
    }

    #[tokio::test]
    async fn mock_unhealthy_check() {
        let client = MockCacheClient::unhealthy();
        assert!(!client.is_healthy().await);
    }

    #[tokio::test]
    async fn set_and_get_roundtrip() {
        let client = MockCacheClient::healthy();
        client.set("key1", "value1", None).await.unwrap();
        let val = client.get("key1").await.unwrap();
        assert_eq!(val, Some("value1".to_string()));
    }

    #[tokio::test]
    async fn get_missing_key_returns_none() {
        let client = MockCacheClient::healthy();
        let val = client.get("nonexistent").await.unwrap();
        assert!(val.is_none());
    }

    #[tokio::test]
    async fn del_existing_key_returns_true() {
        let client = MockCacheClient::healthy();
        client.set("key1", "v", None).await.unwrap();
        let deleted = client.del("key1").await.unwrap();
        assert!(deleted);
        let val = client.get("key1").await.unwrap();
        assert!(val.is_none());
    }

    #[tokio::test]
    async fn del_missing_key_returns_false() {
        let client = MockCacheClient::healthy();
        let deleted = client.del("nonexistent").await.unwrap();
        assert!(!deleted);
    }

    #[tokio::test]
    async fn set_with_ttl_stores_value() {
        let client = MockCacheClient::healthy();
        client
            .set("ttl_key", "ttl_val", Some(Duration::from_secs(60)))
            .await
            .unwrap();
        let val = client.get("ttl_key").await.unwrap();
        assert_eq!(val, Some("ttl_val".to_string()));
    }

    #[tokio::test]
    async fn unhealthy_get_returns_error() {
        let client = MockCacheClient::unhealthy();
        let result = client.get("key").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn unhealthy_set_returns_error() {
        let client = MockCacheClient::unhealthy();
        let result = client.set("key", "val", None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn unhealthy_del_returns_error() {
        let client = MockCacheClient::unhealthy();
        let result = client.del("key").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn len_and_is_empty() {
        let client = MockCacheClient::healthy();
        assert!(client.is_empty());
        assert_eq!(client.len(), 0);

        client.set("a", "1", None).await.unwrap();
        assert!(!client.is_empty());
        assert_eq!(client.len(), 1);
    }

    #[tokio::test]
    async fn overwrite_existing_key() {
        let client = MockCacheClient::healthy();
        client.set("key", "v1", None).await.unwrap();
        client.set("key", "v2", None).await.unwrap();
        let val = client.get("key").await.unwrap();
        assert_eq!(val, Some("v2".to_string()));
        assert_eq!(client.len(), 1);
    }

    #[tokio::test]
    async fn trait_object_works() {
        let client: Box<dyn CacheClient> = Box::new(MockCacheClient::healthy());
        client.set("k", "v", None).await.unwrap();
        assert!(client.is_healthy().await);
    }
}
