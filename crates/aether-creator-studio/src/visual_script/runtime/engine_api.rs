//! Engine API trait for dispatching script calls to the host engine.

use super::error::RuntimeError;
use crate::visual_script::types::Value;

/// Trait for engine-side function dispatch.
///
/// The VM calls this trait when it encounters a `Call` instruction for a function
/// that is not a built-in (like `clamp`, `lerp`, etc.). Implementations provide
/// the actual engine behavior (entity manipulation, physics, audio, etc.).
pub trait EngineApi {
    /// Execute a named function with the given arguments.
    ///
    /// Returns a result value (`Value::None` if the function has no return value).
    fn call(&mut self, function: &str, args: &[Value]) -> Result<Value, RuntimeError>;
}

/// A no-op engine API that returns `Value::None` for any call.
/// Useful for testing and sandboxed execution.
pub struct NoOpApi;

impl EngineApi for NoOpApi {
    fn call(&mut self, _function: &str, _args: &[Value]) -> Result<Value, RuntimeError> {
        Ok(Value::None)
    }
}

/// A recording engine API that captures all calls for testing.
#[derive(Debug, Default)]
pub struct RecordingApi {
    pub calls: Vec<(String, Vec<Value>)>,
}

impl EngineApi for RecordingApi {
    fn call(&mut self, function: &str, args: &[Value]) -> Result<Value, RuntimeError> {
        self.calls.push((function.to_string(), args.to_vec()));
        Ok(Value::None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_noop_api_returns_none() {
        let mut api = NoOpApi;
        let result = api.call("anything", &[Value::Int(1)]).unwrap();
        assert_eq!(result, Value::None);
    }

    #[test]
    fn test_noop_api_accepts_any_function() {
        let mut api = NoOpApi;
        assert!(api.call("set_position", &[]).is_ok());
        assert!(api
            .call("play_sound", &[Value::String("boom".into())])
            .is_ok());
        assert!(api.call("", &[]).is_ok());
    }

    #[test]
    fn test_recording_api_captures_calls() {
        let mut api = RecordingApi::default();
        api.call("log", &[Value::String("hello".into())]).unwrap();
        api.call(
            "set_position",
            &[
                Value::Entity(1),
                Value::Vec3 {
                    x: 1.0,
                    y: 2.0,
                    z: 3.0,
                },
            ],
        )
        .unwrap();

        assert_eq!(api.calls.len(), 2);
        assert_eq!(api.calls[0].0, "log");
        assert_eq!(api.calls[0].1, vec![Value::String("hello".into())]);
        assert_eq!(api.calls[1].0, "set_position");
        assert_eq!(api.calls[1].1.len(), 2);
    }

    #[test]
    fn test_recording_api_starts_empty() {
        let api = RecordingApi::default();
        assert!(api.calls.is_empty());
    }
}
