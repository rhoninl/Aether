//! Type checker for the Behavior DSL.
//!
//! Walks a [`Module`] AST and validates:
//! * Verb arg counts + types.
//! * Effect composition through combinators.
//! * Capability-token presence for each effect used.
//! * Reserved-keyword misuse.
//! * Retry/timeout parameter sanity (the parser already guards `retry(0)`;
//!   the typechecker accepts positive values).
//!
//! Returns a [`CheckedModule`] carrying the module's computed effect set.

use crate::ast::*;
use crate::caps::Capability;
use crate::effects::{Effect, EffectSet};
use crate::error::{BehaviorDslError, BehaviorDslResult};
use crate::types::Type;

/// Signature of a verb: argument types, effect, return type.
#[derive(Debug, Clone)]
pub struct VerbSignature {
    pub verb: Verb,
    pub args: Vec<Type>,
    pub effect: Effect,
    pub ret: Type,
}

impl VerbSignature {
    /// Authoritative signatures for the 5 MVP verbs.
    pub fn all() -> [VerbSignature; 5] {
        [
            VerbSignature {
                verb: Verb::Spawn,
                args: vec![Type::String, Type::Vec3],
                effect: Effect::Pure,
                ret: Type::EntityRef,
            },
            VerbSignature {
                verb: Verb::Move,
                args: vec![Type::EntityRef, Type::Vec3, Type::Float],
                effect: Effect::Movement,
                ret: Type::BehaviorStatus,
            },
            VerbSignature {
                verb: Verb::Damage,
                args: vec![Type::EntityRef, Type::Int],
                effect: Effect::Combat,
                ret: Type::BehaviorStatus,
            },
            VerbSignature {
                verb: Verb::Trigger,
                args: vec![
                    Type::String,
                    Type::Map(Box::new(Type::String), Box::new(Type::Any)),
                ],
                effect: Effect::Network,
                ret: Type::Unit,
            },
            VerbSignature {
                verb: Verb::Dialogue,
                args: vec![
                    Type::EntityRef,
                    Type::String,
                    Type::List(Box::new(Type::DialogueOption)),
                ],
                effect: Effect::Pure,
                ret: Type::ChoiceId,
            },
        ]
    }

    pub fn for_verb(verb: Verb) -> VerbSignature {
        VerbSignature::all()
            .into_iter()
            .find(|s| s.verb == verb)
            .expect("all 5 verbs have signatures")
    }
}

/// Module + computed effect set after type check.
#[derive(Debug, Clone)]
pub struct CheckedModule {
    pub module: Module,
    pub effects: EffectSet,
    /// Soft warnings (e.g. unused binding, if ever supported).
    pub warnings: Vec<BehaviorDslError>,
}

/// Run the type checker on `module`.
pub fn check(module: Module) -> BehaviorDslResult<CheckedModule> {
    let mut checker = Checker { warnings: vec![] };
    let effects = checker.check_node(&module.root)?;
    for eff in effects.iter() {
        if let Some(required) = Capability::required_for(eff) {
            if !module.caps.contains(required) {
                return Err(BehaviorDslError::MissingCapability {
                    verb: format!("effect `{}`", eff),
                    capability: required.name().to_string(),
                    span: module.root.span,
                });
            }
        }
    }
    Ok(CheckedModule {
        module,
        effects,
        warnings: checker.warnings,
    })
}

struct Checker {
    warnings: Vec<BehaviorDslError>,
}

impl Checker {
    fn check_node(&mut self, node: &Node) -> BehaviorDslResult<EffectSet> {
        match &node.kind {
            NodeKind::Verb { verb, args } => self.check_verb(*verb, args, node.span),
            NodeKind::Combinator {
                combinator,
                parameter,
                children,
            } => self.check_combinator(*combinator, *parameter, children, node.span),
        }
    }

