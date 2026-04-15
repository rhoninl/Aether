//! Lightweight tween / property-animation primitives.
//!
//! This module is intentionally decoupled from the skeletal animation stack in
//! [`crate::animation`]. Game systems can use it for simple property animation
//! on props, projectiles, and VFX (position, scale, color, etc.) without
//! pulling in the full skeletal pipeline.

const T_MIN: f32 = 0.0;
const T_MAX: f32 = 1.0;
const HALF: f32 = 0.5;
const CUBIC_SCALE: f32 = 4.0;
const COLOR_CHANNEL_MASK: u32 = 0xFF;
const ALPHA_SHIFT: u32 = 24;
const RED_SHIFT: u32 = 16;
const GREEN_SHIFT: u32 = 8;
const BLUE_SHIFT: u32 = 0;

/// Linear interpolation trait for types that can be tweened.
pub trait Lerp: Copy {
    /// Interpolate between `a` and `b` at parameter `t`.
    ///
    /// `t` is expected to be in `[0, 1]` but implementers should not assume it
    /// is clamped; [`Tween::sample`] performs clamping before calling.
    fn lerp(a: Self, b: Self, t: f32) -> Self;
}

impl Lerp for f32 {
    fn lerp(a: Self, b: Self, t: f32) -> Self {
        a + (b - a) * t
    }
}

impl Lerp for [f32; 3] {
    fn lerp(a: Self, b: Self, t: f32) -> Self {
        [
            f32::lerp(a[0], b[0], t),
            f32::lerp(a[1], b[1], t),
            f32::lerp(a[2], b[2], t),
        ]
    }
}

impl Lerp for u32 {
    /// Per-channel color lerp. The `u32` is treated as packed `0xAARRGGBB`.
    fn lerp(a: Self, b: Self, t: f32) -> Self {
        let lerp_channel = |shift: u32| -> u32 {
            let ca = ((a >> shift) & COLOR_CHANNEL_MASK) as f32;
            let cb = ((b >> shift) & COLOR_CHANNEL_MASK) as f32;
            let mixed = ca + (cb - ca) * t;
            (mixed.round().clamp(0.0, COLOR_CHANNEL_MASK as f32) as u32) & COLOR_CHANNEL_MASK
        };
        (lerp_channel(ALPHA_SHIFT) << ALPHA_SHIFT)
            | (lerp_channel(RED_SHIFT) << RED_SHIFT)
            | (lerp_channel(GREEN_SHIFT) << GREEN_SHIFT)
            | (lerp_channel(BLUE_SHIFT) << BLUE_SHIFT)
    }
}

/// Easing curves applied to the normalized `t` parameter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Easing {
    /// `f(t) = t`
    Linear,
    /// Quadratic ease-in: `f(t) = t*t` — slower start, faster finish.
    EaseIn,
    /// Quadratic ease-out: `f(t) = 1 - (1-t)^2` — faster start, slower finish.
    EaseOut,
    /// Quadratic ease-in-out: symmetric smoothstep-style curve.
    EaseInOut,
    /// Cubic ease-in-out: steeper S-curve than [`Easing::EaseInOut`].
    Cubic,
}

impl Easing {
    /// Apply the easing curve. The input `t` is clamped to `[0, 1]`.
    pub fn apply(self, t: f32) -> f32 {
        let t = t.clamp(T_MIN, T_MAX);
        match self {
            Easing::Linear => t,
            Easing::EaseIn => t * t,
            Easing::EaseOut => {
                let inv = T_MAX - t;
                T_MAX - inv * inv
            }
            Easing::EaseInOut => {
                if t < HALF {
                    2.0 * t * t
                } else {
                    let inv = T_MAX - t;
                    T_MAX - 2.0 * inv * inv
                }
            }
            Easing::Cubic => {
                if t < HALF {
                    CUBIC_SCALE * t * t * t
                } else {
                    let inv = T_MAX - t;
                    T_MAX - CUBIC_SCALE * inv * inv * inv
                }
            }
        }
    }
}

/// A single tween animating a value of type `T` from `from` to `to`.
#[derive(Clone, Copy, Debug)]
pub struct Tween<T: Lerp> {
    pub from: T,
    pub to: T,
    pub duration: f32,
    pub elapsed: f32,
    pub easing: Easing,
}

impl<T: Lerp> Tween<T> {
    /// Create a new linear tween.
    pub fn new(from: T, to: T, duration: f32) -> Self {
        Self {
            from,
            to,
            duration,
            elapsed: 0.0,
            easing: Easing::Linear,
        }
    }

