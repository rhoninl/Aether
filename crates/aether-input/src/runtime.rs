use crate::{
    actions::{ActionPhase, XRButton},
    adapter::{InputFrame, InputFrameError, RuntimeAdapter},
    capabilities::{InputActionPath, InputBackend},
    haptics::{HapticChannel, HapticEffect, HapticRequest},
    locomotion::{ComfortProfile, LocomotionMode, LocomotionProfile},
};

#[derive(Debug, Clone)]
pub struct InputRuntimeInput {
    pub now_ms: u64,
    pub player_id: u64,
}

#[derive(Debug, Clone)]
pub enum SimulationIntent {
    Locomotion {
        player_id: u64,
        mode: LocomotionMode,
        phase: ActionPhase,
        hand: InputActionPath,
        force: f32,
    },
    Interaction {
        player_id: u64,
        hand: InputActionPath,
        button: XRButton,
        phase: ActionPhase,
        force: f32,
        target: Option<u64>,
    },
    Teleport {
        player_id: u64,
        target: Option<u64>,
        phase: ActionPhase,
    },
    None,
}

#[derive(Debug)]
pub struct PlayerInputFrame {
    pub player_id: u64,
    pub backend: InputBackend,
    pub timestamp_ms: u64,
    pub profile_session: String,
    pub active_locomotion: LocomotionMode,
    pub intents: Vec<SimulationIntent>,
    pub haptics: Vec<HapticRequest>,
}

#[derive(Debug)]
pub struct SimulationRuntimeState {
    pub step_index: u64,
    pub dropped_events: u64,
    pub unsupported_events: u64,
}

#[derive(Debug)]
pub struct InputRuntimeConfig {
    pub locomotion: LocomotionProfile,
    pub comfort_profile: ComfortProfile,
    pub allow_unknown_backends: bool,
    pub max_intents_per_frame: usize,
    pub haptics_enabled: bool,
}

impl Default for InputRuntimeConfig {
    fn default() -> Self {
        Self {
            locomotion: LocomotionProfile {
                allowed_modes: vec![LocomotionMode::Teleport, LocomotionMode::Smooth],
                active: LocomotionMode::Smooth,
                comfort: ComfortProfile {
                    enabled: true,
                    style: crate::locomotion::ComfortStyle::SnapTurnStepDeg(30),
                    rotation_speed_deg_per_s: 180.0,
                    snap_turn_enabled: true,
                    seated_mode: false,
                },
                acceleration_mps2: 5.0,
                max_speed_mps: 2.5,
            },
            comfort_profile: ComfortProfile {
                enabled: true,
                style: crate::locomotion::ComfortStyle::VignetteStrength(0.3),
                rotation_speed_deg_per_s: 110.0,
                snap_turn_enabled: true,
                seated_mode: false,
            },
            allow_unknown_backends: true,
            max_intents_per_frame: 128,
            haptics_enabled: true,
        }
    }
}

#[derive(Debug)]
pub struct InputRuntimeOutput {
    pub now_ms: u64,
    pub players: Vec<PlayerInputFrame>,
    pub dropped_events: u64,
    pub unsupported_inputs: u64,
}

impl InputRuntimeOutput {
    fn empty(now_ms: u64) -> Self {
        Self {
            now_ms,
            players: Vec::new(),
            dropped_events: 0,
            unsupported_inputs: 0,
        }
    }
}

#[derive(Debug)]
pub struct InputRuntime {
    cfg: InputRuntimeConfig,
    state: SimulationRuntimeState,
    adapters: Vec<Box<dyn RuntimeAdapter>>,
}

impl Default for InputRuntime {
    fn default() -> Self {
        Self::new(InputRuntimeConfig::default())
    }
}

impl InputRuntime {
    pub fn new(cfg: InputRuntimeConfig) -> Self {
        Self {
            cfg,
            state: SimulationRuntimeState {
                step_index: 0,
                dropped_events: 0,
                unsupported_events: 0,
            },
            adapters: Vec::new(),
        }
    }

    pub fn register_adapter(&mut self, adapter: Box<dyn RuntimeAdapter>) {
        self.adapters.push(adapter);
    }

    pub fn attach_default_adapter(&mut self, adapter: Box<dyn RuntimeAdapter>) {
        self.adapters.push(adapter);
    }

    pub fn configure_locomotion(&mut self, profile: LocomotionProfile) {
        self.cfg.locomotion = profile.clone();
        for adapter in self.adapters.iter_mut() {
            adapter.apply_locomotion_profile(&profile);
        }
    }

    pub fn state(&self) -> &SimulationRuntimeState {
        &self.state
    }

