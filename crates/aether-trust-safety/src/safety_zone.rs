//! Safety zone quick-escape mechanics.
//!
//! Provides instant teleportation to a safe spawn point when a user
//! triggers a panic gesture.

use crate::personal_space::Vec3;

/// Recognized panic gesture names that trigger a safety teleport.
pub const PANIC_GESTURES: &[&str] = &["panic", "safety", "escape"];

/// A safe zone definition for a world.
#[derive(Debug, Clone)]
pub struct SafeZone {
    /// The safe spawn point position.
    pub spawn_point: Vec3,
    /// The world this safe zone belongs to.
    pub world_id: String,
}

/// A request to teleport a user to safety.
#[derive(Debug, Clone)]
pub struct TeleportRequest {
    /// The user being teleported.
    pub user_id: u64,
    /// The destination position.
    pub destination: Vec3,
    /// The world the user is teleporting within.
    pub world_id: String,
}

/// Check whether a gesture name is a recognized panic gesture.
///
/// Comparison is case-insensitive.
pub fn is_panic_gesture(gesture_name: &str) -> bool {
    let lower = gesture_name.to_lowercase();
    PANIC_GESTURES.iter().any(|g| *g == lower)
}

/// Trigger a safety teleport for the given user to the safe zone.
pub fn trigger_safety_teleport(user_id: u64, zone: &SafeZone) -> TeleportRequest {
    TeleportRequest {
        user_id,
        destination: zone.spawn_point.clone(),
        world_id: zone.world_id.clone(),
    }
}

/// Process a gesture event: if it is a panic gesture, produce a teleport request.
pub fn process_gesture(
    gesture_name: &str,
    user_id: u64,
    zone: &SafeZone,
) -> Option<TeleportRequest> {
    if is_panic_gesture(gesture_name) {
        Some(trigger_safety_teleport(user_id, zone))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_zone() -> SafeZone {
        SafeZone {
            spawn_point: Vec3::new(0.0, 1.0, 0.0),
            world_id: "world-42".to_string(),
        }
    }

    #[test]
    fn panic_gesture_recognized() {
        assert!(is_panic_gesture("panic"));
        assert!(is_panic_gesture("safety"));
        assert!(is_panic_gesture("escape"));
    }

    #[test]
    fn panic_gesture_case_insensitive() {
        assert!(is_panic_gesture("PANIC"));
        assert!(is_panic_gesture("Safety"));
        assert!(is_panic_gesture("ESCAPE"));
    }

    #[test]
    fn non_panic_gesture_rejected() {
        assert!(!is_panic_gesture("wave"));
        assert!(!is_panic_gesture("thumbs_up"));
        assert!(!is_panic_gesture(""));
    }

    #[test]
    fn trigger_teleport_creates_request() {
        let zone = test_zone();
        let req = trigger_safety_teleport(99, &zone);
        assert_eq!(req.user_id, 99);
        assert_eq!(req.world_id, "world-42");
        assert_eq!(req.destination, Vec3::new(0.0, 1.0, 0.0));
    }

    #[test]
    fn process_gesture_panic_returns_some() {
        let zone = test_zone();
        let req = process_gesture("panic", 7, &zone);
        assert!(req.is_some());
        let req = req.unwrap();
        assert_eq!(req.user_id, 7);
        assert_eq!(req.destination, zone.spawn_point);
    }

    #[test]
    fn process_gesture_non_panic_returns_none() {
        let zone = test_zone();
        let req = process_gesture("wave", 7, &zone);
        assert!(req.is_none());
    }

    #[test]
    fn teleport_destination_matches_spawn() {
        let zone = SafeZone {
            spawn_point: Vec3::new(10.0, 20.0, 30.0),
            world_id: "test".to_string(),
        };
        let req = trigger_safety_teleport(1, &zone);
        assert_eq!(req.destination.x, 10.0);
        assert_eq!(req.destination.y, 20.0);
        assert_eq!(req.destination.z, 30.0);
    }
}
