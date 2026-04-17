//! Error types for the simulation harness.

use thiserror::Error;

/// All errors surfaced by the simulation harness.
#[derive(Debug, Error)]
pub enum HarnessError {
    /// The scenario file could not be parsed.
    #[error("invalid scenario: {0}")]
    InvalidScenario(String),

    /// The scenario was otherwise well-formed but violated a harness invariant.
    #[error("scenario violation: {0}")]
    ScenarioViolation(String),

    /// IO failure when loading or saving scenarios / expected reports.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON de/serialization failure.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    /// YAML de/serialization failure.
    #[error("yaml error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    /// The repair-patch synthesizer could not produce a minimal patch.
    #[error("cannot synthesize repair patch: {0}")]
    RepairSynthesis(String),
}

pub type HarnessResult<T> = Result<T, HarnessError>;
