//! Portal entity type with activation trigger and state machine.
//!
//! A portal connects two worlds via `AetherUrl` references and activates
//! when a player enters proximity and optionally confirms interaction.

use crate::aether_url::AetherUrl;

/// Default proximity detection radius in world units.
const DEFAULT_PORTAL_RADIUS: f32 = 3.0;
/// Default cooldown after a failed activation (milliseconds).
const DEFAULT_PORTAL_COOLDOWN_MS: u64 = 5_000;
/// Default transition timeout (milliseconds).
const DEFAULT_TRANSITION_TIMEOUT_MS: u64 = 15_000;

/// Visual shape of the portal in the world.
#[derive(Debug, Clone, PartialEq)]
pub enum PortalShape {
    /// Circular portal with a given visual radius.
    Circle { visual_radius: f32 },
    /// Rectangular portal with width and height.
    Rectangle { width: f32, height: f32 },
    /// Reference to a custom mesh asset.
    CustomMesh { mesh_asset_id: String },
}

/// How the portal is activated by a player.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActivationMode {
    /// Automatically activate when player enters proximity radius.
    ProximityOnly,
    /// Player must press an interaction key while in proximity.
    InteractionRequired,
    /// Player enters proximity and must confirm via interaction.
    ProximityAndConfirm,
}

/// State of the portal activation lifecycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortalState {
    /// Portal is idle; no player nearby.
    Idle,
    /// Player is within proximity radius.
    Proximity,
    /// Activation is in progress (handoff initiated).
    Activating,
    /// Transition to destination has started.
    TransitionStarted,
    /// Transition completed successfully.
    Completed,
    /// Transition failed; portal enters cooldown.
    Failed { reason: String },
    /// Portal is on cooldown after a failure.
    Cooldown { until_ms: u64 },
}

/// A portal entity that connects two worlds.
#[derive(Debug, Clone)]
pub struct Portal {
    /// Unique portal identifier.
    pub portal_id: u64,
    /// URL of the source world (where the portal is placed).
    pub source: AetherUrl,
    /// URL of the destination world.
    pub destination: AetherUrl,
    /// Position of the portal in world space.
    pub position: [f32; 3],
    /// Proximity detection radius.
    pub radius: f32,
    /// Visual shape.
    pub shape: PortalShape,
    /// Activation mode.
    pub activation_mode: ActivationMode,
    /// Whether the portal is enabled.
    pub enabled: bool,
    /// Current state.
    state: PortalState,
    /// Cooldown duration in milliseconds.
    cooldown_ms: u64,
    /// Transition timeout in milliseconds.
    transition_timeout_ms: u64,
    /// Timestamp when current state was entered.
    state_entered_ms: u64,
}

impl Portal {
    /// Create a new portal with default settings.
    pub fn new(
        portal_id: u64,
        source: AetherUrl,
        destination: AetherUrl,
        position: [f32; 3],
    ) -> Self {
        Self {
            portal_id,
            source,
            destination,
            position,
            radius: DEFAULT_PORTAL_RADIUS,
            shape: PortalShape::Circle {
                visual_radius: DEFAULT_PORTAL_RADIUS,
            },
            activation_mode: ActivationMode::ProximityAndConfirm,
            enabled: true,
            state: PortalState::Idle,
            cooldown_ms: DEFAULT_PORTAL_COOLDOWN_MS,
            transition_timeout_ms: DEFAULT_TRANSITION_TIMEOUT_MS,
            state_entered_ms: 0,
        }
    }

    pub fn with_radius(mut self, radius: f32) -> Self {
        self.radius = radius;
        self
    }

    pub fn with_shape(mut self, shape: PortalShape) -> Self {
        self.shape = shape;
        self
    }

    pub fn with_activation_mode(mut self, mode: ActivationMode) -> Self {
        self.activation_mode = mode;
        self
    }

    pub fn with_cooldown_ms(mut self, ms: u64) -> Self {
        self.cooldown_ms = ms;
        self
    }

    pub fn with_transition_timeout_ms(mut self, ms: u64) -> Self {
        self.transition_timeout_ms = ms;
        self
    }

    /// Get current portal state.
    pub fn state(&self) -> &PortalState {
        &self.state
    }

