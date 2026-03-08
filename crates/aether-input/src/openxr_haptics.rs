//! OpenXR haptic output dispatch and cooldown management.
//!
//! Converts `HapticRequest` values into OpenXR-compatible haptic actions,
//! enforcing cooldown periods and amplitude clamping.

use crate::haptics::{HapticChannel, HapticEffect, HapticRequest, HapticWave};

/// Default haptic pulse duration in milliseconds.
pub const DEFAULT_HAPTIC_DURATION_MS: u32 = 100;

/// Default haptic amplitude [0.0, 1.0].
pub const DEFAULT_HAPTIC_AMPLITUDE: f32 = 0.5;

/// Minimum allowed haptic amplitude.
pub const MIN_HAPTIC_AMPLITUDE: f32 = 0.0;

/// Maximum allowed haptic amplitude.
pub const MAX_HAPTIC_AMPLITUDE: f32 = 1.0;

/// Default click haptic duration in milliseconds.
pub const CLICK_DURATION_MS: u32 = 20;

/// Default impact haptic duration in milliseconds.
pub const IMPACT_DURATION_MS: u32 = 50;

/// Default buzz haptic duration in milliseconds.
pub const BUZZ_DURATION_MS: u32 = 100;

/// Default click haptic amplitude.
pub const CLICK_AMPLITUDE: f32 = 0.4;

/// Default impact haptic amplitude.
pub const IMPACT_AMPLITUDE: f32 = 1.0;

/// Default buzz haptic amplitude.
pub const BUZZ_AMPLITUDE: f32 = 0.3;

/// Default buzz haptic frequency in Hz.
pub const BUZZ_FREQUENCY_HZ: f32 = 160.0;

/// An OpenXR-compatible haptic action ready for submission to the runtime.
#[derive(Debug, Clone)]
pub struct HapticAction {
    /// Target hand(s) for this haptic action.
    pub target: HapticTarget,
    /// Duration of the haptic in milliseconds.
    pub duration_ms: u32,
    /// Amplitude of the haptic [0.0, 1.0].
    pub amplitude: f32,
    /// Frequency in Hz (0 for runtime default).
    pub frequency_hz: f32,
    /// Whether this haptic should loop.
    pub looped: bool,
}

/// Target hand(s) for a haptic action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HapticTarget {
    Left,
    Right,
    Both,
}

/// Per-hand cooldown state for haptic dispatch.
#[derive(Debug, Clone)]
struct HandCooldown {
    /// Timestamp (ms) when the last haptic was dispatched.
    last_dispatch_ms: u64,
    /// Cooldown duration (ms) before another haptic can be dispatched.
    cooldown_ms: u32,
}

impl HandCooldown {
    fn new() -> Self {
        Self {
            last_dispatch_ms: 0,
            cooldown_ms: 0,
        }
    }

    /// Check if a new haptic can be dispatched at the given time.
    fn can_dispatch(&self, now_ms: u64) -> bool {
        if self.last_dispatch_ms == 0 {
            return true;
        }
        now_ms >= self.last_dispatch_ms + self.cooldown_ms as u64
    }

    /// Record a dispatch at the given time with the given cooldown.
    fn record_dispatch(&mut self, now_ms: u64, cooldown_ms: u32) {
        self.last_dispatch_ms = now_ms;
        self.cooldown_ms = cooldown_ms;
    }
}

/// Dispatches haptic requests to OpenXR-compatible actions.
///
/// Manages per-hand cooldowns and converts high-level `HapticRequest`
/// into `HapticAction` values ready for runtime submission.
#[derive(Debug)]
pub struct HapticDispatcher {
    left_cooldown: HandCooldown,
    right_cooldown: HandCooldown,
    enabled: bool,
    total_dispatched: u64,
    total_suppressed: u64,
}

impl HapticDispatcher {
    /// Create a new haptic dispatcher.
    pub fn new(enabled: bool) -> Self {
        Self {
            left_cooldown: HandCooldown::new(),
            right_cooldown: HandCooldown::new(),
            enabled,
            total_dispatched: 0,
            total_suppressed: 0,
        }
    }

    /// Whether haptics are enabled.
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Set whether haptics are enabled.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Total number of haptic actions dispatched.
    pub fn total_dispatched(&self) -> u64 {
        self.total_dispatched
    }

    /// Total number of haptic requests suppressed due to cooldown.
    pub fn total_suppressed(&self) -> u64 {
        self.total_suppressed
    }

