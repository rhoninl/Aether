#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocomotionMode {
    Teleport,
    Smooth,
    Climb,
    Fly,
}

#[derive(Debug, Clone)]
pub enum ComfortStyle {
    VignetteStrength(f32),
    SnapTurnStepDeg(u16),
    SeatedLockStep,
}

#[derive(Debug, Clone)]
pub struct ComfortProfile {
    pub enabled: bool,
    pub style: ComfortStyle,
    pub rotation_speed_deg_per_s: f32,
    pub snap_turn_enabled: bool,
    pub seated_mode: bool,
}

#[derive(Debug, Clone)]
pub struct TeleportAnchor {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub valid: bool,
}

#[derive(Debug, Clone)]
pub struct LocomotionProfile {
    pub allowed_modes: Vec<LocomotionMode>,
    pub active: LocomotionMode,
    pub comfort: ComfortProfile,
    pub acceleration_mps2: f32,
    pub max_speed_mps: f32,
}

