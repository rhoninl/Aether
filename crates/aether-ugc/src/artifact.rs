#[derive(Debug, Clone)]
pub enum ArtifactType {
    AssetBundle,
    WorldScript,
    AvatarModel,
    VoicePack,
    Unknown,
}

#[derive(Debug, Clone)]
pub enum ArtifactState {
    Uploading,
    Scanning,
    Approved,
    Rejected,
    Published,
    Archived,
}

#[derive(Debug)]
pub struct ArtifactDescriptor {
    pub artifact_id: String,
    pub owner_id: u64,
    pub artifact_type: ArtifactType,
    pub checksum_sha256: String,
    pub size_bytes: u64,
    pub state: ArtifactState,
}

#[derive(Debug)]
pub struct ArtifactUploadSession {
    pub session_id: String,
    pub artifact: ArtifactDescriptor,
    pub created_ms: u64,
    pub updated_ms: u64,
}