    /// Dispatch a haptic request, returning the action if cooldown allows.
    ///
    /// Returns `None` if haptics are disabled or the request is suppressed
    /// by cooldown.
    pub fn dispatch(
        &mut self,
        request: &HapticRequest,
        now_ms: u64,
    ) -> Option<HapticAction> {
        if !self.enabled {
            self.total_suppressed = self.total_suppressed.saturating_add(1);
            return None;
        }

        let target = channel_to_target(&request.channel);

        // Check cooldown for targeted hand(s)
        if !self.check_cooldown(target, now_ms) {
            self.total_suppressed = self.total_suppressed.saturating_add(1);
            return None;
        }

        let action = effect_to_action(&request.effect, target, request.looped);

        // Record dispatch and cooldown
        self.record_cooldown(target, now_ms, request.cooldown_ms);
        self.total_dispatched = self.total_dispatched.saturating_add(1);

        Some(action)
    }

    /// Dispatch multiple haptic requests, returning all successfully dispatched actions.
    pub fn dispatch_batch(
        &mut self,
        requests: &[HapticRequest],
        now_ms: u64,
    ) -> Vec<HapticAction> {
        requests
            .iter()
            .filter_map(|req| self.dispatch(req, now_ms))
            .collect()
    }

    /// Reset all cooldown timers.
    pub fn reset_cooldowns(&mut self) {
        self.left_cooldown = HandCooldown::new();
        self.right_cooldown = HandCooldown::new();
    }

    /// Check if a dispatch to the given target is allowed by cooldown.
    fn check_cooldown(&self, target: HapticTarget, now_ms: u64) -> bool {
        match target {
            HapticTarget::Left => self.left_cooldown.can_dispatch(now_ms),
            HapticTarget::Right => self.right_cooldown.can_dispatch(now_ms),
            HapticTarget::Both => {
                self.left_cooldown.can_dispatch(now_ms)
                    && self.right_cooldown.can_dispatch(now_ms)
            }
        }
    }

    /// Record a dispatch to the given target with cooldown.
    fn record_cooldown(&mut self, target: HapticTarget, now_ms: u64, cooldown_ms: u32) {
        match target {
            HapticTarget::Left => {
                self.left_cooldown.record_dispatch(now_ms, cooldown_ms);
            }
            HapticTarget::Right => {
                self.right_cooldown.record_dispatch(now_ms, cooldown_ms);
            }
            HapticTarget::Both => {
                self.left_cooldown.record_dispatch(now_ms, cooldown_ms);
                self.right_cooldown.record_dispatch(now_ms, cooldown_ms);
            }
        }
    }
}

/// Convert a `HapticChannel` to a `HapticTarget`.
fn channel_to_target(channel: &HapticChannel) -> HapticTarget {
    match channel {
        HapticChannel::Left => HapticTarget::Left,
        HapticChannel::Right => HapticTarget::Right,
        HapticChannel::Combined => HapticTarget::Both,
    }
}

/// Convert a `HapticEffect` to a `HapticAction`.
fn effect_to_action(effect: &HapticEffect, target: HapticTarget, looped: bool) -> HapticAction {
    match effect {
        HapticEffect::Click => HapticAction {
            target,
            duration_ms: CLICK_DURATION_MS,
            amplitude: CLICK_AMPLITUDE,
            frequency_hz: 0.0,
            looped,
        },
        HapticEffect::Impact => HapticAction {
            target,
            duration_ms: IMPACT_DURATION_MS,
            amplitude: IMPACT_AMPLITUDE,
            frequency_hz: 0.0,
            looped,
        },
        HapticEffect::Buzz => HapticAction {
            target,
            duration_ms: BUZZ_DURATION_MS,
            amplitude: BUZZ_AMPLITUDE,
            frequency_hz: BUZZ_FREQUENCY_HZ,
            looped,
        },
        HapticEffect::Custom(wave) => wave_to_action(wave, target, looped),
    }
}

