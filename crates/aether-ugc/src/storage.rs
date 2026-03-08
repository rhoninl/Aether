//! Trait-based storage abstraction with in-memory implementation.

use std::collections::HashMap;
use std::sync::Mutex;

use async_trait::async_trait;

#[derive(Debug, Clone, PartialEq)]
pub enum StorageError {
    NotFound(String),
    WriteFailed(String),
    DeleteFailed(String),
}

impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StorageError::NotFound(key) => write!(f, "key not found: {key}"),
            StorageError::WriteFailed(msg) => write!(f, "write failed: {msg}"),
            StorageError::DeleteFailed(msg) => write!(f, "delete failed: {msg}"),
        }
    }
}

impl std::error::Error for StorageError {}

#[async_trait]
pub trait AssetStorage: Send + Sync {
    async fn store(&self, key: &str, data: &[u8]) -> Result<(), StorageError>;
    async fn retrieve(&self, key: &str) -> Result<Vec<u8>, StorageError>;
    async fn delete(&self, key: &str) -> Result<(), StorageError>;
    async fn exists(&self, key: &str) -> Result<bool, StorageError>;
}

/// In-memory storage implementation for testing.
#[derive(Debug, Default)]
pub struct InMemoryStorage {
    data: Mutex<HashMap<String, Vec<u8>>>,
}

impl InMemoryStorage {
    pub fn new() -> Self {
        Self {
            data: Mutex::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl AssetStorage for InMemoryStorage {
    async fn store(&self, key: &str, data: &[u8]) -> Result<(), StorageError> {
        let mut map = self
            .data
            .lock()
            .map_err(|e| StorageError::WriteFailed(e.to_string()))?;
        map.insert(key.to_string(), data.to_vec());
        Ok(())
    }

    async fn retrieve(&self, key: &str) -> Result<Vec<u8>, StorageError> {
        let map = self
            .data
            .lock()
            .map_err(|e| StorageError::NotFound(e.to_string()))?;
        map.get(key)
            .cloned()
            .ok_or_else(|| StorageError::NotFound(key.to_string()))
    }

    async fn delete(&self, key: &str) -> Result<(), StorageError> {
        let mut map = self
            .data
            .lock()
            .map_err(|e| StorageError::DeleteFailed(e.to_string()))?;
        map.remove(key)
            .map(|_| ())
            .ok_or_else(|| StorageError::NotFound(key.to_string()))
    }

    async fn exists(&self, key: &str) -> Result<bool, StorageError> {
        let map = self
            .data
            .lock()
            .map_err(|e| StorageError::NotFound(e.to_string()))?;
        Ok(map.contains_key(key))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn store_and_retrieve_round_trip() {
        let storage = InMemoryStorage::new();
        let data = b"hello world";
        storage.store("key1", data).await.unwrap();
        let retrieved = storage.retrieve("key1").await.unwrap();
        assert_eq!(retrieved, data);
    }

    #[tokio::test]
    async fn retrieve_missing_returns_not_found() {
        let storage = InMemoryStorage::new();
        let err = storage.retrieve("nonexistent").await.unwrap_err();
        assert!(matches!(err, StorageError::NotFound(_)));
    }

    #[tokio::test]
    async fn delete_removes_key() {
        let storage = InMemoryStorage::new();
        storage.store("key1", b"data").await.unwrap();
        storage.delete("key1").await.unwrap();
        let err = storage.retrieve("key1").await.unwrap_err();
        assert!(matches!(err, StorageError::NotFound(_)));
    }

    #[tokio::test]
    async fn delete_missing_returns_not_found() {
        let storage = InMemoryStorage::new();
        let err = storage.delete("nonexistent").await.unwrap_err();
        assert!(matches!(err, StorageError::NotFound(_)));
    }

    #[tokio::test]
    async fn exists_returns_true_for_stored() {
        let storage = InMemoryStorage::new();
        storage.store("key1", b"data").await.unwrap();
        assert!(storage.exists("key1").await.unwrap());
    }

    #[tokio::test]
    async fn exists_returns_false_for_missing() {
        let storage = InMemoryStorage::new();
        assert!(!storage.exists("nope").await.unwrap());
    }

    #[tokio::test]
    async fn overwrite_existing_key() {
        let storage = InMemoryStorage::new();
        storage.store("key1", b"first").await.unwrap();
        storage.store("key1", b"second").await.unwrap();
        let retrieved = storage.retrieve("key1").await.unwrap();
        assert_eq!(retrieved, b"second");
    }

    #[tokio::test]
    async fn store_empty_data() {
        let storage = InMemoryStorage::new();
        storage.store("empty", b"").await.unwrap();
        let retrieved = storage.retrieve("empty").await.unwrap();
        assert!(retrieved.is_empty());
    }

    #[tokio::test]
    async fn multiple_keys_independent() {
        let storage = InMemoryStorage::new();
        storage.store("a", b"aaa").await.unwrap();
        storage.store("b", b"bbb").await.unwrap();
        assert_eq!(storage.retrieve("a").await.unwrap(), b"aaa");
        assert_eq!(storage.retrieve("b").await.unwrap(), b"bbb");
        storage.delete("a").await.unwrap();
        assert!(!storage.exists("a").await.unwrap());
        assert!(storage.exists("b").await.unwrap());
    }
}
