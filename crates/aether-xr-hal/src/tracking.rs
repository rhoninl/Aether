//! Pose, controller, and hand-tracking value types (design doc §5, P1-A).
//!
//! These types are the value-typed surface every backend produces: a `Pose3`
//! per tracked space, a `ControllerState` per hand, an optional `HandJointSet`
//! when hand-tracking is active, all rolled up into a `TrackingSnapshot` for a
//! single frame. The OpenXR backend populates them from `xrLocateSpace` /
//! action queries; the emulator backend synthesises them from desktop input.

/// Number of joints in the OpenXR hand tracking extension (XR_HAND_JOINT_COUNT).
pub const MAX_HAND_JOINTS: usize = 26;

/// Default prediction offset in nanoseconds (~11ms for 90Hz displays).
pub const DEFAULT_TRACKING_PREDICTION_NS: u64 = 11_111_111;

/// 3D pose plus its time derivatives, in OpenXR's right-handed (+Y up) frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pose3 {
    pub position: [f32; 3],
    /// Quaternion `[x, y, z, w]`.
    pub rotation: [f32; 4],
    pub linear_velocity: [f32; 3],
    pub angular_velocity: [f32; 3],
}

impl Default for Pose3 {
    fn default() -> Self {
        Self {
            position: [0.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0],
            linear_velocity: [0.0; 3],
            angular_velocity: [0.0; 3],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrackingConfidence {
    None,
    Low,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Hand {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy)]
pub struct ControllerAnalog {
    pub trigger: f32,
    pub grip: f32,
    pub thumbstick_x: f32,
    pub thumbstick_y: f32,
}

impl Default for ControllerAnalog {
    fn default() -> Self {
        Self {
            trigger: 0.0,
            grip: 0.0,
            thumbstick_x: 0.0,
            thumbstick_y: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ControllerButtons {
    pub trigger_click: bool,
    pub grip_click: bool,
    pub primary_button: bool,
    pub secondary_button: bool,
    pub thumbstick_click: bool,
    pub thumbstick_touch: bool,
    pub menu_button: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct ControllerState {
    pub hand: Hand,
    pub tracking_confidence: TrackingConfidence,
    pub connected: bool,
    pub grip_pose: Pose3,
    pub aim_pose: Pose3,
    pub analog: ControllerAnalog,
    pub buttons: ControllerButtons,
}

impl ControllerState {
    pub fn disconnected(hand: Hand) -> Self {
        Self {
            hand,
            tracking_confidence: TrackingConfidence::None,
            connected: false,
            grip_pose: Pose3::default(),
            aim_pose: Pose3::default(),
            analog: ControllerAnalog::default(),
            buttons: ControllerButtons::default(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct HandJoint {
    pub pose: Pose3,
    pub radius: f32,
    pub confidence: TrackingConfidence,
}

impl Default for HandJoint {
    fn default() -> Self {
        Self {
            pose: Pose3::default(),
            radius: 0.005,
            confidence: TrackingConfidence::None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct HandJointSet {
    pub hand: Hand,
    pub active: bool,
    pub joints: Vec<HandJoint>,
}

impl HandJointSet {
    pub fn inactive(hand: Hand) -> Self {
        Self {
            hand,
            active: false,
            joints: vec![HandJoint::default(); MAX_HAND_JOINTS],
        }
    }

    pub fn is_valid(&self) -> bool {
        self.joints.len() == MAX_HAND_JOINTS
    }

    pub fn palm(&self) -> Option<&HandJoint> {
        if self.active && self.is_valid() {
            self.joints.first()
        } else {
            None
        }
    }

    pub fn joint(&self, index: usize) -> Option<&HandJoint> {
        if self.active && index < self.joints.len() {
            Some(&self.joints[index])
        } else {
            None
        }
    }
}

#[derive(Debug, Clone)]
pub struct TrackingSnapshot {
    pub timestamp_ns: u64,
    pub predicted_display_time_ns: u64,
    pub head_pose: Pose3,
    pub head_confidence: TrackingConfidence,
    pub left_controller: ControllerState,
    pub right_controller: ControllerState,
    pub left_hand: Option<HandJointSet>,
    pub right_hand: Option<HandJointSet>,
}

impl TrackingSnapshot {
    pub fn empty(timestamp_ns: u64) -> Self {
        Self {
            timestamp_ns,
            predicted_display_time_ns: timestamp_ns,
            head_pose: Pose3::default(),
            head_confidence: TrackingConfidence::None,
            left_controller: ControllerState::disconnected(Hand::Left),
            right_controller: ControllerState::disconnected(Hand::Right),
            left_hand: None,
            right_hand: None,
        }
    }

    pub fn controller(&self, hand: Hand) -> &ControllerState {
        match hand {
            Hand::Left => &self.left_controller,
            Hand::Right => &self.right_controller,
        }
    }

    pub fn hand_joints(&self, hand: Hand) -> Option<&HandJointSet> {
        match hand {
            Hand::Left => self.left_hand.as_ref(),
            Hand::Right => self.right_hand.as_ref(),
        }
    }

    pub fn has_controller_tracking(&self) -> bool {
        (self.left_controller.connected
            && self.left_controller.tracking_confidence != TrackingConfidence::None)
            || (self.right_controller.connected
                && self.right_controller.tracking_confidence != TrackingConfidence::None)
    }

    pub fn has_hand_tracking(&self) -> bool {
        self.left_hand.as_ref().is_some_and(|h| h.active)
            || self.right_hand.as_ref().is_some_and(|h| h.active)
    }
}

/// Single-slot snapshot store. Backends call [`submit_snapshot`] each frame; the
/// application reads the latest via [`latest_snapshot`].
#[derive(Debug)]
pub struct TrackingPipeline {
    latest: Option<TrackingSnapshot>,
    snapshot_count: u64,
    hand_tracking_enabled: bool,
    prediction_offset_ns: u64,
}

impl TrackingPipeline {
    pub fn new(hand_tracking_enabled: bool, prediction_offset_ns: u64) -> Self {
        Self {
            latest: None,
            snapshot_count: 0,
            hand_tracking_enabled,
            prediction_offset_ns,
        }
    }

    pub fn submit_snapshot(&mut self, snapshot: TrackingSnapshot) {
        self.snapshot_count = self.snapshot_count.saturating_add(1);
        self.latest = Some(snapshot);
    }

    pub fn latest_snapshot(&self) -> Option<&TrackingSnapshot> {
        self.latest.as_ref()
    }

    pub fn snapshot_count(&self) -> u64 {
        self.snapshot_count
    }

    pub fn hand_tracking_enabled(&self) -> bool {
        self.hand_tracking_enabled
    }

    pub fn prediction_offset_ns(&self) -> u64 {
        self.prediction_offset_ns
    }

    pub fn predicted_display_time(&self, base_timestamp_ns: u64) -> u64 {
        base_timestamp_ns.saturating_add(self.prediction_offset_ns)
    }

    pub fn clear(&mut self) {
        self.latest = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_pose() -> Pose3 {
        Pose3 {
            position: [1.0, 1.7, -0.5],
            rotation: [0.0, 0.382, 0.0, 0.924],
            linear_velocity: [0.1, 0.0, -0.2],
            angular_velocity: [0.0, 0.5, 0.0],
        }
    }

    fn connected_controller(hand: Hand) -> ControllerState {
        ControllerState {
            hand,
            tracking_confidence: TrackingConfidence::High,
            connected: true,
            grip_pose: sample_pose(),
            aim_pose: sample_pose(),
            analog: ControllerAnalog {
                trigger: 0.8,
                grip: 0.5,
                thumbstick_x: 0.3,
                thumbstick_y: -0.7,
            },
            buttons: ControllerButtons {
                trigger_click: true,
                primary_button: true,
                thumbstick_touch: true,
                ..Default::default()
            },
        }
    }

    fn sample_snapshot() -> TrackingSnapshot {
        TrackingSnapshot {
            timestamp_ns: 1_000_000,
            predicted_display_time_ns: 1_011_111,
            head_pose: sample_pose(),
            head_confidence: TrackingConfidence::High,
            left_controller: connected_controller(Hand::Left),
            right_controller: connected_controller(Hand::Right),
            left_hand: None,
            right_hand: None,
        }
    }

    #[test]
    fn pose3_default_is_identity() {
        let p = Pose3::default();
        assert_eq!(p.position, [0.0, 0.0, 0.0]);
        assert_eq!(p.rotation, [0.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn disconnected_controller_has_no_tracking() {
        let ctrl = ControllerState::disconnected(Hand::Left);
        assert_eq!(ctrl.hand, Hand::Left);
        assert!(!ctrl.connected);
        assert_eq!(ctrl.tracking_confidence, TrackingConfidence::None);
    }

    #[test]
    fn connected_controller_state_round_trips() {
        let ctrl = connected_controller(Hand::Right);
        assert!(ctrl.connected);
        assert_eq!(ctrl.tracking_confidence, TrackingConfidence::High);
        assert_eq!(ctrl.analog.trigger, 0.8);
        assert!(ctrl.buttons.trigger_click);
    }

    #[test]
    fn hand_joint_default_values() {
        let j = HandJoint::default();
        assert_eq!(j.radius, 0.005);
        assert_eq!(j.confidence, TrackingConfidence::None);
    }

    #[test]
    fn inactive_hand_palm_returns_none() {
        let h = HandJointSet::inactive(Hand::Left);
        assert_eq!(h.joints.len(), MAX_HAND_JOINTS);
        assert!(h.is_valid());
        assert!(h.palm().is_none());
        assert!(h.joint(0).is_none());
    }

    #[test]
    fn active_hand_palm_returns_some() {
        let mut h = HandJointSet::inactive(Hand::Right);
        h.active = true;
        assert!(h.palm().is_some());
        assert!(h.joint(0).is_some());
        assert!(h.joint(MAX_HAND_JOINTS).is_none());
    }

    #[test]
    fn empty_snapshot_has_no_tracking() {
        let s = TrackingSnapshot::empty(1_000);
        assert_eq!(s.timestamp_ns, 1_000);
        assert!(!s.has_controller_tracking());
        assert!(!s.has_hand_tracking());
    }

    #[test]
    fn snapshot_controller_lookup_by_hand() {
        let s = sample_snapshot();
        assert_eq!(s.controller(Hand::Left).hand, Hand::Left);
        assert_eq!(s.controller(Hand::Right).hand, Hand::Right);
        assert!(s.has_controller_tracking());
    }

    #[test]
    fn snapshot_with_hand_tracking() {
        let mut s = sample_snapshot();
        let mut h = HandJointSet::inactive(Hand::Left);
        h.active = true;
        s.left_hand = Some(h);
        assert!(s.has_hand_tracking());
        assert!(s.hand_joints(Hand::Left).is_some());
    }

    #[test]
    fn pipeline_submit_then_clear() {
        let mut p = TrackingPipeline::new(false, DEFAULT_TRACKING_PREDICTION_NS);
        assert!(p.latest_snapshot().is_none());
        p.submit_snapshot(sample_snapshot());
        assert_eq!(p.snapshot_count(), 1);
        assert!(p.latest_snapshot().is_some());
        p.clear();
        assert!(p.latest_snapshot().is_none());
        assert_eq!(p.snapshot_count(), 1);
    }

    #[test]
    fn pipeline_predicted_display_time_saturates() {
        let p = TrackingPipeline::new(false, u64::MAX);
        assert_eq!(p.predicted_display_time(1_000), u64::MAX);
    }
}
