#[derive(Debug, Clone)]
pub enum PipelineTaskState {
    Queued,
    Running,
    Finished,
    Failed,
}

#[derive(Debug, Clone)]
pub struct BundleAssetRecord {
    pub asset_id: String,
    pub kind: String,
    pub source_hash: String,
    pub generated_hash: String,
}

#[derive(Debug)]
pub enum BundleError {
    UnsupportedFormat,
    CorruptSource,
    CompressionFailure,
}

