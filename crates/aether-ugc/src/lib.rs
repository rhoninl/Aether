//! UGC upload and processing primitives.

pub mod approval;
pub mod artifact;
pub mod ingest;
pub mod manifest;
pub mod moderation;
pub mod orchestrator;
pub mod pipeline;
pub mod runtime;
pub mod storage;
pub mod upload;
pub mod validation;
pub mod version;

pub use approval::{ApprovalError, ApprovalPolicy, ApprovalStatus, ApprovalWorkflow};
pub use artifact::{ArtifactDescriptor, ArtifactState, ArtifactType, ArtifactUploadSession};
pub use ingest::{ChunkUpload, UploadSession};
pub use ingest::UploadRequest as IngestUploadRequest;
pub use manifest::{ManifestBuilder, ManifestEntry, SignedManifest};
pub use moderation::{ModerationSignal, ModerationStatus, ModerationStatusUpdate};
pub use orchestrator::{PipelineError, PipelineResult, PipelineStage, ValidationPipeline};
pub use pipeline::{AotProfile, ContentAddress, ProcessingStage, UploaderProfile};
pub use runtime::{
    ModerationSignalRequest, UgcRuntime, UgcRuntimeConfig, UgcRuntimeInput, UgcRuntimeOutput,
};
pub use storage::{AssetStorage, InMemoryStorage, StorageError};
pub use upload::{UploadConfig, UploadError, UploadRequest};
pub use validation::{FileType, FileValidation, ValidationError, ValidationReport};
pub use version::{AssetVersion, VersionError, VersionHistory};
