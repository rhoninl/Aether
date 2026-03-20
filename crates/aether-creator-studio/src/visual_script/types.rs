//! Core type system for visual scripting: data types and runtime values.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Data types that ports can carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DataType {
    /// Execution flow -- not data, just ordering.
    Flow,
    /// Boolean value.
    Bool,
    /// 32-bit signed integer.
    Int,
    /// 32-bit floating-point number.
    Float,
    /// UTF-8 string.
    String,
    /// 3D vector (x, y, z).
    Vec3,
    /// Entity reference (opaque id).
    Entity,
    /// Wildcard -- compatible with everything.
    Any,
}

impl DataType {
    /// Check whether a value of type `self` can be connected to a port of type `target`.
    ///
    /// Rules:
    /// - `Any` matches everything (either side).
    /// - `Flow` only connects to `Flow`.
    /// - Same type always matches.
    /// - Numeric coercion: Int -> Float is allowed.
    pub fn is_compatible_with(self, target: DataType) -> bool {
        if self == target {
            return true;
        }
        if self == DataType::Any || target == DataType::Any {
            return true;
        }
        // Int can be promoted to Float
        if self == DataType::Int && target == DataType::Float {
            return true;
        }
        false
    }

    /// Returns true if this is a data type (not Flow).
    pub fn is_data(self) -> bool {
        self != DataType::Flow
    }
}

impl fmt::Display for DataType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DataType::Flow => write!(f, "Flow"),
            DataType::Bool => write!(f, "Bool"),
            DataType::Int => write!(f, "Int"),
            DataType::Float => write!(f, "Float"),
            DataType::String => write!(f, "String"),
            DataType::Vec3 => write!(f, "Vec3"),
            DataType::Entity => write!(f, "Entity"),
            DataType::Any => write!(f, "Any"),
        }
    }
}

/// A runtime value in the visual script.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub enum Value {
    #[default]
    None,
    Bool(bool),
    Int(i32),
    Float(f32),
    String(String),
    Vec3 {
        x: f32,
        y: f32,
        z: f32,
    },
    Entity(u64),
}

impl Value {
    /// Return the DataType this value corresponds to.
    pub fn data_type(&self) -> DataType {
        match self {
            Value::None => DataType::Any,
            Value::Bool(_) => DataType::Bool,
            Value::Int(_) => DataType::Int,
            Value::Float(_) => DataType::Float,
            Value::String(_) => DataType::String,
            Value::Vec3 { .. } => DataType::Vec3,
            Value::Entity(_) => DataType::Entity,
        }
    }

