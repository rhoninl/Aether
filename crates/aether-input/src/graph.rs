//! Action graph: gesture detection (press, hold, double-tap, combo).
//!
//! Processes raw input events and detects complex gestures using state machines.

use std::collections::HashMap;

use crate::mapping::InputSource;

/// Gesture type that an input binding can require.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputGesture {
    /// Fires on initial press.
    Press,
    /// Fires on release.
    Release,
    /// Fires after key has been held for at least `min_duration_ms`.
    Hold { min_duration_ms: u32 },
    /// Fires on the second press within `max_interval_ms` of the first release.
    DoubleTap { max_interval_ms: u32 },
    /// Fires when all keys in the combo are pressed simultaneously.
    Combo(Vec<InputSource>),
}

/// Phase of a detected action event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionEventPhase {
    /// Action just started (pressed / gesture recognized).
    Started,
    /// Action is ongoing (held).
    Ongoing,
    /// Action ended (released / gesture completed).
    Ended,
}

/// An action event produced by gesture detection.
#[derive(Debug, Clone)]
pub struct ActionEvent {
    /// The action name from the binding.
    pub action_name: String,
    /// The gesture phase.
    pub phase: ActionEventPhase,
    /// Analog value (0.0-1.0 for digital, continuous for analog).
    pub value: f32,
    /// Timestamp in milliseconds.
    pub timestamp_ms: u64,
}

/// Internal state for tracking gesture progress.
#[derive(Debug, Clone)]
enum GestureTracker {
    Press {
        pressed: bool,
    },
    Release {
        was_pressed: bool,
    },
    Hold {
        min_duration_ms: u32,
        press_start_ms: Option<u64>,
        fired: bool,
    },
    DoubleTap {
        max_interval_ms: u32,
        state: DoubleTapState,
    },
}

#[derive(Debug, Clone)]
enum DoubleTapState {
    Idle,
    FirstPressed { _start_ms: u64 },
    WaitingSecond { release_ms: u64 },
}

/// Tracks a single binding's gesture state.
#[derive(Debug, Clone)]
struct TrackedBinding {
    action_name: String,
    source: InputSource,
    tracker: GestureTracker,
}

/// Gesture detector: processes raw input presses/releases and emits `ActionEvent`s.
#[derive(Debug)]
pub struct GestureDetector {
    tracked: Vec<TrackedBinding>,
    /// Current pressed state of each source (for combo detection).
    source_state: HashMap<InputSource, bool>,
}

impl GestureDetector {
    /// Create a new empty gesture detector.
    pub fn new() -> Self {
        Self {
            tracked: Vec::new(),
            source_state: HashMap::new(),
        }
    }

    /// Register a binding for gesture tracking.
    pub fn register(&mut self, action_name: &str, source: InputSource, gesture: &InputGesture) {
        let tracker = match gesture {
            InputGesture::Press => GestureTracker::Press { pressed: false },
            InputGesture::Release => GestureTracker::Release { was_pressed: false },
            InputGesture::Hold { min_duration_ms } => GestureTracker::Hold {
                min_duration_ms: *min_duration_ms,
                press_start_ms: None,
                fired: false,
            },
            InputGesture::DoubleTap { max_interval_ms } => GestureTracker::DoubleTap {
                max_interval_ms: *max_interval_ms,
                state: DoubleTapState::Idle,
            },
            InputGesture::Combo(_) => {
                // Combo is handled differently via source_state
                GestureTracker::Press { pressed: false }
            }
        };

        self.tracked.push(TrackedBinding {
            action_name: action_name.to_string(),
            source,
            tracker,
        });
    }