    fn check_verb(
        &mut self,
        verb: Verb,
        args: &[Expr],
        span: Span,
    ) -> BehaviorDslResult<EffectSet> {
        let sig = VerbSignature::for_verb(verb);
        if args.len() != sig.args.len() {
            return Err(BehaviorDslError::WrongArgCount {
                verb: verb.name().to_string(),
                expected: sig.args.len(),
                actual: args.len(),
                span,
            });
        }
        for (idx, (arg, expected)) in args.iter().zip(sig.args.iter()).enumerate() {
            let actual = self.infer_expr(arg)?;
            if !actual.matches(expected) {
                return Err(BehaviorDslError::WrongArgType {
                    verb: verb.name().to_string(),
                    index: idx,
                    expected: expected.name(),
                    actual: actual.name(),
                    span: arg.span,
                });
            }
        }
        Ok(EffectSet::single(sig.effect))
    }

    fn check_combinator(
        &mut self,
        combinator: Combinator,
        parameter: Option<i64>,
        children: &[Node],
        span: Span,
    ) -> BehaviorDslResult<EffectSet> {
        if children.is_empty() {
            return Err(BehaviorDslError::EmptyBody { span });
        }
        if matches!(combinator, Combinator::Invert) && children.len() != 1 {
            return Err(BehaviorDslError::WrongArgCount {
                verb: "invert".to_string(),
                expected: 1,
                actual: children.len(),
                span,
            });
        }
        if matches!(combinator, Combinator::Retry) {
            match parameter {
                Some(n) if n > 0 => {}
                Some(n) => return Err(BehaviorDslError::RetryNonPositive { value: n, span }),
                None => {
                    return Err(BehaviorDslError::WrongArgCount {
                        verb: "retry".to_string(),
                        expected: 1,
                        actual: 0,
                        span,
                    });
                }
            }
        }
        if matches!(combinator, Combinator::Timeout) && parameter.is_none() {
            return Err(BehaviorDslError::WrongArgCount {
                verb: "timeout".to_string(),
                expected: 1,
                actual: 0,
                span,
            });
        }

        // Compute effect set.
        let mut effects = EffectSet::pure();
        let mut child_effects = Vec::with_capacity(children.len());
        for child in children {
            let e = self.check_node(child)?;
            child_effects.push(e.clone());
            effects = effects.union(&e);
        }

        // Parallel siblings must not conflict on write-heavy effects. We
        // flag a pair that produces both Combat and Persistence in the same
        // parallel block — these are the two mutate-world effects that should
        // not race by default.
        if matches!(combinator, Combinator::Parallel) {
            for i in 0..child_effects.len() {
                for j in (i + 1)..child_effects.len() {
                    let a = &child_effects[i];
                    let b = &child_effects[j];
                    if (a.contains(Effect::Combat) && b.contains(Effect::Persistence))
                        || (a.contains(Effect::Persistence) && b.contains(Effect::Combat))
                    {
                        return Err(BehaviorDslError::EffectMismatchParallel {
                            left: a.to_string(),
                            right: b.to_string(),
                            span,
                        });
                    }
                }
            }
        }

        Ok(effects)
    }

    fn infer_expr(&mut self, expr: &Expr) -> BehaviorDslResult<Type> {
        match &expr.kind {
            ExprKind::Literal(lit) => self.infer_literal(lit, expr.span),
            ExprKind::Ident(name) => {
                if crate::ast::builtin_entity_handle(name).is_some() {
                    return Ok(Type::EntityRef);
                }
                if crate::parser::RESERVED.contains(&name.as_str()) {
                    return Err(BehaviorDslError::ReservedKeyword {
                        name: name.clone(),
                        span: expr.span,
                    });
                }
                // Unresolved — we have no binding form in MVP, so anything
                // else is an unresolved reference.
                Err(BehaviorDslError::UnresolvedEntityRef {
                    name: name.clone(),
                    span: expr.span,
                })
            }
            ExprKind::DialogueOption(opt) => {
                let t_label = self.infer_expr(&opt.label)?;
                let t_id = self.infer_expr(&opt.id)?;
                if !t_label.matches(&Type::String) {
                    return Err(BehaviorDslError::WrongArgType {
                        verb: "option".to_string(),
                        index: 0,
                        expected: "String".to_string(),
                        actual: t_label.name(),
                        span: opt.label.span,
                    });
                }
                if !t_id.matches(&Type::String) {
                    return Err(BehaviorDslError::WrongArgType {
                        verb: "option".to_string(),
                        index: 1,
                        expected: "String".to_string(),
                        actual: t_id.name(),
                        span: opt.id.span,
                    });
                }
                Ok(Type::DialogueOption)
            }
        }
    }

