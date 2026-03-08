//! Session management with refresh token rotation.

use chrono::{DateTime, Duration, Utc};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// A user session tracking refresh token state.
#[derive(Debug, Clone)]
pub struct Session {
    pub id: Uuid,
    pub user_id: Uuid,
    pub refresh_token_hash: String,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub revoked: bool,
}

/// Errors from session operations.
#[derive(Debug)]
pub enum SessionError {
    /// Session not found.
    NotFound,
    /// Session has expired.
    Expired,
    /// Session has been revoked.
    Revoked,
    /// Internal store error.
    StoreError(String),
}

impl std::fmt::Display for SessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionError::NotFound => write!(f, "session not found"),
            SessionError::Expired => write!(f, "session has expired"),
            SessionError::Revoked => write!(f, "session has been revoked"),
            SessionError::StoreError(msg) => write!(f, "session store error: {}", msg),
        }
    }
}

impl std::error::Error for SessionError {}

/// Hashes a refresh token for storage (never store raw tokens).
pub fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

/// Trait for session storage backends.
pub trait SessionStore: Send + Sync {
    /// Creates a new session and returns it.
    fn create(
        &self,
        user_id: Uuid,
        refresh_token: &str,
        ttl_secs: u64,
    ) -> Result<Session, SessionError>;

    /// Finds an active session by refresh token.
    /// Returns `SessionError::NotFound` if no matching session exists.
    /// Returns `SessionError::Expired` if the session has expired.
    /// Returns `SessionError::Revoked` if the session has been revoked.
    fn find_by_refresh_token(&self, refresh_token: &str) -> Result<Session, SessionError>;

    /// Revokes a session by its ID.
    fn revoke(&self, session_id: Uuid) -> Result<(), SessionError>;

    /// Revokes all sessions for a user.
    fn revoke_all_for_user(&self, user_id: Uuid) -> Result<u64, SessionError>;
}

/// In-memory session store for testing and single-node deployments.
#[derive(Debug, Clone)]
pub struct InMemorySessionStore {
    sessions: Arc<Mutex<HashMap<Uuid, Session>>>,
}

impl InMemorySessionStore {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Default for InMemorySessionStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionStore for InMemorySessionStore {
    fn create(
        &self,
        user_id: Uuid,
        refresh_token: &str,
        ttl_secs: u64,
    ) -> Result<Session, SessionError> {
        let session = Session {
            id: Uuid::new_v4(),
            user_id,
            refresh_token_hash: hash_token(refresh_token),
            expires_at: Utc::now() + Duration::seconds(ttl_secs as i64),
            created_at: Utc::now(),
            revoked: false,
        };

        let mut sessions = self
            .sessions
            .lock()
            .map_err(|e| SessionError::StoreError(e.to_string()))?;
        sessions.insert(session.id, session.clone());
        Ok(session)
    }

    fn find_by_refresh_token(&self, refresh_token: &str) -> Result<Session, SessionError> {
        let token_hash = hash_token(refresh_token);
        let sessions = self
            .sessions
            .lock()
            .map_err(|e| SessionError::StoreError(e.to_string()))?;

        let session = sessions
            .values()
            .find(|s| s.refresh_token_hash == token_hash)
            .ok_or(SessionError::NotFound)?;

        if session.revoked {
            return Err(SessionError::Revoked);
        }

        if session.expires_at < Utc::now() {
            return Err(SessionError::Expired);
        }

        Ok(session.clone())
    }

    fn revoke(&self, session_id: Uuid) -> Result<(), SessionError> {
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|e| SessionError::StoreError(e.to_string()))?;

        let session = sessions
            .get_mut(&session_id)
            .ok_or(SessionError::NotFound)?;
        session.revoked = true;
        Ok(())
    }

    fn revoke_all_for_user(&self, user_id: Uuid) -> Result<u64, SessionError> {
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|e| SessionError::StoreError(e.to_string()))?;