    /// Try to coerce this value to the target data type.
    pub fn coerce_to(&self, target: DataType) -> Option<Value> {
        if self.data_type() == target {
            return Some(self.clone());
        }
        match (self, target) {
            (Value::Int(v), DataType::Float) => Some(Value::Float(*v as f32)),
            (_, DataType::Any) => Some(self.clone()),
            (Value::None, _) => Some(self.clone()),
            _ => None,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::None => write!(f, "None"),
            Value::Bool(v) => write!(f, "{v}"),
            Value::Int(v) => write!(f, "{v}"),
            Value::Float(v) => write!(f, "{v}"),
            Value::String(v) => write!(f, "\"{v}\""),
            Value::Vec3 { x, y, z } => write!(f, "({x}, {y}, {z})"),
            Value::Entity(id) => write!(f, "Entity({id})"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // DataType compatibility tests

    #[test]
    fn test_same_type_compatible() {
        assert!(DataType::Bool.is_compatible_with(DataType::Bool));
        assert!(DataType::Int.is_compatible_with(DataType::Int));
        assert!(DataType::Float.is_compatible_with(DataType::Float));
        assert!(DataType::String.is_compatible_with(DataType::String));
        assert!(DataType::Vec3.is_compatible_with(DataType::Vec3));
        assert!(DataType::Entity.is_compatible_with(DataType::Entity));
        assert!(DataType::Flow.is_compatible_with(DataType::Flow));
        assert!(DataType::Any.is_compatible_with(DataType::Any));
    }

    #[test]
    fn test_any_compatible_with_everything() {
        let types = [
            DataType::Flow,
            DataType::Bool,
            DataType::Int,
            DataType::Float,
            DataType::String,
            DataType::Vec3,
            DataType::Entity,
        ];
        for t in types {
            assert!(DataType::Any.is_compatible_with(t), "Any -> {t}");
            assert!(t.is_compatible_with(DataType::Any), "{t} -> Any");
        }
    }

    #[test]
    fn test_int_to_float_compatible() {
        assert!(DataType::Int.is_compatible_with(DataType::Float));
    }

    #[test]
    fn test_float_to_int_not_compatible() {
        assert!(!DataType::Float.is_compatible_with(DataType::Int));
    }

    #[test]
    fn test_flow_incompatible_with_data() {
        assert!(!DataType::Flow.is_compatible_with(DataType::Bool));
        assert!(!DataType::Flow.is_compatible_with(DataType::Int));
        assert!(!DataType::Bool.is_compatible_with(DataType::Flow));
    }

    #[test]
    fn test_string_incompatible_with_int() {
        assert!(!DataType::String.is_compatible_with(DataType::Int));
        assert!(!DataType::Int.is_compatible_with(DataType::String));
    }

    #[test]
    fn test_entity_incompatible_with_bool() {
        assert!(!DataType::Entity.is_compatible_with(DataType::Bool));
    }

    #[test]
    fn test_is_data() {
        assert!(!DataType::Flow.is_data());
        assert!(DataType::Bool.is_data());
        assert!(DataType::Int.is_data());
        assert!(DataType::Float.is_data());
        assert!(DataType::String.is_data());
        assert!(DataType::Vec3.is_data());
        assert!(DataType::Entity.is_data());
        assert!(DataType::Any.is_data());
    }

    #[test]
    fn test_data_type_display() {
        assert_eq!(format!("{}", DataType::Flow), "Flow");
        assert_eq!(format!("{}", DataType::Bool), "Bool");
        assert_eq!(format!("{}", DataType::Int), "Int");
        assert_eq!(format!("{}", DataType::Float), "Float");
        assert_eq!(format!("{}", DataType::String), "String");
        assert_eq!(format!("{}", DataType::Vec3), "Vec3");
        assert_eq!(format!("{}", DataType::Entity), "Entity");
        assert_eq!(format!("{}", DataType::Any), "Any");
    }

    // Value tests

    #[test]
    fn test_value_data_type() {
        assert_eq!(Value::None.data_type(), DataType::Any);
        assert_eq!(Value::Bool(true).data_type(), DataType::Bool);
        assert_eq!(Value::Int(42).data_type(), DataType::Int);
        assert_eq!(Value::Float(1.5).data_type(), DataType::Float);
        assert_eq!(Value::String("hi".into()).data_type(), DataType::String);
        assert_eq!(
            Value::Vec3 {
                x: 1.0,
                y: 2.0,
                z: 3.0
            }
            .data_type(),
            DataType::Vec3
        );
        assert_eq!(Value::Entity(1).data_type(), DataType::Entity);
    }

    #[test]
    fn test_coerce_int_to_float() {
        let v = Value::Int(5);
        let coerced = v.coerce_to(DataType::Float).unwrap();
        assert_eq!(coerced, Value::Float(5.0));
    }

    #[test]
    fn test_coerce_to_same_type() {
        let v = Value::Bool(true);
        let coerced = v.coerce_to(DataType::Bool).unwrap();
        assert_eq!(coerced, Value::Bool(true));
    }

    #[test]
    fn test_coerce_to_any() {
        let v = Value::Int(42);
        let coerced = v.coerce_to(DataType::Any).unwrap();
        assert_eq!(coerced, Value::Int(42));
    }

    #[test]
    fn test_coerce_none_to_anything() {
        let v = Value::None;
        assert!(v.coerce_to(DataType::Bool).is_some());
        assert!(v.coerce_to(DataType::Int).is_some());
    }

    #[test]
    fn test_coerce_incompatible_fails() {
        let v = Value::Float(1.5);
        assert!(v.coerce_to(DataType::Int).is_none());

        let v = Value::String("hi".into());
        assert!(v.coerce_to(DataType::Bool).is_none());
    }

    #[test]
    fn test_value_default() {
        assert_eq!(Value::default(), Value::None);
    }

    #[test]
    fn test_value_display() {
        assert_eq!(format!("{}", Value::None), "None");
        assert_eq!(format!("{}", Value::Bool(true)), "true");
        assert_eq!(format!("{}", Value::Int(42)), "42");
        assert_eq!(format!("{}", Value::Float(1.5)), "1.5");
        assert_eq!(format!("{}", Value::String("hi".into())), "\"hi\"");
        assert!(format!(
            "{}",
            Value::Vec3 {
                x: 1.0,
                y: 2.0,
                z: 3.0
            }
        )
        .contains("1"));
        assert_eq!(format!("{}", Value::Entity(99)), "Entity(99)");
    }

    // Serialization round-trip

    #[test]
    fn test_data_type_serde_round_trip() {
        let dt = DataType::Vec3;
        let json = serde_json::to_string(&dt).unwrap();
        let parsed: DataType = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, dt);
    }

    #[test]
    fn test_value_serde_round_trip() {
        let values = vec![
            Value::None,
            Value::Bool(false),
            Value::Int(-10),
            Value::Float(3.14),
            Value::String("test".into()),
            Value::Vec3 {
                x: 1.0,
                y: 2.0,
                z: 3.0,
            },
            Value::Entity(42),
        ];
        for v in &values {
            let json = serde_json::to_string(v).unwrap();
            let parsed: Value = serde_json::from_str(&json).unwrap();
            assert_eq!(&parsed, v, "round-trip failed for {v}");
        }
    }
}
