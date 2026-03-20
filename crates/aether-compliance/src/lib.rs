//! Privacy and legal compliance for GDPR account deletion, data export,
//! pseudonymization, retention scheduling, and legal hold management.

pub mod deletion;
pub mod export;
pub mod keystore;
pub mod legal_hold;
pub mod pseudonymize;
pub mod retention;

pub use deletion::{
    DeleteRequest, DeleteScope, DeletionError, DeletionPipeline, DeletionStatus, DeletionStep,
    LegalHold, NoOpExecutor, ProfileDeletion, StepExecutor, StepResult,
};
pub use export::{DataExporter, ExportBundle, ExportSection, ExportStatus};
pub use keystore::{ComplianceKeystore, KeyPurpose, KeystoreEntry, KeystoreError};
pub use legal_hold::{Hold, HoldError, HoldManager};
pub use pseudonymize::{generate_salt, pseudonymize_id, pseudonymize_rows, PseudonymizedRow};
pub use retention::{RetentionRecord, RetentionSchedule, RetentionState, RetentionWindow};
