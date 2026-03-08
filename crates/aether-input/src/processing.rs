//! Input processing pipeline: raw input -> dead zone -> sensitivity -> action mapping -> gesture -> action events.

use std::collections::HashSet;

use crate::deadzone::{apply_dead_zone, apply_sensitivity, DeadZoneConfig, SensitivityCurve};
use crate::desktop::KeyCode;
use crate::graph::{ActionEvent, GestureDetector};
use crate::mapping::{ActionMap, InputSource};

/// Raw input state snapshot fed into the pipeline each frame.
#[derive(Debug, Clone)]
pub struct RawInputState {
    /// Currently pressed keyboard keys.
    pub pressed_keys: HashSet<KeyCode>,
    /// Thumbstick / analog stick X axis [-1, 1].
    pub axis_x: f32,
    /// Thumbstick / analog stick Y axis [-1, 1].
    pub axis_y: f32,
    /// Mouse delta X (pixels or arbitrary units).
    pub mouse_dx: f32,
    /// Mouse delta Y (pixels or arbitrary units).
    pub mouse_dy: f32,
}

impl Default for RawInputState {
    fn default() -> Self {
        Self {
            pressed_keys: HashSet::new(),
            axis_x: 0.0,
            axis_y: 0.0,
            mouse_dx: 0.0,
            mouse_dy: 0.0,
        }
    }
}

/// Processed axis values after dead zone and sensitivity.
#[derive(Debug, Clone, Copy)]
pub struct ProcessedAxes {
    pub x: f32,
    pub y: f32,
}

/// The input processing pipeline.
pub struct InputPipeline {
    action_map: ActionMap,
    dead_zone: DeadZoneConfig,
    sensitivity: SensitivityCurve,
    gesture_detector: GestureDetector,
    prev_keys: HashSet<KeyCode>,
    initialized: bool,
}

impl std::fmt::Debug for InputPipeline {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InputPipeline")
            .field("dead_zone", &self.dead_zone)
            .field("initialized", &self.initialized)
            .finish()
    }
}

impl InputPipeline {
    /// Create a new pipeline with the given configuration.
    pub fn new(
        action_map: ActionMap,
        dead_zone: DeadZoneConfig,
        sensitivity: SensitivityCurve,
    ) -> Self {
        let mut gesture_detector = GestureDetector::new();

        // Register all bindings from the action map
        for binding in action_map.bindings() {
            gesture_detector.register(
                &binding.action_name,
                binding.input.clone(),
                &binding.gesture,
            );
        }

        Self {
            action_map,
            dead_zone,
            sensitivity,
            gesture_detector,
            prev_keys: HashSet::new(),
            initialized: false,
        }
    }

    /// Process raw input and return action events.
    ///
    /// Call this once per frame with the current input state and timestamp.
    pub fn process(&mut self, state: &RawInputState, now_ms: u64) -> Vec<ActionEvent> {
        let mut events = Vec::new();

        // 1. Detect key press/release transitions
        let just_pressed: Vec<KeyCode> = state
            .pressed_keys
            .iter()
            .filter(|k| !self.prev_keys.contains(k))
            .copied()
            .collect();

        let just_released: Vec<KeyCode> = self
            .prev_keys
            .iter()
            .filter(|k| !state.pressed_keys.contains(k))
            .copied()
            .collect();

        // 2. Feed transitions into gesture detector
        for key in &just_pressed {
            let source = InputSource::Keyboard(*key);
            let mut key_events = self.gesture_detector.update(&source, true, now_ms);
            events.append(&mut key_events);
        }

        for key in &just_released {
            let source = InputSource::Keyboard(*key);
            let mut key_events = self.gesture_detector.update(&source, false, now_ms);
            events.append(&mut key_events);
        }

        // 3. Tick gesture detector for time-based events (hold, double-tap timeout)
        let mut tick_events = self.gesture_detector.tick(now_ms);
        events.append(&mut tick_events);

        // 4. Save previous key state
        self.prev_keys = state.pressed_keys.clone();
        self.initialized = true;

        events
    }

