#[derive(Debug, Clone)]
pub enum TerrainBrush {
    Raise,
    Lower,
    Smooth,
    Flatten,
}

#[derive(Debug, Clone)]
pub struct SculptBrush {
    pub radius_m: f32,
    pub intensity: f32,
    pub falloff: f32,
}

#[derive(Debug, Clone)]
pub struct PaintStroke {
    pub world_id: String,
    pub brush: TerrainBrush,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub paint_index: u16,
    pub radius_m: f32,
}

#[derive(Debug, Clone)]
pub struct TerrainTool {
    pub brush: SculptBrush,
    pub selected: TerrainBrush,
}

