/// Structured error type for Lua script errors with correlation tracking.
#[derive(Debug)]
pub struct LuaScriptError {
    pub script_id: u64,
    pub script_name: String,
    pub error: String,
    pub stack_trace: Option<String>,
    pub correlation_id: u64,
}

/// Extracts a human-readable error message from a mlua::Error.
pub fn format_lua_error(err: &mlua::Error) -> String {
    match err {
        mlua::Error::RuntimeError(msg) => msg.clone(),
        mlua::Error::SyntaxError { message, .. } => message.clone(),
        mlua::Error::MemoryError(msg) => format!("memory error: {msg}"),
        mlua::Error::CallbackError { cause, .. } => format_lua_error(cause),
        other => format!("{other}"),
    }
}

/// Extracts a stack trace from a mlua::Error if available.
pub fn extract_stack_trace(err: &mlua::Error) -> Option<String> {
    match err {
        mlua::Error::CallbackError { traceback, .. } => Some(traceback.clone()),
        mlua::Error::RuntimeError(msg) => {
            // Runtime errors sometimes embed a traceback after a newline
            if msg.contains("\nstack traceback:") {
                let idx = msg.find("\nstack traceback:").unwrap();
                Some(msg[idx + 1..].to_string())
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Produces a formatted display string for a LuaScriptError.
pub fn format_error_display(err: &LuaScriptError) -> String {
    let mut out = format!(
        "Error in {} (script_id={}, correlation_id={}):\n  {}",
        err.script_name, err.script_id, err.correlation_id, err.error
    );
    if let Some(ref trace) = err.stack_trace {
        out.push_str(&format!("\n\n  Stack trace:\n    {}", trace.replace('\n', "\n    ")));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_lua_error_extracts_message_from_runtime_error() {
        let err = mlua::Error::RuntimeError("attempt to index a nil value".to_string());
        let msg = format_lua_error(&err);
        assert_eq!(msg, "attempt to index a nil value");
    }

    #[test]
    fn format_lua_error_extracts_syntax_error() {
        let err = mlua::Error::SyntaxError {
            message: "unexpected symbol near 'end'".to_string(),
            incomplete_input: false,
        };
        let msg = format_lua_error(&err);
        assert_eq!(msg, "unexpected symbol near 'end'");
    }

    #[test]
    fn extract_stack_trace_returns_none_for_non_script_errors() {
        let err = mlua::Error::RuntimeError("simple error".to_string());
        assert!(extract_stack_trace(&err).is_none());
    }

    #[test]
    fn extract_stack_trace_returns_some_for_callback_error() {
        let err = mlua::Error::CallbackError {
            traceback: "stack traceback:\n  [C]: in function 'error'".to_string(),
            cause: std::sync::Arc::new(mlua::Error::RuntimeError("inner".to_string())),
        };
        let trace = extract_stack_trace(&err);
        assert!(trace.is_some());
        assert!(trace.unwrap().contains("stack traceback:"));
    }

    #[test]
    fn format_error_display_produces_human_readable_output() {
        let err = LuaScriptError {
            script_id: 42,
            script_name: "npc_ai.lua".to_string(),
            error: "attempt to index a nil value".to_string(),
            stack_trace: Some("npc_ai.lua:15: in function 'on_tick'".to_string()),
            correlation_id: 98765,
        };
        let display = format_error_display(&err);
        assert!(display.contains("npc_ai.lua"));
        assert!(display.contains("script_id=42"));
        assert!(display.contains("correlation_id=98765"));
        assert!(display.contains("attempt to index a nil value"));
        assert!(display.contains("Stack trace:"));
        assert!(display.contains("on_tick"));
    }

    #[test]
    fn format_error_display_without_stack_trace() {
        let err = LuaScriptError {
            script_id: 1,
            script_name: "test.lua".to_string(),
            error: "some error".to_string(),
            stack_trace: None,
            correlation_id: 100,
        };
        let display = format_error_display(&err);
        assert!(display.contains("some error"));
        assert!(!display.contains("Stack trace:"));
    }
}
