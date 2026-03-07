#[derive(Debug, Clone)]
pub enum GizmoMode {
    Move,
    Rotate,
    Scale,
    Snap,
}

#[derive(Debug, Clone)]
pub enum ScriptMode {
    Visual,
    Text,
}

#[derive(Debug, Clone)]
pub struct PropPlacement {
    pub id: String,
    pub template: String,
    pub snapped: bool,
    pub snap_distance_m: f32,
}

#[derive(Debug, Clone)]
pub struct ScriptEdit {
    pub script_id: String,
    pub mode: ScriptMode,
    pub path: String,
}

