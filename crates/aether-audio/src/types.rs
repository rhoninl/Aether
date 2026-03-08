#[derive(Debug, Clone, Copy)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct ListenerState {
    pub position: Vec3,
    pub forward: Vec3,
    pub up: Vec3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AudioId(pub u64);

#[derive(Debug, Clone, Copy)]
pub struct DistanceInfo {
    pub meters: f32,
}

#[derive(Debug, Clone)]
pub struct AudioSource {
    pub id: AudioId,
    pub position: Vec3,
    pub volume: f32,
    pub world_id: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioLod {
    Near,
    Mid,
    Far,
    Distant,
}
