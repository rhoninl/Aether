use crate::actions::InteractionEvent;
use crate::capabilities::{InputBackend, InputFrameHint};
use crate::locomotion::LocomotionProfile;

#[derive(Debug)]
pub enum InputFrameError {
    ParseError,
    MissingBackend,
    UnsupportedFeature(String),
}

#[derive(Debug)]
pub struct InputFrame {
    pub backend: InputBackend,
    pub player_id: u64,
    pub timestamp_ms: u64,
    pub events: Vec<InteractionEvent>,
}

pub trait RuntimeAdapter {
    fn backend(&self) -> InputBackend;
    fn advertised_capabilities(&self) -> InputFrameHint;
    fn poll_frame(&mut self) -> Result<InputFrame, InputFrameError>;
    fn apply_locomotion_profile(&mut self, profile: &LocomotionProfile);
}
