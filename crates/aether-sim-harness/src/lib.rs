//! Ghost-world simulation harness.
//!
//! The harness scores agent mutations *before* they commit: given a
//! world snapshot and a scenario (typed sequence of inputs), it runs a
//! fast deterministic replay, scores the resulting state against VR
//! comfort and MMO coherence heuristics, and returns a [`SimReport`]
//! with a [`Verdict`]. Failing verdicts carry a minimal [`RepairPatch`]
//! when one can be synthesized.
//!
//! # Example
//!
//! ```
//! use aether_sim_harness::{DefaultHarness, Harness, Scenario, WorldSnapshot};
//! let mut h = DefaultHarness::new();
//! let scenario = Scenario::new("demo").push_ticks(60);
//! let report = h.run(WorldSnapshot::Empty, scenario);
//! assert!(report.verdict.is_pass());
//! ```

pub mod error;
pub mod harness;
pub mod replay;
pub mod scenario;
pub mod scorer;
pub mod telemetry;
pub mod verdict;

pub use error::{HarnessError, HarnessResult};
pub use harness::{run_scenario, DefaultHarness, Harness, SimReport};
pub use replay::{Replay, ReplayOutput, SimState};
pub use scenario::{AgentAction, Input, NetEvent, Scenario, WorldSnapshot};
pub use scorer::{CoherenceReason, CoherenceScore, ComfortReason, ComfortScore};
pub use telemetry::{DurationNs, Event, Telemetry};
pub use verdict::{Cid, FailureReason, RepairOp, RepairPatch, Verdict, Warning};
