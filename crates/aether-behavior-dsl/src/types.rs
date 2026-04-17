//! Types of the Aether Behavior DSL.
//!
//! The DSL has a small closed type system: there are no user-defined types.

use serde::{Deserialize, Serialize};
use std::fmt;

/// The static type of a DSL expression or verb argument.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Type {
    Int,
    Float,
    Bool,
    String,
    EntityRef,
    Vec3,
    Timer,
    /// Homogeneous list of the given element type. Used for `List<DialogueOption>`
    /// via the "any" sentinel below.
    List(Box<Type>),
    /// String-keyed map from string to a payload type. Used for `Map<String,Any>`
    /// — the Any side is modelled as [`Type::Any`].
    Map(Box<Type>, Box<Type>),
    /// Any type — matches any other type. Used only for heterogeneous map
    /// payloads in the `trigger` verb.
    Any,
    /// Result sentinel for a behavior step.
    BehaviorStatus,
    /// Unit — the return type of a terminal action.
    Unit,
    /// Dialogue-option struct: `(label, id)`.
    DialogueOption,
    /// Return type of the `dialogue` verb.
    ChoiceId,
}

impl Type {
    /// Returns the canonical source-form name for this type.
    pub fn name(&self) -> String {
        match self {
            Type::Int => "Int".to_string(),
            Type::Float => "Float".to_string(),
            Type::Bool => "Bool".to_string(),
            Type::String => "String".to_string(),
            Type::EntityRef => "EntityRef".to_string(),
            Type::Vec3 => "Vec3".to_string(),
            Type::Timer => "Timer".to_string(),
            Type::List(inner) => format!("List<{}>", inner.name()),
            Type::Map(k, v) => format!("Map<{}, {}>", k.name(), v.name()),
            Type::Any => "Any".to_string(),
            Type::BehaviorStatus => "BehaviorStatus".to_string(),
            Type::Unit => "()".to_string(),
            Type::DialogueOption => "DialogueOption".to_string(),
            Type::ChoiceId => "ChoiceId".to_string(),
        }
    }

    /// Structural type equality with `Any` as a wildcard on either side.
    pub fn matches(&self, other: &Type) -> bool {
        match (self, other) {
            (Type::Any, _) | (_, Type::Any) => true,
            (Type::List(a), Type::List(b)) => a.matches(b),
            (Type::Map(ka, va), Type::Map(kb, vb)) => ka.matches(kb) && va.matches(vb),
            (a, b) => a == b,
        }
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name())
    }
}

/// Tick result of a behavior node, consumed by the host.
///
/// WASM encoding (returned from `tick`):
/// * `0` — Success
/// * `1` — Failure
/// * `2` — Running
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BehaviorStatus {
    Success,
    Failure,
    Running,
}

impl BehaviorStatus {
    /// Returns the canonical WASM i32 encoding.
    pub fn as_i32(self) -> i32 {
        match self {
            BehaviorStatus::Success => 0,
            BehaviorStatus::Failure => 1,
            BehaviorStatus::Running => 2,
        }
    }
}

/// Attempt to parse a type name (as it appears in source).
pub fn parse_type_name(name: &str) -> Option<Type> {
    match name {
        "Int" => Some(Type::Int),
        "Float" => Some(Type::Float),
        "Bool" => Some(Type::Bool),
        "String" => Some(Type::String),
        "EntityRef" => Some(Type::EntityRef),
        "Vec3" => Some(Type::Vec3),
        "Timer" => Some(Type::Timer),
        "BehaviorStatus" => Some(Type::BehaviorStatus),
        "DialogueOption" => Some(Type::DialogueOption),
        "ChoiceId" => Some(Type::ChoiceId),
        "Any" => Some(Type::Any),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn any_matches_everything() {
        assert!(Type::Any.matches(&Type::Int));
        assert!(Type::Int.matches(&Type::Any));
    }

    #[test]
    fn list_matches_are_structural() {
        let a = Type::List(Box::new(Type::Int));
        let b = Type::List(Box::new(Type::Int));
        assert!(a.matches(&b));
        let c = Type::List(Box::new(Type::Float));
        assert!(!a.matches(&c));
    }

    #[test]
    fn status_encoding_stable() {
        assert_eq!(BehaviorStatus::Success.as_i32(), 0);
        assert_eq!(BehaviorStatus::Failure.as_i32(), 1);
        assert_eq!(BehaviorStatus::Running.as_i32(), 2);
    }
}
