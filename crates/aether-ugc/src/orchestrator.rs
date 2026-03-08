//! Validation pipeline orchestration: receive -> validate -> hash -> store -> approve.

use uuid::Uuid;

use crate::approval::{ApprovalPolicy, ApprovalStatus, ApprovalWorkflow};
use crate::manifest::compute_content_hash;
use crate::storage::{AssetStorage, StorageError};
use crate::upload::{UploadConfig, UploadError, UploadRequest};
use crate::version::{VersionError, VersionHistory};

#[derive(Debug, Clone, PartialEq)]
pub enum PipelineStage {
    Received,
    Validated,
    Hashed,
    Stored,
    Approved,
    Rejected { reason: String },
}

#[derive(Debug)]
pub struct PipelineResult {
    pub asset_id: Uuid,
    pub version: u32,
    pub content_hash: String,
    pub stage: PipelineStage,
    pub approval_status: ApprovalStatus,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PipelineError {
    Upload(UploadError),
    Version(VersionError),
    Storage(StorageError),
    ApprovalFailed(String),
}

impl std::fmt::Display for PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PipelineError::Upload(e) => write!(f, "upload: {e}"),
            PipelineError::Version(e) => write!(f, "version: {e}"),
            PipelineError::Storage(e) => write!(f, "storage: {e}"),
            PipelineError::ApprovalFailed(e) => write!(f, "approval: {e}"),
        }
    }
}

impl std::error::Error for PipelineError {}

/// Orchestrate an upload through the full validation pipeline.
pub struct ValidationPipeline {
    upload_config: UploadConfig,
    approval_policy: ApprovalPolicy,
}

impl ValidationPipeline {
    pub fn new(upload_config: UploadConfig, approval_policy: ApprovalPolicy) -> Self {
        Self {
            upload_config,
            approval_policy,
        }
    }

