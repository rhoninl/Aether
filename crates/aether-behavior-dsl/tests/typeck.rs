//! Integration tests for the type checker.

use aether_behavior_dsl::{check, parse, Effect};

#[test]
fn patrol_typechecks_and_has_movement_effect() {
    let src = include_str!("golden/g01_patrol.beh");
    let m = parse(src).unwrap();
    let checked = check(m).expect("type-checks");
    assert!(checked.effects.contains(Effect::Movement));
}

#[test]
fn missing_movement_cap_fails() {
    let src = "behavior X { version 1 move(self, vec3(0,0,0), 1.0); }";
    let m = parse(src).unwrap();
    let err = check(m).unwrap_err();
    assert_eq!(err.code(), "BDSL-E0004");
}

#[test]
fn wrong_arg_type_fails() {
    let src = "behavior X { @caps(Combat) version 1 damage(self, \"not-an-int\"); }";
    let m = parse(src).unwrap();
    let err = check(m).unwrap_err();
    assert_eq!(err.code(), "BDSL-E0003");
}

#[test]
fn wrong_arg_count_fails() {
    let src = "behavior X { @caps(Movement) version 1 move(self, vec3(0,0,0)); }";
    let m = parse(src).unwrap();
    let err = check(m).unwrap_err();
    assert_eq!(err.code(), "BDSL-E0002");
}

#[test]
fn unresolved_entity_ref_fails() {
    let src = "behavior X { @caps(Combat) version 1 damage(whoever, 1); }";
    let m = parse(src).unwrap();
    let err = check(m).unwrap_err();
    assert_eq!(err.code(), "BDSL-E0006");
}
