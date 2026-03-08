//! Portal rendering state machine.
//!
//! Tracks the visual state of a portal on the client side, including
//! preview rendering, transition animation, and loading screen phases.

/// Default fade duration in milliseconds.
const DEFAULT_FADE_DURATION_MS: u64 = 800;

/// Rendering state of a portal visual.
#[derive(Debug, Clone, PartialEq)]
pub enum PortalRenderState {
    /// Default idle visual, no player nearby.
    Idle,
    /// Player in proximity; showing preview of destination world.
    Previewing {
        /// Handle/ID of the preview texture.
        preview_texture: u64,
        /// Time spent in this state (ms).
        elapsed_ms: u64,
    },
    /// Transition initiated; animating portal opening.
    Activating {
        /// Animation progress (0.0 to 1.0).
        progress: f32,
        /// Time spent in this state (ms).
        elapsed_ms: u64,
    },
    /// Full-screen fade or loading screen during transition.
    TransitionFade {
        /// Fade progress (0.0 = fully visible, 1.0 = fully faded).
        fade_progress: f32,
        /// Time spent in this state (ms).
        elapsed_ms: u64,
        /// Whether asset loading is complete.
        loading_complete: bool,
    },
    /// Player has arrived at destination; fading in.
    Arrived {
        /// Fade-in progress (0.0 = fully faded, 1.0 = fully visible).
        fade_in_progress: f32,
        /// Time spent in this state (ms).
        elapsed_ms: u64,
    },
}

/// Manages the portal rendering state machine.
#[derive(Debug)]
pub struct PortalRenderer {
    /// Current render state.
    state: PortalRenderState,
    /// Duration of fade transitions (ms).
    fade_duration_ms: u64,
    /// Timestamp when current state was entered.
    state_entered_ms: u64,
    /// Portal ID this renderer is associated with.
    portal_id: u64,
}

impl PortalRenderer {
    /// Create a new portal renderer for a given portal.
    pub fn new(portal_id: u64) -> Self {
        Self {
            state: PortalRenderState::Idle,
            fade_duration_ms: DEFAULT_FADE_DURATION_MS,
            state_entered_ms: 0,
            portal_id,
        }
    }

    pub fn with_fade_duration_ms(mut self, ms: u64) -> Self {
        self.fade_duration_ms = ms;
        self
    }

    /// Get the current render state.
    pub fn state(&self) -> &PortalRenderState {
        &self.state
    }

    /// Get the portal ID.
    pub fn portal_id(&self) -> u64 {
        self.portal_id
    }

    /// Transition to previewing state.
    pub fn start_preview(
        &mut self,
        preview_texture: u64,
        now_ms: u64,
    ) -> Option<&PortalRenderState> {
        match &self.state {
            PortalRenderState::Idle => {
                self.set_state(
                    PortalRenderState::Previewing {
                        preview_texture,
                        elapsed_ms: 0,
                    },
                    now_ms,
                );
                Some(&self.state)
            }
            _ => None,
        }
    }

    /// Stop previewing and return to idle.
    pub fn stop_preview(&mut self, now_ms: u64) -> Option<&PortalRenderState> {
        match &self.state {
            PortalRenderState::Previewing { .. } => {
                self.set_state(PortalRenderState::Idle, now_ms);
                Some(&self.state)
            }
            _ => None,
        }
    }

    /// Start the activation animation.
    pub fn start_activation(&mut self, now_ms: u64) -> Option<&PortalRenderState> {
        match &self.state {
            PortalRenderState::Previewing { .. } => {
                self.set_state(
                    PortalRenderState::Activating {
                        progress: 0.0,
                        elapsed_ms: 0,
                    },
                    now_ms,
                );
                Some(&self.state)
            }
            _ => None,
        }
    }

    /// Start the transition fade / loading screen.
    pub fn start_transition_fade(&mut self, now_ms: u64) -> Option<&PortalRenderState> {
        match &self.state {
            PortalRenderState::Activating { .. } => {
                self.set_state(
                    PortalRenderState::TransitionFade {
                        fade_progress: 0.0,
                        elapsed_ms: 0,
                        loading_complete: false,
                    },
                    now_ms,
                );
                Some(&self.state)
            }
            _ => None,
        }
    }

    /// Mark that asset loading is complete during the transition fade.
    pub fn mark_loading_complete(&mut self) {
        if let PortalRenderState::TransitionFade {
            loading_complete, ..
        } = &mut self.state
        {
            *loading_complete = true;
        }
    }

    /// Transition to the arrived state (player is at destination, fading in).
    pub fn start_arrival(&mut self, now_ms: u64) -> Option<&PortalRenderState> {
        match &self.state {
            PortalRenderState::TransitionFade {
                loading_complete: true,
                ..
            } => {
                self.set_state(
                    PortalRenderState::Arrived {
                        fade_in_progress: 0.0,
                        elapsed_ms: 0,
                    },
                    now_ms,
                );
                Some(&self.state)
            }
            _ => None,
        }
    }

