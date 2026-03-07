#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NetEntity(pub u64);

#[derive(Debug, Clone, Copy)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}
