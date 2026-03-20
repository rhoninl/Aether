//! Content moderation pipeline for automated scanning, human review, and report handling.
//!
//! This crate provides:
//! - **Scanner**: Trait-based content scanning with pluggable implementations
//! - **Decision engine**: Configurable auto-approve/auto-flag rules
//! - **Review queue**: Priority-ordered human review with claim/decide workflow
//! - **Report system**: User-submitted reports with aggregation and escalation
//! - **WASM analysis**: Static analysis of WASM bytecode for malicious patterns
//! - **Status tracking**: State machine for moderation lifecycle
//! - **Severity classification**: Graduated severity with enforcement actions
//! - **Content ratings**: Age-appropriate content classification

pub mod decision;
pub mod mesh;
pub mod queue;
pub mod ratings;
pub mod report;
pub mod reports;
pub mod scanner;
pub mod severity;
pub mod status;
pub mod wasm_scan;

// Re-export key types for convenience.
pub use decision::{Decision, DecisionConfig, DecisionEngine, DecisionRule};
pub use mesh::{GeometryRule, MeshFinding, MeshScanner};
pub use queue::{QueueError, ReviewAction, ReviewItem, ReviewPriority, ReviewQueue, ReviewState};
pub use ratings::{RatingCategory, RatingDecision};
pub use report::{Report, ReportAggregator, ReportCategory, ReportSummary};
pub use reports::{ModerationReport, ModerationSeverity, ReportCase};
pub use scanner::{
    AggregatedScanResult, ContentFlag, ContentItem, ContentScanner, ContentType, FlagCategory,
    ModerationAction, ScanResult, ScannerPipeline,
};
pub use severity::{ContentSeverity, EnforcementAction};
pub use status::{InvalidTransition, ModerationStatus};
pub use wasm_scan::{
    WasmAnalysisResult, WasmAnalyzer, WasmPattern, WasmViolation, WasmViolationKind,
};
