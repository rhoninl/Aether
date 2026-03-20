#[derive(Debug, Clone)]
pub struct TerrainChunk {
    pub chunk_id: u64,
    pub tile_x: u32,
    pub tile_y: u32,
    pub asset_id: String,
}

#[derive(Debug, Clone)]
pub struct PropInstance {
    pub prop_id: String,
    pub template_id: String,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub yaw_deg: f32,
    pub scale: f32,
}

#[derive(Debug, Clone)]
pub struct LightingSetup {
    pub skybox_asset: String,
    pub sun_intensity: f32,
    pub ambient_intensity: f32,
    pub aeenv_profile_path: String,
}

#[derive(Debug, Clone)]
pub struct TileLayer {
    pub layer_id: String,
    pub chunks: Vec<TerrainChunk>,
}

#[derive(Debug, Clone)]
pub struct SpawnPoint {
    pub id: u64,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub yaw_deg: f32,
    pub is_default: bool,
}
