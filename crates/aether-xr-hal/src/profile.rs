//! Interaction profiles and binding paths.
//!
//! See design doc §5.6 and §7. Profiles correspond to the OpenXR
//! `xrSuggestInteractionProfileBindings` profile arg; binding paths correspond to
//! the slash-separated component paths (e.g. `/user/hand/left/input/trigger/value`).

/// Interaction profile a binding is suggested against.
///
/// Each variant maps to a canonical OpenXR interaction profile path; the OpenXR
/// backend translates these into the full `/interaction_profiles/...` strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InteractionProfile {
    /// `/interaction_profiles/oculus/touch_controller`
    Touch,
    /// `/interaction_profiles/valve/index_controller`
    Index,
    /// `/interaction_profiles/htc/vive_controller`
    Vive,
    /// `/interaction_profiles/ext/hand_interaction_ext` (hand tracking).
    Hand,
}

/// OpenXR-style component path (e.g. `/user/hand/left/input/trigger/value`).
///
/// Newtype around `String` so the type system can distinguish a binding path from
/// an arbitrary string and so that future validation (e.g. ensuring leading `/`)
/// can be added without breaking call sites.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BindingPath(pub String);

impl BindingPath {
    pub fn new(path: impl Into<String>) -> Self {
        Self(path.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for BindingPath {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for BindingPath {
    fn from(s: String) -> Self {
        Self(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binding_path_roundtrips_str() {
        let p = BindingPath::from("/user/hand/left/input/trigger/value");
        assert_eq!(p.as_str(), "/user/hand/left/input/trigger/value");
    }

    #[test]
    fn interaction_profile_is_hashable() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(InteractionProfile::Touch);
        set.insert(InteractionProfile::Touch);
        assert_eq!(set.len(), 1);
    }
}
