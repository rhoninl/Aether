//! Abstract syntax tree for the Aether Behavior DSL.
//!
//! Every syntactic node carries a [`Span`] pointing back to its byte range in
//! the source so downstream tooling can emit precise diagnostics.

use serde::{Deserialize, Serialize};
use std::fmt;

use crate::caps::CapabilitySet;

/// Byte-range source span: `[start, end)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub const DUMMY: Span = Span { start: 0, end: 0 };

    pub fn new(start: usize, end: usize) -> Self {
        Span { start, end }
    }

    /// Combine two spans into the minimal span covering both.
    pub fn join(self, other: Span) -> Span {
        Span {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "@{}..{}", self.start, self.end)
    }
}

/// A literal value appearing in source.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Literal {
    Int(i64),
    Float(f64),
    Bool(bool),
    String(String),
    /// `vec3(x, y, z)`.
    Vec3(Box<Expr>, Box<Expr>, Box<Expr>),
    /// `[e1, e2, ...]`.
    List(Vec<Expr>),
    /// `{ "key": value, ... }` — string-keyed.
    Map(Vec<(String, Expr)>),
}

/// DialogueOption literal: `option("label", "id")`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DialogueOption {
    pub label: Box<Expr>,
    pub id: Box<Expr>,
    pub span: Span,
}

/// A DSL expression.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ExprKind {
    /// A literal value.
    Literal(Literal),
    /// An identifier reference (variable, binding).
    Ident(String),
    /// Constructed DialogueOption.
    DialogueOption(DialogueOption),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

impl Expr {
    pub fn literal(lit: Literal, span: Span) -> Self {
        Expr {
            kind: ExprKind::Literal(lit),
            span,
        }
    }

    pub fn ident(name: impl Into<String>, span: Span) -> Self {
        Expr {
            kind: ExprKind::Ident(name.into()),
            span,
        }
    }
}

/// Built-in verb (action node) in the DSL.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Verb {
    Spawn,
    Move,
    Damage,
    Trigger,
    Dialogue,
}

impl Verb {
    pub fn name(&self) -> &'static str {
        match self {
            Verb::Spawn => "spawn",
            Verb::Move => "move",
            Verb::Damage => "damage",
            Verb::Trigger => "trigger",
            Verb::Dialogue => "dialogue",
        }
    }

    pub fn from_name(name: &str) -> Option<Verb> {
        match name {
            "spawn" => Some(Verb::Spawn),
            "move" => Some(Verb::Move),
            "damage" => Some(Verb::Damage),
            "trigger" => Some(Verb::Trigger),
            "dialogue" => Some(Verb::Dialogue),
            _ => None,
        }
    }

    /// The ordered list of verbs, for code generation.
    pub fn all() -> &'static [Verb] {
        &[
            Verb::Spawn,
            Verb::Move,
            Verb::Damage,
            Verb::Trigger,
            Verb::Dialogue,
        ]
    }
}

/// Combinator nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Combinator {
    Sequence,
    Selector,
    Parallel,
    Invert,
    Retry,
    Timeout,
}

impl Combinator {
    pub fn name(&self) -> &'static str {
        match self {
            Combinator::Sequence => "sequence",
            Combinator::Selector => "selector",
            Combinator::Parallel => "parallel",
            Combinator::Invert => "invert",
            Combinator::Retry => "retry",
            Combinator::Timeout => "timeout",
        }
    }

    pub fn from_name(name: &str) -> Option<Combinator> {
        match name {
            "sequence" => Some(Combinator::Sequence),
            "selector" => Some(Combinator::Selector),
            "parallel" => Some(Combinator::Parallel),
            "invert" => Some(Combinator::Invert),
            "retry" => Some(Combinator::Retry),
            "timeout" => Some(Combinator::Timeout),
            _ => None,
        }
    }
}

/// A single behavior-tree node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum NodeKind {
    /// A verb call.
    Verb { verb: Verb, args: Vec<Expr> },
    /// A combinator with children + optional integer parameter.
    Combinator {
        combinator: Combinator,
        /// For `retry(n)` / `timeout(ms)`: the numeric parameter.
        parameter: Option<i64>,
        children: Vec<Node>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub kind: NodeKind,
    pub span: Span,
}

/// Built-in entity identifiers that bind implicitly in every behavior.
///
/// Returns `Some(handle)` — a stable i32 handle understood by the runtime —
/// if `name` is a well-known entity reference, otherwise `None`.
pub fn builtin_entity_handle(name: &str) -> Option<i32> {
    match name {
        "self" => Some(-1),
        "player" => Some(-2),
        "target" => Some(-3),
        "nearest_player" => Some(-4),
        _ => None,
    }
}

/// Top-level module: `behavior <name> { @caps(...) version <n> <body> }`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Module {
    pub name: String,
    pub caps: CapabilitySet,
    pub version: u32,
    pub root: Node,
    pub span: Span,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn span_join_is_monotonic() {
        let a = Span::new(3, 7);
        let b = Span::new(5, 11);
        assert_eq!(a.join(b), Span::new(3, 11));
        assert_eq!(b.join(a), Span::new(3, 11));
    }

    #[test]
    fn verb_round_trip() {
        for v in Verb::all() {
            assert_eq!(Verb::from_name(v.name()), Some(*v));
        }
    }

    #[test]
    fn combinator_round_trip() {
        for name in ["sequence", "selector", "parallel", "invert", "retry", "timeout"] {
            let c = Combinator::from_name(name).unwrap();
            assert_eq!(c.name(), name);
        }
    }
}