    /// Process a raw input event (source pressed/released) and return any triggered actions.
    pub fn update(&mut self, source: &InputSource, pressed: bool, now_ms: u64) -> Vec<ActionEvent> {
        self.source_state.insert(source.clone(), pressed);
        let mut events = Vec::new();

        for binding in self.tracked.iter_mut() {
            if binding.source != *source {
                continue;
            }

            match &mut binding.tracker {
                GestureTracker::Press { pressed: was } => {
                    if pressed && !*was {
                        events.push(ActionEvent {
                            action_name: binding.action_name.clone(),
                            phase: ActionEventPhase::Started,
                            value: 1.0,
                            timestamp_ms: now_ms,
                        });
                    } else if !pressed && *was {
                        events.push(ActionEvent {
                            action_name: binding.action_name.clone(),
                            phase: ActionEventPhase::Ended,
                            value: 0.0,
                            timestamp_ms: now_ms,
                        });
                    }
                    *was = pressed;
                }
                GestureTracker::Release { was_pressed } => {
                    if pressed {
                        *was_pressed = true;
                    } else if *was_pressed {
                        events.push(ActionEvent {
                            action_name: binding.action_name.clone(),
                            phase: ActionEventPhase::Started,
                            value: 1.0,
                            timestamp_ms: now_ms,
                        });
                        *was_pressed = false;
                    }
                }
                GestureTracker::Hold {
                    min_duration_ms: _,
                    press_start_ms,
                    fired,
                } => {
                    if pressed && press_start_ms.is_none() {
                        *press_start_ms = Some(now_ms);
                        *fired = false;
                    } else if !pressed {
                        if *fired {
                            events.push(ActionEvent {
                                action_name: binding.action_name.clone(),
                                phase: ActionEventPhase::Ended,
                                value: 0.0,
                                timestamp_ms: now_ms,
                            });
                        }
                        *press_start_ms = None;
                        *fired = false;
                    }
                }
                GestureTracker::DoubleTap {
                    max_interval_ms,
                    state,
                } => match state {
                    DoubleTapState::Idle => {
                        if pressed {
                            *state = DoubleTapState::FirstPressed { _start_ms: now_ms };
                        }
                    }
                    DoubleTapState::FirstPressed { .. } => {
                        if !pressed {
                            *state = DoubleTapState::WaitingSecond { release_ms: now_ms };
                        }
                    }
                    DoubleTapState::WaitingSecond { release_ms } => {
                        if pressed {
                            if now_ms - *release_ms <= *max_interval_ms as u64 {
                                events.push(ActionEvent {
                                    action_name: binding.action_name.clone(),
                                    phase: ActionEventPhase::Started,
                                    value: 1.0,
                                    timestamp_ms: now_ms,
                                });
                            }
                            *state = DoubleTapState::Idle;
                        }
                    }
                },
            }
        }

        events
    }

    /// Tick the detector to check for time-based gesture completions (e.g., hold).
    /// Call this every frame even if no input changed.
    pub fn tick(&mut self, now_ms: u64) -> Vec<ActionEvent> {
        let mut events = Vec::new();

        for binding in self.tracked.iter_mut() {
            match &mut binding.tracker {
                GestureTracker::Hold {
                    min_duration_ms,
                    press_start_ms: Some(start),
                    fired,
                } => {
                    if !*fired && now_ms - *start >= *min_duration_ms as u64 {
                        *fired = true;
                        events.push(ActionEvent {
                            action_name: binding.action_name.clone(),
                            phase: ActionEventPhase::Started,
                            value: 1.0,
                            timestamp_ms: now_ms,
                        });
                    }
                }
                GestureTracker::Hold { .. } => {}
                GestureTracker::DoubleTap {
                    max_interval_ms,
                    state,
                } => {
                    if let DoubleTapState::WaitingSecond { release_ms } = state {
                        if now_ms - *release_ms > *max_interval_ms as u64 {
                            *state = DoubleTapState::Idle;
                        }
                    }
                }
                _ => {}
            }
        }

        events
    }

    /// Check if all sources in a combo are currently pressed.
    pub fn check_combo(&self, sources: &[InputSource]) -> bool {
        sources
            .iter()
            .all(|s| self.source_state.get(s).copied().unwrap_or(false))
    }
}