    /// Builder-style setter for the easing curve.
    pub fn with_easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    /// Sample the current value. At `elapsed == 0` returns `from`; at or past
    /// `duration` returns `to`. Zero-duration tweens snap to `to`.
    pub fn sample(&self) -> T {
        let raw = if self.duration <= 0.0 {
            T_MAX
        } else {
            (self.elapsed / self.duration).clamp(T_MIN, T_MAX)
        };
        let eased = self.easing.apply(raw);
        T::lerp(self.from, self.to, eased)
    }

    /// Advance the tween by `dt` seconds. Negative `dt` is ignored.
    pub fn advance(&mut self, dt: f32) {
        if dt <= 0.0 {
            return;
        }
        self.elapsed += dt;
        if self.elapsed > self.duration {
            self.elapsed = self.duration;
        }
    }

    /// True once the tween has reached its end.
    pub fn finished(&self) -> bool {
        self.duration <= 0.0 || self.elapsed >= self.duration
    }
}

/// An ordered sequence of tweens played back-to-back.
#[derive(Clone, Debug)]
pub struct TweenTrack<T: Lerp> {
    pub tweens: Vec<Tween<T>>,
    pub current: usize,
}

impl<T: Lerp> TweenTrack<T> {
    pub fn new(tweens: Vec<Tween<T>>) -> Self {
        Self { tweens, current: 0 }
    }

    /// Sample the currently active tween, or `None` if the track is empty or
    /// completed.
    pub fn sample(&self) -> Option<T> {
        self.tweens.get(self.current).map(|tween| tween.sample())
    }

    /// Advance the track by `dt` seconds. Overflow from a finished tween rolls
    /// into the next tween in the sequence.
    pub fn advance(&mut self, dt: f32) {
        if dt <= 0.0 {
            return;
        }
        let mut remaining = dt;
        while remaining > 0.0 && self.current < self.tweens.len() {
            let tween = &mut self.tweens[self.current];
            let slack = tween.duration - tween.elapsed;
            if slack <= 0.0 {
                self.current += 1;
                continue;
            }
            if remaining >= slack {
                tween.elapsed = tween.duration;
                remaining -= slack;
                self.current += 1;
            } else {
                tween.elapsed += remaining;
                remaining = 0.0;
            }
        }
    }

    /// True when every tween in the track has completed.
    pub fn finished(&self) -> bool {
        self.current >= self.tweens.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 1e-4;

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() <= EPSILON
    }

    #[test]
    fn tween_f32_samples_start_at_zero_elapsed() {
        let tween = Tween::<f32>::new(0.0, 10.0, 1.0);
        assert!(approx_eq(tween.sample(), 0.0));
        assert!(!tween.finished());
    }

    #[test]
    fn tween_f32_samples_midpoint_after_half_advance() {
        let mut tween = Tween::<f32>::new(0.0, 10.0, 1.0);
        tween.advance(0.5);
        assert!(approx_eq(tween.sample(), 5.0));
        assert!(!tween.finished());
    }

    #[test]
    fn tween_f32_reaches_end_and_reports_finished() {
        let mut tween = Tween::<f32>::new(0.0, 10.0, 1.0);
        tween.advance(0.5);
        tween.advance(0.5);
        assert!(approx_eq(tween.sample(), 10.0));
        assert!(tween.finished());
    }

    #[test]
    fn tween_advance_clamps_at_duration() {
        let mut tween = Tween::<f32>::new(0.0, 4.0, 1.0);
        tween.advance(5.0);
        assert!(approx_eq(tween.sample(), 4.0));
        assert!(tween.finished());
    }

    #[test]
    fn easing_in_curves_below_identity_at_half() {
        // EaseIn: f(0.5) = 0.25 < 0.5
        assert!(Easing::EaseIn.apply(0.5) < 0.5);
        assert!(approx_eq(Easing::EaseIn.apply(0.5), 0.25));
    }

    #[test]
    fn easing_out_curves_above_identity_at_half() {
        // EaseOut: f(0.5) = 0.75 > 0.5
        assert!(Easing::EaseOut.apply(0.5) > 0.5);
        assert!(approx_eq(Easing::EaseOut.apply(0.5), 0.75));
    }

    #[test]
    fn easing_linear_is_identity() {
        assert!(approx_eq(Easing::Linear.apply(0.0), 0.0));
        assert!(approx_eq(Easing::Linear.apply(0.3), 0.3));
        assert!(approx_eq(Easing::Linear.apply(1.0), 1.0));
    }

    #[test]
    fn easing_clamps_input_to_unit_range() {
        assert!(approx_eq(Easing::Linear.apply(-0.5), 0.0));
        assert!(approx_eq(Easing::Linear.apply(2.0), 1.0));
        assert!(approx_eq(Easing::EaseIn.apply(2.0), 1.0));
    }

