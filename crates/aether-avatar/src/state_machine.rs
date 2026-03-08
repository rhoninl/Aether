//! Animation state machine evaluator.
//!
//! Manages transitions between procedural animation states (idle, locomote,
//! gesture, fall) with blend weight computation.

use crate::animation::{
    BlendStateInput, BlendTransitionKind, LocomotionIntent, ProceduralStateMachine,
};

/// Default transition duration in milliseconds.
const DEFAULT_TRANSITION_MS: u64 = 200;
/// Gesture auto-expire duration in milliseconds.
const GESTURE_EXPIRE_MS: u64 = 2000;

/// Output of the animation state machine: weighted blend of states.
#[derive(Debug, Clone)]
pub struct AnimationOutput {
    /// Active state weights. Weights should sum to approximately 1.0.
    pub layers: Vec<(ProceduralStateMachine, f32)>,
    /// The primary (highest weight) state.
    pub primary_state: ProceduralStateMachine,
}

/// An active transition between two states.
#[derive(Debug, Clone)]
struct ActiveTransition {
    from: ProceduralStateMachine,
    to: ProceduralStateMachine,
    elapsed_ms: u64,
    duration_ms: u64,
    kind: BlendTransitionKind,
}

/// The animation state machine.
#[derive(Debug, Clone)]
pub struct AnimationStateMachine {
    current_state: ProceduralStateMachine,
    active_transition: Option<ActiveTransition>,
    gesture_timer_ms: u64,
    transition_duration_ms: u64,
}

impl AnimationStateMachine {
    /// Create a new state machine starting in Idle.
    pub fn new() -> Self {
        Self {
            current_state: ProceduralStateMachine::Idle,
            active_transition: None,
            gesture_timer_ms: 0,
            transition_duration_ms: DEFAULT_TRANSITION_MS,
        }
    }

    /// Create with a custom transition duration.
    pub fn with_transition_duration(duration_ms: u64) -> Self {
        Self {
            transition_duration_ms: duration_ms,
            ..Self::new()
        }
    }

    /// Get the current primary state.
    pub fn current_state(&self) -> ProceduralStateMachine {
        self.current_state
    }

    /// Update the state machine with a time delta and input.
    ///
    /// # Arguments
    /// * `dt_ms` - Time elapsed since last update in milliseconds
    /// * `input` - Current input state
    ///
    /// # Returns
    /// An `AnimationOutput` with the current blend weights.
    pub fn update(&mut self, dt_ms: u64, input: &BlendStateInput) -> AnimationOutput {
        // Determine the desired target state from input
        let target = self.determine_target_state(input);

        // If target differs from current and no transition is active, start one
        if target != self.current_state && self.active_transition.is_none() {
            self.active_transition = Some(ActiveTransition {
                from: self.current_state,
                to: target,
                elapsed_ms: 0,
                duration_ms: self.transition_duration_ms,
                kind: BlendTransitionKind::SmoothStep,
            });
        }

        // Update gesture timer
        if self.current_state == ProceduralStateMachine::Gesture {
            self.gesture_timer_ms += dt_ms;
            if self.gesture_timer_ms >= GESTURE_EXPIRE_MS && self.active_transition.is_none() {
                self.active_transition = Some(ActiveTransition {
                    from: ProceduralStateMachine::Gesture,
                    to: ProceduralStateMachine::GestureRecover,
                    elapsed_ms: 0,
                    duration_ms: self.transition_duration_ms,
                    kind: BlendTransitionKind::SmoothStep,
                });
            }
        }

        // Process active transition
        if let Some(ref mut transition) = self.active_transition {
            transition.elapsed_ms += dt_ms;
            if transition.elapsed_ms >= transition.duration_ms {
                // Transition complete
                self.current_state = transition.to;
                if transition.to == ProceduralStateMachine::Gesture {
                    self.gesture_timer_ms = 0;
                }
                self.active_transition = None;

                return AnimationOutput {
                    layers: vec![(self.current_state, 1.0)],
                    primary_state: self.current_state,
                };
            }

            // Compute blend weight
            let t = transition.elapsed_ms as f32 / transition.duration_ms as f32;
            let blend_t = match transition.kind {
                BlendTransitionKind::Linear => t,
                BlendTransitionKind::SmoothStep => smooth_step(t),
            };

            let from_weight = 1.0 - blend_t;
            let to_weight = blend_t;

            let primary = if to_weight > from_weight {
                transition.to
            } else {
                transition.from
            };

            return AnimationOutput {
                layers: vec![
                    (transition.from, from_weight),
                    (transition.to, to_weight),
                ],
                primary_state: primary,
            };
        }

        // No transition active
        AnimationOutput {
            layers: vec![(self.current_state, 1.0)],
            primary_state: self.current_state,
        }
    }

