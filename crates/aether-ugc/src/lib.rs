//! UGC upload and processing primitives.

pub mod artifact;
pub mod ingest;
pub mod moderation;
pub mod pipeline;
pub mod validation;
pub mod runtime;

pub use artifact::{ArtifactDescriptor, ArtifactState, ArtifactType, ArtifactUploadSession};
pub use ingest::{ChunkUpload, UploadRequest, UploadSession};
pub use moderation::{ModerationSignal, ModerationStatus, ModerationStatusUpdate};
pub use pipeline::{AotProfile, ContentAddress, ProcessingStage, UploaderProfile};
pub use validation::{FileType, FileValidation, ValidationError, ValidationReport};
pub use runtime::{
    UgcRuntime, UgcRuntimeConfig, UgcRuntimeInput, UgcRuntimeOutput, ModerationSignalRequest,
};
