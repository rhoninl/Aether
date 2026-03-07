#[derive(Debug, Clone)]
pub enum ProcessingStage {
    Validate,
    Scan,
    CompileAot,
    Publish,
    Archive,
}

#[derive(Debug)]
pub enum AotProfile {
    X86_64,
    Amd64,
    Arm64,
}

#[derive(Debug)]
pub struct UploaderProfile {
    pub artifact_id: String,
    pub requested_profiles: Vec<AotProfile>,
    pub stage: ProcessingStage,
}

#[derive(Debug, Clone)]
pub struct ContentAddress {
    pub sha256: String,
    pub size: u64,
    pub object_key: String,
}

