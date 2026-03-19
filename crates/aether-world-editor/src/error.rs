//! Error types for the world editor.

use crate::mode::WorldMode;

/// Errors arising from mode transitions.
#[derive(Debug, thiserror::Error)]
pub enum ModeError {
    #[error("invalid mode transition from {from:?} to {to:?}")]
    InvalidTransition { from: WorldMode, to: WorldMode },

    #[error("operation requires editor mode")]
    NotInEditorMode,
}

/// Errors arising from version management.
#[derive(Debug, thiserror::Error)]
pub enum VersionError {
    #[error("invalid semver format: {0}")]
    InvalidSemver(String),

    #[error("version {0} already exists")]
    DuplicateVersion(String),

    #[error("version {0} not found")]
    NotFound(String),

    #[error("serialization error: {0}")]
    Serialization(String),
}

/// Errors arising from project I/O.
#[derive(Debug, thiserror::Error)]
pub enum ProjectError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("manifest error: {0}")]
    Manifest(String),

    #[error("version error: {0}")]
    Version(#[from] VersionError),

    #[error("project directory already exists: {0}")]
    AlreadyExists(String),

    #[error("not a valid world project: {0}")]
    InvalidProject(String),
}