    /// Complete the arrival and return to idle.
    pub fn complete_arrival(&mut self, now_ms: u64) -> Option<&PortalRenderState> {
        match &self.state {
            PortalRenderState::Arrived { .. } => {
                self.set_state(PortalRenderState::Idle, now_ms);
                Some(&self.state)
            }
            _ => None,
        }
    }

    /// Update the renderer each frame. Advances progress values based on elapsed time.
    pub fn tick(&mut self, now_ms: u64) {
        let elapsed = now_ms.saturating_sub(self.state_entered_ms);
        match &mut self.state {
            PortalRenderState::Previewing { elapsed_ms, .. } => {
                *elapsed_ms = elapsed;
            }
            PortalRenderState::Activating {
                progress,
                elapsed_ms,
            } => {
                *elapsed_ms = elapsed;
                // Activation completes over fade_duration_ms
                *progress = if self.fade_duration_ms > 0 {
                    (elapsed as f32 / self.fade_duration_ms as f32).min(1.0)
                } else {
                    1.0
                };
            }
            PortalRenderState::TransitionFade {
                fade_progress,
                elapsed_ms,
                ..
            } => {
                *elapsed_ms = elapsed;
                *fade_progress = if self.fade_duration_ms > 0 {
                    (elapsed as f32 / self.fade_duration_ms as f32).min(1.0)
                } else {
                    1.0
                };
            }
            PortalRenderState::Arrived {
                fade_in_progress,
                elapsed_ms,
            } => {
                *elapsed_ms = elapsed;
                *fade_in_progress = if self.fade_duration_ms > 0 {
                    (elapsed as f32 / self.fade_duration_ms as f32).min(1.0)
                } else {
                    1.0
                };
            }
            PortalRenderState::Idle => {}
        }
    }

    /// Force reset to idle.
    pub fn reset(&mut self, now_ms: u64) {
        self.set_state(PortalRenderState::Idle, now_ms);
    }

