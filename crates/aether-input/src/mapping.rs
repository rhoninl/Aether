//! Configurable action mapping: binds physical inputs to named actions.

use crate::desktop::{KeyCode, MouseAxis, MouseButton};
use crate::graph::InputGesture;

/// A physical input source.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum InputSource {
    /// A keyboard key.
    Keyboard(KeyCode),
    /// A mouse button.
    MouseButton(MouseButton),
    /// A mouse axis (continuous).
    MouseAxis(MouseAxis),
    /// A gamepad button by index.
    GamepadButton(u8),
    /// A gamepad axis by index.
    GamepadAxis(u8),
}

/// A single binding from a physical input to a named action.
#[derive(Debug, Clone)]
pub struct ActionBinding {
    /// The action name this binding triggers (e.g., "jump", "grab", "move_forward").
    pub action_name: String,
    /// The physical input source.
    pub input: InputSource,
    /// The gesture type required (press, hold, double-tap, etc.).
    pub gesture: InputGesture,
}

/// A collection of action bindings with lookup support.
#[derive(Debug, Clone)]
pub struct ActionMap {
    bindings: Vec<ActionBinding>,
}

impl ActionMap {
    /// Create a new empty action map.
    pub fn new() -> Self {
        Self {
            bindings: Vec::new(),
        }
    }

    /// Add a binding from an input source + gesture to a named action.
    pub fn bind(&mut self, action_name: &str, input: InputSource, gesture: InputGesture) {
        self.bindings.push(ActionBinding {
            action_name: action_name.to_string(),
            input,
            gesture,
        });
    }

    /// Look up all bindings for a given input source.
    pub fn resolve(&self, source: &InputSource) -> Vec<&ActionBinding> {
        self.bindings
            .iter()
            .filter(|b| b.input == *source)
            .collect()
    }

    /// Look up all bindings for a given action name.
    pub fn resolve_action(&self, action_name: &str) -> Vec<&ActionBinding> {
        self.bindings
            .iter()
            .filter(|b| b.action_name == action_name)
            .collect()
    }

    /// Get all bindings.
    pub fn bindings(&self) -> &[ActionBinding] {
        &self.bindings
    }

    /// Remove all bindings for a given action name.
    pub fn unbind_action(&mut self, action_name: &str) {
        self.bindings.retain(|b| b.action_name != action_name);
    }

    /// Remove all bindings for a given input source.
    pub fn unbind_source(&mut self, source: &InputSource) {
        self.bindings.retain(|b| b.input != *source);
    }

    /// Create a default WASD + mouse desktop action map.
    pub fn default_desktop() -> Self {
        let mut map = Self::new();
        map.bind(
            "move_forward",
            InputSource::Keyboard(KeyCode::W),
            InputGesture::Press,
        );
        map.bind(
            "move_backward",
            InputSource::Keyboard(KeyCode::S),
            InputGesture::Press,
        );
        map.bind(
            "move_left",
            InputSource::Keyboard(KeyCode::A),
            InputGesture::Press,
        );
        map.bind(
            "move_right",
            InputSource::Keyboard(KeyCode::D),
            InputGesture::Press,
        );
        map.bind(
            "jump",
            InputSource::Keyboard(KeyCode::Space),
            InputGesture::Press,
        );
        map.bind(
            "sprint",
            InputSource::Keyboard(KeyCode::Shift),
            InputGesture::Press,
        );
        map.bind(
            "interact",
            InputSource::Keyboard(KeyCode::E),
            InputGesture::Press,
        );
        map.bind(
            "grab",
            InputSource::MouseButton(MouseButton::Left),
            InputGesture::Press,
        );
        map.bind(
            "aim",
            InputSource::MouseButton(MouseButton::Right),
            InputGesture::Press,
        );
        map.bind(
            "look_x",
            InputSource::MouseAxis(MouseAxis::X),
            InputGesture::Press,
        );
        map.bind(
            "look_y",
            InputSource::MouseAxis(MouseAxis::Y),
            InputGesture::Press,
        );
        map
    }
}

