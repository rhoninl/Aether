//! Haptic feedback trait and value types (design doc §5.7).

// TODO(P1-C): the design moves `HapticTarget` / `HapticEffect` from
// `aether-input::openxr_haptics` into this crate. P1-C owns the canonical
// definitions; the placeholders below let P2-C (the trait surface) compile in
// isolation. When P1-C lands, replace these with the merged value types.

/// Identifies which haptic actuator the effect should target.
///
/// In OpenXR this is a subaction path (e.g. `/user/hand/left`) attached to a
/// haptic action. The HAL exposes it as an explicit value type so application
/// code never has to deal with raw path strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HapticTarget {
    LeftHand,
    RightHand,
}

/// Parameters for a single haptic pulse (`xrApplyHapticFeedback` with an
/// `XrHapticVibration`).
///
/// `frequency_hz == 0.0` lets the runtime pick its preferred frequency, matching
/// `XR_FREQUENCY_UNSPECIFIED`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HapticEffect {
    /// Vibration duration in nanoseconds.
    pub duration_ns: u64,
    /// Frequency in Hz, or `0.0` for runtime-default.
    pub frequency_hz: f32,
    /// Amplitude in `[0.0, 1.0]`.
    pub amplitude: f32,
}

/// Submits and stops haptic feedback on a session's haptic actions.
pub trait XrHaptics {
    type Error: std::error::Error + Send + Sync + 'static;

    /// Apply a haptic effect to the given target (`xrApplyHapticFeedback`).
    fn apply(&self, target: HapticTarget, effect: HapticEffect) -> Result<(), Self::Error>;

    /// Stop any in-flight haptic effect on the given target
    /// (`xrStopHapticFeedback`).
    fn stop(&self, target: HapticTarget) -> Result<(), Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn haptic_effect_is_copy() {
        let e = HapticEffect {
            duration_ns: 100_000_000,
            frequency_hz: 320.0,
            amplitude: 0.5,
        };
        let copy = e;
        assert_eq!(copy.duration_ns, 100_000_000);
    }
}