    /// Check whether a player position is within the portal's proximity radius.
    pub fn is_in_proximity(&self, player_pos: &[f32; 3]) -> bool {
        let dx = player_pos[0] - self.position[0];
        let dy = player_pos[1] - self.position[1];
        let dz = player_pos[2] - self.position[2];
        let dist_sq = dx * dx + dy * dy + dz * dz;
        dist_sq <= self.radius * self.radius
    }

    /// Notify the portal that a player has entered proximity.
    /// Returns the new state if a transition occurred.
    pub fn on_player_enter_proximity(&mut self, now_ms: u64) -> Option<PortalState> {
        if !self.enabled {
            return None;
        }

        match &self.state {
            PortalState::Idle => {
                if self.activation_mode == ActivationMode::ProximityOnly {
                    self.set_state(PortalState::Activating, now_ms);
                } else {
                    self.set_state(PortalState::Proximity, now_ms);
                }
                Some(self.state.clone())
            }
            PortalState::Cooldown { until_ms } => {
                if now_ms >= *until_ms {
                    // Cooldown expired; treat as idle
                    self.set_state(PortalState::Idle, now_ms);
                    self.on_player_enter_proximity(now_ms)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Notify the portal that a player has left proximity.
    pub fn on_player_leave_proximity(&mut self, now_ms: u64) -> Option<PortalState> {
        match &self.state {
            PortalState::Proximity => {
                self.set_state(PortalState::Idle, now_ms);
                Some(self.state.clone())
            }
            _ => None,
        }
    }

    /// Player pressed the interaction key while in proximity.
    pub fn on_interact(&mut self, now_ms: u64) -> Option<PortalState> {
        if !self.enabled {
            return None;
        }

        match &self.state {
            PortalState::Proximity => {
                match self.activation_mode {
                    ActivationMode::InteractionRequired | ActivationMode::ProximityAndConfirm => {
                        self.set_state(PortalState::Activating, now_ms);
                        Some(self.state.clone())
                    }
                    ActivationMode::ProximityOnly => {
                        // Already should have activated on proximity
                        None
                    }
                }
            }
            _ => None,
        }
    }

    /// Mark the portal as having started the transition (handoff in progress).
    pub fn on_transition_started(&mut self, now_ms: u64) -> Option<PortalState> {
        match &self.state {
            PortalState::Activating => {
                self.set_state(PortalState::TransitionStarted, now_ms);
                Some(self.state.clone())
            }
            _ => None,
        }
    }

    /// Mark the portal transition as completed.
    pub fn on_transition_completed(&mut self, now_ms: u64) -> Option<PortalState> {
        match &self.state {
            PortalState::TransitionStarted => {
                self.set_state(PortalState::Completed, now_ms);
                Some(self.state.clone())
            }
            _ => None,
        }
    }

    /// Mark the portal transition as failed and enter cooldown.
    pub fn on_transition_failed(&mut self, reason: String, now_ms: u64) -> Option<PortalState> {
        match &self.state {
            PortalState::Activating | PortalState::TransitionStarted => {
                self.set_state(
                    PortalState::Failed {
                        reason: reason.clone(),
                    },
                    now_ms,
                );
                // Immediately enter cooldown
                self.set_state(
                    PortalState::Cooldown {
                        until_ms: now_ms + self.cooldown_ms,
                    },
                    now_ms,
                );
                Some(self.state.clone())
            }
            _ => None,
        }
    }

    /// Reset the portal to idle (e.g., after player fully arrives at destination).
    pub fn reset(&mut self, now_ms: u64) {
        self.set_state(PortalState::Idle, now_ms);
    }

    /// Check if the transition has timed out.
    pub fn is_transition_timed_out(&self, now_ms: u64) -> bool {
        match &self.state {
            PortalState::Activating | PortalState::TransitionStarted => {
                now_ms.saturating_sub(self.state_entered_ms) > self.transition_timeout_ms
            }
            _ => false,
        }
    }

    /// Time spent in the current state (milliseconds).
    pub fn time_in_state(&self, now_ms: u64) -> u64 {
        now_ms.saturating_sub(self.state_entered_ms)
    }

    fn set_state(&mut self, new_state: PortalState, now_ms: u64) {
        self.state = new_state;
        self.state_entered_ms = now_ms;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_source() -> AetherUrl {
        AetherUrl::parse("aether://host/source-world").unwrap()
    }

    fn make_dest() -> AetherUrl {
        AetherUrl::parse("aether://host/dest-world").unwrap()
    }

    fn make_portal() -> Portal {
        Portal::new(1, make_source(), make_dest(), [0.0, 0.0, 0.0])
    }

    // --- Proximity detection ---

    #[test]
    fn player_inside_radius() {
        let portal = make_portal().with_radius(5.0);
        assert!(portal.is_in_proximity(&[3.0, 0.0, 0.0]));
    }

    #[test]
    fn player_at_exact_radius() {
        let portal = make_portal().with_radius(5.0);
        assert!(portal.is_in_proximity(&[5.0, 0.0, 0.0]));
    }

    #[test]
    fn player_outside_radius() {
        let portal = make_portal().with_radius(5.0);
        assert!(!portal.is_in_proximity(&[6.0, 0.0, 0.0]));
    }

    #[test]
    fn proximity_3d_distance() {
        let portal = make_portal().with_radius(5.0);
        // sqrt(3^2 + 3^2 + 3^2) = sqrt(27) ~= 5.196 > 5.0
        assert!(!portal.is_in_proximity(&[3.0, 3.0, 3.0]));
        // sqrt(2^2 + 2^2 + 2^2) = sqrt(12) ~= 3.46 < 5.0
        assert!(portal.is_in_proximity(&[2.0, 2.0, 2.0]));
    }

    // --- State machine: ProximityAndConfirm ---

    #[test]
    fn initial_state_is_idle() {
        let portal = make_portal();
        assert_eq!(*portal.state(), PortalState::Idle);
    }

    #[test]
    fn proximity_and_confirm_lifecycle() {
        let mut portal = make_portal().with_activation_mode(ActivationMode::ProximityAndConfirm);

        // Enter proximity
        let result = portal.on_player_enter_proximity(100);
        assert_eq!(result, Some(PortalState::Proximity));

        // Interact
        let result = portal.on_interact(200);
        assert_eq!(result, Some(PortalState::Activating));

        // Transition started
        let result = portal.on_transition_started(300);
        assert_eq!(result, Some(PortalState::TransitionStarted));

        // Completed
        let result = portal.on_transition_completed(400);
        assert_eq!(result, Some(PortalState::Completed));
    }

    // --- State machine: ProximityOnly ---

    #[test]
    fn proximity_only_auto_activates() {
        let mut portal = make_portal().with_activation_mode(ActivationMode::ProximityOnly);

        let result = portal.on_player_enter_proximity(100);
        assert_eq!(result, Some(PortalState::Activating));
    }

    // --- State machine: InteractionRequired ---

    #[test]
    fn interaction_required_needs_interact() {
        let mut portal = make_portal().with_activation_mode(ActivationMode::InteractionRequired);

        let result = portal.on_player_enter_proximity(100);
        assert_eq!(result, Some(PortalState::Proximity));

        let result = portal.on_interact(200);
        assert_eq!(result, Some(PortalState::Activating));
    }

    // --- Leave proximity ---

    #[test]
    fn leave_proximity_resets_to_idle() {
        let mut portal = make_portal();
        portal.on_player_enter_proximity(100);

        let result = portal.on_player_leave_proximity(200);
        assert_eq!(result, Some(PortalState::Idle));
    }

    #[test]
    fn leave_proximity_no_effect_when_activating() {
        let mut portal = make_portal();
        portal.on_player_enter_proximity(100);
        portal.on_interact(200);

        let result = portal.on_player_leave_proximity(300);
        assert_eq!(result, None);
        assert_eq!(*portal.state(), PortalState::Activating);
    }

    // --- Failure and cooldown ---

    #[test]
    fn failure_enters_cooldown() {
        let mut portal = make_portal().with_cooldown_ms(1000);
        portal.on_player_enter_proximity(100);
        portal.on_interact(200);

        let result = portal.on_transition_failed("server down".to_string(), 300);
        assert!(result.is_some());
        assert!(matches!(
            *portal.state(),
            PortalState::Cooldown { until_ms: 1300 }
        ));
    }

    #[test]
    fn cooldown_blocks_reactivation() {
        let mut portal = make_portal().with_cooldown_ms(1000);
        portal.on_player_enter_proximity(100);
        portal.on_interact(200);
        portal.on_transition_failed("err".to_string(), 300);

        // Try to enter proximity during cooldown
        let result = portal.on_player_enter_proximity(500);
        assert_eq!(result, None);
    }

    #[test]
    fn cooldown_expires_allows_reactivation() {
        let mut portal = make_portal().with_cooldown_ms(1000);
        portal.on_player_enter_proximity(100);
        portal.on_interact(200);
        portal.on_transition_failed("err".to_string(), 300);

        // After cooldown expires
        let result = portal.on_player_enter_proximity(1400);
        assert!(result.is_some());
    }

    // --- Disabled portal ---

    #[test]
    fn disabled_portal_ignores_proximity() {
        let mut portal = make_portal();
        portal.enabled = false;

        let result = portal.on_player_enter_proximity(100);
        assert_eq!(result, None);
    }

    #[test]
    fn disabled_portal_ignores_interact() {
        let mut portal = make_portal();
        portal.on_player_enter_proximity(100);
        portal.enabled = false;

        let result = portal.on_interact(200);
        assert_eq!(result, None);
    }

    // --- Transition timeout ---

    #[test]
    fn transition_timeout_detection() {
        let mut portal = make_portal().with_transition_timeout_ms(5000);
        portal.on_player_enter_proximity(100);
        portal.on_interact(200);

        assert!(!portal.is_transition_timed_out(3000));
        assert!(portal.is_transition_timed_out(5300));
    }

    #[test]
    fn timeout_not_detected_in_idle() {
        let portal = make_portal().with_transition_timeout_ms(5000);
        assert!(!portal.is_transition_timed_out(99999));
    }

    // --- Invalid transitions ---

    #[test]
    fn interact_in_idle_has_no_effect() {
        let mut portal = make_portal();
        let result = portal.on_interact(100);
        assert_eq!(result, None);
    }

    #[test]
    fn transition_started_from_idle_has_no_effect() {
        let mut portal = make_portal();
        let result = portal.on_transition_started(100);
        assert_eq!(result, None);
    }

    #[test]
    fn completed_from_idle_has_no_effect() {
        let mut portal = make_portal();
        let result = portal.on_transition_completed(100);
        assert_eq!(result, None);
    }

    #[test]
    fn failed_from_idle_has_no_effect() {
        let mut portal = make_portal();
        let result = portal.on_transition_failed("err".to_string(), 100);
        assert_eq!(result, None);
    }

    // --- Time in state ---

    #[test]
    fn time_in_state_tracking() {
        let mut portal = make_portal();
        portal.on_player_enter_proximity(1000);
        assert_eq!(portal.time_in_state(1500), 500);
    }

    // --- Reset ---

    #[test]
    fn reset_returns_to_idle() {
        let mut portal = make_portal();
        portal.on_player_enter_proximity(100);
        portal.on_interact(200);
        portal.reset(300);
        assert_eq!(*portal.state(), PortalState::Idle);
    }

    // --- Builder pattern ---

    #[test]
    fn builder_methods() {
        let portal = make_portal()
            .with_radius(10.0)
            .with_shape(PortalShape::Rectangle {
                width: 4.0,
                height: 6.0,
            })
            .with_activation_mode(ActivationMode::ProximityOnly)
            .with_cooldown_ms(2000)
            .with_transition_timeout_ms(8000);

        assert_eq!(portal.radius, 10.0);
        assert_eq!(
            portal.shape,
            PortalShape::Rectangle {
                width: 4.0,
                height: 6.0
            }
        );
        assert_eq!(portal.activation_mode, ActivationMode::ProximityOnly);
        assert_eq!(portal.cooldown_ms, 2000);
        assert_eq!(portal.transition_timeout_ms, 8000);
    }

    // --- Failure from TransitionStarted ---

    #[test]
    fn failure_from_transition_started() {
        let mut portal = make_portal().with_cooldown_ms(1000);
        portal.on_player_enter_proximity(100);
        portal.on_interact(200);
        portal.on_transition_started(300);

        let result = portal.on_transition_failed("network error".to_string(), 400);
        assert!(result.is_some());
        assert!(matches!(*portal.state(), PortalState::Cooldown { .. }));
    }

    // --- Default constants ---

    #[test]
    fn default_constants() {
        assert_eq!(DEFAULT_PORTAL_RADIUS, 3.0);
        assert_eq!(DEFAULT_PORTAL_COOLDOWN_MS, 5_000);
        assert_eq!(DEFAULT_TRANSITION_TIMEOUT_MS, 15_000);
    }
}