    #[test]
    fn easing_inout_is_symmetric_around_half() {
        assert!(approx_eq(Easing::EaseInOut.apply(0.5), 0.5));
        let lo = Easing::EaseInOut.apply(0.25);
        let hi = Easing::EaseInOut.apply(0.75);
        assert!(approx_eq(lo + hi, 1.0));
    }

    #[test]
    fn easing_cubic_is_symmetric_around_half() {
        assert!(approx_eq(Easing::Cubic.apply(0.5), 0.5));
        let lo = Easing::Cubic.apply(0.25);
        let hi = Easing::Cubic.apply(0.75);
        assert!(approx_eq(lo + hi, 1.0));
    }

    #[test]
    fn tween_with_easing_uses_curve() {
        let mut tween = Tween::<f32>::new(0.0, 10.0, 1.0).with_easing(Easing::EaseIn);
        tween.advance(0.5);
        // EaseIn at 0.5 => 0.25 => value 2.5
        assert!(approx_eq(tween.sample(), 2.5));
    }

    #[test]
    fn tween_vec3_lerps_per_component() {
        let mut tween = Tween::<[f32; 3]>::new([0.0, 0.0, 0.0], [2.0, 4.0, 6.0], 1.0);
        tween.advance(0.5);
        let v = tween.sample();
        assert!(approx_eq(v[0], 1.0));
        assert!(approx_eq(v[1], 2.0));
        assert!(approx_eq(v[2], 3.0));
    }

    #[test]
    fn lerp_vec3_direct_per_component() {
        let v = <[f32; 3] as Lerp>::lerp([1.0, 2.0, 3.0], [3.0, 4.0, 5.0], 0.5);
        assert!(approx_eq(v[0], 2.0));
        assert!(approx_eq(v[1], 3.0));
        assert!(approx_eq(v[2], 4.0));
    }

    #[test]
    fn lerp_u32_color_halfway_green() {
        // 0xFF000000 -> 0xFF00FF00 at t=0.5 should produce green ~0x80
        let c = <u32 as Lerp>::lerp(0xFF000000, 0xFF00FF00, 0.5);
        let green = (c >> 8) & 0xFF;
        assert!((green as i32 - 0x80).abs() <= 1, "green was 0x{:X}", green);
        // Alpha stays solid
        assert_eq!((c >> 24) & 0xFF, 0xFF);
        // Red and blue stay zero
        assert_eq!((c >> 16) & 0xFF, 0x00);
        assert_eq!(c & 0xFF, 0x00);
    }

    #[test]
    fn tween_u32_color_endpoints() {
        let mut tween = Tween::<u32>::new(0xFF000000, 0xFFFFFFFF, 1.0);
        assert_eq!(tween.sample(), 0xFF000000);
        tween.advance(1.0);
        assert_eq!(tween.sample(), 0xFFFFFFFF);
        assert!(tween.finished());
    }

    #[test]
    fn tween_zero_duration_is_instantly_finished() {
        let tween = Tween::<f32>::new(1.0, 9.0, 0.0);
        assert!(tween.finished());
        assert!(approx_eq(tween.sample(), 9.0));
    }

    #[test]
    fn tween_negative_dt_is_noop() {
        let mut tween = Tween::<f32>::new(0.0, 10.0, 1.0);
        tween.advance(-1.0);
        assert!(approx_eq(tween.sample(), 0.0));
        assert_eq!(tween.elapsed, 0.0);
    }

    #[test]
    fn tween_track_advances_through_multiple_tweens() {
        let tweens = vec![
            Tween::<f32>::new(0.0, 10.0, 1.0),
            Tween::<f32>::new(10.0, 20.0, 1.0),
            Tween::<f32>::new(20.0, 30.0, 1.0),
        ];
        let mut track = TweenTrack::new(tweens);
        assert_eq!(track.current, 0);
        assert!(approx_eq(track.sample().unwrap(), 0.0));

        track.advance(0.5);
        assert!(approx_eq(track.sample().unwrap(), 5.0));
        assert_eq!(track.current, 0);

        track.advance(1.0); // rolls past first tween into second at elapsed 0.5
        assert_eq!(track.current, 1);
        assert!(approx_eq(track.sample().unwrap(), 15.0));

        track.advance(2.0); // rolls past second, finishes third
        assert!(track.finished());
        assert!(track.sample().is_none());
    }

    #[test]
    fn tween_track_empty_is_finished() {
        let track: TweenTrack<f32> = TweenTrack::new(vec![]);
        assert!(track.finished());
        assert!(track.sample().is_none());
    }

    #[test]
    fn tween_track_advance_noop_on_empty() {
        let mut track: TweenTrack<f32> = TweenTrack::new(vec![]);
        track.advance(1.0);
        assert!(track.finished());
    }
}
