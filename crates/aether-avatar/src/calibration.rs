//! T-pose calibration for scaling IK proportions to individual users.
//!
//! Captures a reference T-pose from tracking data and computes body
//! proportions (arm lengths, leg lengths, height) to calibrate the IK solver.

use crate::ik::BodyProportions;
use crate::skeleton::vec3_distance;
use crate::tracking::TrackingFrame;

/// Minimum plausible arm span in meters (child).
const MIN_ARM_SPAN: f32 = 0.6;
/// Maximum plausible arm span in meters (very tall person).
const MAX_ARM_SPAN: f32 = 2.5;
/// Minimum plausible height in meters.
const MIN_HEIGHT: f32 = 1.0;
/// Maximum plausible height in meters.
const MAX_HEIGHT: f32 = 2.3;
/// Ratio of upper arm to total arm length.
const UPPER_ARM_RATIO: f32 = 0.53;
/// Ratio of thigh to total leg length.
const THIGH_RATIO: f32 = 0.52;
/// Ratio of shoulder width to arm span.
const SHOULDER_SPAN_RATIO: f32 = 0.24;
/// Ratio of spine length to total height.
const SPINE_HEIGHT_RATIO: f32 = 0.30;

/// Data captured during T-pose calibration.
#[derive(Debug, Clone)]
pub struct CalibrationData {
    /// Measured arm span (left hand to right hand, through head).
    pub arm_span: f32,
    /// Measured height (head to estimated floor).
    pub height: f32,
    /// Computed body proportions.
    pub proportions: BodyProportions,
}

/// Error returned when calibration data is implausible.
#[derive(Debug, Clone, PartialEq)]
pub enum CalibrationError {
    /// Hands are missing from the tracking frame.
    MissingHands,
    /// Arm span is outside the plausible range.
    ImplausibleArmSpan(f32),
    /// Height is outside the plausible range.
    ImplausibleHeight(f32),
}

/// Calibrate body proportions from a T-pose tracking frame.
///
/// The user should be standing upright with arms extended horizontally.
///
/// # Arguments
/// * `frame` - A `TrackingFrame` captured during the T-pose
/// * `floor_y` - The Y coordinate of the floor plane (typically 0.0)
///
/// # Returns
/// A `CalibrationData` with computed proportions, or an error if the
/// tracking data is implausible.
pub fn calibrate_from_tpose(
    frame: &TrackingFrame,
    floor_y: f32,
) -> Result<CalibrationData, CalibrationError> {
    let (left_hand, right_hand) = frame.hands.as_ref().ok_or(CalibrationError::MissingHands)?;

    let lh = [left_hand.x, left_hand.y, left_hand.z];
    let rh = [right_hand.x, right_hand.y, right_hand.z];
    let head = [frame.head.x, frame.head.y, frame.head.z];

    // Compute arm span (left hand -> head -> right hand)
    let left_arm_len = vec3_distance(lh, head);
    let right_arm_len = vec3_distance(rh, head);
    let arm_span = left_arm_len + right_arm_len;

    if !(MIN_ARM_SPAN..=MAX_ARM_SPAN).contains(&arm_span) {
        return Err(CalibrationError::ImplausibleArmSpan(arm_span));
    }

    // Compute height from head to floor
    let height = frame.head.y - floor_y;
    if !(MIN_HEIGHT..=MAX_HEIGHT).contains(&height) {
        return Err(CalibrationError::ImplausibleHeight(height));
    }

    // Derive proportions
    let half_arm_span = arm_span * 0.5;
    let shoulder_half_width = arm_span * SHOULDER_SPAN_RATIO * 0.5;
    let arm_length = half_arm_span - shoulder_half_width;
    let upper_arm_length = arm_length * UPPER_ARM_RATIO;
    let forearm_length = arm_length * (1.0 - UPPER_ARM_RATIO);

    let spine_length = height * SPINE_HEIGHT_RATIO;
    let leg_length = height - spine_length - 0.15; // 0.15 for head+neck
    let thigh_length = leg_length * THIGH_RATIO;
    let shin_length = leg_length * (1.0 - THIGH_RATIO);

    let proportions = BodyProportions {
        spine_length,
        shoulder_half_width,
        upper_arm_length,
        forearm_length,
        thigh_length,
        shin_length,
    };

    Ok(CalibrationData {
        arm_span,
        height,
        proportions,
    })
}

