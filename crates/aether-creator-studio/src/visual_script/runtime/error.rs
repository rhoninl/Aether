//! Error types for the visual script runtime.

use std::fmt;

/// Errors that can occur during script execution.
#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeError {
    /// Attempted to access a register beyond the allocated count.
    RegisterOutOfBounds { index: u32, count: u32 },
    /// Jump target label was not found in the instruction stream.
    LabelNotFound(u32),
    /// Execution exceeded the maximum allowed operations.
    ExecutionLimitExceeded { limit: u64 },
    /// Register count exceeds the configured maximum.
    StackOverflow { requested: u32, limit: u32 },
    /// Variable count exceeds the configured maximum.
    VariableLimitExceeded { limit: usize },
    /// Division by zero.
    DivisionByZero,
    /// Operand type mismatch for an operation.
    TypeError { expected: String, got: String },
    /// Call to an unknown function not handled by the VM or engine API.
    UnknownFunction(String),
    /// Error returned by the engine API.
    ApiError(String),
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuntimeError::RegisterOutOfBounds { index, count } => {
                write!(f, "register out of bounds: index {index}, count {count}")
            }
            RuntimeError::LabelNotFound(id) => {
                write!(f, "label not found: L{id}")
            }
            RuntimeError::ExecutionLimitExceeded { limit } => {
                write!(f, "execution limit exceeded: {limit} ops")
            }
            RuntimeError::StackOverflow { requested, limit } => {
                write!(
                    f,
                    "stack overflow: requested {requested} registers, limit {limit}"
                )
            }
            RuntimeError::VariableLimitExceeded { limit } => {
                write!(f, "variable limit exceeded: max {limit}")
            }
            RuntimeError::DivisionByZero => write!(f, "division by zero"),
            RuntimeError::TypeError { expected, got } => {
                write!(f, "type error: expected {expected}, got {got}")
            }
            RuntimeError::UnknownFunction(name) => {
                write!(f, "unknown function: {name}")
            }
            RuntimeError::ApiError(msg) => {
                write!(f, "engine API error: {msg}")
            }
        }
    }
}

impl std::error::Error for RuntimeError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_out_of_bounds_display() {
        let e = RuntimeError::RegisterOutOfBounds { index: 5, count: 3 };
        let s = format!("{e}");
        assert!(s.contains("5"));
        assert!(s.contains("3"));
    }

    #[test]
    fn test_label_not_found_display() {
        let e = RuntimeError::LabelNotFound(42);
        assert!(format!("{e}").contains("42"));
    }

    #[test]
    fn test_execution_limit_display() {
        let e = RuntimeError::ExecutionLimitExceeded { limit: 10000 };
        assert!(format!("{e}").contains("10000"));
    }

    #[test]
    fn test_stack_overflow_display() {
        let e = RuntimeError::StackOverflow {
            requested: 512,
            limit: 256,
        };
        let s = format!("{e}");
        assert!(s.contains("512"));
        assert!(s.contains("256"));
    }

    #[test]
    fn test_variable_limit_display() {
        let e = RuntimeError::VariableLimitExceeded { limit: 1024 };
        assert!(format!("{e}").contains("1024"));
    }

    #[test]
    fn test_division_by_zero_display() {
        let e = RuntimeError::DivisionByZero;
        assert!(format!("{e}").contains("division by zero"));
    }

    #[test]
    fn test_type_error_display() {
        let e = RuntimeError::TypeError {
            expected: "Float".into(),
            got: "String".into(),
        };
        let s = format!("{e}");
        assert!(s.contains("Float"));
        assert!(s.contains("String"));
    }

    #[test]
    fn test_unknown_function_display() {
        let e = RuntimeError::UnknownFunction("foo".into());
        assert!(format!("{e}").contains("foo"));
    }

    #[test]
    fn test_api_error_display() {
        let e = RuntimeError::ApiError("connection lost".into());
        assert!(format!("{e}").contains("connection lost"));
    }

    #[test]
    fn test_error_equality() {
        assert_eq!(RuntimeError::DivisionByZero, RuntimeError::DivisionByZero);
        assert_ne!(
            RuntimeError::LabelNotFound(1),
            RuntimeError::LabelNotFound(2)
        );
    }
}
