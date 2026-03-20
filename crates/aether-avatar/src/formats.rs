#[derive(Debug, Clone)]
pub struct AvatarAssetId(pub String);

#[derive(Debug, Clone)]
pub enum AvatarFormat {
    Vrm1_0,
    Vrm0_3,
    AetherBinary,
    Unknown,
}

#[derive(Debug, Clone)]
pub enum AvatarFormatError {
    UnsupportedFormat,
    CorruptPayload,
    MissingMeta,
}

#[derive(Debug, Clone)]
pub struct AvatarMetadata {
    pub asset_id: AvatarAssetId,
    pub format: AvatarFormat,
    pub source_hint: String,
    pub bone_count: u16,
    pub material_count: u8,
}

#[derive(Debug, Clone)]
pub struct AvatarImportDecision {
    pub accepted: bool,
    pub reason: Option<String>,
    pub metadata: Option<AvatarMetadata>,
}
