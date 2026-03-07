#[derive(Debug, Clone)]
pub struct IkPoint {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Debug, Clone)]
pub enum TrackingSource {
    ThreePoint,
    SixPoint,
    FullBody,
    HmdOnly,
}

#[derive(Debug, Clone)]
pub struct TrackingFrame {
    pub player_id: u64,
    pub source: TrackingSource,
    pub head: IkPoint,
    pub hands: Option<(IkPoint, IkPoint)>,
    pub feet: Option<(IkPoint, IkPoint, IkPoint)>,
    pub hips: Option<IkPoint>,
    pub timestamp_ms: u64,
}

#[derive(Debug, Clone)]
pub struct IkConfiguration {
    pub player_id: u64,
    pub joint_limit_deg: f32,
    pub solver_iterations: u16,
    pub source: TrackingSource,
}