/// Convert a `HapticWave` to a `HapticAction`.
fn wave_to_action(wave: &HapticWave, target: HapticTarget, looped: bool) -> HapticAction {
    match wave {
        HapticWave::Sine { freq_hz, amplitude } => HapticAction {
            target,
            duration_ms: DEFAULT_HAPTIC_DURATION_MS,
            amplitude: amplitude.clamp(MIN_HAPTIC_AMPLITUDE, MAX_HAPTIC_AMPLITUDE),
            frequency_hz: *freq_hz,
            looped,
        },
        HapticWave::Pulse {
            amplitude,
            duration_ms,
        } => HapticAction {
            target,
            duration_ms: *duration_ms as u32,
            amplitude: amplitude.clamp(MIN_HAPTIC_AMPLITUDE, MAX_HAPTIC_AMPLITUDE),
            frequency_hz: 0.0,
            looped,
        },
        HapticWave::Ramp {
            from_amplitude,
            to_amplitude: _,
            duration_ms,
        } => HapticAction {
            target,
            duration_ms: *duration_ms as u32,
            amplitude: from_amplitude.clamp(MIN_HAPTIC_AMPLITUDE, MAX_HAPTIC_AMPLITUDE),
            frequency_hz: 0.0,
            looped,
        },
    }
}

