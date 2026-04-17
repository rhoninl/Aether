//! Capability tokens for the Behavior DSL.
//!
//! Capabilities are declared by the author via `@caps(...)` in the module
//! header and are required to unlock certain effects. Specifically:
//!
//! | Capability  | Unlocks effect |
//! |-------------|----------------|
//! | Network     | Network        |
//! | Persistence | Persistence    |
//! | Economy     | Economy        |
//! | Movement    | Movement       |
//! | Combat      | Combat         |
//!
//! Pure code needs no capability. The compiler refuses a behavior whose
//! effect set contains an effect without its matching capability.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

use crate::effects::Effect;

/// Capability token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Ord, PartialOrd)]
pub enum Capability {
    Network,
    Persistence,
    Economy,
    Movement,
    Combat,
}

impl Capability {
    pub fn name(&self) -> &'static str {
        match self {
            Capability::Network => "Network",
            Capability::Persistence => "Persistence",
            Capability::Economy => "Economy",
            Capability::Movement => "Movement",
            Capability::Combat => "Combat",
        }
    }

    pub fn from_name(name: &str) -> Option<Capability> {
        match name {
            "Network" => Some(Capability::Network),
            "Persistence" => Some(Capability::Persistence),
            "Economy" => Some(Capability::Economy),
            "Movement" => Some(Capability::Movement),
            "Combat" => Some(Capability::Combat),
            _ => None,
        }
    }

    /// Capability required to use the given effect, if any.
    pub fn required_for(effect: Effect) -> Option<Capability> {
        match effect {
            Effect::Pure => None,
            Effect::Movement => Some(Capability::Movement),
            Effect::Combat => Some(Capability::Combat),
            Effect::Network => Some(Capability::Network),
            Effect::Persistence => Some(Capability::Persistence),
            Effect::Economy => Some(Capability::Economy),
        }
    }
}

/// The set of capabilities declared on a behavior module.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilitySet(BTreeSet<Capability>);

impl CapabilitySet {
    pub fn new() -> Self {
        CapabilitySet(BTreeSet::new())
    }

    pub fn insert(&mut self, cap: Capability) {
        self.0.insert(cap);
    }

    pub fn contains(&self, cap: Capability) -> bool {
        self.0.contains(&cap)
    }

    pub fn iter(&self) -> impl Iterator<Item = Capability> + '_ {
        self.0.iter().copied()
    }

    pub fn as_vec(&self) -> Vec<Capability> {
        self.0.iter().copied().collect()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effect_to_capability_mapping() {
        assert_eq!(Capability::required_for(Effect::Pure), None);
        assert_eq!(
            Capability::required_for(Effect::Movement),
            Some(Capability::Movement)
        );
        assert_eq!(
            Capability::required_for(Effect::Combat),
            Some(Capability::Combat)
        );
        assert_eq!(
            Capability::required_for(Effect::Network),
            Some(Capability::Network)
        );
        assert_eq!(
            Capability::required_for(Effect::Persistence),
            Some(Capability::Persistence)
        );
        assert_eq!(
            Capability::required_for(Effect::Economy),
            Some(Capability::Economy)
        );
    }
}
