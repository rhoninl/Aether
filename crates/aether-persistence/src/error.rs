//! Unified error type for all persistence backends.

use std::fmt;

/// Errors returned by persistence backend operations.
#[derive(Debug)]
pub enum PersistenceError {
    /// Failed to establish or maintain a connection.
    ConnectionFailed(String),
    /// A query or command failed.
    QueryFailed(String),
    /// Operation timed out.
    Timeout,
    /// Serialization or deserialization error.
    SerializationError(String),
    /// Database migration error.
    MigrationError(String),
    /// Backend is not connected.
    NotConnected,
}

impl fmt::Display for PersistenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PersistenceError::ConnectionFailed(msg) => write!(f, "connection failed: {msg}"),
            PersistenceError::QueryFailed(msg) => write!(f, "query failed: {msg}"),
            PersistenceError::Timeout => write!(f, "operation timed out"),
            PersistenceError::SerializationError(msg) => write!(f, "serialization error: {msg}"),
            PersistenceError::MigrationError(msg) => write!(f, "migration error: {msg}"),
            PersistenceError::NotConnected => write!(f, "not connected"),
        }
    }
}

impl std::error::Error for PersistenceError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_formats_correctly() {
        let err = PersistenceError::ConnectionFailed("refused".into());
        assert_eq!(err.to_string(), "connection failed: refused");

        let err = PersistenceError::QueryFailed("syntax error".into());
        assert_eq!(err.to_string(), "query failed: syntax error");

        let err = PersistenceError::Timeout;
        assert_eq!(err.to_string(), "operation timed out");

        let err = PersistenceError::SerializationError("bad json".into());
        assert_eq!(err.to_string(), "serialization error: bad json");

        let err = PersistenceError::MigrationError("duplicate version".into());
        assert_eq!(err.to_string(), "migration error: duplicate version");

        let err = PersistenceError::NotConnected;
        assert_eq!(err.to_string(), "not connected");
    }

    #[test]
    fn error_is_debug() {
        let err = PersistenceError::Timeout;
        let debug = format!("{err:?}");
        assert!(debug.contains("Timeout"));
    }

    #[test]
    fn implements_std_error() {
        let err: Box<dyn std::error::Error> =
            Box::new(PersistenceError::ConnectionFailed("test".into()));
        assert!(err.to_string().contains("connection failed"));
    }
}
