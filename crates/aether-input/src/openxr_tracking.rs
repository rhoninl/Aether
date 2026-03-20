//! OpenXR tracking pipeline: head tracking, controller input, and hand tracking.
//!
//! Provides data structures for capturing per-frame tracking snapshots from
//! the HMD, controllers, and optional hand tracking extension.

use crate::actions::Pose3;

/// Number of joints in the OpenXR hand tracking extension (XR_HAND_JOINT_COUNT).
pub const MAX_HAND_JOINTS: usize = 26;

/// Default prediction offset in nanoseconds (~11ms for 90Hz displays).
pub const DEFAULT_TRACKING_PREDICTION_NS: u64 = 11_111_111;

/// Tracking confidence level for a tracked device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrackingConfidence {
    /// Tracking data is not available.
    None,
    /// Tracking data is available but may have reduced accuracy.
    Low,
    /// Tracking data is available with full accuracy.
    High,
}

/// Identifies which hand a controller or hand tracking data belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Hand {
    Left,
    Right,
}

/// Analog values for a single VR controller.
#[derive(Debug, Clone, Copy)]
pub struct ControllerAnalog {
    /// Trigger pull value [0.0, 1.0].
    pub trigger: f32,
    /// Grip squeeze value [0.0, 1.0].
    pub grip: f32,
    /// Thumbstick X axis [-1.0, 1.0].
    pub thumbstick_x: f32,
    /// Thumbstick Y axis [-1.0, 1.0].
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

/// Digital button states for a single VR controller.
#[derive(Debug, Clone, Copy, Default)]
pub struct ControllerButtons {
    /// Whether the trigger is pressed past its click threshold.
    pub trigger_click: bool,
    /// Whether the grip is pressed past its click threshold.
    pub grip_click: bool,
    /// Primary face button (A on right, X on left for Quest controllers).
    pub primary_button: bool,
    /// Secondary face button (B on right, Y on left for Quest controllers).
    pub secondary_button: bool,
    /// Whether the thumbstick is pressed down.
    pub thumbstick_click: bool,
    /// Whether the thumbstick is being touched (capacitive).
    pub thumbstick_touch: bool,
    /// Menu button (typically only on left controller).
    pub menu_button: bool,
}

/// Complete state of a single VR controller for one frame.
#[derive(Debug, Clone, Copy)]
pub struct ControllerState {
    /// Which hand this controller belongs to.
    pub hand: Hand,
    /// Tracking confidence for this controller's pose.
    pub tracking_confidence: TrackingConfidence,
    /// Whether the controller is currently connected.
    pub connected: bool,
    /// 6-DOF pose of the controller grip.
    pub grip_pose: Pose3,
    /// 6-DOF pose of the controller aim point.
    pub aim_pose: Pose3,
    /// Analog input values.
    pub analog: ControllerAnalog,
    /// Digital button states.
    pub buttons: ControllerButtons,
}

impl ControllerState {
    /// Create a default disconnected controller state for the given hand.
    pub fn disconnected(hand: Hand) -> Self {
        Self {
            hand,
            tracking_confidence: TrackingConfidence::None,
            connected: false,
            grip_pose: Pose3 {
                position: [0.0, 0.0, 0.0],
                rotation: [0.0, 0.0, 0.0, 1.0],
                linear_velocity: [0.0, 0.0, 0.0],
                angular_velocity: [0.0, 0.0, 0.0],
            },
            aim_pose: Pose3 {
                position: [0.0, 0.0, 0.0],
                rotation: [0.0, 0.0, 0.0, 1.0],
                linear_velocity: [0.0, 0.0, 0.0],
                angular_velocity: [0.0, 0.0, 0.0],
            },
            analog: ControllerAnalog::default(),
            buttons: ControllerButtons::default(),
        }
    }
}

/// A single joint in a hand tracking skeleton.
#[derive(Debug, Clone, Copy)]
pub struct HandJoint {
    /// 6-DOF pose of this joint.
    pub pose: Pose3,
    /// Radius of the joint (for collision/visualization).
    pub radius: f32,
    /// Tracking confidence for this specific joint.
    pub confidence: TrackingConfidence,
}

impl Default for HandJoint {
    fn default() -> Self {
        Self {
            pose: Pose3 {
                position: [0.0, 0.0, 0.0],
                rotation: [0.0, 0.0, 0.0, 1.0],
                linear_velocity: [0.0, 0.0, 0.0],
                angular_velocity: [0.0, 0.0, 0.0],
            },
            radius: 0.005,
            confidence: TrackingConfidence::None,
        }
    }
}

/// Hand tracking data for one hand.
#[derive(Debug, Clone)]
pub struct HandJointSet {
    /// Which hand this data belongs to.
    pub hand: Hand,
    /// Whether hand tracking is currently active for this hand.
    pub active: bool,
    /// Joint data for all 26 joints.
    pub joints: Vec<HandJoint>,
}

impl HandJointSet {
    /// Create a new inactive hand joint set.
    pub fn inactive(hand: Hand) -> Self {
        Self {
            hand,
            active: false,
            joints: vec![HandJoint::default(); MAX_HAND_JOINTS],
        }
    }

