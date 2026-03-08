//! Account deletion pipeline with step-based state machine.
//!
//! Implements GDPR Article 17 (Right to Erasure) through a multi-step
//! deletion process that handles legal holds, pseudonymization, and
//! data removal.

use serde::{Deserialize, Serialize};

/// The scope of data to delete.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeleteScope {
    Profile,
    Social,
    Chat,
    Telemetry,
    Economy,
    All,
}

/// Legacy legal hold status on a deletion request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LegalHold {
    Active { reason: String, case_id: String },
    Expired,
    None,
}

/// A request to delete user data.
#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteRequest {
    pub request_id: String,
    pub user_id: u64,
    pub scope: Vec<DeleteScope>,
    pub legal_hold: LegalHold,
}

/// The status of a deletion pipeline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeletionStatus {
    Requested,
    OnHold { reason: String },
    InProgress { current_step: usize },
    Completed,
    Failed { error: String },
}

/// Individual steps in the deletion pipeline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeletionStep {
    ExportUserData,
    PseudonymizeLedger,
    DeleteProfile,
    DeleteSocialData,
    DeleteSessionData,
    ArchiveDeletionSalt,
}

impl DeletionStep {
    /// Returns the default ordered sequence of deletion steps.
    pub fn default_sequence() -> Vec<DeletionStep> {
        vec![
            DeletionStep::ExportUserData,
            DeletionStep::PseudonymizeLedger,
            DeletionStep::DeleteProfile,
            DeletionStep::DeleteSocialData,
            DeletionStep::DeleteSessionData,
            DeletionStep::ArchiveDeletionSalt,
        ]
    }
}

/// Result of executing a single deletion step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepResult {
    /// Step completed successfully.
    Ok,
    /// Step failed with an error message.
    Error(String),
}

/// A trait for executing individual deletion steps.
///
/// Implementations provide the actual data deletion logic for each step.
pub trait StepExecutor {
    fn execute(&self, step: &DeletionStep, user_id: u64) -> StepResult;
}

/// A no-op executor that succeeds on every step. Useful for testing.
pub struct NoOpExecutor;

impl StepExecutor for NoOpExecutor {
    fn execute(&self, _step: &DeletionStep, _user_id: u64) -> StepResult {
        StepResult::Ok
    }
}

/// An executor that fails on a specific step. Useful for testing error paths.
pub struct FailOnStepExecutor {
    pub fail_on: DeletionStep,
    pub error_msg: String,
}

impl StepExecutor for FailOnStepExecutor {
    fn execute(&self, step: &DeletionStep, _user_id: u64) -> StepResult {
        if *step == self.fail_on {
            StepResult::Error(self.error_msg.clone())
        } else {
            StepResult::Ok
        }
    }
}

/// The deletion pipeline manages the state machine for deleting user data.
#[derive(Debug)]
pub struct DeletionPipeline {
    pub request: DeleteRequest,
    pub status: DeletionStatus,
    pub steps: Vec<DeletionStep>,
    pub completed_steps: Vec<DeletionStep>,
}

impl DeletionPipeline {
    /// Create a new deletion pipeline from a request.
    pub fn new(request: DeleteRequest) -> Self {
        let steps = DeletionStep::default_sequence();
        Self {
            request,
            status: DeletionStatus::Requested,
            steps,
            completed_steps: Vec::new(),
        }
    }

    /// Create a pipeline with custom steps.
    pub fn with_steps(request: DeleteRequest, steps: Vec<DeletionStep>) -> Self {
        Self {
            request,
            status: DeletionStatus::Requested,
            steps,
            completed_steps: Vec::new(),
        }
    }

