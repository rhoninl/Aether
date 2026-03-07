#[derive(Debug, Clone)]
pub struct WorldManifestPatch {
    pub world_id: String,
    pub physics: Option<PhysicsSettingsPatch>,
    pub spawn_points: Vec<SpawnPointEdit>,
    pub props: Vec<PropEdit>,
    pub scripts: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PhysicsSettingsPatch {
    pub gravity: Option<f32>,
    pub tick_rate: Option<u32>,
    pub max_players: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct SpawnPointEdit {
    pub id: u64,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub yaw_deg: f32,
}

#[derive(Debug, Clone)]
pub struct TerrainEdit {
    pub chunk_id: u64,
    pub height_delta: f32,
    pub texture_weight: f32,
}

#[derive(Debug, Clone)]
pub struct ManifestEdit {
    pub world_id: String,
    pub physics: PhysicsSettingsPatch,
    pub terrain: Vec<TerrainEdit>,
}

#[derive(Debug, Clone)]
pub enum ScriptEdit {
    VisualNode {
        world_id: String,
        node_id: String,
        payload: Vec<u8>,
    },
    Text {
        world_id: String,
        filename: String,
        source: String,
    },
}

#[derive(Debug, Clone)]
pub struct PropEdit {
    pub prop_id: String,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub yaw_deg: f32,
}