impl Default for GestureDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::desktop::KeyCode;

    fn key(k: KeyCode) -> InputSource {
        InputSource::Keyboard(k)
    }

    #[test]
    fn press_fires_on_key_down() {
        let mut det = GestureDetector::new();
        det.register("jump", key(KeyCode::Space), &InputGesture::Press);

        let events = det.update(&key(KeyCode::Space), true, 100);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].action_name, "jump");
        assert_eq!(events[0].phase, ActionEventPhase::Started);
    }

    #[test]
    fn press_fires_ended_on_key_up() {
        let mut det = GestureDetector::new();
        det.register("jump", key(KeyCode::Space), &InputGesture::Press);

        det.update(&key(KeyCode::Space), true, 100);
        let events = det.update(&key(KeyCode::Space), false, 200);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].phase, ActionEventPhase::Ended);
    }

    #[test]
    fn press_does_not_repeat_while_held() {
        let mut det = GestureDetector::new();
        det.register("jump", key(KeyCode::Space), &InputGesture::Press);

        let e1 = det.update(&key(KeyCode::Space), true, 100);
        assert_eq!(e1.len(), 1);

        // Same state, no new event
        let e2 = det.update(&key(KeyCode::Space), true, 200);
        assert!(e2.is_empty());
    }

    #[test]
    fn release_fires_on_key_up_after_press() {
        let mut det = GestureDetector::new();
        det.register("drop", key(KeyCode::E), &InputGesture::Release);

        // Press - no event
        let e1 = det.update(&key(KeyCode::E), true, 100);
        assert!(e1.is_empty());

        // Release - fires
        let e2 = det.update(&key(KeyCode::E), false, 200);
        assert_eq!(e2.len(), 1);
        assert_eq!(e2[0].action_name, "drop");
    }

    #[test]
    fn release_without_prior_press_does_not_fire() {
        let mut det = GestureDetector::new();
        det.register("drop", key(KeyCode::E), &InputGesture::Release);

        let events = det.update(&key(KeyCode::E), false, 100);
        assert!(events.is_empty());
    }

    #[test]
    fn hold_fires_after_duration() {
        let mut det = GestureDetector::new();
        det.register(
            "charge",
            key(KeyCode::Space),
            &InputGesture::Hold {
                min_duration_ms: 500,
            },
        );

        // Press
        let e1 = det.update(&key(KeyCode::Space), true, 100);
        assert!(e1.is_empty());

        // Tick before duration
        let e2 = det.tick(400);
        assert!(e2.is_empty());

        // Tick after duration
        let e3 = det.tick(600);
        assert_eq!(e3.len(), 1);
        assert_eq!(e3[0].action_name, "charge");
        assert_eq!(e3[0].phase, ActionEventPhase::Started);
    }

    #[test]
    fn hold_cancels_on_early_release() {
        let mut det = GestureDetector::new();
        det.register(
            "charge",
            key(KeyCode::Space),
            &InputGesture::Hold {
                min_duration_ms: 500,
            },
        );

        det.update(&key(KeyCode::Space), true, 100);
        // Release before hold duration
        let events = det.update(&key(KeyCode::Space), false, 300);
        // No hold event should have fired
        assert!(events.is_empty());

        // Tick should also produce nothing
        let tick_events = det.tick(700);
        assert!(tick_events.is_empty());
    }

    #[test]
    fn hold_fires_ended_on_release_after_hold() {
        let mut det = GestureDetector::new();
        det.register(
            "charge",
            key(KeyCode::Space),
            &InputGesture::Hold {
                min_duration_ms: 200,
            },
        );

        det.update(&key(KeyCode::Space), true, 100);
        det.tick(300); // Hold fires

        let events = det.update(&key(KeyCode::Space), false, 400);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].phase, ActionEventPhase::Ended);
    }

    #[test]
    fn double_tap_fires_within_interval() {
        let mut det = GestureDetector::new();
        det.register(
            "dash",
            key(KeyCode::W),
            &InputGesture::DoubleTap {
                max_interval_ms: 300,
            },
        );

        // First tap: press + release
        det.update(&key(KeyCode::W), true, 100);
        det.update(&key(KeyCode::W), false, 150);

        // Second tap within interval
        let events = det.update(&key(KeyCode::W), true, 250);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].action_name, "dash");
    }

    #[test]
    fn double_tap_times_out() {
        let mut det = GestureDetector::new();
        det.register(
            "dash",
            key(KeyCode::W),
            &InputGesture::DoubleTap {
                max_interval_ms: 300,
            },
        );

        // First tap
        det.update(&key(KeyCode::W), true, 100);
        det.update(&key(KeyCode::W), false, 150);

        // Tick past timeout
        det.tick(500);

        // Second tap after timeout - should NOT fire double-tap
        let events = det.update(&key(KeyCode::W), true, 600);
        assert!(events.is_empty());
    }

    #[test]
    fn combo_check_all_pressed() {
        let mut det = GestureDetector::new();
        det.update(&key(KeyCode::Ctrl), true, 100);
        det.update(&key(KeyCode::Shift), true, 100);

        assert!(det.check_combo(&[key(KeyCode::Ctrl), key(KeyCode::Shift)]));
    }

    #[test]
    fn combo_check_partial_not_met() {
        let mut det = GestureDetector::new();
        det.update(&key(KeyCode::Ctrl), true, 100);

        assert!(!det.check_combo(&[key(KeyCode::Ctrl), key(KeyCode::Shift)]));
    }

    #[test]
    fn multiple_gestures_same_source() {
        let mut det = GestureDetector::new();
        det.register("jump", key(KeyCode::Space), &InputGesture::Press);
        det.register(
            "charge_jump",
            key(KeyCode::Space),
            &InputGesture::Hold {
                min_duration_ms: 500,
            },
        );

        // Press fires "jump" immediately
        let e1 = det.update(&key(KeyCode::Space), true, 100);
        assert_eq!(e1.len(), 1);
        assert_eq!(e1[0].action_name, "jump");

        // After hold duration, fires "charge_jump"
        let e2 = det.tick(600);
        assert_eq!(e2.len(), 1);
        assert_eq!(e2[0].action_name, "charge_jump");
    }

    #[test]
    fn unregistered_source_produces_no_events() {
        let mut det = GestureDetector::new();
        det.register("jump", key(KeyCode::Space), &InputGesture::Press);

        let events = det.update(&key(KeyCode::W), true, 100);
        assert!(events.is_empty());
    }

    #[test]
    fn hold_does_not_fire_twice() {
        let mut det = GestureDetector::new();
        det.register(
            "charge",
            key(KeyCode::Space),
            &InputGesture::Hold {
                min_duration_ms: 200,
            },
        );

        det.update(&key(KeyCode::Space), true, 100);
        let e1 = det.tick(300);
        assert_eq!(e1.len(), 1);

        // Second tick should not fire again
        let e2 = det.tick(400);
        assert!(e2.is_empty());
    }
}