    /// Place a legal hold, pausing the deletion.
    ///
    /// Can only place a hold when status is Requested or InProgress.
    pub fn place_hold(&mut self, reason: String) -> Result<(), DeletionError> {
        match &self.status {
            DeletionStatus::Requested | DeletionStatus::InProgress { .. } => {
                self.status = DeletionStatus::OnHold { reason };
                Ok(())
            }
            DeletionStatus::OnHold { .. } => Err(DeletionError::AlreadyOnHold),
            DeletionStatus::Completed => Err(DeletionError::AlreadyCompleted),
            DeletionStatus::Failed { .. } => Err(DeletionError::AlreadyFailed),
        }
    }

    /// Release a legal hold, allowing deletion to resume.
    pub fn release_hold(&mut self) -> Result<(), DeletionError> {
        match &self.status {
            DeletionStatus::OnHold { .. } => {
                if self.completed_steps.is_empty() {
                    self.status = DeletionStatus::Requested;
                } else {
                    self.status = DeletionStatus::InProgress {
                        current_step: self.completed_steps.len(),
                    };
                }
                Ok(())
            }
            _ => Err(DeletionError::NotOnHold),
        }
    }

    /// Advance the pipeline by executing the next step.
    ///
    /// Returns the resulting status after the step executes.
    pub fn advance(
        &mut self,
        executor: &dyn StepExecutor,
    ) -> Result<DeletionStatus, DeletionError> {
        match &self.status {
            DeletionStatus::OnHold { .. } => {
                return Err(DeletionError::OnHoldCannotAdvance)
            }
            DeletionStatus::Completed => return Err(DeletionError::AlreadyCompleted),
            DeletionStatus::Failed { .. } => return Err(DeletionError::AlreadyFailed),
            DeletionStatus::Requested | DeletionStatus::InProgress { .. } => {}
        }

        let step_index = self.completed_steps.len();
        if step_index >= self.steps.len() {
            self.status = DeletionStatus::Completed;
            return Ok(self.status.clone());
        }

        let step = &self.steps[step_index];
        self.status = DeletionStatus::InProgress {
            current_step: step_index,
        };

        match executor.execute(step, self.request.user_id) {
            StepResult::Ok => {
                self.completed_steps.push(step.clone());
                if self.completed_steps.len() == self.steps.len() {
                    self.status = DeletionStatus::Completed;
                } else {
                    self.status = DeletionStatus::InProgress {
                        current_step: self.completed_steps.len(),
                    };
                }
                Ok(self.status.clone())
            }
            StepResult::Error(err) => {
                self.status = DeletionStatus::Failed {
                    error: err.clone(),
                };
                Ok(self.status.clone())
            }
        }
    }

    /// Run the entire pipeline to completion.
    pub fn run_all(
        &mut self,
        executor: &dyn StepExecutor,
    ) -> Result<DeletionStatus, DeletionError> {
        loop {
            match &self.status {
                DeletionStatus::Completed | DeletionStatus::Failed { .. } => {
                    return Ok(self.status.clone());
                }
                DeletionStatus::OnHold { .. } => {
                    return Err(DeletionError::OnHoldCannotAdvance);
                }
                _ => {
                    self.advance(executor)?;
                }
            }
        }
    }
}

/// Errors from deletion pipeline operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeletionError {
    AlreadyOnHold,
    AlreadyCompleted,
    AlreadyFailed,
    NotOnHold,
    OnHoldCannotAdvance,
}

impl std::fmt::Display for DeletionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeletionError::AlreadyOnHold => write!(f, "pipeline is already on hold"),
            DeletionError::AlreadyCompleted => {
                write!(f, "pipeline is already completed")
            }
            DeletionError::AlreadyFailed => write!(f, "pipeline has already failed"),
            DeletionError::NotOnHold => write!(f, "pipeline is not on hold"),
            DeletionError::OnHoldCannotAdvance => {
                write!(f, "cannot advance pipeline while on hold")
            }
        }
    }
}

impl std::error::Error for DeletionError {}