impl Default for ActionMap {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_map_resolves_nothing() {
        let map = ActionMap::new();
        let result = map.resolve(&InputSource::Keyboard(KeyCode::W));
        assert!(result.is_empty());
    }

    #[test]
    fn single_binding_resolved_by_source() {
        let mut map = ActionMap::new();
        map.bind(
            "move_forward",
            InputSource::Keyboard(KeyCode::W),
            InputGesture::Press,
        );
        let result = map.resolve(&InputSource::Keyboard(KeyCode::W));
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].action_name, "move_forward");
    }

    #[test]
    fn multiple_bindings_same_source() {
        let mut map = ActionMap::new();
        map.bind(
            "move_forward",
            InputSource::Keyboard(KeyCode::W),
            InputGesture::Press,
        );
        map.bind(
            "dash",
            InputSource::Keyboard(KeyCode::W),
            InputGesture::DoubleTap {
                max_interval_ms: 300,
            },
        );
        let result = map.resolve(&InputSource::Keyboard(KeyCode::W));
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn multiple_sources_same_action() {
        let mut map = ActionMap::new();
        map.bind(
            "jump",
            InputSource::Keyboard(KeyCode::Space),
            InputGesture::Press,
        );
        map.bind("jump", InputSource::GamepadButton(0), InputGesture::Press);
        let result = map.resolve_action("jump");
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn unbound_source_returns_empty() {
        let mut map = ActionMap::new();
        map.bind(
            "move_forward",
            InputSource::Keyboard(KeyCode::W),
            InputGesture::Press,
        );
        let result = map.resolve(&InputSource::Keyboard(KeyCode::S));
        assert!(result.is_empty());
    }

    #[test]
    fn unbind_action_removes_all_bindings() {
        let mut map = ActionMap::new();
        map.bind(
            "jump",
            InputSource::Keyboard(KeyCode::Space),
            InputGesture::Press,
        );
        map.bind("jump", InputSource::GamepadButton(0), InputGesture::Press);
        map.bind(
            "move_forward",
            InputSource::Keyboard(KeyCode::W),
            InputGesture::Press,
        );
        map.unbind_action("jump");
        assert!(map.resolve_action("jump").is_empty());
        assert_eq!(map.resolve_action("move_forward").len(), 1);
    }

    #[test]
    fn unbind_source_removes_matching_bindings() {
        let mut map = ActionMap::new();
        map.bind(
            "jump",
            InputSource::Keyboard(KeyCode::Space),
            InputGesture::Press,
        );
        map.bind(
            "sprint",
            InputSource::Keyboard(KeyCode::Space),
            InputGesture::Hold {
                min_duration_ms: 200,
            },
        );
        map.unbind_source(&InputSource::Keyboard(KeyCode::Space));
        assert!(map
            .resolve(&InputSource::Keyboard(KeyCode::Space))
            .is_empty());
    }

    #[test]
    fn default_desktop_has_wasd() {
        let map = ActionMap::default_desktop();
        assert!(!map.resolve(&InputSource::Keyboard(KeyCode::W)).is_empty());
        assert!(!map.resolve(&InputSource::Keyboard(KeyCode::A)).is_empty());
        assert!(!map.resolve(&InputSource::Keyboard(KeyCode::S)).is_empty());
        assert!(!map.resolve(&InputSource::Keyboard(KeyCode::D)).is_empty());
    }

    #[test]
    fn default_desktop_has_mouse_bindings() {
        let map = ActionMap::default_desktop();
        assert!(!map
            .resolve(&InputSource::MouseButton(MouseButton::Left))
            .is_empty());
        assert!(!map
            .resolve(&InputSource::MouseAxis(MouseAxis::X))
            .is_empty());
    }

    #[test]
    fn bindings_returns_all() {
        let mut map = ActionMap::new();
        map.bind("a", InputSource::Keyboard(KeyCode::W), InputGesture::Press);
        map.bind("b", InputSource::Keyboard(KeyCode::S), InputGesture::Press);
        assert_eq!(map.bindings().len(), 2);
    }
}