/// Clamp an amplitude value to the valid [0.0, 1.0] range.
pub fn clamp_amplitude(amplitude: f32) -> f32 {
    amplitude.clamp(MIN_HAPTIC_AMPLITUDE, MAX_HAPTIC_AMPLITUDE)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_request(
        channel: HapticChannel,
        effect: HapticEffect,
        cooldown_ms: u32,
    ) -> HapticRequest {
        HapticRequest {
            player_id: 1,
            channel,
            effect,
            cooldown_ms,
            looped: false,
        }
    }

    // ---- channel_to_target ----

    #[test]
    fn channel_left_maps_to_target_left() {
        assert_eq!(channel_to_target(&HapticChannel::Left), HapticTarget::Left);
    }

    #[test]
    fn channel_right_maps_to_target_right() {
        assert_eq!(
            channel_to_target(&HapticChannel::Right),
            HapticTarget::Right
        );
    }

    #[test]
    fn channel_combined_maps_to_target_both() {
        assert_eq!(
            channel_to_target(&HapticChannel::Combined),
            HapticTarget::Both
        );
    }

    // ---- effect_to_action ----

    #[test]
    fn click_effect_produces_short_pulse() {
        let action = effect_to_action(&HapticEffect::Click, HapticTarget::Left, false);
        assert_eq!(action.duration_ms, CLICK_DURATION_MS);
        assert_eq!(action.amplitude, CLICK_AMPLITUDE);
        assert_eq!(action.frequency_hz, 0.0);
        assert!(!action.looped);
    }

    #[test]
    fn impact_effect_produces_strong_pulse() {
        let action = effect_to_action(&HapticEffect::Impact, HapticTarget::Right, false);
        assert_eq!(action.duration_ms, IMPACT_DURATION_MS);
        assert_eq!(action.amplitude, IMPACT_AMPLITUDE);
    }

    #[test]
    fn buzz_effect_produces_sustained_vibration() {
        let action = effect_to_action(&HapticEffect::Buzz, HapticTarget::Both, false);
        assert_eq!(action.duration_ms, BUZZ_DURATION_MS);
        assert_eq!(action.amplitude, BUZZ_AMPLITUDE);
        assert_eq!(action.frequency_hz, BUZZ_FREQUENCY_HZ);
    }

    #[test]
    fn custom_sine_wave_effect() {
        let wave = HapticWave::Sine {
            freq_hz: 200.0,
            amplitude: 0.7,
        };
        let action = effect_to_action(
            &HapticEffect::Custom(wave),
            HapticTarget::Left,
            false,
        );
        assert_eq!(action.frequency_hz, 200.0);
        assert_eq!(action.amplitude, 0.7);
        assert_eq!(action.duration_ms, DEFAULT_HAPTIC_DURATION_MS);
    }

    #[test]
    fn custom_pulse_wave_effect() {
        let wave = HapticWave::Pulse {
            amplitude: 0.9,
            duration_ms: 150,
        };
        let action = effect_to_action(
            &HapticEffect::Custom(wave),
            HapticTarget::Right,
            false,
        );
        assert_eq!(action.amplitude, 0.9);
        assert_eq!(action.duration_ms, 150);
    }

    #[test]
    fn custom_ramp_wave_effect() {
        let wave = HapticWave::Ramp {
            from_amplitude: 0.2,
            to_amplitude: 0.8,
            duration_ms: 200,
        };
        let action = effect_to_action(
            &HapticEffect::Custom(wave),
            HapticTarget::Both,
            true,
        );
        assert_eq!(action.amplitude, 0.2);
        assert_eq!(action.duration_ms, 200);
        assert!(action.looped);
    }

    #[test]
    fn looped_effect_sets_looped_flag() {
        let action = effect_to_action(&HapticEffect::Buzz, HapticTarget::Left, true);
        assert!(action.looped);
    }

    // ---- Amplitude clamping ----

    #[test]
    fn clamp_amplitude_within_range() {
        assert_eq!(clamp_amplitude(0.5), 0.5);
    }

    #[test]
    fn clamp_amplitude_below_min() {
        assert_eq!(clamp_amplitude(-0.5), MIN_HAPTIC_AMPLITUDE);
    }

    #[test]
    fn clamp_amplitude_above_max() {
        assert_eq!(clamp_amplitude(1.5), MAX_HAPTIC_AMPLITUDE);
    }

    #[test]
    fn custom_sine_amplitude_clamped() {
        let wave = HapticWave::Sine {
            freq_hz: 100.0,
            amplitude: 2.0,
        };
        let action = effect_to_action(
            &HapticEffect::Custom(wave),
            HapticTarget::Left,
            false,
        );
        assert_eq!(action.amplitude, MAX_HAPTIC_AMPLITUDE);
    }

    #[test]
    fn custom_sine_negative_amplitude_clamped() {
        let wave = HapticWave::Sine {
            freq_hz: 100.0,
            amplitude: -1.0,
        };
        let action = effect_to_action(
            &HapticEffect::Custom(wave),
            HapticTarget::Left,
            false,
        );
        assert_eq!(action.amplitude, MIN_HAPTIC_AMPLITUDE);
    }

    // ---- HapticDispatcher basic ----

    #[test]
    fn new_dispatcher_has_zero_counts() {
        let disp = HapticDispatcher::new(true);
        assert_eq!(disp.total_dispatched(), 0);
        assert_eq!(disp.total_suppressed(), 0);
    }

    #[test]
    fn disabled_dispatcher_suppresses_all() {
        let mut disp = HapticDispatcher::new(false);
        let req = make_request(HapticChannel::Left, HapticEffect::Click, 0);
        let result = disp.dispatch(&req, 100);
        assert!(result.is_none());
        assert_eq!(disp.total_suppressed(), 1);
        assert_eq!(disp.total_dispatched(), 0);
    }

    #[test]
    fn enabled_dispatcher_dispatches() {
        let mut disp = HapticDispatcher::new(true);
        let req = make_request(HapticChannel::Left, HapticEffect::Click, 0);
        let result = disp.dispatch(&req, 100);
        assert!(result.is_some());
        assert_eq!(disp.total_dispatched(), 1);
        assert_eq!(disp.total_suppressed(), 0);
    }

    #[test]
    fn dispatched_action_has_correct_target() {
        let mut disp = HapticDispatcher::new(true);
        let req = make_request(HapticChannel::Right, HapticEffect::Impact, 0);
        let action = disp.dispatch(&req, 100).unwrap();
        assert_eq!(action.target, HapticTarget::Right);
    }

    // ---- Cooldown enforcement ----

    #[test]
    fn cooldown_blocks_rapid_dispatch() {
        let mut disp = HapticDispatcher::new(true);
        let req = make_request(HapticChannel::Left, HapticEffect::Click, 50);

        // First dispatch succeeds
        assert!(disp.dispatch(&req, 100).is_some());
        // Second dispatch within cooldown is suppressed
        assert!(disp.dispatch(&req, 120).is_none());
        assert_eq!(disp.total_suppressed(), 1);
    }

    #[test]
    fn cooldown_allows_dispatch_after_expiry() {
        let mut disp = HapticDispatcher::new(true);
        let req = make_request(HapticChannel::Left, HapticEffect::Click, 50);

        assert!(disp.dispatch(&req, 100).is_some());
        // After cooldown expires
        assert!(disp.dispatch(&req, 160).is_some());
        assert_eq!(disp.total_dispatched(), 2);
    }

    #[test]
    fn cooldown_at_exact_expiry_allows_dispatch() {
        let mut disp = HapticDispatcher::new(true);
        let req = make_request(HapticChannel::Left, HapticEffect::Click, 50);

        assert!(disp.dispatch(&req, 100).is_some());
        // Exactly at cooldown boundary
        assert!(disp.dispatch(&req, 150).is_some());
    }

    #[test]
    fn left_cooldown_does_not_block_right() {
        let mut disp = HapticDispatcher::new(true);
        let left_req = make_request(HapticChannel::Left, HapticEffect::Click, 100);
        let right_req = make_request(HapticChannel::Right, HapticEffect::Click, 100);

        assert!(disp.dispatch(&left_req, 100).is_some());
        // Right hand should still dispatch even though left is on cooldown
        assert!(disp.dispatch(&right_req, 110).is_some());
        assert_eq!(disp.total_dispatched(), 2);
    }

    #[test]
    fn combined_cooldown_blocks_both_hands() {
        let mut disp = HapticDispatcher::new(true);
        let combined_req = make_request(HapticChannel::Combined, HapticEffect::Impact, 100);
        let left_req = make_request(HapticChannel::Left, HapticEffect::Click, 0);

        assert!(disp.dispatch(&combined_req, 100).is_some());
        // Left hand should be on cooldown from combined dispatch
        assert!(disp.dispatch(&left_req, 110).is_none());
    }

    #[test]
    fn combined_blocked_if_either_hand_on_cooldown() {
        let mut disp = HapticDispatcher::new(true);
        let left_req = make_request(HapticChannel::Left, HapticEffect::Click, 100);
        let combined_req = make_request(HapticChannel::Combined, HapticEffect::Impact, 0);

        assert!(disp.dispatch(&left_req, 100).is_some());
        // Combined should be blocked because left is on cooldown
        assert!(disp.dispatch(&combined_req, 110).is_none());
    }

    #[test]
    fn zero_cooldown_allows_immediate_redispatch() {
        let mut disp = HapticDispatcher::new(true);
        let req = make_request(HapticChannel::Left, HapticEffect::Click, 0);

        assert!(disp.dispatch(&req, 100).is_some());
        assert!(disp.dispatch(&req, 100).is_some());
        assert_eq!(disp.total_dispatched(), 2);
    }

    // ---- set_enabled ----

    #[test]
    fn set_enabled_toggles_dispatch() {
        let mut disp = HapticDispatcher::new(true);
        let req = make_request(HapticChannel::Left, HapticEffect::Click, 0);

        assert!(disp.dispatch(&req, 100).is_some());

        disp.set_enabled(false);
        assert!(disp.dispatch(&req, 200).is_none());

        disp.set_enabled(true);
        assert!(disp.dispatch(&req, 300).is_some());
    }

    // ---- reset_cooldowns ----

    #[test]
    fn reset_cooldowns_allows_immediate_dispatch() {
        let mut disp = HapticDispatcher::new(true);
        let req = make_request(HapticChannel::Left, HapticEffect::Click, 1000);

        assert!(disp.dispatch(&req, 100).is_some());
        assert!(disp.dispatch(&req, 110).is_none());

        disp.reset_cooldowns();
        assert!(disp.dispatch(&req, 120).is_some());
    }

    // ---- dispatch_batch ----

    #[test]
    fn dispatch_batch_processes_multiple_requests() {
        let mut disp = HapticDispatcher::new(true);
        let requests = vec![
            make_request(HapticChannel::Left, HapticEffect::Click, 0),
            make_request(HapticChannel::Right, HapticEffect::Impact, 0),
        ];
        let actions = disp.dispatch_batch(&requests, 100);
        assert_eq!(actions.len(), 2);
        assert_eq!(disp.total_dispatched(), 2);
    }

    #[test]
    fn dispatch_batch_respects_cooldown() {
        let mut disp = HapticDispatcher::new(true);
        let requests = vec![
            make_request(HapticChannel::Left, HapticEffect::Click, 100),
            make_request(HapticChannel::Left, HapticEffect::Impact, 100),
        ];
        let actions = disp.dispatch_batch(&requests, 100);
        // Second request to same hand should be suppressed by cooldown
        assert_eq!(actions.len(), 1);
        assert_eq!(disp.total_dispatched(), 1);
        assert_eq!(disp.total_suppressed(), 1);
    }

    #[test]
    fn dispatch_batch_empty_returns_empty() {
        let mut disp = HapticDispatcher::new(true);
        let actions = disp.dispatch_batch(&[], 100);
        assert!(actions.is_empty());
    }

    // ---- Looped request ----

    #[test]
    fn looped_request_produces_looped_action() {
        let mut disp = HapticDispatcher::new(true);
        let req = HapticRequest {
            player_id: 1,
            channel: HapticChannel::Left,
            effect: HapticEffect::Buzz,
            cooldown_ms: 0,
            looped: true,
        };
        let action = disp.dispatch(&req, 100).unwrap();
        assert!(action.looped);
    }
}