/// A completed deletion record for audit purposes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileDeletion {
    pub user_id: u64,
    pub scope: Vec<DeleteScope>,
    pub started_ms: u64,
    pub requested_by: u64,
    pub status: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_request(user_id: u64) -> DeleteRequest {
        DeleteRequest {
            request_id: "req-001".into(),
            user_id,
            scope: vec![DeleteScope::All],
            legal_hold: LegalHold::None,
        }
    }

    #[test]
    fn new_pipeline_starts_as_requested() {
        let pipeline = DeletionPipeline::new(make_request(42));
        assert_eq!(pipeline.status, DeletionStatus::Requested);
        assert!(pipeline.completed_steps.is_empty());
        assert_eq!(pipeline.steps.len(), 6);
    }

    #[test]
    fn advance_moves_through_steps() {
        let mut pipeline = DeletionPipeline::new(make_request(42));
        let executor = NoOpExecutor;

        let status = pipeline.advance(&executor).unwrap();
        assert_eq!(status, DeletionStatus::InProgress { current_step: 1 });
        assert_eq!(pipeline.completed_steps.len(), 1);
        assert_eq!(pipeline.completed_steps[0], DeletionStep::ExportUserData);
    }

    #[test]
    fn advance_completes_after_all_steps() {
        let mut pipeline = DeletionPipeline::new(make_request(42));
        let executor = NoOpExecutor;

        for _ in 0..6 {
            pipeline.advance(&executor).unwrap();
        }
        assert_eq!(pipeline.status, DeletionStatus::Completed);
        assert_eq!(pipeline.completed_steps.len(), 6);
    }

    #[test]
    fn advance_on_completed_pipeline_fails() {
        let mut pipeline = DeletionPipeline::new(make_request(42));
        let executor = NoOpExecutor;
        pipeline.run_all(&executor).unwrap();

        let result = pipeline.advance(&executor);
        assert_eq!(result, Err(DeletionError::AlreadyCompleted));
    }

    #[test]
    fn step_failure_sets_failed_status() {
        let mut pipeline = DeletionPipeline::new(make_request(42));
        let executor = FailOnStepExecutor {
            fail_on: DeletionStep::ExportUserData,
            error_msg: "export service unavailable".into(),
        };

        let status = pipeline.advance(&executor).unwrap();
        assert_eq!(
            status,
            DeletionStatus::Failed {
                error: "export service unavailable".into()
            }
        );
    }

    #[test]
    fn advance_on_failed_pipeline_errors() {
        let mut pipeline = DeletionPipeline::new(make_request(42));
        let executor = FailOnStepExecutor {
            fail_on: DeletionStep::ExportUserData,
            error_msg: "fail".into(),
        };
        pipeline.advance(&executor).unwrap();

        let result = pipeline.advance(&NoOpExecutor);
        assert_eq!(result, Err(DeletionError::AlreadyFailed));
    }

    #[test]
    fn place_hold_pauses_pipeline() {
        let mut pipeline = DeletionPipeline::new(make_request(42));
        pipeline.place_hold("investigation".into()).unwrap();
        assert_eq!(
            pipeline.status,
            DeletionStatus::OnHold {
                reason: "investigation".into()
            }
        );
    }

    #[test]
    fn cannot_advance_while_on_hold() {
        let mut pipeline = DeletionPipeline::new(make_request(42));
        pipeline.place_hold("investigation".into()).unwrap();

        let result = pipeline.advance(&NoOpExecutor);
        assert_eq!(result, Err(DeletionError::OnHoldCannotAdvance));
    }

    #[test]
    fn release_hold_resumes_from_requested() {
        let mut pipeline = DeletionPipeline::new(make_request(42));
        pipeline.place_hold("reason".into()).unwrap();
        pipeline.release_hold().unwrap();
        assert_eq!(pipeline.status, DeletionStatus::Requested);
    }

    #[test]
    fn release_hold_resumes_from_in_progress() {
        let mut pipeline = DeletionPipeline::new(make_request(42));
        pipeline.advance(&NoOpExecutor).unwrap(); // complete step 0
        pipeline.place_hold("reason".into()).unwrap();
        pipeline.release_hold().unwrap();
        assert_eq!(
            pipeline.status,
            DeletionStatus::InProgress { current_step: 1 }
        );
    }

    #[test]
    fn double_hold_is_rejected() {
        let mut pipeline = DeletionPipeline::new(make_request(42));
        pipeline.place_hold("reason1".into()).unwrap();
        let result = pipeline.place_hold("reason2".into());
        assert_eq!(result, Err(DeletionError::AlreadyOnHold));
    }

    #[test]
    fn release_when_not_on_hold_fails() {
        let mut pipeline = DeletionPipeline::new(make_request(42));
        let result = pipeline.release_hold();
        assert_eq!(result, Err(DeletionError::NotOnHold));
    }

    #[test]
    fn cannot_hold_completed_pipeline() {
        let mut pipeline = DeletionPipeline::new(make_request(42));
        pipeline.run_all(&NoOpExecutor).unwrap();
        let result = pipeline.place_hold("too late".into());
        assert_eq!(result, Err(DeletionError::AlreadyCompleted));
    }

    #[test]
    fn run_all_completes_successfully() {
        let mut pipeline = DeletionPipeline::new(make_request(42));
        let status = pipeline.run_all(&NoOpExecutor).unwrap();
        assert_eq!(status, DeletionStatus::Completed);
        assert_eq!(pipeline.completed_steps.len(), 6);
    }

    #[test]
    fn run_all_stops_on_failure() {
        let mut pipeline = DeletionPipeline::new(make_request(42));
        let executor = FailOnStepExecutor {
            fail_on: DeletionStep::DeleteProfile,
            error_msg: "db down".into(),
        };
        let status = pipeline.run_all(&executor).unwrap();
        assert_eq!(
            status,
            DeletionStatus::Failed {
                error: "db down".into()
            }
        );
        // Only 2 steps completed before the failure
        assert_eq!(pipeline.completed_steps.len(), 2);
    }

    #[test]
    fn run_all_stops_on_hold() {
        let mut pipeline = DeletionPipeline::new(make_request(42));
        pipeline.place_hold("pending".into()).unwrap();
        let result = pipeline.run_all(&NoOpExecutor);
        assert_eq!(result, Err(DeletionError::OnHoldCannotAdvance));
    }

    #[test]
    fn custom_steps_pipeline() {
        let request = make_request(42);
        let steps = vec![
            DeletionStep::PseudonymizeLedger,
            DeletionStep::ArchiveDeletionSalt,
        ];
        let mut pipeline = DeletionPipeline::with_steps(request, steps);
        let status = pipeline.run_all(&NoOpExecutor).unwrap();
        assert_eq!(status, DeletionStatus::Completed);
        assert_eq!(pipeline.completed_steps.len(), 2);
    }

    #[test]
    fn default_step_sequence_order() {
        let steps = DeletionStep::default_sequence();
        assert_eq!(steps[0], DeletionStep::ExportUserData);
        assert_eq!(steps[1], DeletionStep::PseudonymizeLedger);
        assert_eq!(steps[2], DeletionStep::DeleteProfile);
        assert_eq!(steps[3], DeletionStep::DeleteSocialData);
        assert_eq!(steps[4], DeletionStep::DeleteSessionData);
        assert_eq!(steps[5], DeletionStep::ArchiveDeletionSalt);
    }

    #[test]
    fn failure_midway_preserves_completed_steps() {
        let mut pipeline = DeletionPipeline::new(make_request(42));
        let executor = FailOnStepExecutor {
            fail_on: DeletionStep::PseudonymizeLedger,
            error_msg: "hash error".into(),
        };
        // Step 0 succeeds
        pipeline.advance(&executor).unwrap();
        assert_eq!(pipeline.completed_steps.len(), 1);
        // Step 1 fails
        pipeline.advance(&executor).unwrap();
        assert_eq!(pipeline.completed_steps.len(), 1);
    }
}