    fn infer_literal(&mut self, lit: &Literal, span: Span) -> BehaviorDslResult<Type> {
        match lit {
            Literal::Int(_) => Ok(Type::Int),
            Literal::Float(_) => Ok(Type::Float),
            Literal::Bool(_) => Ok(Type::Bool),
            Literal::String(_) => Ok(Type::String),
            Literal::Vec3(x, y, z) => {
                for (i, component) in [x, y, z].iter().enumerate() {
                    let t = self.infer_expr(component)?;
                    if !(t.matches(&Type::Float) || t.matches(&Type::Int)) {
                        return Err(BehaviorDslError::WrongArgType {
                            verb: "vec3".to_string(),
                            index: i,
                            expected: "Float".to_string(),
                            actual: t.name(),
                            span: component.span,
                        });
                    }
                }
                Ok(Type::Vec3)
            }
            Literal::List(items) => {
                if items.is_empty() {
                    // Empty list — default to List<Any>.
                    return Ok(Type::List(Box::new(Type::Any)));
                }
                let first = self.infer_expr(&items[0])?;
                for (i, item) in items.iter().enumerate().skip(1) {
                    let t = self.infer_expr(item)?;
                    if !t.matches(&first) {
                        return Err(BehaviorDslError::WrongArgType {
                            verb: "list".to_string(),
                            index: i,
                            expected: first.name(),
                            actual: t.name(),
                            span: item.span,
                        });
                    }
                }
                Ok(Type::List(Box::new(first)))
            }
            Literal::Map(entries) => {
                for (_, v) in entries {
                    self.infer_expr(v)?;
                }
                // Map literal type is Map<String, Any> by default.
                let _ = span;
                Ok(Type::Map(Box::new(Type::String), Box::new(Type::Any)))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;

    #[test]
    fn pure_module_checks_without_caps() {
        let src = "behavior Foo { version 1 trigger(\"hello\", {}); }";
        // trigger is Network-effect, so this must fail without @caps(Network).
        let m = parse(src).unwrap();
        let err = check(m).unwrap_err();
        assert_eq!(err.code(), "BDSL-E0004");
    }

    #[test]
    fn trigger_with_caps_passes() {
        let src =
            "behavior Foo { @caps(Network) version 1 trigger(\"hello\", {\"k\": 1}); }";
        let m = parse(src).unwrap();
        let checked = check(m).unwrap();
        assert!(checked.effects.contains(Effect::Network));
    }

    #[test]
    fn spawn_is_pure_and_needs_no_caps() {
        let src = "behavior Foo { version 1 spawn(\"goblin\", vec3(1.0, 0.0, 0.0)); }";
        let m = parse(src).unwrap();
        let checked = check(m).unwrap();
        assert!(checked.effects.is_pure());
    }

    #[test]
    fn move_requires_movement_cap() {
        let src = "behavior Foo { version 1 move(self, vec3(0,0,0), 1.0); }";
        let m = parse(src).unwrap();
        let err = check(m).unwrap_err();
        assert_eq!(err.code(), "BDSL-E0004");
    }

    #[test]
    fn wrong_arg_count_flagged() {
        let src = "behavior Foo { @caps(Movement) version 1 move(self, vec3(0,0,0)); }";
        let m = parse(src).unwrap();
        let err = check(m).unwrap_err();
        assert_eq!(err.code(), "BDSL-E0002");
    }

    #[test]
    fn wrong_arg_type_flagged() {
        let src = "behavior Foo { @caps(Movement) version 1 move(self, 3, 1.0); }";
        let m = parse(src).unwrap();
        let err = check(m).unwrap_err();
        assert_eq!(err.code(), "BDSL-E0003");
    }
}