    /// Process an upload request through all pipeline stages.
    ///
    /// Stages: validate -> hash -> store -> approve
    pub async fn process(
        &self,
        request: &UploadRequest,
        history: &mut VersionHistory,
        storage: &dyn AssetStorage,
    ) -> Result<PipelineResult, PipelineError> {
        // Stage 1: Validate upload
        self.upload_config
            .validate(request)
            .map_err(PipelineError::Upload)?;

        // Stage 2: Compute content hash
        let content_hash = compute_content_hash(&request.data);
        let size_bytes = request.data.len() as u64;

        // Stage 3: Store the data
        let storage_key = format!("{}/{}", history.asset_id(), content_hash);
        storage
            .store(&storage_key, &request.data)
            .await
            .map_err(PipelineError::Storage)?;

        // Stage 4: Create version record
        let version = history
            .add_version(content_hash.clone(), size_bytes, request.parent_version)
            .map_err(PipelineError::Version)?;
        let version_number = version.version;
        let asset_id = version.asset_id;

        // Stage 5: Approval
        let mut workflow = ApprovalWorkflow::new();
        let approval_status =
            if self
                .approval_policy
                .should_auto_approve(&request.creator_id, size_bytes)
            {
                workflow
                    .transition(ApprovalStatus::Approved)
                    .map_err(|e| PipelineError::ApprovalFailed(e.to_string()))?
                    .clone()
            } else {
                workflow
                    .transition(ApprovalStatus::Scanning)
                    .map_err(|e| PipelineError::ApprovalFailed(e.to_string()))?
                    .clone()
            };

        let stage = match &approval_status {
            ApprovalStatus::Approved => PipelineStage::Approved,
            _ => PipelineStage::Stored,
        };

        Ok(PipelineResult {
            asset_id,
            version: version_number,
            content_hash,
            stage,
            approval_status,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::InMemoryStorage;
    use crate::validation::FileType;

    fn make_request(name: &str, data: &[u8], file_type: FileType) -> UploadRequest {
        UploadRequest {
            creator_id: Uuid::new_v4(),
            asset_name: name.to_string(),
            file_type,
            data: data.to_vec(),
            parent_version: None,
        }
    }

    #[tokio::test]
    async fn happy_path_no_auto_approve() {
        let pipeline = ValidationPipeline::new(UploadConfig::default(), ApprovalPolicy::default());
        let storage = InMemoryStorage::new();
        let asset_id = Uuid::new_v4();
        let mut history = VersionHistory::new(asset_id);

        let request = make_request("model.glb", b"some data", FileType::Glb);
        let result = pipeline.process(&request, &mut history, &storage).await.unwrap();

        assert_eq!(result.version, 1);
        assert_eq!(result.stage, PipelineStage::Stored);
        assert_eq!(result.approval_status, ApprovalStatus::Scanning);
        assert!(!result.content_hash.is_empty());
    }

    #[tokio::test]
    async fn happy_path_auto_approve_trusted_creator() {
        let creator = Uuid::new_v4();
        let policy = ApprovalPolicy {
            auto_approve_below_bytes: 0,
            trusted_creator_ids: vec![creator],
        };
        let pipeline = ValidationPipeline::new(UploadConfig::default(), policy);
        let storage = InMemoryStorage::new();
        let mut history = VersionHistory::new(Uuid::new_v4());

        let request = UploadRequest {
            creator_id: creator,
            asset_name: "model.glb".into(),
            file_type: FileType::Glb,
            data: vec![1, 2, 3],
            parent_version: None,
        };

        let result = pipeline.process(&request, &mut history, &storage).await.unwrap();
        assert_eq!(result.stage, PipelineStage::Approved);
        assert_eq!(result.approval_status, ApprovalStatus::Approved);
    }

    #[tokio::test]
    async fn happy_path_auto_approve_small_file() {
        let policy = ApprovalPolicy {
            auto_approve_below_bytes: 1000,
            trusted_creator_ids: Vec::new(),
        };
        let pipeline = ValidationPipeline::new(UploadConfig::default(), policy);
        let storage = InMemoryStorage::new();
        let mut history = VersionHistory::new(Uuid::new_v4());

        let request = make_request("small.png", &[0u8; 100], FileType::Png);
        let result = pipeline.process(&request, &mut history, &storage).await.unwrap();
        assert_eq!(result.stage, PipelineStage::Approved);
    }

    #[tokio::test]
    async fn rejected_at_validation_empty_data() {
        let pipeline = ValidationPipeline::new(UploadConfig::default(), ApprovalPolicy::default());
        let storage = InMemoryStorage::new();
        let mut history = VersionHistory::new(Uuid::new_v4());

        let request = make_request("model.glb", b"", FileType::Glb);
        let err = pipeline.process(&request, &mut history, &storage).await.unwrap_err();
        assert!(matches!(err, PipelineError::Upload(UploadError::EmptyData)));
    }

    #[tokio::test]
    async fn rejected_at_validation_size_exceeded() {
        let config = UploadConfig {
            max_upload_bytes: 10,
            ..Default::default()
        };
        let pipeline = ValidationPipeline::new(config, ApprovalPolicy::default());
        let storage = InMemoryStorage::new();
        let mut history = VersionHistory::new(Uuid::new_v4());

        let request = make_request("big.glb", &[0u8; 100], FileType::Glb);
        let err = pipeline.process(&request, &mut history, &storage).await.unwrap_err();
        assert!(matches!(
            err,
            PipelineError::Upload(UploadError::SizeExceeded { .. })
        ));
    }

    #[tokio::test]
    async fn rejected_at_validation_unknown_type() {
        let pipeline = ValidationPipeline::new(UploadConfig::default(), ApprovalPolicy::default());
        let storage = InMemoryStorage::new();
        let mut history = VersionHistory::new(Uuid::new_v4());

        let request = make_request("file.xyz", b"data", FileType::Unknown);
        let err = pipeline.process(&request, &mut history, &storage).await.unwrap_err();
        assert!(matches!(
            err,
            PipelineError::Upload(UploadError::UnsupportedType(_))
        ));
    }

    #[tokio::test]
    async fn data_stored_after_validation() {
        let pipeline = ValidationPipeline::new(UploadConfig::default(), ApprovalPolicy::default());
        let storage = InMemoryStorage::new();
        let asset_id = Uuid::new_v4();
        let mut history = VersionHistory::new(asset_id);

        let data = b"asset data here";
        let request = make_request("model.glb", data, FileType::Glb);
        let result = pipeline.process(&request, &mut history, &storage).await.unwrap();

        let key = format!("{}/{}", asset_id, result.content_hash);
        let stored = storage.retrieve(&key).await.unwrap();
        assert_eq!(stored, data);
    }

    #[tokio::test]
    async fn version_incremented_on_second_upload() {
        let pipeline = ValidationPipeline::new(UploadConfig::default(), ApprovalPolicy::default());
        let storage = InMemoryStorage::new();
        let mut history = VersionHistory::new(Uuid::new_v4());

        let r1 = make_request("m.glb", b"v1data", FileType::Glb);
        let res1 = pipeline.process(&r1, &mut history, &storage).await.unwrap();
        assert_eq!(res1.version, 1);

        let r2 = make_request("m.glb", b"v2data", FileType::Glb);
        let res2 = pipeline.process(&r2, &mut history, &storage).await.unwrap();
        assert_eq!(res2.version, 2);
    }

    #[tokio::test]
    async fn content_hash_is_sha256_of_data() {
        let pipeline = ValidationPipeline::new(UploadConfig::default(), ApprovalPolicy::default());
        let storage = InMemoryStorage::new();
        let mut history = VersionHistory::new(Uuid::new_v4());

        let data = b"hash me";
        let request = make_request("m.glb", data, FileType::Glb);
        let result = pipeline.process(&request, &mut history, &storage).await.unwrap();

        let expected = compute_content_hash(data);
        assert_eq!(result.content_hash, expected);
    }

    #[tokio::test]
    async fn parent_version_mismatch_rejected() {
        let pipeline = ValidationPipeline::new(UploadConfig::default(), ApprovalPolicy::default());
        let storage = InMemoryStorage::new();
        let mut history = VersionHistory::new(Uuid::new_v4());

        // Upload v1
        let r1 = make_request("m.glb", b"v1", FileType::Glb);
        pipeline.process(&r1, &mut history, &storage).await.unwrap();

        // Try to upload v2 with wrong parent
        let r2 = UploadRequest {
            creator_id: Uuid::new_v4(),
            asset_name: "m.glb".into(),
            file_type: FileType::Glb,
            data: b"v2".to_vec(),
            parent_version: Some(99),
        };
        let err = pipeline.process(&r2, &mut history, &storage).await.unwrap_err();
        assert!(matches!(err, PipelineError::Version(_)));
    }
}
