//! Effect system for the Behavior DSL.
//!
//! Each verb declares an effect. Combinators union the effects of their
//! children. The type checker uses this to enforce that a behavior only uses
//! capabilities it has declared via `@caps(...)`.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Single effect label.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Ord, PartialOrd)]
pub enum Effect {
    Pure,
    Movement,
    Combat,
    Network,
    Persistence,
    Economy,
}

impl Effect {
    /// Parse a source identifier into an `Effect`.
    pub fn from_name(name: &str) -> Option<Effect> {
        match name {
            "Pure" => Some(Effect::Pure),
            "Movement" => Some(Effect::Movement),
            "Combat" => Some(Effect::Combat),
            "Network" => Some(Effect::Network),
            "Persistence" => Some(Effect::Persistence),
            "Economy" => Some(Effect::Economy),
            _ => None,
        }
    }

    /// Returns the canonical source-form name for this effect.
    pub fn name(&self) -> &'static str {
        match self {
            Effect::Pure => "Pure",
            Effect::Movement => "Movement",
            Effect::Combat => "Combat",
            Effect::Network => "Network",
            Effect::Persistence => "Persistence",
            Effect::Economy => "Economy",
        }
    }
}

impl fmt::Display for Effect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// A set of effects.
///
/// Stored sorted + deduped for deterministic display and snapshotting.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectSet(Vec<Effect>);

impl EffectSet {
    /// Create a new empty set (implicitly `Pure`).
    pub fn pure() -> Self {
        EffectSet(Vec::new())
    }

    /// Create a set from a single effect, dropping `Pure` (which is the identity).
    pub fn single(effect: Effect) -> Self {
        if matches!(effect, Effect::Pure) {
            return EffectSet::pure();
        }
        EffectSet(vec![effect])
    }

    /// Union two effect sets.
    pub fn union(mut self, other: &EffectSet) -> Self {
        for e in &other.0 {
            if !self.0.contains(e) {
                self.0.push(*e);
            }
        }
        self.0.sort();
        self
    }

    /// Iterator over effects.
    pub fn iter(&self) -> impl Iterator<Item = Effect> + '_ {
        self.0.iter().copied()
    }

    /// Returns `true` if the set contains the given effect.
    pub fn contains(&self, e: Effect) -> bool {
        self.0.contains(&e)
    }

    /// Returns `true` if the set is empty (pure).
    pub fn is_pure(&self) -> bool {
        self.0.is_empty()
    }

    /// Number of non-Pure effects.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Is empty check.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// All effects as a sorted vector (stable for serialization).
    pub fn as_vec(&self) -> Vec<Effect> {
        self.0.clone()
    }
}

impl fmt::Display for EffectSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_empty() {
            return f.write_str("Pure");
        }
        let names: Vec<&str> = self.0.iter().map(|e| e.name()).collect();
        f.write_str(&names.join("|"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pure_is_identity() {
        let a = EffectSet::single(Effect::Pure);
        assert!(a.is_pure());
        let b = EffectSet::single(Effect::Movement);
        let c = b.clone().union(&a);
        assert_eq!(b, c);
    }

    #[test]
    fn union_is_idempotent_and_sorted() {
        let a = EffectSet::single(Effect::Network);
        let b = EffectSet::single(Effect::Economy);
        let combined = a.clone().union(&b).union(&a);
        let effects = combined.as_vec();
        assert!(effects.contains(&Effect::Economy));
        assert!(effects.contains(&Effect::Network));
        assert_eq!(effects.len(), 2);
    }
}
