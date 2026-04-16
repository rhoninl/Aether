//! Minimal, data-only bridge between gameplay/ECS code and the audio runtime.
//!
//! This module intentionally does *no* audio playback. It only produces
//! [`SoundRequest`] values that the real audio runtime consumes elsewhere, so
//! gameplay systems can trigger spatial sounds without wiring the HRTF
//! pipeline directly.
//!
//! ## Why a local `Transform3`?
//!
//! `aether-physics` already defines a `Transform` component, but it stores
//! rotation as a quaternion (`[f32; 4]`) and pulls in the whole physics
//! component graph. `aether-audio` has no dependencies today and we want to
//! keep it that way. [`Transform3`] is a tiny, audio-local view — position plus
//! a yaw angle — which is all the spatial bridge needs. Callers that live in
//! worlds using `aether_physics::Transform` can build a `Transform3` from it
//! trivially.

/// Opaque identifier for a loaded/registered sound asset.
///
/// The audio runtime owns the actual mapping; the bridge only shuffles the id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SoundHandle(pub u32);

/// Minimal 3D transform understood by the audio bridge.
///
/// Kept local (see module docs) and uses a yaw angle because spatial audio
/// only needs a facing direction in the horizontal plane.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform3 {
    pub position: [f32; 3],
    pub rotation_y: f32,
}

/// A data-only request to play a sound. The real audio runtime consumes these.
#[derive(Clone, Debug, PartialEq)]
pub struct SoundRequest {
    pub handle: SoundHandle,
    pub pos: [f32; 3],
    pub volume: f32,
    pub loop_sound: bool,
}

/// Default playback volume when the caller does not specify one.
pub const DEFAULT_VOLUME: f32 = 1.0;

/// Minimum permitted volume after clamping.
pub const MIN_VOLUME: f32 = 0.0;

/// Maximum permitted volume after clamping.
pub const MAX_VOLUME: f32 = 1.0;

/// Clamp a caller-supplied volume into the `[MIN_VOLUME, MAX_VOLUME]` range.
///
/// NaN is replaced by [`DEFAULT_VOLUME`] so downstream mixers never see NaN gain.
fn clamp_volume(volume: f32) -> f32 {
    if volume.is_nan() {
        return DEFAULT_VOLUME;
    }
    volume.clamp(MIN_VOLUME, MAX_VOLUME)
}

/// Produce a one-shot sound request at `pos` using [`DEFAULT_VOLUME`].
pub fn play_sound(handle: SoundHandle, pos: [f32; 3]) -> SoundRequest {
    SoundRequest {
        handle,
        pos,
        volume: DEFAULT_VOLUME,
        loop_sound: false,
    }
}

/// Produce a one-shot sound request anchored to `transform.position`.
///
/// The yaw (`rotation_y`) is ignored here — spatial directionality is derived from
/// the listener, not the source — but the helper exists so gameplay code can pass
/// an entity's transform directly without destructuring.
pub fn play_sound_at_transform(handle: SoundHandle, transform: &Transform3) -> SoundRequest {
    play_sound(handle, transform.position)
}

/// Produce a one-shot sound request with a caller-supplied `volume`.
///
/// `volume` is clamped into `[MIN_VOLUME, MAX_VOLUME]`; NaN is replaced by
/// [`DEFAULT_VOLUME`]. Clamping (rather than rejecting out-of-range values) keeps
/// the API infallible so gameplay callers never have to branch on validation.
pub fn play_sound_with_volume(handle: SoundHandle, pos: [f32; 3], volume: f32) -> SoundRequest {
    SoundRequest {
        handle,
        pos,
        volume: clamp_volume(volume),
        loop_sound: false,
    }
}

/// Produce a looping sound request at `pos` using [`DEFAULT_VOLUME`].
pub fn play_sound_looped(handle: SoundHandle, pos: [f32; 3]) -> SoundRequest {
    SoundRequest {
        handle,
        pos,
        volume: DEFAULT_VOLUME,
        loop_sound: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_HANDLE: SoundHandle = SoundHandle(42);
    const TEST_POS: [f32; 3] = [1.0, 2.0, 3.0];

    #[test]
    fn play_sound_uses_default_volume_and_no_loop() {
        let req = play_sound(TEST_HANDLE, TEST_POS);
        assert_eq!(req.handle, TEST_HANDLE);
        assert_eq!(req.pos, TEST_POS);
        assert_eq!(req.volume, DEFAULT_VOLUME);
        assert!(!req.loop_sound);
    }

    #[test]
    fn play_sound_at_transform_copies_position() {
        let transform = Transform3 {
            position: [10.0, -5.0, 2.5],
            rotation_y: 1.57,
        };
        let req = play_sound_at_transform(TEST_HANDLE, &transform);
        assert_eq!(req.handle, TEST_HANDLE);
        assert_eq!(req.pos, transform.position);
        assert_eq!(req.volume, DEFAULT_VOLUME);
        assert!(!req.loop_sound);
    }

    #[test]
    fn play_sound_with_volume_preserves_in_range() {
        let req = play_sound_with_volume(TEST_HANDLE, TEST_POS, 0.42);
        assert_eq!(req.volume, 0.42);
        assert_eq!(req.pos, TEST_POS);
        assert!(!req.loop_sound);
    }

    #[test]
    fn play_sound_with_volume_clamps_out_of_range() {
        let too_loud = play_sound_with_volume(TEST_HANDLE, TEST_POS, 4.0);
        assert_eq!(too_loud.volume, MAX_VOLUME);

        let too_quiet = play_sound_with_volume(TEST_HANDLE, TEST_POS, -1.5);
        assert_eq!(too_quiet.volume, MIN_VOLUME);
    }

    #[test]
    fn play_sound_with_volume_replaces_nan_with_default() {
        let req = play_sound_with_volume(TEST_HANDLE, TEST_POS, f32::NAN);
        assert_eq!(req.volume, DEFAULT_VOLUME);
    }

    #[test]
    fn play_sound_looped_sets_loop_flag() {
        let req = play_sound_looped(TEST_HANDLE, TEST_POS);
        assert!(req.loop_sound);
        assert_eq!(req.volume, DEFAULT_VOLUME);
        assert_eq!(req.pos, TEST_POS);
    }

    #[test]
    fn identical_inputs_compare_equal() {
        let a = play_sound(TEST_HANDLE, TEST_POS);
        let b = play_sound(TEST_HANDLE, TEST_POS);
        assert_eq!(a, b);

        let looped_a = play_sound_looped(TEST_HANDLE, TEST_POS);
        let looped_b = play_sound_looped(TEST_HANDLE, TEST_POS);
        assert_eq!(looped_a, looped_b);
        assert_ne!(a, looped_a);
    }

    #[test]
    fn sound_handle_is_hashable_for_ecs_maps() {
        use std::collections::HashMap;
        let mut counts: HashMap<SoundHandle, u32> = HashMap::new();
        *counts.entry(SoundHandle(1)).or_default() += 1;
        *counts.entry(SoundHandle(1)).or_default() += 1;
        *counts.entry(SoundHandle(2)).or_default() += 1;
        assert_eq!(counts[&SoundHandle(1)], 2);
        assert_eq!(counts[&SoundHandle(2)], 1);
    }
}
