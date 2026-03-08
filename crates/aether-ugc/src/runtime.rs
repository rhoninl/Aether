use std::collections::HashMap;

use crate::artifact::{ArtifactDescriptor, ArtifactState, ArtifactType, ArtifactUploadSession};
use crate::ingest::{ChunkUpload, UploadRequest, UploadSession};
use crate::moderation::{ModerationSignal, ModerationStatus, ModerationStatusUpdate};
use crate::validation::{ValidationError, ValidationReport};

#[derive(Debug)]
pub struct UgcRuntimeConfig {
    pub enforce_checksum: bool,
    pub max_chunk_size_bytes: u64,
    pub max_chunks_per_upload: u32,
    pub auto_scan: bool,
}

impl Default for UgcRuntimeConfig {
    fn default() -> Self {
        Self {
            enforce_checksum: true,
            max_chunk_size_bytes: 4_000_000,
            max_chunks_per_upload: 4_096,
            auto_scan: true,
        }
    }
}

#[derive(Debug)]
pub struct UgcRuntimeInput {
    pub now_ms: u64,
    pub start_uploads: Vec<UploadRequest>,
    pub chunk_uploads: Vec<ChunkUpload>,
    pub moderation_signals: Vec<ModerationSignalRequest>,
    pub moderation_updates: Vec<ModerationStatusUpdate>,
    pub publish_requests: Vec<String>,
    pub archive_requests: Vec<String>,
}

#[derive(Debug)]
pub struct ModerationSignalRequest {
    pub artifact_id: String,
    pub signal: ModerationSignal,
}

#[derive(Debug)]
pub struct UgcRuntimeOutput {
    pub now_ms: u64,
    pub sessions: Vec<UploadSession>,
    pub state_changes: Vec<String>,
    pub moderation_events: Vec<String>,
    pub ready_for_publish: Vec<String>,
    pub published: Vec<String>,
    pub archived: Vec<String>,
    pub validation_reports: Vec<ValidationReport>,
}

#[derive(Debug)]
pub struct UgcRuntime {
    cfg: UgcRuntimeConfig,
    state: UgcRuntimeState,
}

#[derive(Debug)]
struct UgcRuntimeState {
    next_session: u64,
    sessions: HashMap<String, ArtifactUploadSession>,
    artifacts: HashMap<String, ArtifactDescriptor>,
    moderation: HashMap<String, ModerationStatus>,
}

impl Default for UgcRuntimeState {
    fn default() -> Self {
        Self {
            next_session: 0,
            sessions: HashMap::new(),
            artifacts: HashMap::new(),
            moderation: HashMap::new(),
        }
    }
}

impl Default for UgcRuntime {
    fn default() -> Self {
        Self::new(UgcRuntimeConfig::default())
    }
}

impl UgcRuntime {
    pub fn new(cfg: UgcRuntimeConfig) -> Self {
        Self {
            cfg,
            state: UgcRuntimeState::default(),
        }
    }

    pub fn step(&mut self, input: UgcRuntimeInput) -> UgcRuntimeOutput {
        let mut output = UgcRuntimeOutput {
            now_ms: input.now_ms,
            sessions: Vec::new(),
            state_changes: Vec::new(),
            moderation_events: Vec::new(),
            ready_for_publish: Vec::new(),
            published: Vec::new(),
            archived: Vec::new(),
            validation_reports: Vec::new(),
        };

        for request in input.start_uploads {
            if let Some((artifact_id, report)) = self.begin_upload(&request, input.now_ms) {
                output.validation_reports.push(report);
                output.state_changes.push(format!("session_start:{artifact_id}:{}", request.owner_id));
            }
        }

        for upload in input.chunk_uploads {
            self.receive_chunk(upload, input.now_ms, &mut output);
        }

        for request in input.moderation_signals {
            self.handle_moderation_signal(request, input.now_ms, &mut output);
        }

        for update in input.moderation_updates {
            self.apply_moderation_update(update, input.now_ms, &mut output);
        }

        for artifact_id in input.publish_requests {
            if self.publish_artifact(&artifact_id, &mut output) {
                output.published.push(artifact_id);
            }
        }

        for artifact_id in input.archive_requests {
            if self.archive_artifact(&artifact_id, &mut output) {
                output.archived.push(artifact_id);
            }
        }

        for session in self.state.sessions.values() {
            output.sessions.push(UploadSession {
                session_id: session.session_id.clone(),
                owner_id: session.artifact.owner_id,
                started_ms: session.created_ms,
                total_chunks: session.total_chunks,
                received_chunks: session.received_chunks,
                checksum: Some(session.artifact.checksum_sha256.clone()),
            });
        }
        output
    }