    /// Process analog axes through dead zone and sensitivity curve.
    pub fn process_axes(&self, x: f32, y: f32) -> ProcessedAxes {
        let (dz_x, dz_y) = apply_dead_zone(x, y, &self.dead_zone);
        let sx = apply_sensitivity(dz_x, &self.sensitivity);
        let sy = apply_sensitivity(dz_y, &self.sensitivity);
        ProcessedAxes { x: sx, y: sy }
    }

    /// Get a reference to the action map.
    pub fn action_map(&self) -> &ActionMap {
        &self.action_map
    }

    /// Get a reference to the dead zone config.
    pub fn dead_zone_config(&self) -> &DeadZoneConfig {
        &self.dead_zone
    }

    /// Update the dead zone configuration.
    pub fn set_dead_zone(&mut self, config: DeadZoneConfig) {
        self.dead_zone = config;
    }

    /// Update the sensitivity curve.
    pub fn set_sensitivity(&mut self, curve: SensitivityCurve) {
        self.sensitivity = curve;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deadzone::{DeadZoneConfig, DeadZoneShape, SensitivityCurve};
    use crate::graph::{ActionEventPhase, InputGesture};
    use crate::mapping::ActionMap;

    const EPSILON: f32 = 1e-5;

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < EPSILON
    }

    fn make_simple_pipeline() -> InputPipeline {
        let mut map = ActionMap::new();
        map.bind(
            "jump",
            InputSource::Keyboard(KeyCode::Space),
            InputGesture::Press,
        );
        map.bind(
            "move_forward",
            InputSource::Keyboard(KeyCode::W),
            InputGesture::Press,
        );
        map.bind(
            "charge",
            InputSource::Keyboard(KeyCode::E),
            InputGesture::Hold {
                min_duration_ms: 500,
            },
        );

        InputPipeline::new(
            map,
            DeadZoneConfig::default(),
            SensitivityCurve::Linear,
        )
    }

