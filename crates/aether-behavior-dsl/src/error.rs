//! Structured error codes for the Aether Behavior DSL.
//!
//! Error codes follow the `BDSL-E####` scheme. Each error carries a source
//! span plus, where applicable, a human-readable suggested fix. The raw error
//! codes are also exposed via [`BehaviorDslError::code`] for tooling.

use thiserror::Error;

use crate::ast::Span;

/// Error code strings (authoritative, user-visible).
pub mod codes {
    pub const E0001_UNKNOWN_VERB: &str = "BDSL-E0001";
    pub const E0002_WRONG_ARG_COUNT: &str = "BDSL-E0002";
    pub const E0003_WRONG_ARG_TYPE: &str = "BDSL-E0003";
    pub const E0004_MISSING_CAPABILITY: &str = "BDSL-E0004";
    pub const E0005_EFFECT_MISMATCH_PARALLEL: &str = "BDSL-E0005";
    pub const E0006_UNRESOLVED_ENTITY_REF: &str = "BDSL-E0006";
    pub const E0007_UNKNOWN_TYPE: &str = "BDSL-E0007";
    pub const E0008_DUPLICATE_BINDING: &str = "BDSL-E0008";
    pub const E0009_UNUSED_BINDING: &str = "BDSL-E0009";
    pub const E0010_RESERVED_KEYWORD: &str = "BDSL-E0010";
    pub const E0011_UNEXPECTED_TOKEN: &str = "BDSL-E0011";
    pub const E0012_UNEXPECTED_EOF: &str = "BDSL-E0012";
    pub const E0013_INVALID_NUMBER_LITERAL: &str = "BDSL-E0013";
    pub const E0014_INVALID_STRING_LITERAL: &str = "BDSL-E0014";
    pub const E0015_MISSING_MODULE_HEADER: &str = "BDSL-E0015";
    pub const E0016_MISSING_VERSION: &str = "BDSL-E0016";
    pub const E0017_UNKNOWN_CAPABILITY: &str = "BDSL-E0017";
    pub const E0018_UNKNOWN_COMBINATOR: &str = "BDSL-E0018";
    pub const E0019_EMPTY_BODY: &str = "BDSL-E0019";
    pub const E0020_RETRY_NONPOSITIVE: &str = "BDSL-E0020";
}