    fn begin_upload(
        &mut self,
        request: &UploadRequest,
        now_ms: u64,
    ) -> Option<(String, ValidationReport)> {
        if request.chunk_count > self.cfg.max_chunks_per_upload {
            let report = ValidationReport {
                file_name: request.file_name.clone(),
                accepted: false,
                error: Some(ValidationError::TooLarge),
                checksum: None,
            };
            return Some((String::new(), report));
        }

        let file_type = self.infer_file_type(&request.mime_hint, &request.file_name);
        if self.cfg.enforce_checksum && request.file_name.trim().is_empty() {
            let report = ValidationReport {
                file_name: request.file_name.clone(),
                accepted: false,
                error: Some(ValidationError::Corrupt),
                checksum: None,
            };
            return Some((String::new(), report));
        }

        let artifact_id = format!("art-{}-{}", request.owner_id, self.state.next_session);
        self.state.next_session = self.state.next_session.saturating_add(1);

        let artifact = ArtifactDescriptor {
            artifact_id: artifact_id.clone(),
            owner_id: request.owner_id,
            artifact_type: file_type,
            checksum_sha256: format!("chk-{}-{}", request.owner_id, request.file_size),
            size_bytes: request.file_size,
            state: ArtifactState::Uploading,
        };
        let session = ArtifactUploadSession {
            session_id: artifact_id.clone(),
            artifact,
            total_chunks: request.chunk_count,
            received_chunks: 0,
            created_ms: now_ms,
            updated_ms: now_ms,
        };
        let file_validation = ValidationReport {
            file_name: request.file_name.clone(),
            accepted: request.file_size > 0 && !request.file_name.is_empty(),
            error: if request.file_size == 0 {
                Some(ValidationError::Corrupt)
            } else {
                None
            },
            checksum: Some(format!("chk-{}-{}", request.owner_id, request.chunk_count)),
        };

        self.state.sessions.insert(artifact_id.clone(), session.clone());
        self.state
            .artifacts
            .insert(artifact_id.clone(), session.artifact.clone());
        self.state
            .moderation
            .insert(artifact_id.clone(), ModerationStatus::Pending);
        if self.cfg.auto_scan {
            self.state
                .moderation
                .insert(artifact_id.clone(), ModerationStatus::Running);
        }
        self.state
            .artifacts
            .get_mut(&artifact_id)
            .map(|descriptor| {
                descriptor.state = ArtifactState::Scanning;
            });

        Some((artifact_id, file_validation))
    }

    fn receive_chunk(&mut self, upload: ChunkUpload, now_ms: u64, output: &mut UgcRuntimeOutput) {
        let mut unknown = false;
        if upload.chunk_index >= 65535 {
            unknown = true;
        }

        let session = match self.state.sessions.get_mut(&upload.session_id) {
            Some(session) => session,
            None => {
                output
                    .state_changes
                    .push(format!("chunk_unknown_session:{}", upload.session_id));
                return;
            }
        };

        if upload.data_len as u64 > self.cfg.max_chunk_size_bytes {
            output.state_changes.push(format!(
                "chunk_too_large:{}:{}",
                upload.session_id, upload.chunk_index
            ));
            return;
        }

        if session.received_chunks < upload.chunk_index {
            session.received_chunks = upload.chunk_index;
        }
        session.updated_ms = now_ms;
        output.state_changes.push(format!(
            "chunk:{}:{}:{}",
            session.session_id, upload.chunk_index, upload.chunk_sha256
        ));

        if session.total_chunks > 0 && session.received_chunks + 1 >= session.total_chunks {
            if let Some(artifact) = self.state.artifacts.get_mut(&session.session_id) {
                artifact.state = ArtifactState::Scanning;
                output
                    .state_changes
                    .push(format!("upload_ready_scan:{}", artifact.artifact_id));
                if unknown {
                    output
                        .state_changes
                        .push(format!("chunk_warning_unknowns:{}", artifact.artifact_id));
                }
            }
        }

        let sid = session.session_id.clone();
        if let Some(address) = self.address(&sid, now_ms) {
            output.validation_reports.push(address);
        }
    }

    fn address(&self, artifact_id: &str, now_ms: u64) -> Option<ValidationReport> {
        self.state.artifacts.get(artifact_id).map(|artifact| ValidationReport {
            file_name: artifact.artifact_id.clone(),
            accepted: true,
            error: if artifact.size_bytes == 0 {
                Some(ValidationError::Corrupt)
            } else {
                None
            },
            checksum: Some(format!("validated:{now_ms}")),
        })
    }