    fn set_state(&mut self, new_state: PortalRenderState, now_ms: u64) {
        self.state = new_state;
        self.state_entered_ms = now_ms;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Initial state ---

    #[test]
    fn starts_idle() {
        let renderer = PortalRenderer::new(1);
        assert_eq!(*renderer.state(), PortalRenderState::Idle);
        assert_eq!(renderer.portal_id(), 1);
    }

    // --- Full lifecycle ---

    #[test]
    fn full_render_lifecycle() {
        let mut renderer = PortalRenderer::new(1).with_fade_duration_ms(500);

        // Idle -> Previewing
        let result = renderer.start_preview(42, 100);
        assert!(result.is_some());
        assert!(matches!(
            renderer.state(),
            PortalRenderState::Previewing {
                preview_texture: 42,
                ..
            }
        ));

        // Previewing -> Activating
        let result = renderer.start_activation(200);
        assert!(result.is_some());
        assert!(matches!(
            renderer.state(),
            PortalRenderState::Activating { .. }
        ));

        // Activating -> TransitionFade
        let result = renderer.start_transition_fade(300);
        assert!(result.is_some());
        assert!(matches!(
            renderer.state(),
            PortalRenderState::TransitionFade {
                loading_complete: false,
                ..
            }
        ));

        // Mark loading complete
        renderer.mark_loading_complete();
        assert!(matches!(
            renderer.state(),
            PortalRenderState::TransitionFade {
                loading_complete: true,
                ..
            }
        ));

        // TransitionFade -> Arrived
        let result = renderer.start_arrival(400);
        assert!(result.is_some());
        assert!(matches!(
            renderer.state(),
            PortalRenderState::Arrived { .. }
        ));

        // Arrived -> Idle
        let result = renderer.complete_arrival(500);
        assert!(result.is_some());
        assert_eq!(*renderer.state(), PortalRenderState::Idle);
    }

    // --- Invalid transitions ---

    #[test]
    fn cannot_preview_from_activating() {
        let mut renderer = PortalRenderer::new(1);
        renderer.start_preview(1, 100);
        renderer.start_activation(200);

        let result = renderer.start_preview(2, 300);
        assert!(result.is_none());
    }

    #[test]
    fn cannot_activate_from_idle() {
        let mut renderer = PortalRenderer::new(1);
        let result = renderer.start_activation(100);
        assert!(result.is_none());
    }

    #[test]
    fn cannot_transition_fade_from_idle() {
        let mut renderer = PortalRenderer::new(1);
        let result = renderer.start_transition_fade(100);
        assert!(result.is_none());
    }

    #[test]
    fn cannot_arrive_without_loading_complete() {
        let mut renderer = PortalRenderer::new(1);
        renderer.start_preview(1, 100);
        renderer.start_activation(200);
        renderer.start_transition_fade(300);
        // Don't mark loading complete

        let result = renderer.start_arrival(400);
        assert!(result.is_none());
    }

    #[test]
    fn cannot_complete_arrival_from_idle() {
        let mut renderer = PortalRenderer::new(1);
        let result = renderer.complete_arrival(100);
        assert!(result.is_none());
    }

    // --- Stop preview ---

    #[test]
    fn stop_preview_returns_to_idle() {
        let mut renderer = PortalRenderer::new(1);
        renderer.start_preview(1, 100);
        let result = renderer.stop_preview(200);
        assert!(result.is_some());
        assert_eq!(*renderer.state(), PortalRenderState::Idle);
    }

    #[test]
    fn stop_preview_from_idle_no_effect() {
        let mut renderer = PortalRenderer::new(1);
        let result = renderer.stop_preview(100);
        assert!(result.is_none());
    }

    // --- Tick and progress ---

    #[test]
    fn tick_updates_activation_progress() {
        let mut renderer = PortalRenderer::new(1).with_fade_duration_ms(1000);
        renderer.start_preview(1, 0);
        renderer.start_activation(0);

        renderer.tick(500);
        if let PortalRenderState::Activating { progress, elapsed_ms } = renderer.state() {
            assert!((progress - 0.5).abs() < 0.01);
            assert_eq!(*elapsed_ms, 500);
        } else {
            panic!("expected Activating state");
        }

        renderer.tick(1000);
        if let PortalRenderState::Activating { progress, .. } = renderer.state() {
            assert!((progress - 1.0).abs() < 0.01);
        }
    }

    #[test]
    fn tick_updates_fade_progress() {
        let mut renderer = PortalRenderer::new(1).with_fade_duration_ms(400);
        renderer.start_preview(1, 0);
        renderer.start_activation(0);
        renderer.start_transition_fade(0);

        renderer.tick(200);
        if let PortalRenderState::TransitionFade {
            fade_progress,
            elapsed_ms,
            ..
        } = renderer.state()
        {
            assert!((fade_progress - 0.5).abs() < 0.01);
            assert_eq!(*elapsed_ms, 200);
        } else {
            panic!("expected TransitionFade state");
        }
    }

    #[test]
    fn tick_updates_arrival_fade_in() {
        let mut renderer = PortalRenderer::new(1).with_fade_duration_ms(400);
        renderer.start_preview(1, 0);
        renderer.start_activation(0);
        renderer.start_transition_fade(0);
        renderer.mark_loading_complete();
        renderer.start_arrival(0);

        renderer.tick(200);
        if let PortalRenderState::Arrived {
            fade_in_progress,
            elapsed_ms,
        } = renderer.state()
        {
            assert!((fade_in_progress - 0.5).abs() < 0.01);
            assert_eq!(*elapsed_ms, 200);
        } else {
            panic!("expected Arrived state");
        }
    }

    #[test]
    fn tick_updates_preview_elapsed() {
        let mut renderer = PortalRenderer::new(1);
        renderer.start_preview(1, 100);
        renderer.tick(350);
        if let PortalRenderState::Previewing { elapsed_ms, .. } = renderer.state() {
            assert_eq!(*elapsed_ms, 250);
        } else {
            panic!("expected Previewing state");
        }
    }

    #[test]
    fn tick_on_idle_no_op() {
        let mut renderer = PortalRenderer::new(1);
        renderer.tick(100);
        assert_eq!(*renderer.state(), PortalRenderState::Idle);
    }

    #[test]
    fn progress_capped_at_one() {
        let mut renderer = PortalRenderer::new(1).with_fade_duration_ms(100);
        renderer.start_preview(1, 0);
        renderer.start_activation(0);
        renderer.tick(99999);
        if let PortalRenderState::Activating { progress, .. } = renderer.state() {
            assert!((*progress - 1.0).abs() < f32::EPSILON);
        }
    }

    // --- Reset ---

    #[test]
    fn reset_returns_to_idle() {
        let mut renderer = PortalRenderer::new(1);
        renderer.start_preview(1, 100);
        renderer.start_activation(200);
        renderer.reset(300);
        assert_eq!(*renderer.state(), PortalRenderState::Idle);
    }

    // --- Mark loading complete when not in fade ---

    #[test]
    fn mark_loading_complete_in_wrong_state_no_panic() {
        let mut renderer = PortalRenderer::new(1);
        // Should not panic when called in idle state
        renderer.mark_loading_complete();
        assert_eq!(*renderer.state(), PortalRenderState::Idle);
    }

    // --- Default constants ---

    #[test]
    fn default_constants() {
        assert_eq!(DEFAULT_FADE_DURATION_MS, 800);
    }

    // --- Zero fade duration ---

    #[test]
    fn zero_fade_duration_instant_progress() {
        let mut renderer = PortalRenderer::new(1).with_fade_duration_ms(0);
        renderer.start_preview(1, 0);
        renderer.start_activation(0);
        renderer.tick(1);
        if let PortalRenderState::Activating { progress, .. } = renderer.state() {
            assert!((*progress - 1.0).abs() < f32::EPSILON);
        }
    }
}