    /// Force an immediate state change without transition.
    pub fn force_state(&mut self, state: ProceduralStateMachine) {
        self.current_state = state;
        self.active_transition = None;
        self.gesture_timer_ms = 0;
    }

    /// Determine what state we should be targeting based on input.
    fn determine_target_state(&self, input: &BlendStateInput) -> ProceduralStateMachine {
        // Fall takes priority
        if input.in_air {
            return ProceduralStateMachine::Fall;
        }

        // Gesture takes priority over locomotion
        if input.gesture.is_some() && self.current_state != ProceduralStateMachine::Gesture {
            return ProceduralStateMachine::Gesture;
        }

        // Recovery from gesture
        if self.current_state == ProceduralStateMachine::GestureRecover {
            return match input.locomotion {
                LocomotionIntent::Stationary => ProceduralStateMachine::Idle,
                _ => ProceduralStateMachine::Locomote,
            };
        }

        // Locomotion
        match input.locomotion {
            LocomotionIntent::Stationary => {
                if self.current_state == ProceduralStateMachine::Gesture {
                    ProceduralStateMachine::Gesture // stay in gesture
                } else {
                    ProceduralStateMachine::Idle
                }
            }
            LocomotionIntent::Walk
            | LocomotionIntent::Sprint
            | LocomotionIntent::Crouch => ProceduralStateMachine::Locomote,
            LocomotionIntent::Jump => ProceduralStateMachine::Fall,
            LocomotionIntent::FallRecovery => ProceduralStateMachine::Idle,
        }
    }
}

impl Default for AnimationStateMachine {
    fn default() -> Self {
        Self::new()
    }
}

