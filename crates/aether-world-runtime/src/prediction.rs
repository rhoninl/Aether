//! Entity prediction, interpolation, and server correction.

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

/// Default maximum number of snapshots stored per entity.
const DEFAULT_MAX_SNAPSHOTS: usize = 32;

/// A snapshot of an entity's state at a specific tick.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EntityState {
    pub entity_id: u64,
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    pub velocity: [f32; 3],
    pub tick: u64,
}

/// Result of comparing predicted state against server state.
#[derive(Debug, Clone, PartialEq)]
pub struct CorrectionDelta {
    pub entity_id: u64,
    pub position_error: [f32; 3],
    pub position_error_magnitude: f32,
    pub needs_correction: bool,
}

/// Stores timestamped entity snapshots and provides interpolation.
#[derive(Debug)]
pub struct InterpolationBuffer {
    snapshots: VecDeque<EntityState>,
    max_snapshots: usize,
}

impl InterpolationBuffer {
    pub fn new() -> Self {
        Self {
            snapshots: VecDeque::new(),
            max_snapshots: DEFAULT_MAX_SNAPSHOTS,
        }
    }

    pub fn with_max_snapshots(max: usize) -> Self {
        Self {
            snapshots: VecDeque::new(),
            max_snapshots: max.max(2),
        }
    }

    /// Push a new snapshot. Must have a tick >= the latest snapshot.
    pub fn push(&mut self, state: EntityState) {
        if let Some(last) = self.snapshots.back() {
            if state.tick < last.tick {
                return; // reject out-of-order
            }
        }

        if self.snapshots.len() >= self.max_snapshots {
            self.snapshots.pop_front();
        }
        self.snapshots.push_back(state);
    }

    /// Interpolate between the two snapshots bracketing the given tick fraction.
    /// `tick_fraction` is a float tick (e.g., 5.3 means 30% between tick 5 and tick 6).
    pub fn interpolate(&self, tick_fraction: f64) -> Option<EntityState> {
        if self.snapshots.len() < 2 {
            return self.snapshots.back().cloned();
        }

        // Find the two snapshots that bracket tick_fraction
        let target_tick = tick_fraction.floor() as u64;
        let t = (tick_fraction - tick_fraction.floor()) as f32;

        let mut before = None;
        let mut after = None;

        for (i, snap) in self.snapshots.iter().enumerate() {
            if snap.tick <= target_tick {
                before = Some(i);
            }
            if snap.tick > target_tick && after.is_none() {
                after = Some(i);
            }
        }

        match (before, after) {
            (Some(b), Some(a)) => {
                let snap_a = &self.snapshots[b];
                let snap_b = &self.snapshots[a];
                Some(lerp_entity_state(snap_a, snap_b, t))
            }
            (Some(b), None) => Some(self.snapshots[b].clone()),
            (None, Some(a)) => Some(self.snapshots[a].clone()),
            (None, None) => None,
        }
    }

    /// Get the latest snapshot.
    pub fn latest(&self) -> Option<&EntityState> {
        self.snapshots.back()
    }

    /// Get the number of stored snapshots.
    pub fn len(&self) -> usize {
        self.snapshots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.snapshots.is_empty()
    }

    /// Clear all snapshots.
    pub fn clear(&mut self) {
        self.snapshots.clear();
    }
}

impl Default for InterpolationBuffer {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute the correction delta between a predicted and authoritative state.
pub fn compute_correction(
    predicted: &EntityState,
    server: &EntityState,
    correction_threshold: f32,
) -> CorrectionDelta {
    let dx = server.position[0] - predicted.position[0];
    let dy = server.position[1] - predicted.position[1];
    let dz = server.position[2] - predicted.position[2];
    let magnitude_sq = dx * dx + dy * dy + dz * dz;
    let magnitude = magnitude_sq.sqrt();

    CorrectionDelta {
        entity_id: predicted.entity_id,
        position_error: [dx, dy, dz],
        position_error_magnitude: magnitude,
        needs_correction: magnitude > correction_threshold,
    }
}

/// Linearly interpolate between two entity states.
pub fn lerp_entity_state(a: &EntityState, b: &EntityState, t: f32) -> EntityState {
    let t = t.clamp(0.0, 1.0);
    EntityState {
        entity_id: a.entity_id,
        position: [
            a.position[0] + (b.position[0] - a.position[0]) * t,
            a.position[1] + (b.position[1] - a.position[1]) * t,
            a.position[2] + (b.position[2] - a.position[2]) * t,
        ],
        rotation: slerp_quat(a.rotation, b.rotation, t),
        velocity: [
            a.velocity[0] + (b.velocity[0] - a.velocity[0]) * t,
            a.velocity[1] + (b.velocity[1] - a.velocity[1]) * t,
            a.velocity[2] + (b.velocity[2] - a.velocity[2]) * t,
        ],
        tick: if t < 0.5 { a.tick } else { b.tick },
    }
}

/// Spherical linear interpolation for unit quaternions.
fn slerp_quat(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    let mut dot = a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3];

