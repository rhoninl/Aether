//! Schema errors.
//!
//! Every error carries a JSON pointer (RFC 6901) to the offending field and a
//! `suggested_fix` string that an AI agent can consume verbatim. This is the
//! single most important ergonomic for agent-authored worlds: if the agent
//! cannot see *where* it went wrong, it will blindly regenerate the whole
//! document.

use thiserror::Error;

/// Convenience alias.
pub type SchemaResult<T> = Result<T, SchemaError>;

/// Canonical schema error.
///
/// Each variant includes:
/// - `pointer`: an RFC 6901 JSON pointer (e.g. `/chunks/3/coord/x`). An empty
///   or `/` pointer means the root document.
/// - `message`: the raw underlying failure from the parser or serializer.
/// - `suggested_fix`: a concise, actionable instruction.
#[derive(Debug, Error)]
pub enum SchemaError {
    #[error("parse error at {pointer}: {message} (fix: {suggested_fix})")]
    Parse {
        pointer: String,
        message: String,
        suggested_fix: String,
    },

    #[error("serialize error at {pointer}: {message} (fix: {suggested_fix})")]
    Serialize {
        pointer: String,
        message: String,
        suggested_fix: String,
    },

    #[error("validation failed at {pointer}: {message} (fix: {suggested_fix})")]
    Validation {
        pointer: String,
        message: String,
        suggested_fix: String,
    },

    #[error("unsupported schema version {found}; expected one of {expected:?} (fix: {suggested_fix})")]
    UnsupportedVersion {
        found: u32,
        expected: Vec<u32>,
        suggested_fix: String,
    },

    #[error(
        "migration from v{from} to v{to} failed at {pointer}: {message} (fix: {suggested_fix})"
    )]
    Migration {
        from: u32,
        to: u32,
        pointer: String,
        message: String,
        suggested_fix: String,
    },

    #[error("content address mismatch at {pointer}: expected {expected}, got {actual} (fix: {suggested_fix})")]
    CidMismatch {
        pointer: String,
        expected: String,
        actual: String,
        suggested_fix: String,
    },
}

impl SchemaError {
    /// Build a [`SchemaError::Validation`] with a helper constructor.
    pub fn validation(
        pointer: impl Into<String>,
        message: impl Into<String>,
        suggested_fix: impl Into<String>,
    ) -> Self {
        SchemaError::Validation {
            pointer: pointer.into(),
            message: message.into(),
            suggested_fix: suggested_fix.into(),
        }
    }

    /// Return the JSON pointer associated with this error, or `/` at root.
    pub fn pointer(&self) -> &str {
        match self {
            SchemaError::Parse { pointer, .. }
            | SchemaError::Serialize { pointer, .. }
            | SchemaError::Validation { pointer, .. }
            | SchemaError::Migration { pointer, .. }
            | SchemaError::CidMismatch { pointer, .. } => pointer,
            SchemaError::UnsupportedVersion { .. } => "/schema_version",
        }
    }

    /// Agent-consumable fix string.
    pub fn suggested_fix(&self) -> &str {
        match self {
            SchemaError::Parse { suggested_fix, .. }
            | SchemaError::Serialize { suggested_fix, .. }
            | SchemaError::Validation { suggested_fix, .. }
            | SchemaError::UnsupportedVersion { suggested_fix, .. }
            | SchemaError::Migration { suggested_fix, .. }
            | SchemaError::CidMismatch { suggested_fix, .. } => suggested_fix,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation_error_carries_pointer_and_fix() {
        let e = SchemaError::validation("/chunks/0", "bad chunk", "set LOD in [0,4]");
        assert_eq!(e.pointer(), "/chunks/0");
        assert_eq!(e.suggested_fix(), "set LOD in [0,4]");
        assert!(e.to_string().contains("set LOD in [0,4]"));
    }

    #[test]
    fn unsupported_version_uses_schema_version_pointer() {
        let e = SchemaError::UnsupportedVersion {
            found: 99,
            expected: vec![1],
            suggested_fix: "migrate document to v1".into(),
        };
        assert_eq!(e.pointer(), "/schema_version");
    }
}
