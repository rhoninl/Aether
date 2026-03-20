#[derive(Debug, Clone)]
pub enum ChunkKind {
    Terrain,
    PropMesh,
    Lighting,
}

#[derive(Debug, Clone)]
pub struct ChunkDescriptor {
    pub world_id: String,
    pub chunk_id: u64,
    pub kind: ChunkKind,
    pub lod: u8,
    pub path: String,
    pub size_bytes: u64,
    pub checksum_sha256: String,
}

#[derive(Debug, Clone)]
pub struct ChunkStreamingPolicy {
    pub max_inflight: u16,
    pub min_prefetch_distance: f32,
    pub target_bytes_per_second: u64,
}