    // If dot is negative, negate one quaternion to take the short path
    let mut b = b;
    if dot < 0.0 {
        b = [-b[0], -b[1], -b[2], -b[3]];
        dot = -dot;
    }

    // If quaternions are very close, use linear interpolation
    if dot > 0.9995 {
        let result = [
            a[0] + (b[0] - a[0]) * t,
            a[1] + (b[1] - a[1]) * t,
            a[2] + (b[2] - a[2]) * t,
            a[3] + (b[3] - a[3]) * t,
        ];
        return normalize_quat(result);
    }

    let theta = dot.clamp(-1.0, 1.0).acos();
    let sin_theta = theta.sin();

    if sin_theta.abs() < 1e-6 {
        return a;
    }

    let weight_a = ((1.0 - t) * theta).sin() / sin_theta;
    let weight_b = (t * theta).sin() / sin_theta;

    [
        a[0] * weight_a + b[0] * weight_b,
        a[1] * weight_a + b[1] * weight_b,
        a[2] * weight_a + b[2] * weight_b,
        a[3] * weight_a + b[3] * weight_b,
    ]
}

fn normalize_quat(q: [f32; 4]) -> [f32; 4] {
    let len = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
    if len < 1e-10 {
        return [0.0, 0.0, 0.0, 1.0];
    }
    [q[0] / len, q[1] / len, q[2] / len, q[3] / len]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_state(entity_id: u64, tick: u64, x: f32, y: f32, z: f32) -> EntityState {
        EntityState {
            entity_id,
            position: [x, y, z],
            rotation: [0.0, 0.0, 0.0, 1.0],
            velocity: [0.0, 0.0, 0.0],
            tick,
        }
    }

    #[test]
    fn test_buffer_push_and_latest() {
        let mut buf = InterpolationBuffer::new();
        assert!(buf.is_empty());

        buf.push(make_state(1, 1, 0.0, 0.0, 0.0));
        assert_eq!(buf.len(), 1);
        assert_eq!(buf.latest().unwrap().tick, 1);

        buf.push(make_state(1, 2, 1.0, 0.0, 0.0));
        assert_eq!(buf.len(), 2);
        assert_eq!(buf.latest().unwrap().tick, 2);
    }

    #[test]
    fn test_buffer_rejects_out_of_order() {
        let mut buf = InterpolationBuffer::new();
        buf.push(make_state(1, 5, 0.0, 0.0, 0.0));
        buf.push(make_state(1, 3, 1.0, 0.0, 0.0)); // should be rejected
        assert_eq!(buf.len(), 1);
        assert_eq!(buf.latest().unwrap().tick, 5);
    }

    #[test]
    fn test_buffer_evicts_when_full() {
        let mut buf = InterpolationBuffer::with_max_snapshots(3);
        buf.push(make_state(1, 1, 0.0, 0.0, 0.0));
        buf.push(make_state(1, 2, 1.0, 0.0, 0.0));
        buf.push(make_state(1, 3, 2.0, 0.0, 0.0));
        buf.push(make_state(1, 4, 3.0, 0.0, 0.0));

        assert_eq!(buf.len(), 3);
        // Oldest (tick 1) should have been evicted
        assert_eq!(buf.snapshots.front().unwrap().tick, 2);
    }

    #[test]
    fn test_interpolate_position_midpoint() {
        let mut buf = InterpolationBuffer::new();
        buf.push(make_state(1, 0, 0.0, 0.0, 0.0));
        buf.push(make_state(1, 1, 10.0, 0.0, 0.0));

        let result = buf.interpolate(0.5).unwrap();
        assert!((result.position[0] - 5.0).abs() < 0.01);
    }

    #[test]
    fn test_interpolate_at_exact_tick() {
        let mut buf = InterpolationBuffer::new();
        buf.push(make_state(1, 0, 0.0, 0.0, 0.0));
        buf.push(make_state(1, 1, 10.0, 0.0, 0.0));

        let result = buf.interpolate(0.0).unwrap();
        assert!((result.position[0] - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_interpolate_single_snapshot_returns_it() {
        let mut buf = InterpolationBuffer::new();
        buf.push(make_state(1, 5, 3.0, 4.0, 5.0));

        let result = buf.interpolate(5.5).unwrap();
        assert_eq!(result.position, [3.0, 4.0, 5.0]);
    }

    #[test]
    fn test_interpolate_empty_returns_none() {
        let buf = InterpolationBuffer::new();
        assert!(buf.interpolate(1.0).is_none());
    }

    #[test]
    fn test_lerp_entity_state_basic() {
        let a = make_state(1, 0, 0.0, 0.0, 0.0);
        let b = make_state(1, 1, 10.0, 20.0, 30.0);

        let mid = lerp_entity_state(&a, &b, 0.5);
        assert!((mid.position[0] - 5.0).abs() < 0.01);
        assert!((mid.position[1] - 10.0).abs() < 0.01);
        assert!((mid.position[2] - 15.0).abs() < 0.01);
    }

    #[test]
    fn test_lerp_clamps_t() {
        let a = make_state(1, 0, 0.0, 0.0, 0.0);
        let b = make_state(1, 1, 10.0, 0.0, 0.0);

        let over = lerp_entity_state(&a, &b, 2.0);
        assert!((over.position[0] - 10.0).abs() < 0.01);

        let under = lerp_entity_state(&a, &b, -1.0);
        assert!((under.position[0] - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_lerp_velocity() {
        let mut a = make_state(1, 0, 0.0, 0.0, 0.0);
        a.velocity = [0.0, 0.0, 0.0];
        let mut b = make_state(1, 1, 10.0, 0.0, 0.0);
        b.velocity = [10.0, 0.0, 0.0];

        let mid = lerp_entity_state(&a, &b, 0.5);
        assert!((mid.velocity[0] - 5.0).abs() < 0.01);
    }

    #[test]
    fn test_slerp_identity_quaternions() {
        let identity = [0.0, 0.0, 0.0, 1.0];
        let result = slerp_quat(identity, identity, 0.5);
        assert!((result[3] - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_slerp_90_degree_rotation() {
        // Rotation around Y axis: 0 degrees
        let a = [0.0, 0.0, 0.0, 1.0];
        // Rotation around Y axis: ~90 degrees (sin(45) ~= 0.707)
        let b = [0.0, 0.7071, 0.0, 0.7071];

        let mid = slerp_quat(a, b, 0.5);
        // Should be ~45 degrees around Y
        let len = (mid[0] * mid[0] + mid[1] * mid[1] + mid[2] * mid[2] + mid[3] * mid[3]).sqrt();
        assert!((len - 1.0).abs() < 0.01, "quaternion should be unit length");
    }

    #[test]
    fn test_compute_correction_within_threshold() {
        let predicted = make_state(1, 5, 10.0, 20.0, 30.0);
        let server = make_state(1, 5, 10.01, 20.0, 30.0);

        let delta = compute_correction(&predicted, &server, 0.1);
        assert!(!delta.needs_correction);
        assert!(delta.position_error_magnitude < 0.1);
    }

    #[test]
    fn test_compute_correction_exceeds_threshold() {
        let predicted = make_state(1, 5, 10.0, 20.0, 30.0);
        let server = make_state(1, 5, 15.0, 20.0, 30.0);

        let delta = compute_correction(&predicted, &server, 0.1);
        assert!(delta.needs_correction);
        assert!((delta.position_error[0] - 5.0).abs() < 0.01);
        assert!((delta.position_error_magnitude - 5.0).abs() < 0.01);
    }

    #[test]
    fn test_compute_correction_3d_distance() {
        let predicted = make_state(1, 5, 0.0, 0.0, 0.0);
        let server = make_state(1, 5, 3.0, 4.0, 0.0);

        let delta = compute_correction(&predicted, &server, 1.0);
        assert!(delta.needs_correction);
        assert!((delta.position_error_magnitude - 5.0).abs() < 0.01);
    }

    #[test]
    fn test_buffer_clear() {
        let mut buf = InterpolationBuffer::new();
        buf.push(make_state(1, 1, 0.0, 0.0, 0.0));
        buf.push(make_state(1, 2, 1.0, 0.0, 0.0));
        assert_eq!(buf.len(), 2);

        buf.clear();
        assert!(buf.is_empty());
        assert!(buf.latest().is_none());
    }
}