/// Canonical error type emitted by the parser and type checker.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum BehaviorDslError {
    #[error("{code}: unknown verb `{name}` at {span} — did you mean one of the 5 MVP verbs (spawn, move, damage, trigger, dialogue)?", code = codes::E0001_UNKNOWN_VERB)]
    UnknownVerb { name: String, span: Span },

    #[error("{code}: verb `{verb}` expects {expected} arg(s), got {actual} at {span}", code = codes::E0002_WRONG_ARG_COUNT)]
    WrongArgCount {
        verb: String,
        expected: usize,
        actual: usize,
        span: Span,
    },

    #[error("{code}: arg #{index} of `{verb}` expects `{expected}`, got `{actual}` at {span}", code = codes::E0003_WRONG_ARG_TYPE)]
    WrongArgType {
        verb: String,
        index: usize,
        expected: String,
        actual: String,
        span: Span,
    },

    #[error("{code}: verb `{verb}` requires capability `{capability}` — add `@caps({capability})` to the module header at {span}", code = codes::E0004_MISSING_CAPABILITY)]
    MissingCapability {
        verb: String,
        capability: String,
        span: Span,
    },

    #[error("{code}: parallel combinator children have conflicting effects {left} vs {right} at {span}", code = codes::E0005_EFFECT_MISMATCH_PARALLEL)]
    EffectMismatchParallel {
        left: String,
        right: String,
        span: Span,
    },

    #[error("{code}: unresolved entity reference `{name}` at {span}", code = codes::E0006_UNRESOLVED_ENTITY_REF)]
    UnresolvedEntityRef { name: String, span: Span },

    #[error("{code}: unknown type `{name}` at {span}", code = codes::E0007_UNKNOWN_TYPE)]
    UnknownType { name: String, span: Span },

    #[error("{code}: duplicate binding `{name}` at {span} — first declared at {first}", code = codes::E0008_DUPLICATE_BINDING)]
    DuplicateBinding {
        name: String,
        span: Span,
        first: Span,
    },

    #[error("{code}: unused binding `{name}` at {span}", code = codes::E0009_UNUSED_BINDING)]
    UnusedBinding { name: String, span: Span },

    #[error("{code}: `{name}` is a reserved keyword at {span}", code = codes::E0010_RESERVED_KEYWORD)]
    ReservedKeyword { name: String, span: Span },

    #[error("{code}: unexpected token `{found}` at {span} — expected {expected}", code = codes::E0011_UNEXPECTED_TOKEN)]
    UnexpectedToken {
        found: String,
        expected: String,
        span: Span,
    },

    #[error("{code}: unexpected end of file — expected {expected}", code = codes::E0012_UNEXPECTED_EOF)]
    UnexpectedEof { expected: String },

    #[error("{code}: invalid number literal `{literal}` at {span}", code = codes::E0013_INVALID_NUMBER_LITERAL)]
    InvalidNumberLiteral { literal: String, span: Span },

    #[error("{code}: invalid string literal at {span}: {reason}", code = codes::E0014_INVALID_STRING_LITERAL)]
    InvalidStringLiteral { reason: String, span: Span },

    #[error("{code}: missing module header — behaviors must start with `behavior <name> {{ ... }}`", code = codes::E0015_MISSING_MODULE_HEADER)]
    MissingModuleHeader,

    #[error("{code}: missing `version <n>` at {span}", code = codes::E0016_MISSING_VERSION)]
    MissingVersion { span: Span },

    #[error("{code}: unknown capability `{name}` at {span} — valid: Network, Persistence, Economy, Movement, Combat", code = codes::E0017_UNKNOWN_CAPABILITY)]
    UnknownCapability { name: String, span: Span },

    #[error("{code}: unknown combinator `{name}` at {span} — valid: sequence, selector, parallel, invert, retry, timeout", code = codes::E0018_UNKNOWN_COMBINATOR)]
    UnknownCombinator { name: String, span: Span },

    #[error("{code}: combinator body is empty at {span}", code = codes::E0019_EMPTY_BODY)]
    EmptyBody { span: Span },

    #[error("{code}: retry count must be positive, got {value} at {span}", code = codes::E0020_RETRY_NONPOSITIVE)]
    RetryNonPositive { value: i64, span: Span },
}