/// Smooth step interpolation: 3t^2 - 2t^3
fn smooth_step(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::animation::ProceduralGesture;

    const EPSILON: f32 = 0.01;

    fn idle_input() -> BlendStateInput {
        BlendStateInput {
            gesture: None,
            locomotion: LocomotionIntent::Stationary,
            in_air: false,
        }
    }

    fn walk_input() -> BlendStateInput {
        BlendStateInput {
            gesture: None,
            locomotion: LocomotionIntent::Walk,
            in_air: false,
        }
    }

    fn gesture_input() -> BlendStateInput {
        BlendStateInput {
            gesture: Some(ProceduralGesture::Wave),
            locomotion: LocomotionIntent::Stationary,
            in_air: false,
        }
    }

    fn fall_input() -> BlendStateInput {
        BlendStateInput {
            gesture: None,
            locomotion: LocomotionIntent::Stationary,
            in_air: true,
        }
    }

    #[test]
    fn test_starts_idle() {
        let sm = AnimationStateMachine::new();
        assert_eq!(sm.current_state(), ProceduralStateMachine::Idle);
    }

    #[test]
    fn test_idle_stays_idle() {
        let mut sm = AnimationStateMachine::new();
        let output = sm.update(16, &idle_input());
        assert_eq!(output.primary_state, ProceduralStateMachine::Idle);
        assert_eq!(output.layers.len(), 1);
        assert!((output.layers[0].1 - 1.0).abs() < EPSILON);
    }

    #[test]
    fn test_transition_to_locomote() {
        let mut sm = AnimationStateMachine::with_transition_duration(100);
        // Start transition
        let _output = sm.update(0, &walk_input());
        // First frame should start blending
        let output = sm.update(50, &walk_input());
        assert_eq!(output.layers.len(), 2);

        // Complete transition
        let output = sm.update(60, &walk_input());
        assert_eq!(output.primary_state, ProceduralStateMachine::Locomote);
        assert_eq!(output.layers.len(), 1);
    }

    #[test]
    fn test_blend_weights_sum_to_one() {
        let mut sm = AnimationStateMachine::with_transition_duration(100);
        sm.update(0, &walk_input()); // start transition
        let output = sm.update(50, &walk_input()); // mid transition

        let total_weight: f32 = output.layers.iter().map(|(_, w)| w).sum();
        assert!(
            (total_weight - 1.0).abs() < EPSILON,
            "weights should sum to 1.0, got {}",
            total_weight
        );
    }

    #[test]
    fn test_fall_takes_priority() {
        let mut sm = AnimationStateMachine::with_transition_duration(100);
        sm.update(0, &fall_input());
        let output = sm.update(200, &fall_input());
        assert_eq!(output.primary_state, ProceduralStateMachine::Fall);
    }

    #[test]
    fn test_gesture_transition() {
        let mut sm = AnimationStateMachine::with_transition_duration(100);
        sm.update(0, &gesture_input());
        let output = sm.update(200, &gesture_input());
        assert_eq!(output.primary_state, ProceduralStateMachine::Gesture);
    }

    #[test]
    fn test_gesture_auto_expires() {
        let mut sm = AnimationStateMachine::with_transition_duration(100);
        // Enter gesture
        sm.update(0, &gesture_input());
        sm.update(200, &gesture_input()); // complete transition to gesture
        assert_eq!(sm.current_state(), ProceduralStateMachine::Gesture);

        // Wait for gesture to expire -- the large dt completes the transition
        // to GestureRecover immediately.
        let output = sm.update(GESTURE_EXPIRE_MS, &idle_input());
        assert_eq!(
            output.primary_state,
            ProceduralStateMachine::GestureRecover
        );
        assert_eq!(sm.current_state(), ProceduralStateMachine::GestureRecover);
    }

    #[test]
    fn test_force_state() {
        let mut sm = AnimationStateMachine::new();
        sm.force_state(ProceduralStateMachine::Locomote);
        assert_eq!(sm.current_state(), ProceduralStateMachine::Locomote);
    }

    #[test]
    fn test_force_state_cancels_transition() {
        let mut sm = AnimationStateMachine::with_transition_duration(1000);
        sm.update(0, &walk_input()); // start transition
        sm.force_state(ProceduralStateMachine::Fall);
        // Use fall_input so it doesn't immediately start transitioning away from Fall
        let output = sm.update(16, &fall_input());
        // Should not be blending -- forced state is immediate
        assert_eq!(output.layers.len(), 1);
    }

    #[test]
    fn test_smooth_step_boundaries() {
        assert!((smooth_step(0.0)).abs() < EPSILON);
        assert!((smooth_step(1.0) - 1.0).abs() < EPSILON);
        // Midpoint should be 0.5
        assert!((smooth_step(0.5) - 0.5).abs() < EPSILON);
    }

    #[test]
    fn test_smooth_step_monotonic() {
        let mut prev = 0.0;
        for i in 1..=10 {
            let t = i as f32 / 10.0;
            let val = smooth_step(t);
            assert!(val >= prev, "smooth_step should be monotonic");
            prev = val;
        }
    }

    #[test]
    fn test_transition_back_to_idle() {
        let mut sm = AnimationStateMachine::with_transition_duration(100);
        // Go to locomote
        sm.update(0, &walk_input());
        sm.update(200, &walk_input());
        assert_eq!(sm.current_state(), ProceduralStateMachine::Locomote);

        // Go back to idle
        sm.update(0, &idle_input());
        sm.update(200, &idle_input());
        assert_eq!(sm.current_state(), ProceduralStateMachine::Idle);
    }

    #[test]
    fn test_jump_triggers_fall() {
        let mut sm = AnimationStateMachine::with_transition_duration(100);
        let jump_input = BlendStateInput {
            gesture: None,
            locomotion: LocomotionIntent::Jump,
            in_air: false,
        };
        sm.update(0, &jump_input);
        sm.update(200, &jump_input);
        assert_eq!(sm.current_state(), ProceduralStateMachine::Fall);
    }

    #[test]
    fn test_mid_transition_blend() {
        let mut sm = AnimationStateMachine::with_transition_duration(100);
        sm.update(0, &walk_input());
        let output = sm.update(25, &walk_input()); // 25% through transition

        assert_eq!(output.layers.len(), 2);
        let idle_weight = output
            .layers
            .iter()
            .find(|(s, _)| *s == ProceduralStateMachine::Idle)
            .map(|(_, w)| *w)
            .unwrap_or(0.0);
        let locomote_weight = output
            .layers
            .iter()
            .find(|(s, _)| *s == ProceduralStateMachine::Locomote)
            .map(|(_, w)| *w)
            .unwrap_or(0.0);

        assert!(idle_weight > locomote_weight, "idle should still dominate at 25%");
    }
}