/// Scale existing proportions by a uniform factor.
pub fn scale_proportions(proportions: &BodyProportions, scale: f32) -> BodyProportions {
    BodyProportions {
        spine_length: proportions.spine_length * scale,
        shoulder_half_width: proportions.shoulder_half_width * scale,
        upper_arm_length: proportions.upper_arm_length * scale,
        forearm_length: proportions.forearm_length * scale,
        thigh_length: proportions.thigh_length * scale,
        shin_length: proportions.shin_length * scale,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tracking::{IkPoint, TrackingSource};

    const EPSILON: f32 = 0.01;

    fn tpose_frame(height: f32, arm_span: f32) -> TrackingFrame {
        let half_span = arm_span * 0.5;
        TrackingFrame {
            player_id: 1,
            source: TrackingSource::ThreePoint,
            head: IkPoint {
                x: 0.0,
                y: height,
                z: 0.0,
            },
            hands: Some((
                IkPoint {
                    x: -half_span,
                    y: height - 0.15, // shoulders slightly below head
                    z: 0.0,
                },
                IkPoint {
                    x: half_span,
                    y: height - 0.15,
                    z: 0.0,
                },
            )),
            feet: None,
            hips: None,
            timestamp_ms: 0,
        }
    }

    #[test]
    fn test_calibrate_average_person() {
        let frame = tpose_frame(1.7, 1.7);
        let result = calibrate_from_tpose(&frame, 0.0).unwrap();

        assert!((result.height - 1.7).abs() < EPSILON);
        assert!(result.arm_span > 1.5);
        assert!(result.proportions.upper_arm_length > 0.0);
        assert!(result.proportions.forearm_length > 0.0);
        assert!(result.proportions.spine_length > 0.0);
        assert!(result.proportions.thigh_length > 0.0);
        assert!(result.proportions.shin_length > 0.0);
    }

    #[test]
    fn test_calibrate_tall_person() {
        let frame = tpose_frame(2.0, 2.1);
        let result = calibrate_from_tpose(&frame, 0.0).unwrap();

        assert!(result.proportions.upper_arm_length > 0.3);
        assert!(result.proportions.thigh_length > 0.4);
    }

    #[test]
    fn test_calibrate_short_person() {
        let frame = tpose_frame(1.5, 1.4);
        let result = calibrate_from_tpose(&frame, 0.0).unwrap();

        assert!(result.proportions.upper_arm_length < 0.35);
        assert!(result.proportions.upper_arm_length > 0.15);
    }

    #[test]
    fn test_calibrate_proportional_scaling() {
        let frame1 = tpose_frame(1.5, 1.4);
        let frame2 = tpose_frame(2.0, 2.0);
        let cal1 = calibrate_from_tpose(&frame1, 0.0).unwrap();
        let cal2 = calibrate_from_tpose(&frame2, 0.0).unwrap();

        // Taller person should have longer bones
        assert!(cal2.proportions.upper_arm_length > cal1.proportions.upper_arm_length);
        assert!(cal2.proportions.thigh_length > cal1.proportions.thigh_length);
        assert!(cal2.proportions.spine_length > cal1.proportions.spine_length);
    }

    #[test]
    fn test_calibrate_missing_hands() {
        let frame = TrackingFrame {
            player_id: 1,
            source: TrackingSource::HmdOnly,
            head: IkPoint {
                x: 0.0,
                y: 1.7,
                z: 0.0,
            },
            hands: None,
            feet: None,
            hips: None,
            timestamp_ms: 0,
        };

        let result = calibrate_from_tpose(&frame, 0.0);
        assert_eq!(result.unwrap_err(), CalibrationError::MissingHands);
    }

    #[test]
    fn test_calibrate_implausible_arm_span() {
        // Arms way too short
        let frame = tpose_frame(1.7, 0.3);
        let result = calibrate_from_tpose(&frame, 0.0);
        assert!(matches!(
            result.unwrap_err(),
            CalibrationError::ImplausibleArmSpan(_)
        ));
    }

    #[test]
    fn test_calibrate_implausible_height() {
        // Head at floor level
        let frame = tpose_frame(0.5, 1.0);
        let result = calibrate_from_tpose(&frame, 0.0);
        assert!(matches!(
            result.unwrap_err(),
            CalibrationError::ImplausibleHeight(_)
        ));
    }

    #[test]
    fn test_calibrate_with_floor_offset() {
        let frame = tpose_frame(2.7, 1.7); // head at 2.7, floor at 1.0
        let result = calibrate_from_tpose(&frame, 1.0).unwrap();
        assert!((result.height - 1.7).abs() < EPSILON);
    }

    #[test]
    fn test_scale_proportions() {
        let base = BodyProportions::default();
        let scaled = scale_proportions(&base, 2.0);
        assert!((scaled.upper_arm_length - base.upper_arm_length * 2.0).abs() < EPSILON);
        assert!((scaled.spine_length - base.spine_length * 2.0).abs() < EPSILON);
    }

    #[test]
    fn test_upper_arm_longer_than_forearm() {
        let frame = tpose_frame(1.7, 1.7);
        let cal = calibrate_from_tpose(&frame, 0.0).unwrap();
        // Anatomically, upper arm is slightly longer than forearm
        assert!(cal.proportions.upper_arm_length > cal.proportions.forearm_length);
    }

    #[test]
    fn test_thigh_longer_than_shin() {
        let frame = tpose_frame(1.7, 1.7);
        let cal = calibrate_from_tpose(&frame, 0.0).unwrap();
        assert!(cal.proportions.thigh_length > cal.proportions.shin_length);
    }
}
