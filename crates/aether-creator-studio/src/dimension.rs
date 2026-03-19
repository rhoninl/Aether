//! World dimension discriminator.
//!
//! Chosen at world creation time and immutable after creation.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Chosen at world creation time. Immutable after creation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WorldDimension {
    TwoD,
    ThreeD,
}

impl fmt::Display for WorldDimension {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WorldDimension::TwoD => write!(f, "2D"),
            WorldDimension::ThreeD => write!(f, "3D"),
        }
    }
}

impl Default for WorldDimension {
    fn default() -> Self {
        WorldDimension::ThreeD
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_two_d() {
        assert_eq!(format!("{}", WorldDimension::TwoD), "2D");
    }

    #[test]
    fn test_display_three_d() {
        assert_eq!(format!("{}", WorldDimension::ThreeD), "3D");
    }

    #[test]
    fn test_default_is_three_d() {
        assert_eq!(WorldDimension::default(), WorldDimension::ThreeD);
    }

    #[test]
    fn test_equality() {
        assert_eq!(WorldDimension::TwoD, WorldDimension::TwoD);
        assert_eq!(WorldDimension::ThreeD, WorldDimension::ThreeD);
        assert_ne!(WorldDimension::TwoD, WorldDimension::ThreeD);
    }

    #[test]
    fn test_clone() {
        let d = WorldDimension::TwoD;
        let d2 = d.clone();
        assert_eq!(d, d2);
    }

    #[test]
    fn test_debug() {
        let dbg = format!("{:?}", WorldDimension::TwoD);
        assert_eq!(dbg, "TwoD");
        let dbg = format!("{:?}", WorldDimension::ThreeD);
        assert_eq!(dbg, "ThreeD");
    }

    #[test]
    fn test_serialize_round_trip_two_d() {
        let d = WorldDimension::TwoD;
        let json = serde_json::to_string(&d).unwrap();
        let back: WorldDimension = serde_json::from_str(&json).unwrap();
        assert_eq!(d, back);
    }

    #[test]
    fn test_serialize_round_trip_three_d() {
        let d = WorldDimension::ThreeD;
        let json = serde_json::to_string(&d).unwrap();
        let back: WorldDimension = serde_json::from_str(&json).unwrap();
        assert_eq!(d, back);
    }

    #[test]
    fn test_deserialize_from_known_string() {
        let back: WorldDimension = serde_json::from_str("\"TwoD\"").unwrap();
        assert_eq!(back, WorldDimension::TwoD);

        let back: WorldDimension = serde_json::from_str("\"ThreeD\"").unwrap();
        assert_eq!(back, WorldDimension::ThreeD);
    }

    #[test]
    fn test_deserialize_invalid_string() {
        let result: Result<WorldDimension, _> = serde_json::from_str("\"FourD\"");
        assert!(result.is_err());
    }

    #[test]
    fn test_copy_semantics() {
        let d = WorldDimension::TwoD;
        let d2 = d; // Copy
        assert_eq!(d, d2); // original still usable
    }

    #[test]
    fn test_hash_consistency() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(WorldDimension::TwoD);
        set.insert(WorldDimension::ThreeD);
        set.insert(WorldDimension::TwoD); // duplicate
        assert_eq!(set.len(), 2);
    }
}