impl BehaviorDslError {
    /// Returns the canonical error-code string (`BDSL-E####`).
    pub fn code(&self) -> &'static str {
        match self {
            BehaviorDslError::UnknownVerb { .. } => codes::E0001_UNKNOWN_VERB,
            BehaviorDslError::WrongArgCount { .. } => codes::E0002_WRONG_ARG_COUNT,
            BehaviorDslError::WrongArgType { .. } => codes::E0003_WRONG_ARG_TYPE,
            BehaviorDslError::MissingCapability { .. } => codes::E0004_MISSING_CAPABILITY,
            BehaviorDslError::EffectMismatchParallel { .. } => {
                codes::E0005_EFFECT_MISMATCH_PARALLEL
            }
            BehaviorDslError::UnresolvedEntityRef { .. } => codes::E0006_UNRESOLVED_ENTITY_REF,
            BehaviorDslError::UnknownType { .. } => codes::E0007_UNKNOWN_TYPE,
            BehaviorDslError::DuplicateBinding { .. } => codes::E0008_DUPLICATE_BINDING,
            BehaviorDslError::UnusedBinding { .. } => codes::E0009_UNUSED_BINDING,
            BehaviorDslError::ReservedKeyword { .. } => codes::E0010_RESERVED_KEYWORD,
            BehaviorDslError::UnexpectedToken { .. } => codes::E0011_UNEXPECTED_TOKEN,
            BehaviorDslError::UnexpectedEof { .. } => codes::E0012_UNEXPECTED_EOF,
            BehaviorDslError::InvalidNumberLiteral { .. } => codes::E0013_INVALID_NUMBER_LITERAL,
            BehaviorDslError::InvalidStringLiteral { .. } => codes::E0014_INVALID_STRING_LITERAL,
            BehaviorDslError::MissingModuleHeader => codes::E0015_MISSING_MODULE_HEADER,
            BehaviorDslError::MissingVersion { .. } => codes::E0016_MISSING_VERSION,
            BehaviorDslError::UnknownCapability { .. } => codes::E0017_UNKNOWN_CAPABILITY,
            BehaviorDslError::UnknownCombinator { .. } => codes::E0018_UNKNOWN_COMBINATOR,
            BehaviorDslError::EmptyBody { .. } => codes::E0019_EMPTY_BODY,
            BehaviorDslError::RetryNonPositive { .. } => codes::E0020_RETRY_NONPOSITIVE,
        }
    }

    /// Returns the source span if the error is span-attached.
    pub fn span(&self) -> Option<Span> {
        match self {
            BehaviorDslError::UnknownVerb { span, .. }
            | BehaviorDslError::WrongArgCount { span, .. }
            | BehaviorDslError::WrongArgType { span, .. }
            | BehaviorDslError::MissingCapability { span, .. }
            | BehaviorDslError::EffectMismatchParallel { span, .. }
            | BehaviorDslError::UnresolvedEntityRef { span, .. }
            | BehaviorDslError::UnknownType { span, .. }
            | BehaviorDslError::DuplicateBinding { span, .. }
            | BehaviorDslError::UnusedBinding { span, .. }
            | BehaviorDslError::ReservedKeyword { span, .. }
            | BehaviorDslError::UnexpectedToken { span, .. }
            | BehaviorDslError::InvalidNumberLiteral { span, .. }
            | BehaviorDslError::InvalidStringLiteral { span, .. }
            | BehaviorDslError::MissingVersion { span }
            | BehaviorDslError::UnknownCapability { span, .. }
            | BehaviorDslError::UnknownCombinator { span, .. }
            | BehaviorDslError::EmptyBody { span }
            | BehaviorDslError::RetryNonPositive { span, .. } => Some(*span),
            BehaviorDslError::UnexpectedEof { .. } | BehaviorDslError::MissingModuleHeader => None,
        }
    }
}

/// Convenience alias for crate-level results.
pub type BehaviorDslResult<T> = Result<T, BehaviorDslError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_codes_are_distinct() {
        let all = [
            codes::E0001_UNKNOWN_VERB,
            codes::E0002_WRONG_ARG_COUNT,
            codes::E0003_WRONG_ARG_TYPE,
            codes::E0004_MISSING_CAPABILITY,
            codes::E0005_EFFECT_MISMATCH_PARALLEL,
            codes::E0006_UNRESOLVED_ENTITY_REF,
            codes::E0007_UNKNOWN_TYPE,
            codes::E0008_DUPLICATE_BINDING,
            codes::E0009_UNUSED_BINDING,
            codes::E0010_RESERVED_KEYWORD,
            codes::E0011_UNEXPECTED_TOKEN,
            codes::E0012_UNEXPECTED_EOF,
            codes::E0013_INVALID_NUMBER_LITERAL,
            codes::E0014_INVALID_STRING_LITERAL,
            codes::E0015_MISSING_MODULE_HEADER,
            codes::E0016_MISSING_VERSION,
            codes::E0017_UNKNOWN_CAPABILITY,
            codes::E0018_UNKNOWN_COMBINATOR,
            codes::E0019_EMPTY_BODY,
            codes::E0020_RETRY_NONPOSITIVE,
        ];
        let mut sorted: Vec<_> = all.iter().copied().collect();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), 20, "error codes must be unique");
    }
}