    pub fn step(&mut self, request: InputRuntimeInput) -> InputRuntimeOutput {
        self.state.step_index = self.state.step_index.saturating_add(1);
        let mut output = InputRuntimeOutput::empty(request.now_ms);
        let mut frames = Vec::with_capacity(self.adapters.len());
        let mut dropped_events = 0u64;
        let mut unsupported_inputs = 0u64;

        let mut polled: Vec<(InputFrame, String)> = Vec::with_capacity(self.adapters.len());
        for adapter in self.adapters.iter_mut() {
            let frame = match adapter.poll_frame() {
                Ok(frame) => frame,
                Err(InputFrameError::UnsupportedFeature(_message)) => {
                    self.state.unsupported_events = self.state.unsupported_events.saturating_add(1);
                    unsupported_inputs = unsupported_inputs.saturating_add(1);
                    continue;
                }
                Err(_) => {
                    self.state.dropped_events = self.state.dropped_events.saturating_add(1);
                    dropped_events = dropped_events.saturating_add(1);
                    continue;
                }
            };
            if frame.player_id != request.player_id && request.player_id != 0 {
                continue;
            }
            let session_id = adapter.advertised_capabilities().session_id.clone();
            polled.push((frame, session_id));
        }

        let profile = self.cfg.locomotion.clone();
        for (frame, session_id) in polled {
            let mut player_frame = self.consume_frame(request.now_ms, &frame, &profile);
            player_frame.profile_session = session_id;
            frames.push(player_frame);
        }

        output.dropped_events = self.state.dropped_events.saturating_add(dropped_events);
        output.unsupported_inputs = self.state.unsupported_events.saturating_add(unsupported_inputs);
        output.players = frames;
        output
    }

    fn consume_frame(
        &mut self,
        now_ms: u64,
        frame: &InputFrame,
        profile: &LocomotionProfile,
    ) -> PlayerInputFrame {
        let mut intents = Vec::new();
        let mut haptics = Vec::new();

        for event in frame.events.iter().take(self.cfg.max_intents_per_frame) {
            if self.cfg.haptics_enabled && matches!(event.button, XRButton::Trigger) {
                haptics.push(HapticRequest {
                    player_id: event.player_id,
                    channel: HapticChannel::Combined,
                    effect: if event.force > 0.7 {
                        HapticEffect::Impact
                    } else {
                        HapticEffect::Buzz
                    },
                    cooldown_ms: 16,
                    looped: false,
                });
            }

            let target = event.target.as_ref().map(|t| t.entity_id).filter(|id| *id != 0);
            let intent = match (event.button, event.hand) {
                (XRButton::Trigger, InputActionPath::LeftHand | InputActionPath::RightHand) => {
                    SimulationIntent::Locomotion {
                        player_id: event.player_id,
                        mode: self.pick_locomotion_mode(event),
                        phase: event.phase,
                        hand: event.hand.clone(),
                        force: event.force,
                    }
                }
                (XRButton::A | XRButton::B, _) => SimulationIntent::Interaction {
                    player_id: event.player_id,
                    hand: event.hand.clone(),
                    button: event.button,
                    phase: event.phase,
                    force: event.force,
                    target,
                },
                (XRButton::System, _) => SimulationIntent::Teleport {
                    player_id: event.player_id,
                    target,
                    phase: event.phase,
                },
                _ => {
                    if !self.cfg.allow_unknown_backends {
                        SimulationIntent::None
                    } else {
                        SimulationIntent::Interaction {
                            player_id: event.player_id,
                            hand: event.hand.clone(),
                            button: event.button,
                            phase: event.phase,
                            force: event.force,
                            target,
                        }
                    }
                }
            };
            if matches!(intent, SimulationIntent::None) {
                continue;
            }
            intents.push(intent);
        }

        if !self.cfg.comfort_profile.enabled {
            for intent in intents.iter_mut() {
                if let SimulationIntent::Locomotion { mode, .. } = intent {
                    let _ = mode;
                }
            }
        }

        let active_mode = if profile.allowed_modes.contains(&LocomotionMode::Teleport)
            && self.cfg.locomotion.active == LocomotionMode::Teleport
        {
            LocomotionMode::Teleport
        } else if profile.allowed_modes.contains(&LocomotionMode::Smooth) {
            LocomotionMode::Smooth
        } else if profile.allowed_modes.contains(&LocomotionMode::Fly) {
            LocomotionMode::Fly
        } else {
            LocomotionMode::Climb
        };

        PlayerInputFrame {
            player_id: frame.player_id,
            backend: frame.backend,
            timestamp_ms: now_ms,
            profile_session: frame.timestamp_ms.to_string(),
            active_locomotion: active_mode,
            intents,
            haptics,
        }
    }

