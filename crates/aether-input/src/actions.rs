use crate::capabilities::InputActionPath;
pub use aether_xr_hal::tracking::Pose3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XRButton {
    Trigger,
    Grip,
    A,
    B,
    X,
    Y,
    Thumbstick,
    Squeeze,
    Menu,
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionPhase {
    Started,
    Updated,
    Canceled,
}

#[derive(Debug, Clone)]
pub struct InteractionTarget {
    pub entity_id: u64,
    pub hit_distance_m: f32,
    pub has_physics: bool,
}

#[derive(Debug, Clone)]
pub struct GrabState {
    pub hand: InputActionPath,
    pub target: Option<InteractionTarget>,
    pub anchored: bool,
}

#[derive(Debug, Clone)]
pub struct InteractionEvent {
    pub player_id: u64,
    pub hand: InputActionPath,
    pub button: XRButton,
    pub phase: ActionPhase,
    pub force: f32,
    pub target: Option<InteractionTarget>,
    pub hand_pose: Option<Pose3>,
}
