//! Integration tests for the parser.

use aether_behavior_dsl::ast::NodeKind;
use aether_behavior_dsl::{parse, Capability, Combinator, Verb};

#[test]
fn parses_patrol_behavior() {
    let src = include_str!("golden/g01_patrol.beh");
    let m = parse(src).expect("parses");
    assert_eq!(m.name, "Patrol");
    assert_eq!(m.version, 1);
    assert!(m.caps.contains(Capability::Movement));
    match &m.root.kind {
        NodeKind::Combinator {
            combinator,
            children,
            ..
        } => {
            assert_eq!(*combinator, Combinator::Sequence);
            assert_eq!(children.len(), 2);
        }
        _ => panic!("expected sequence root"),
    }
}

#[test]
fn parses_merchant_with_dialogue_options() {
    let src = include_str!("golden/g03_merchant.beh");
    let m = parse(src).expect("parses");
    assert_eq!(m.name, "Merchant");
    // Walk to the dialogue call.
    let NodeKind::Combinator { children, .. } = &m.root.kind else {
        panic!("expected sequence")
    };
    let NodeKind::Verb { verb, args } = &children[0].kind else {
        panic!("expected verb")
    };
    assert_eq!(*verb, Verb::Dialogue);
    assert_eq!(args.len(), 3);
}

#[test]
fn rejects_source_without_header() {
    let err = parse("// empty").unwrap_err();
    assert_eq!(err.code(), "BDSL-E0015");
}

#[test]
fn rejects_missing_version() {
    let err = parse("behavior X { spawn(\"x\", vec3(0,0,0)); }").unwrap_err();
    // We'll report MissingVersion if the next token isn't `version`.
    assert_eq!(err.code(), "BDSL-E0016");
}

#[test]
fn rejects_unknown_combinator_with_suggestion() {
    let err = parse("behavior X { version 1 repeat { spawn(\"x\", vec3(0,0,0)); } }").unwrap_err();
    assert_eq!(err.code(), "BDSL-E0018");
}

#[test]
fn retry_parameter_must_be_positive() {
    let err = parse("behavior X { version 1 retry(-1) { spawn(\"x\", vec3(0,0,0)); } }")
        .unwrap_err();
    assert_eq!(err.code(), "BDSL-E0020");
}
