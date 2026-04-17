use thiserror::Error;

/// Errors returned by canonical (de)serialization at a crate boundary.
///
/// Matches the shape of `aether_schemas::SchemaError` so call sites can
/// migrate to the real crate without edits.
#[derive(Debug, Error)]
pub enum SchemaError {
    #[error("canonical encode failed: {0}")]
    Encode(String),
    #[error("canonical decode failed: {0}")]
    Decode(String),
    #[error("unknown schema version: {0}")]
    UnknownVersion(u16),
    #[error("content-id mismatch: expected {expected}, got {actual}")]
    CidMismatch { expected: String, actual: String },
    #[error("required field missing: {0}")]
    MissingField(&'static str),
}
