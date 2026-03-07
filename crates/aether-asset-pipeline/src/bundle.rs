#[derive(Debug, Clone)]
pub struct BundleManifest {
    pub bundle_id: String,
    pub scene: String,
    pub lod_chain: Vec<LODTier>,
    pub dependencies: Vec<Dependency>,
}

#[derive(Debug, Clone)]
pub struct Dependency {
    pub asset_id: String,
    pub checksum: String,
}

#[derive(Debug, Clone)]
pub struct LODTier {
    pub index: u8,
    pub index_size_bytes: u64,
    pub mesh_id: String,
}

#[derive(Debug, Clone)]
pub enum BundleFormat {
    AEmesh,
    AEenv,
    Legacy,
}