    /// Check if the joint set has the correct number of joints.
    pub fn is_valid(&self) -> bool {
        self.joints.len() == MAX_HAND_JOINTS
    }

    /// Get the palm joint (index 0 in the OpenXR hand tracking spec).
    pub fn palm(&self) -> Option<&HandJoint> {
        if self.active && self.is_valid() {
            self.joints.first()
        } else {
            None
        }
    }

    /// Get a joint by index.
    pub fn joint(&self, index: usize) -> Option<&HandJoint> {
        if self.active && index < self.joints.len() {
            Some(&self.joints[index])
        } else {
            None
        }
    }
}

/// A complete tracking snapshot for one frame.
///
/// Contains head tracking, controller state, and optional hand tracking data.
#[derive(Debug, Clone)]
pub struct TrackingSnapshot {
    /// Timestamp for this snapshot in nanoseconds.
    pub timestamp_ns: u64,
    /// Predicted display time in nanoseconds.
    pub predicted_display_time_ns: u64,
    /// HMD (head-mounted display) pose.
    pub head_pose: Pose3,
    /// HMD tracking confidence.
    pub head_confidence: TrackingConfidence,
    /// Left controller state.
    pub left_controller: ControllerState,
    /// Right controller state.
    pub right_controller: ControllerState,
    /// Optional left hand tracking data.
    pub left_hand: Option<HandJointSet>,
    /// Optional right hand tracking data.
    pub right_hand: Option<HandJointSet>,
}

impl TrackingSnapshot {
    /// Create a new tracking snapshot with default (no tracking) values.
    pub fn empty(timestamp_ns: u64) -> Self {
        Self {
            timestamp_ns,
            predicted_display_time_ns: timestamp_ns,
            head_pose: Pose3 {
                position: [0.0, 0.0, 0.0],
                rotation: [0.0, 0.0, 0.0, 1.0],
                linear_velocity: [0.0, 0.0, 0.0],
                angular_velocity: [0.0, 0.0, 0.0],
            },
            head_confidence: TrackingConfidence::None,
            left_controller: ControllerState::disconnected(Hand::Left),
            right_controller: ControllerState::disconnected(Hand::Right),
            left_hand: None,
            right_hand: None,
        }
    }

    /// Get the controller state for a specific hand.
    pub fn controller(&self, hand: Hand) -> &ControllerState {
        match hand {
            Hand::Left => &self.left_controller,
            Hand::Right => &self.right_controller,
        }
    }

    /// Get hand tracking data for a specific hand, if available.
    pub fn hand_joints(&self, hand: Hand) -> Option<&HandJointSet> {
        match hand {
            Hand::Left => self.left_hand.as_ref(),
            Hand::Right => self.right_hand.as_ref(),
        }
    }

    /// Check if at least one controller is connected and tracked.
    pub fn has_controller_tracking(&self) -> bool {
        (self.left_controller.connected
            && self.left_controller.tracking_confidence != TrackingConfidence::None)
            || (self.right_controller.connected
                && self.right_controller.tracking_confidence != TrackingConfidence::None)
    }

    /// Check if hand tracking is active for either hand.
    pub fn has_hand_tracking(&self) -> bool {
        self.left_hand.as_ref().is_some_and(|h| h.active)
            || self.right_hand.as_ref().is_some_and(|h| h.active)
    }
}

/// Accumulates tracking snapshots and provides a latest-snapshot query.
///
/// In a real implementation, this would poll the OpenXR runtime each frame.
/// This struct serves as the abstraction layer for that polling.
#[derive(Debug)]
pub struct TrackingPipeline {
    /// The most recent tracking snapshot.
    latest: Option<TrackingSnapshot>,
    /// Total number of snapshots received.
    snapshot_count: u64,
    /// Whether hand tracking is enabled.
    hand_tracking_enabled: bool,
    /// Prediction offset in nanoseconds.
    prediction_offset_ns: u64,
}

impl TrackingPipeline {
    /// Create a new tracking pipeline.
    pub fn new(hand_tracking_enabled: bool, prediction_offset_ns: u64) -> Self {
        Self {
            latest: None,
            snapshot_count: 0,
            hand_tracking_enabled,
            prediction_offset_ns,
        }
    }