        let mut count = 0u64;
        for session in sessions.values_mut() {
            if session.user_id == user_id && !session.revoked {
                session.revoked = true;
                count += 1;
            }
        }
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_token_deterministic() {
        let h1 = hash_token("my-refresh-token");
        let h2 = hash_token("my-refresh-token");
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_hash_token_different_inputs() {
        let h1 = hash_token("token-a");
        let h2 = hash_token("token-b");
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_hash_token_is_hex() {
        let h = hash_token("test");
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(h.len(), 64); // SHA-256 = 32 bytes = 64 hex chars
    }

    #[test]
    fn test_create_session() {
        let store = InMemorySessionStore::new();
        let user_id = Uuid::new_v4();
        let session = store.create(user_id, "refresh-token-abc", 3600).unwrap();

        assert_eq!(session.user_id, user_id);
        assert_eq!(session.refresh_token_hash, hash_token("refresh-token-abc"));
        assert!(!session.revoked);
        assert!(session.expires_at > Utc::now());
    }

    #[test]
    fn test_find_session_by_refresh_token() {
        let store = InMemorySessionStore::new();
        let user_id = Uuid::new_v4();
        let created = store.create(user_id, "my-token", 3600).unwrap();

        let found = store.find_by_refresh_token("my-token").unwrap();
        assert_eq!(found.id, created.id);
        assert_eq!(found.user_id, user_id);
    }

    #[test]
    fn test_find_session_not_found() {
        let store = InMemorySessionStore::new();
        let err = store
            .find_by_refresh_token("nonexistent-token")
            .unwrap_err();
        assert!(matches!(err, SessionError::NotFound));
    }

    #[test]
    fn test_find_expired_session() {
        let store = InMemorySessionStore::new();
        let user_id = Uuid::new_v4();
        // Create with 0 TTL so it expires immediately
        store.create(user_id, "expired-token", 0).unwrap();

        let err = store.find_by_refresh_token("expired-token").unwrap_err();
        assert!(matches!(err, SessionError::Expired));
    }

    #[test]
    fn test_revoke_session() {
        let store = InMemorySessionStore::new();
        let user_id = Uuid::new_v4();
        let session = store.create(user_id, "revoke-me", 3600).unwrap();

        store.revoke(session.id).unwrap();

        let err = store.find_by_refresh_token("revoke-me").unwrap_err();
        assert!(matches!(err, SessionError::Revoked));
    }

    #[test]
    fn test_revoke_nonexistent_session() {
        let store = InMemorySessionStore::new();
        let err = store.revoke(Uuid::new_v4()).unwrap_err();
        assert!(matches!(err, SessionError::NotFound));
    }

    #[test]
    fn test_revoke_all_for_user() {
        let store = InMemorySessionStore::new();
        let user_id = Uuid::new_v4();
        let other_user_id = Uuid::new_v4();

        store.create(user_id, "token-1", 3600).unwrap();
        store.create(user_id, "token-2", 3600).unwrap();
        store.create(other_user_id, "token-3", 3600).unwrap();

        let count = store.revoke_all_for_user(user_id).unwrap();
        assert_eq!(count, 2);

        // User's sessions are revoked
        assert!(matches!(
            store.find_by_refresh_token("token-1").unwrap_err(),
            SessionError::Revoked
        ));
        assert!(matches!(
            store.find_by_refresh_token("token-2").unwrap_err(),
            SessionError::Revoked
        ));

        // Other user's session is unaffected
        assert!(store.find_by_refresh_token("token-3").is_ok());
    }

    #[test]
    fn test_revoke_all_for_user_no_sessions() {
        let store = InMemorySessionStore::new();
        let count = store.revoke_all_for_user(Uuid::new_v4()).unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_multiple_sessions_per_user() {
        let store = InMemorySessionStore::new();
        let user_id = Uuid::new_v4();

        let s1 = store.create(user_id, "device-1-token", 3600).unwrap();
        let s2 = store.create(user_id, "device-2-token", 3600).unwrap();

        assert_ne!(s1.id, s2.id);
        assert!(store.find_by_refresh_token("device-1-token").is_ok());
        assert!(store.find_by_refresh_token("device-2-token").is_ok());
    }

    #[test]
    fn test_session_error_display() {
        assert!(SessionError::NotFound.to_string().contains("not found"));
        assert!(SessionError::Expired.to_string().contains("expired"));
        assert!(SessionError::Revoked.to_string().contains("revoked"));
        assert!(SessionError::StoreError("x".into())
            .to_string()
            .contains("store error"));
    }

    #[test]
    fn test_in_memory_session_store_default() {
        let store = InMemorySessionStore::default();
        let user_id = Uuid::new_v4();
        let session = store.create(user_id, "default-test", 3600).unwrap();
        assert_eq!(session.user_id, user_id);
    }
}