    fn pick_locomotion_mode(&self, event: &crate::actions::InteractionEvent) -> LocomotionMode {
        if event.force > 0.75 {
            if self.cfg.locomotion.allowed_modes.contains(&LocomotionMode::Teleport) {
                LocomotionMode::Teleport
            } else {
                LocomotionMode::Smooth
            }
        } else if event.hand == InputActionPath::HMD {
            LocomotionMode::Fly
        } else {
            self.cfg.locomotion.active
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use crate::{actions::{ActionPhase, InteractionEvent, InteractionTarget, XRButton}, capabilities::{InputActionPath, InputBackend, InputBackend::OpenXr}, locomotion::{ComfortProfile, ComfortStyle, LocomotionMode, LocomotionProfile}};

    use crate::adapter::{InputFrame, InputFrameError, RuntimeAdapter};

    #[derive(Debug)]
    struct TestAdapter {
        frames: VecDeque<InputFrame>,
        backend: InputBackend,
        profile: crate::capabilities::InputFrameHint,
        locomotion: LocomotionProfile,
    }

    impl TestAdapter {
        fn new(backend: InputBackend, player_id: u64, backend_session: &str) -> Self {
            Self {
                frames: VecDeque::new(),
                backend,
                profile: crate::capabilities::InputFrameHint {
                    backend,
                    session_id: backend_session.into(),
                    capabilities: vec![],
                },
                locomotion: LocomotionProfile {
                    allowed_modes: vec![LocomotionMode::Teleport, LocomotionMode::Smooth],
                    active: LocomotionMode::Teleport,
                    comfort: ComfortProfile {
                        enabled: true,
                        style: ComfortStyle::SnapTurnStepDeg(30),
                        rotation_speed_deg_per_s: 140.0,
                        snap_turn_enabled: true,
                        seated_mode: true,
                    },
                    acceleration_mps2: 4.0,
                    max_speed_mps: 2.8,
                },
            }
            .with_event(player_id, InteractionEvent {
                player_id,
                hand: InputActionPath::RightHand,
                button: XRButton::Trigger,
                phase: ActionPhase::Started,
                force: 0.9,
                target: Some(InteractionTarget {
                    entity_id: 12,
                    hit_distance_m: 2.0,
                    has_physics: true,
                }),
                hand_pose: None,
            })
        }
        fn with_event(mut self, player_id: u64, event: InteractionEvent) -> Self {
            self.frames.push_back(InputFrame {
                backend: self.backend,
                player_id,
                timestamp_ms: 42,
                events: vec![event],
            });
            self
        }
    }

    impl RuntimeAdapter for TestAdapter {
        fn backend(&self) -> InputBackend {
            self.backend
        }
        fn advertised_capabilities(&self) -> crate::capabilities::InputFrameHint {
            self.profile.clone()
        }
        fn poll_frame(&mut self) -> Result<InputFrame, InputFrameError> {
            match self.frames.pop_front() {
                Some(frame) => Ok(frame),
                None => Err(InputFrameError::ParseError),
            }
        }
        fn apply_locomotion_profile(&mut self, profile: &LocomotionProfile) {
            self.locomotion = profile.clone();
        }
    }

    #[test]
    fn step_routes_to_simulation_events() {
        let mut runtime = InputRuntime::new(InputRuntimeConfig {
            locomotion: LocomotionProfile {
                allowed_modes: vec![LocomotionMode::Teleport],
                active: LocomotionMode::Smooth,
                comfort: ComfortProfile {
                    enabled: true,
                    style: ComfortStyle::SnapTurnStepDeg(20),
                    rotation_speed_deg_per_s: 120.0,
                    snap_turn_enabled: true,
                    seated_mode: false,
                },
                acceleration_mps2: 5.0,
                max_speed_mps: 3.2,
            },
            comfort_profile: ComfortProfile {
                enabled: true,
                style: ComfortStyle::SnapTurnStepDeg(15),
                rotation_speed_deg_per_s: 120.0,
                snap_turn_enabled: true,
                seated_mode: false,
            },
            allow_unknown_backends: true,
            max_intents_per_frame: 16,
            haptics_enabled: true,
        });

        runtime.register_adapter(Box::new(TestAdapter::new(OpenXr, 7, "test-session")));
        let out = runtime.step(InputRuntimeInput {
            now_ms: 1_000,
            player_id: 7,
        });
        assert_eq!(out.players.len(), 1);
        assert!(!out.players[0].intents.is_empty());
    }
}