    /// Submit a new tracking snapshot.
    pub fn submit_snapshot(&mut self, snapshot: TrackingSnapshot) {
        self.snapshot_count = self.snapshot_count.saturating_add(1);
        self.latest = Some(snapshot);
    }

    /// Get the latest tracking snapshot, if available.
    pub fn latest_snapshot(&self) -> Option<&TrackingSnapshot> {
        self.latest.as_ref()
    }

    /// Get the total number of snapshots received.
    pub fn snapshot_count(&self) -> u64 {
        self.snapshot_count
    }

    /// Whether hand tracking is enabled.
    pub fn hand_tracking_enabled(&self) -> bool {
        self.hand_tracking_enabled
    }

    /// Get the prediction offset in nanoseconds.
    pub fn prediction_offset_ns(&self) -> u64 {
        self.prediction_offset_ns
    }

    /// Compute the predicted display time from a base timestamp.
    pub fn predicted_display_time(&self, base_timestamp_ns: u64) -> u64 {
        base_timestamp_ns.saturating_add(self.prediction_offset_ns)
    }

    /// Clear the latest snapshot.
    pub fn clear(&mut self) {
        self.latest = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity_pose() -> Pose3 {
        Pose3 {
            position: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            linear_velocity: [0.0, 0.0, 0.0],
            angular_velocity: [0.0, 0.0, 0.0],
        }
    }

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
                grip_click: false,
                primary_button: true,
                secondary_button: false,
                thumbstick_click: false,
                thumbstick_touch: true,
                menu_button: false,
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

    // ---- TrackingConfidence ----

    #[test]
    fn tracking_confidence_equality() {
        assert_eq!(TrackingConfidence::None, TrackingConfidence::None);
        assert_eq!(TrackingConfidence::Low, TrackingConfidence::Low);
        assert_eq!(TrackingConfidence::High, TrackingConfidence::High);
        assert_ne!(TrackingConfidence::None, TrackingConfidence::High);
    }

    // ---- ControllerAnalog ----

    #[test]
    fn controller_analog_default_is_zeroed() {
        let analog = ControllerAnalog::default();
        assert_eq!(analog.trigger, 0.0);
        assert_eq!(analog.grip, 0.0);
        assert_eq!(analog.thumbstick_x, 0.0);
        assert_eq!(analog.thumbstick_y, 0.0);
    }

    // ---- ControllerButtons ----

    #[test]
    fn controller_buttons_default_all_false() {
        let buttons = ControllerButtons::default();
        assert!(!buttons.trigger_click);
        assert!(!buttons.grip_click);
        assert!(!buttons.primary_button);
        assert!(!buttons.secondary_button);
        assert!(!buttons.thumbstick_click);
        assert!(!buttons.thumbstick_touch);
        assert!(!buttons.menu_button);
    }

    // ---- ControllerState ----

    #[test]
    fn disconnected_controller_has_no_tracking() {
        let ctrl = ControllerState::disconnected(Hand::Left);
        assert_eq!(ctrl.hand, Hand::Left);
        assert!(!ctrl.connected);
        assert_eq!(ctrl.tracking_confidence, TrackingConfidence::None);
    }

    #[test]
    fn disconnected_controller_identity_poses() {
        let ctrl = ControllerState::disconnected(Hand::Right);
        assert_eq!(ctrl.grip_pose.position, [0.0, 0.0, 0.0]);
        assert_eq!(ctrl.grip_pose.rotation, [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(ctrl.aim_pose.position, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn connected_controller_has_tracking() {
        let ctrl = connected_controller(Hand::Right);
        assert!(ctrl.connected);
        assert_eq!(ctrl.tracking_confidence, TrackingConfidence::High);
        assert_eq!(ctrl.hand, Hand::Right);
    }

    #[test]
    fn controller_analog_values_accessible() {
        let ctrl = connected_controller(Hand::Left);
        assert_eq!(ctrl.analog.trigger, 0.8);
        assert_eq!(ctrl.analog.grip, 0.5);
        assert_eq!(ctrl.analog.thumbstick_x, 0.3);
        assert_eq!(ctrl.analog.thumbstick_y, -0.7);
    }

    #[test]
    fn controller_button_values_accessible() {
        let ctrl = connected_controller(Hand::Left);
        assert!(ctrl.buttons.trigger_click);
        assert!(!ctrl.buttons.grip_click);
        assert!(ctrl.buttons.primary_button);
        assert!(!ctrl.buttons.secondary_button);
        assert!(ctrl.buttons.thumbstick_touch);
    }

    // ---- HandJoint ----

    #[test]
    fn hand_joint_default_values() {
        let joint = HandJoint::default();
        assert_eq!(joint.pose.position, [0.0, 0.0, 0.0]);
        assert_eq!(joint.radius, 0.005);
        assert_eq!(joint.confidence, TrackingConfidence::None);
    }

    // ---- HandJointSet ----

    #[test]
    fn inactive_hand_has_correct_joint_count() {
        let hand = HandJointSet::inactive(Hand::Left);
        assert_eq!(hand.joints.len(), MAX_HAND_JOINTS);
        assert!(!hand.active);
        assert!(hand.is_valid());
    }

    #[test]
    fn inactive_hand_palm_returns_none() {
        let hand = HandJointSet::inactive(Hand::Left);
        assert!(hand.palm().is_none());
    }

    #[test]
    fn active_hand_palm_returns_some() {
        let mut hand = HandJointSet::inactive(Hand::Right);
        hand.active = true;
        let palm = hand.palm();
        assert!(palm.is_some());
    }

    #[test]
    fn hand_joint_by_index() {
        let mut hand = HandJointSet::inactive(Hand::Left);
        hand.active = true;
        hand.joints[5].radius = 0.01;
        let joint = hand.joint(5).unwrap();
        assert_eq!(joint.radius, 0.01);
    }

    #[test]
    fn hand_joint_out_of_range_returns_none() {
        let mut hand = HandJointSet::inactive(Hand::Left);
        hand.active = true;
        assert!(hand.joint(MAX_HAND_JOINTS).is_none());
    }

    #[test]
    fn inactive_hand_joint_by_index_returns_none() {
        let hand = HandJointSet::inactive(Hand::Left);
        assert!(hand.joint(0).is_none());
    }

    #[test]
    fn hand_joint_set_invalid_count() {
        let mut hand = HandJointSet::inactive(Hand::Left);
        hand.joints.pop(); // Remove one joint
        assert!(!hand.is_valid());
    }

    // ---- TrackingSnapshot ----

    #[test]
    fn empty_snapshot_has_no_tracking() {
        let snap = TrackingSnapshot::empty(1_000);
        assert_eq!(snap.timestamp_ns, 1_000);
        assert_eq!(snap.head_confidence, TrackingConfidence::None);
        assert!(!snap.left_controller.connected);
        assert!(!snap.right_controller.connected);
        assert!(!snap.has_controller_tracking());
        assert!(!snap.has_hand_tracking());
    }

    #[test]
    fn snapshot_head_pose_accessible() {
        let snap = sample_snapshot();
        assert_eq!(snap.head_pose.position[0], 1.0);
        assert_eq!(snap.head_pose.position[1], 1.7);
        assert_eq!(snap.head_confidence, TrackingConfidence::High);
    }

    #[test]
    fn snapshot_controller_by_hand() {
        let snap = sample_snapshot();
        let left = snap.controller(Hand::Left);
        assert_eq!(left.hand, Hand::Left);
        assert!(left.connected);

        let right = snap.controller(Hand::Right);
        assert_eq!(right.hand, Hand::Right);
        assert!(right.connected);
    }

    #[test]
    fn snapshot_has_controller_tracking() {
        let snap = sample_snapshot();
        assert!(snap.has_controller_tracking());
    }

    #[test]
    fn snapshot_no_controller_tracking_when_disconnected() {
        let snap = TrackingSnapshot::empty(1_000);
        assert!(!snap.has_controller_tracking());
    }

    #[test]
    fn snapshot_has_no_hand_tracking_by_default() {
        let snap = sample_snapshot();
        assert!(!snap.has_hand_tracking());
        assert!(snap.hand_joints(Hand::Left).is_none());
        assert!(snap.hand_joints(Hand::Right).is_none());
    }

    #[test]
    fn snapshot_with_hand_tracking() {
        let mut snap = sample_snapshot();
        let mut left_hand = HandJointSet::inactive(Hand::Left);
        left_hand.active = true;
        snap.left_hand = Some(left_hand);

        assert!(snap.has_hand_tracking());
        assert!(snap.hand_joints(Hand::Left).is_some());
        assert!(snap.hand_joints(Hand::Right).is_none());
    }

    #[test]
    fn snapshot_one_controller_connected_has_tracking() {
        let mut snap = TrackingSnapshot::empty(1_000);
        snap.right_controller = connected_controller(Hand::Right);
        assert!(snap.has_controller_tracking());
    }

    #[test]
    fn snapshot_connected_but_no_confidence_no_tracking() {
        let mut snap = TrackingSnapshot::empty(1_000);
        snap.left_controller.connected = true;
        snap.left_controller.tracking_confidence = TrackingConfidence::None;
        assert!(!snap.has_controller_tracking());
    }

    #[test]
    fn snapshot_predicted_display_time() {
        let snap = sample_snapshot();
        assert_eq!(snap.predicted_display_time_ns, 1_011_111);
    }

    // ---- TrackingPipeline ----

    #[test]
    fn new_pipeline_has_no_snapshot() {
        let pipeline = TrackingPipeline::new(false, DEFAULT_TRACKING_PREDICTION_NS);
        assert!(pipeline.latest_snapshot().is_none());
        assert_eq!(pipeline.snapshot_count(), 0);
    }

    #[test]
    fn submit_snapshot_stores_latest() {
        let mut pipeline = TrackingPipeline::new(false, DEFAULT_TRACKING_PREDICTION_NS);
        let snap = sample_snapshot();
        pipeline.submit_snapshot(snap);

        assert!(pipeline.latest_snapshot().is_some());
        assert_eq!(pipeline.snapshot_count(), 1);
        assert_eq!(pipeline.latest_snapshot().unwrap().timestamp_ns, 1_000_000);
    }

    #[test]
    fn submit_multiple_snapshots_keeps_latest() {
        let mut pipeline = TrackingPipeline::new(false, DEFAULT_TRACKING_PREDICTION_NS);

        let snap1 = TrackingSnapshot::empty(1_000);
        let snap2 = TrackingSnapshot::empty(2_000);
        pipeline.submit_snapshot(snap1);
        pipeline.submit_snapshot(snap2);

        assert_eq!(pipeline.snapshot_count(), 2);
        assert_eq!(pipeline.latest_snapshot().unwrap().timestamp_ns, 2_000);
    }

    #[test]
    fn pipeline_hand_tracking_enabled() {
        let pipeline = TrackingPipeline::new(true, DEFAULT_TRACKING_PREDICTION_NS);
        assert!(pipeline.hand_tracking_enabled());
    }

    #[test]
    fn pipeline_hand_tracking_disabled() {
        let pipeline = TrackingPipeline::new(false, DEFAULT_TRACKING_PREDICTION_NS);
        assert!(!pipeline.hand_tracking_enabled());
    }

    #[test]
    fn pipeline_prediction_offset() {
        let pipeline = TrackingPipeline::new(false, 5_000_000);
        assert_eq!(pipeline.prediction_offset_ns(), 5_000_000);
    }

    #[test]
    fn pipeline_predicted_display_time() {
        let pipeline = TrackingPipeline::new(false, DEFAULT_TRACKING_PREDICTION_NS);
        let predicted = pipeline.predicted_display_time(1_000_000);
        assert_eq!(predicted, 1_000_000 + DEFAULT_TRACKING_PREDICTION_NS);
    }

    #[test]
    fn pipeline_predicted_display_time_saturates() {
        let pipeline = TrackingPipeline::new(false, u64::MAX);
        let predicted = pipeline.predicted_display_time(1_000);
        assert_eq!(predicted, u64::MAX);
    }

    #[test]
    fn pipeline_clear_removes_snapshot() {
        let mut pipeline = TrackingPipeline::new(false, DEFAULT_TRACKING_PREDICTION_NS);
        pipeline.submit_snapshot(sample_snapshot());
        assert!(pipeline.latest_snapshot().is_some());

        pipeline.clear();
        assert!(pipeline.latest_snapshot().is_none());
        // Count is preserved
        assert_eq!(pipeline.snapshot_count(), 1);
    }

    // ---- Identity pose ----

    #[test]
    fn identity_pose_is_zero_position_unit_quaternion() {
        let pose = identity_pose();
        assert_eq!(pose.position, [0.0, 0.0, 0.0]);
        assert_eq!(pose.rotation, [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(pose.linear_velocity, [0.0, 0.0, 0.0]);
        assert_eq!(pose.angular_velocity, [0.0, 0.0, 0.0]);
    }
}