    #[test]
    fn pipeline_press_generates_started_event() {
        let mut pipeline = make_simple_pipeline();
        let mut state = RawInputState::default();
        state.pressed_keys.insert(KeyCode::Space);

        let events = pipeline.process(&state, 100);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].action_name, "jump");
        assert_eq!(events[0].phase, ActionEventPhase::Started);
    }

    #[test]
    fn pipeline_release_generates_ended_event() {
        let mut pipeline = make_simple_pipeline();

        // Press
        let mut state = RawInputState::default();
        state.pressed_keys.insert(KeyCode::Space);
        pipeline.process(&state, 100);

        // Release
        let state2 = RawInputState::default();
        let events = pipeline.process(&state2, 200);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].action_name, "jump");
        assert_eq!(events[0].phase, ActionEventPhase::Ended);
    }

    #[test]
    fn pipeline_held_key_no_repeat() {
        let mut pipeline = make_simple_pipeline();
        let mut state = RawInputState::default();
        state.pressed_keys.insert(KeyCode::Space);

        let e1 = pipeline.process(&state, 100);
        assert_eq!(e1.len(), 1);

        let e2 = pipeline.process(&state, 200);
        // No repeat for Press gesture
        assert!(e2.is_empty());
    }

    #[test]
    fn pipeline_hold_gesture_fires_after_duration() {
        let mut pipeline = make_simple_pipeline();

        // Press E
        let mut state = RawInputState::default();
        state.pressed_keys.insert(KeyCode::E);
        let e1 = pipeline.process(&state, 100);
        assert!(e1.is_empty()); // Hold doesn't fire immediately

        // Still held, tick past duration
        let e2 = pipeline.process(&state, 700);
        assert_eq!(e2.len(), 1);
        assert_eq!(e2[0].action_name, "charge");
        assert_eq!(e2[0].phase, ActionEventPhase::Started);
    }

    #[test]
    fn pipeline_multiple_keys_simultaneous() {
        let mut pipeline = make_simple_pipeline();
        let mut state = RawInputState::default();
        state.pressed_keys.insert(KeyCode::Space);
        state.pressed_keys.insert(KeyCode::W);

        let events = pipeline.process(&state, 100);
        assert_eq!(events.len(), 2);

        let names: Vec<&str> = events.iter().map(|e| e.action_name.as_str()).collect();
        assert!(names.contains(&"jump"));
        assert!(names.contains(&"move_forward"));
    }

    #[test]
    fn pipeline_axes_with_dead_zone() {
        let pipeline = InputPipeline::new(
            ActionMap::new(),
            DeadZoneConfig {
                inner_radius: 0.2,
                outer_radius: 0.9,
                shape: DeadZoneShape::Circular,
            },
            SensitivityCurve::Linear,
        );

        // Inside dead zone
        let axes = pipeline.process_axes(0.1, 0.1);
        assert!(approx_eq(axes.x, 0.0));
        assert!(approx_eq(axes.y, 0.0));

        // Outside dead zone
        let axes2 = pipeline.process_axes(1.0, 0.0);
        assert!(approx_eq(axes2.x, 1.0), "x={}", axes2.x);
    }

    #[test]
    fn pipeline_axes_with_sensitivity_curve() {
        let pipeline = InputPipeline::new(
            ActionMap::new(),
            DeadZoneConfig {
                inner_radius: 0.0,
                outer_radius: 1.0,
                shape: DeadZoneShape::Circular,
            },
            SensitivityCurve::Quadratic,
        );

        let axes = pipeline.process_axes(0.5, 0.0);
        // Quadratic: 0.5^2 = 0.25
        assert!(approx_eq(axes.x, 0.25), "x={}", axes.x);
    }

    #[test]
    fn pipeline_unbound_key_produces_no_events() {
        let mut pipeline = make_simple_pipeline();
        let mut state = RawInputState::default();
        state.pressed_keys.insert(KeyCode::R); // Not bound
        let events = pipeline.process(&state, 100);
        assert!(events.is_empty());
    }

    #[test]
    fn pipeline_set_dead_zone_updates_config() {
        let mut pipeline = make_simple_pipeline();
        pipeline.set_dead_zone(DeadZoneConfig {
            inner_radius: 0.3,
            outer_radius: 0.8,
            shape: DeadZoneShape::Square,
        });
        assert!(approx_eq(pipeline.dead_zone_config().inner_radius, 0.3));
    }

    #[test]
    fn pipeline_set_sensitivity_updates_curve() {
        let mut pipeline = InputPipeline::new(
            ActionMap::new(),
            DeadZoneConfig {
                inner_radius: 0.0,
                outer_radius: 1.0,
                shape: DeadZoneShape::Circular,
            },
            SensitivityCurve::Linear,
        );
        pipeline.set_sensitivity(SensitivityCurve::Cubic);
        let axes = pipeline.process_axes(0.5, 0.0);
        // Cubic: 0.5^3 = 0.125
        assert!(approx_eq(axes.x, 0.125), "x={}", axes.x);
    }

    #[test]
    fn pipeline_full_integration() {
        // Full pipeline: key press -> action event -> process axes -> movement direction
        let mut map = ActionMap::new();
        map.bind(
            "move_forward",
            InputSource::Keyboard(KeyCode::W),
            InputGesture::Press,
        );

        let mut pipeline = InputPipeline::new(
            map,
            DeadZoneConfig::default(),
            SensitivityCurve::Linear,
        );

        let mut state = RawInputState::default();
        state.pressed_keys.insert(KeyCode::W);
        state.axis_x = 0.5;
        state.axis_y = -0.8;

        let events = pipeline.process(&state, 100);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].action_name, "move_forward");

        let axes = pipeline.process_axes(state.axis_x, state.axis_y);
        // With default dead zone (0.1 inner), axis values should be remapped
        assert!(axes.x != 0.0 || axes.y != 0.0);
    }
}