    fn handle_moderation_signal(
        &mut self,
        request: ModerationSignalRequest,
        now_ms: u64,
        output: &mut UgcRuntimeOutput,
    ) {
        match request.signal {
            ModerationSignal::TriggerScan => {
                let status = if self
                    .state
                    .artifacts
                    .get(&request.artifact_id)
                    .is_some_and(|artifact| artifact.size_bytes % 2 == 0)
                {
                    ModerationStatus::Cleared
                } else {
                    ModerationStatus::Rejected("automatic policy rejection".into())
                };
                self.state.moderation.insert(request.artifact_id.clone(), status);
                output.moderation_events.push(format!(
                    "scan_triggered:{}:{}",
                    request.artifact_id, now_ms
                ));
            }
            ModerationSignal::ScanComplete { approved, reason } => {
                let status = if approved {
                    ModerationStatus::Cleared
                } else {
                    ModerationStatus::Rejected(reason.unwrap_or_default())
                };
                self.state.moderation.insert(request.artifact_id.clone(), status);
            }
        }
        if let Some(artifact) = self.state.artifacts.get_mut(&request.artifact_id) {
            let moderation = self
                .state
                .moderation
                .get(&request.artifact_id)
                .cloned()
                .unwrap_or(ModerationStatus::Pending);
            match moderation {
                ModerationStatus::Cleared => artifact.state = ArtifactState::Approved,
                ModerationStatus::Rejected(_) => artifact.state = ArtifactState::Rejected,
                _ => artifact.state = ArtifactState::Scanning,
            }
            if let ModerationStatus::Cleared = moderation {
                output.ready_for_publish.push(request.artifact_id.clone());
            }
        }
    }

    fn apply_moderation_update(
        &mut self,
        update: ModerationStatusUpdate,
        now_ms: u64,
        output: &mut UgcRuntimeOutput,
    ) {
        if let Some(artifact) = self.state.artifacts.get_mut(&update.artifact_id) {
            artifact.state = match update.status {
                ModerationStatus::Pending => ArtifactState::Scanning,
                ModerationStatus::Running => ArtifactState::Scanning,
                ModerationStatus::Cleared => ArtifactState::Approved,
                ModerationStatus::Rejected(_) => ArtifactState::Rejected,
            };
            self.state
                .moderation
                .insert(update.artifact_id.clone(), update.status.clone());
            output
                .moderation_events
                .push(format!("moderation_update:{}:{}", update.artifact_id, update.updated_ms.max(now_ms)));
        }
    }

    fn publish_artifact(&mut self, artifact_id: &str, output: &mut UgcRuntimeOutput) -> bool {
        let status = self
            .state
            .moderation
            .get(artifact_id)
            .cloned()
            .unwrap_or(ModerationStatus::Pending);
        if !matches!(status, ModerationStatus::Cleared) {
            output
                .state_changes
                .push(format!("publish_denied:{artifact_id}:not_approved"));
            return false;
        }

        if let Some(artifact) = self.state.artifacts.get_mut(artifact_id) {
            artifact.state = ArtifactState::Published;
            output
                .state_changes
                .push(format!("publish:{artifact_id}:{}", artifact.owner_id));
            return true;
        }
        false
    }

    fn archive_artifact(&mut self, artifact_id: &str, output: &mut UgcRuntimeOutput) -> bool {
        if let Some(artifact) = self.state.artifacts.get_mut(artifact_id) {
            artifact.state = ArtifactState::Archived;
            output
                .state_changes
                .push(format!("archive:{artifact_id}:{}", artifact.owner_id));
            return true;
        }
        false
    }

    fn infer_file_type(&self, mime_hint: &str, file_name: &str) -> ArtifactType {
        match (mime_hint, file_name) {
            (mime, _) if mime.contains("gltf") => ArtifactType::AssetBundle,
            (mime, _) if mime.contains("glb") => ArtifactType::AssetBundle,
            (mime, _) if mime.contains("image") || mime.contains("png") => ArtifactType::AvatarModel,
            (mime, _) if mime.contains("wav") || mime.contains("mp3") => ArtifactType::VoicePack,
            (mime, _) if mime.contains("wasm") => ArtifactType::WorldScript,
            (mime, _) if mime.contains("lua") => ArtifactType::WorldScript,
            (_, name) if name.ends_with(".glb") || name.ends_with(".gltf") => ArtifactType::AssetBundle,
            (_, name) if name.ends_with(".png") => ArtifactType::AvatarModel,
            (_, name) if name.ends_with(".wasm") => ArtifactType::WorldScript,
            (_, name) if name.ends_with(".lua") => ArtifactType::WorldScript,
            (_, name) if name.ends_with(".wav") || name.ends_with(".mp3") => ArtifactType::VoicePack,
            _ => ArtifactType::AssetBundle,
        }
    }
}

impl std::fmt::Display for ModerationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModerationStatus::Pending => write!(f, "pending"),
            ModerationStatus::Running => write!(f, "running"),
            ModerationStatus::Cleared => write!(f, "cleared"),
            ModerationStatus::Rejected(reason) => write!(f, "rejected:{reason:?}"),
        }
    }
}

impl std::fmt::Display for ArtifactType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::fmt::Display for UploadSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "session:{}:{}/{}",
            self.session_id, self.received_chunks, self.total_chunks
        )
    }
}

impl UploadSession {
    fn session_id_for_artifact(artifact_id: &str) -> String {
        artifact_id.to_string()
    }
}
