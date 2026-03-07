#[derive(Debug)]
pub struct UploadSession {
    pub session_id: String,
    pub owner_id: u64,
    pub started_ms: u64,
    pub total_chunks: u32,
    pub received_chunks: u32,
    pub checksum: Option<String>,
}

#[derive(Debug)]
pub struct ChunkUpload {
    pub session_id: String,
    pub chunk_index: u32,
    pub data_len: usize,
    pub chunk_sha256: String,
}

#[derive(Debug)]
pub struct UploadRequest {
    pub owner_id: u64,
    pub file_name: String,
    pub file_size: u64,
    pub mime_hint: String,
    pub chunk_count: u32,
}

