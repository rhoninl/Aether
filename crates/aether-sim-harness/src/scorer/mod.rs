//! Pluggable scoring of a [`crate::replay::SimState`].
//!
//! Each scorer returns a score in `[0.0, 1.0]` (higher = healthier) plus
//! a list of machine-readable reasons for any deductions. The harness
//! combines scores into a verdict using per-scorer pass thresholds.

pub mod mmo_coherence;
pub mod vr_comfort;

pub use mmo_coherence::{CoherenceReason, CoherenceScore};
pub use vr_comfort::{ComfortReason, ComfortScore};
