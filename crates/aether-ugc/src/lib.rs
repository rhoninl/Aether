//! UGC upload and processing primitives.

pub mod artifact;
pub mod ingest;
pub mod moderation;
pub mod pipeline;
pub mod pipeline_canonical;
pub mod validation;
pub mod runtime;

pub use artifact::{ArtifactDescriptor, ArtifactState, ArtifactType, ArtifactUploadSession};
pub use ingest::{ChunkUpload, UploadRequest, UploadSession};
pub use moderation::{ModerationSignal, ModerationStatus, ModerationStatusUpdate};
pub use pipeline::{AotProfile, ContentAddress, ProcessingStage, UploaderProfile};
pub use pipeline_canonical::{
    CanonicalArtifact, CanonicalPipelineError, CanonicalPipelineState, CanonicalUgcPipeline,
};
pub use validation::{FileType, FileValidation, ValidationError, ValidationReport};
pub use runtime::{
    UgcRuntime, UgcRuntimeConfig, UgcRuntimeInput, UgcRuntimeOutput, ModerationSignalRequest,
};
