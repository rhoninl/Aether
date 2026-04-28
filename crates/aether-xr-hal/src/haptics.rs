//! Haptic feedback trait + value types (design doc §5.7, P1-C/P2-C).
//!
//! Naming: `HapticEffect` already exists in `aether-input::haptics` as a
//! high-level pattern enum (Click/Impact/Buzz/Custom). The HAL uses
//! `HapticPulse` for the OpenXR `XR_TYPE_HAPTIC_VIBRATION` parameter struct,
//! and `HapticAction` for the dispatcher's per-request output. Both names live
//! here so backends and the dispatcher see the same types.

pub const MIN_HAPTIC_AMPLITUDE: f32 = 0.0;
pub const MAX_HAPTIC_AMPLITUDE: f32 = 1.0;

/// Identifies which haptic actuator(s) the effect should target. In OpenXR
/// this is one or more subaction paths (`/user/hand/left`, `/user/hand/right`)
/// attached to a haptic action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HapticTarget {
    Left,
    Right,
    Both,
}

/// One OpenXR haptic-vibration call. `frequency_hz == 0.0` lets the runtime
/// pick its preferred frequency (matches `XR_FREQUENCY_UNSPECIFIED`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HapticPulse {
    pub duration_ns: u64,
    pub frequency_hz: f32,
    pub amplitude: f32,
}

/// High-level dispatcher output carrying target, duration (ms), amplitude,
/// frequency, and loop flag. The dispatcher in `aether-input::openxr_haptics`
/// produces these; backends translate to one or more [`HapticPulse`] calls.
#[derive(Debug, Clone)]
pub struct HapticAction {
    pub target: HapticTarget,
    pub duration_ms: u32,
    pub amplitude: f32,
    pub frequency_hz: f32,
    pub looped: bool,
}

/// Clamp an amplitude into the OpenXR-permitted [0.0, 1.0] range.
pub fn clamp_amplitude(amplitude: f32) -> f32 {
    amplitude.clamp(MIN_HAPTIC_AMPLITUDE, MAX_HAPTIC_AMPLITUDE)
}

/// Submits and stops haptic feedback on a session's haptic actions.
pub trait XrHaptics {
    type Error: std::error::Error + Send + Sync + 'static;

    /// `xrApplyHapticFeedback` with `XR_TYPE_HAPTIC_VIBRATION`.
    fn apply(&self, target: HapticTarget, pulse: HapticPulse) -> Result<(), Self::Error>;

    /// `xrStopHapticFeedback`.
    fn stop(&self, target: HapticTarget) -> Result<(), Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn haptic_pulse_is_copy() {
        let p = HapticPulse {
            duration_ns: 100_000_000,
            frequency_hz: 320.0,
            amplitude: 0.5,
        };
        let copy = p;
        assert_eq!(copy.duration_ns, 100_000_000);
    }

    #[test]
    fn clamp_amplitude_caps_inputs() {
        assert_eq!(clamp_amplitude(-0.5), 0.0);
        assert_eq!(clamp_amplitude(0.5), 0.5);
        assert_eq!(clamp_amplitude(1.5), 1.0);
    }
}
