//! Integration tests for the VM: built-ins, variables, limits, hot-reload.

use std::collections::HashMap;

use super::*;
use crate::visual_script::compiler::{BinaryOp, CompiledScript, IrInstruction};
use crate::visual_script::runtime::engine_api::{NoOpApi, RecordingApi};
use crate::visual_script::runtime::error::RuntimeError;
use crate::visual_script::types::Value;

/// Helper to build a CompiledScript from raw instructions.
fn make_script(instructions: Vec<IrInstruction>, register_count: u32) -> CompiledScript {
    CompiledScript {
        instructions,
        node_instruction_map: HashMap::new(),
        register_count,
        wasm_bytes: vec![],
    }
}

fn default_config() -> VmConfig {
    VmConfig::default()
}

// --- Built-in function tests ---

#[test]
fn test_builtin_clamp() {
    let script = make_script(
        vec![
            IrInstruction::LoadConst(0, Value::Float(15.0)),
            IrInstruction::LoadConst(1, Value::Float(0.0)),
            IrInstruction::LoadConst(2, Value::Float(10.0)),
            IrInstruction::Call {
                function: "clamp".into(),
                args: vec![0, 1, 2],
                result: Some(3),
            },
            IrInstruction::Return,
        ],
        4,
    );
    let mut vm = ScriptVm::new(&script, default_config()).unwrap();
    vm.execute(&mut NoOpApi).unwrap();
    assert_eq!(vm.read_register(3), Some(&Value::Float(10.0)));
}

#[test]
fn test_builtin_clamp_within_range() {
    let script = make_script(
        vec![
            IrInstruction::LoadConst(0, Value::Float(5.0)),
            IrInstruction::LoadConst(1, Value::Float(0.0)),
            IrInstruction::LoadConst(2, Value::Float(10.0)),
            IrInstruction::Call {
                function: "clamp".into(),
                args: vec![0, 1, 2],
                result: Some(3),
            },
            IrInstruction::Return,
        ],
        4,
    );
    let mut vm = ScriptVm::new(&script, default_config()).unwrap();
    vm.execute(&mut NoOpApi).unwrap();
    assert_eq!(vm.read_register(3), Some(&Value::Float(5.0)));
}

#[test]
fn test_builtin_lerp() {
    let script = make_script(
        vec![
            IrInstruction::LoadConst(0, Value::Float(0.0)),
            IrInstruction::LoadConst(1, Value::Float(10.0)),
            IrInstruction::LoadConst(2, Value::Float(0.5)),
            IrInstruction::Call {
                function: "lerp".into(),
                args: vec![0, 1, 2],
                result: Some(3),
            },
            IrInstruction::Return,
        ],
        4,
    );
    let mut vm = ScriptVm::new(&script, default_config()).unwrap();
    vm.execute(&mut NoOpApi).unwrap();
    assert_eq!(vm.read_register(3), Some(&Value::Float(5.0)));
}

#[test]
fn test_builtin_lerp_at_zero() {
    let script = make_script(
        vec![
            IrInstruction::LoadConst(0, Value::Float(2.0)),
            IrInstruction::LoadConst(1, Value::Float(8.0)),
            IrInstruction::LoadConst(2, Value::Float(0.0)),
            IrInstruction::Call {
                function: "lerp".into(),
                args: vec![0, 1, 2],
                result: Some(3),
            },
            IrInstruction::Return,
        ],
        4,
    );
    let mut vm = ScriptVm::new(&script, default_config()).unwrap();
    vm.execute(&mut NoOpApi).unwrap();
    assert_eq!(vm.read_register(3), Some(&Value::Float(2.0)));
}

#[test]
fn test_builtin_lerp_at_one() {
    let script = make_script(
        vec![
            IrInstruction::LoadConst(0, Value::Float(2.0)),
            IrInstruction::LoadConst(1, Value::Float(8.0)),
            IrInstruction::LoadConst(2, Value::Float(1.0)),
            IrInstruction::Call {
                function: "lerp".into(),
                args: vec![0, 1, 2],
                result: Some(3),
            },
            IrInstruction::Return,
        ],
        4,
    );
    let mut vm = ScriptVm::new(&script, default_config()).unwrap();
    vm.execute(&mut NoOpApi).unwrap();
    assert_eq!(vm.read_register(3), Some(&Value::Float(8.0)));
}

// --- Variable tests ---

#[test]
fn test_set_and_get_variable() {
    let script = make_script(
        vec![
            IrInstruction::LoadConst(0, Value::String("hp".into())),
            IrInstruction::LoadConst(1, Value::Int(100)),
            IrInstruction::Call {
                function: "set_variable".into(),
                args: vec![0, 1],
                result: None,
            },
            IrInstruction::Return,
        ],
        2,
    );
    let mut vm = ScriptVm::new(&script, default_config()).unwrap();
    vm.execute(&mut NoOpApi).unwrap();
    assert_eq!(vm.get_variable("hp"), Some(&Value::Int(100)));
}

#[test]
fn test_variable_overwrite() {
    let script = make_script(
        vec![
            IrInstruction::LoadConst(0, Value::String("x".into())),
            IrInstruction::LoadConst(1, Value::Int(1)),
            IrInstruction::Call {
                function: "set_variable".into(),
                args: vec![0, 1],
                result: None,
            },
            IrInstruction::LoadConst(1, Value::Int(2)),
            IrInstruction::Call {
                function: "set_variable".into(),
                args: vec![0, 1],
                result: None,
            },
            IrInstruction::Return,
        ],
        2,
    );
    let mut vm = ScriptVm::new(&script, default_config()).unwrap();
    vm.execute(&mut NoOpApi).unwrap();
    assert_eq!(vm.get_variable("x"), Some(&Value::Int(2)));
}

#[test]
fn test_preload_variable() {
    let script = make_script(vec![IrInstruction::Return], 0);
    let mut vm = ScriptVm::new(&script, default_config()).unwrap();
    vm.set_variable("score".into(), Value::Int(0)).unwrap();
    assert_eq!(vm.get_variable("score"), Some(&Value::Int(0)));
}

#[test]
fn test_variable_limit_exceeded() {
    let config = VmConfig {
        max_vars: 2,
        ..VmConfig::default()
    };
    let script = make_script(vec![IrInstruction::Return], 0);
    let mut vm = ScriptVm::new(&script, config).unwrap();
    vm.set_variable("a".into(), Value::Int(1)).unwrap();
    vm.set_variable("b".into(), Value::Int(2)).unwrap();
    let err = vm.set_variable("c".into(), Value::Int(3)).unwrap_err();
    assert_eq!(err, RuntimeError::VariableLimitExceeded { limit: 2 });
}

#[test]
fn test_variable_limit_allows_overwrite() {
    let config = VmConfig {
        max_vars: 1,
        ..VmConfig::default()
    };
    let script = make_script(vec![IrInstruction::Return], 0);
    let mut vm = ScriptVm::new(&script, config).unwrap();
    vm.set_variable("a".into(), Value::Int(1)).unwrap();
    vm.set_variable("a".into(), Value::Int(2)).unwrap();
    assert_eq!(vm.get_variable("a"), Some(&Value::Int(2)));
}

// --- Execution limit tests ---

#[test]
fn test_execution_limit_exceeded() {
    let config = VmConfig {
        max_ops: 3,
        ..VmConfig::default()
    };
    let script = make_script(
        vec![
            IrInstruction::Nop,
            IrInstruction::Nop,
            IrInstruction::Nop,
            IrInstruction::Nop,
            IrInstruction::Return,
        ],
        0,
    );
    let mut vm = ScriptVm::new(&script, config).unwrap();
    let err = vm.execute(&mut NoOpApi).unwrap_err();
    assert_eq!(err, RuntimeError::ExecutionLimitExceeded { limit: 3 });
}

#[test]
fn test_execution_within_limit() {
    let config = VmConfig {
        max_ops: 5,
        ..VmConfig::default()
    };
    let script = make_script(
        vec![
            IrInstruction::Nop,
            IrInstruction::Nop,
            IrInstruction::Nop,
            IrInstruction::Return,
        ],
        0,
    );
    let mut vm = ScriptVm::new(&script, config).unwrap();
    vm.execute(&mut NoOpApi).unwrap();
    assert_eq!(vm.ops_executed(), 4);
}

#[test]
fn test_stack_overflow_on_creation() {
    let config = VmConfig {
        max_stack: 2,
        ..VmConfig::default()
    };
    let script = make_script(vec![IrInstruction::Return], 5);
    let err = ScriptVm::new(&script, config).unwrap_err();
    assert_eq!(
        err,
        RuntimeError::StackOverflow {
            requested: 5,
            limit: 2,
        }
    );
}

// --- Register bounds tests ---

#[test]
fn test_register_out_of_bounds_read() {
    let script = make_script(
        vec![
            IrInstruction::Not { dest: 0, src: 5 },
            IrInstruction::Return,
        ],
        1,
    );
    let mut vm = ScriptVm::new(&script, default_config()).unwrap();
    let err = vm.execute(&mut NoOpApi).unwrap_err();
    assert!(matches!(err, RuntimeError::RegisterOutOfBounds { .. }));
}

#[test]
fn test_register_out_of_bounds_write() {
    let script = make_script(
        vec![
            IrInstruction::LoadConst(5, Value::Int(1)),
            IrInstruction::Return,
        ],
        1,
    );
    let mut vm = ScriptVm::new(&script, default_config()).unwrap();
    let err = vm.execute(&mut NoOpApi).unwrap_err();
    assert!(matches!(err, RuntimeError::RegisterOutOfBounds { .. }));
}

// --- Hot-reload tests ---

#[test]
fn test_hot_reload_preserves_variables() {
    let script1 = make_script(
        vec![
            IrInstruction::LoadConst(0, Value::String("x".into())),
            IrInstruction::LoadConst(1, Value::Int(42)),
            IrInstruction::Call {
                function: "set_variable".into(),
                args: vec![0, 1],
                result: None,
            },
            IrInstruction::Return,
        ],
        2,
    );
    let script2 = make_script(vec![IrInstruction::Return], 0);

    let mut vm = ScriptVm::new(&script1, default_config()).unwrap();
    vm.execute(&mut NoOpApi).unwrap();
    assert_eq!(vm.get_variable("x"), Some(&Value::Int(42)));

    vm.reload(&script2).unwrap();
    assert_eq!(vm.get_variable("x"), Some(&Value::Int(42)));
}

#[test]
fn test_hot_reload_resets_registers() {
    let script1 = make_script(
        vec![
            IrInstruction::LoadConst(0, Value::Int(42)),
            IrInstruction::Return,
        ],
        1,
    );
    let script2 = make_script(vec![IrInstruction::Return], 1);

    let mut vm = ScriptVm::new(&script1, default_config()).unwrap();
    vm.execute(&mut NoOpApi).unwrap();
    assert_eq!(vm.read_register(0), Some(&Value::Int(42)));

    vm.reload(&script2).unwrap();
    assert_eq!(vm.read_register(0), Some(&Value::None));
}

#[test]
fn test_hot_reload_rejects_too_many_registers() {
    let script1 = make_script(vec![IrInstruction::Return], 1);
    let config = VmConfig {
        max_stack: 2,
        ..VmConfig::default()
    };
    let mut vm = ScriptVm::new(&script1, config).unwrap();

    let script2 = make_script(vec![IrInstruction::Return], 5);
    let err = vm.reload(&script2).unwrap_err();
    assert!(matches!(err, RuntimeError::StackOverflow { .. }));
}

// --- Truthiness tests ---

#[test]
fn test_value_truthiness() {
    assert!(!value_is_truthy(&Value::None));
    assert!(value_is_truthy(&Value::Bool(true)));
    assert!(!value_is_truthy(&Value::Bool(false)));
    assert!(value_is_truthy(&Value::Int(1)));
    assert!(!value_is_truthy(&Value::Int(0)));
    assert!(value_is_truthy(&Value::Float(0.1)));
    assert!(!value_is_truthy(&Value::Float(0.0)));
    assert!(value_is_truthy(&Value::String("hi".into())));
    assert!(!value_is_truthy(&Value::String("".into())));
    assert!(value_is_truthy(&Value::Vec3 {
        x: 1.0,
        y: 0.0,
        z: 0.0
    }));
    assert!(!value_is_truthy(&Value::Vec3 {
        x: 0.0,
        y: 0.0,
        z: 0.0
    }));
    assert!(value_is_truthy(&Value::Entity(1)));
    assert!(!value_is_truthy(&Value::Entity(0)));
}

// --- Equality tests ---

#[test]
fn test_values_equal_same_types() {
    assert!(values_equal(&Value::None, &Value::None));
    assert!(values_equal(&Value::Bool(true), &Value::Bool(true)));
    assert!(!values_equal(&Value::Bool(true), &Value::Bool(false)));
    assert!(values_equal(&Value::Int(5), &Value::Int(5)));
    assert!(!values_equal(&Value::Int(5), &Value::Int(6)));
    assert!(values_equal(&Value::Float(1.0), &Value::Float(1.0)));
    assert!(values_equal(
        &Value::String("a".into()),
        &Value::String("a".into())
    ));
    assert!(values_equal(&Value::Entity(1), &Value::Entity(1)));
    assert!(values_equal(
        &Value::Vec3 {
            x: 1.0,
            y: 2.0,
            z: 3.0
        },
        &Value::Vec3 {
            x: 1.0,
            y: 2.0,
            z: 3.0
        },
    ));
}

#[test]
fn test_values_equal_int_float_cross() {
    assert!(values_equal(&Value::Int(5), &Value::Float(5.0)));
    assert!(values_equal(&Value::Float(5.0), &Value::Int(5)));
}

#[test]
fn test_values_equal_different_types() {
    assert!(!values_equal(&Value::Int(1), &Value::String("1".into())));
    assert!(!values_equal(&Value::Bool(true), &Value::Int(1)));
}

// --- VmConfig tests ---

#[test]
fn test_vm_config_defaults() {
    let config = VmConfig::default();
    assert_eq!(config.max_ops, DEFAULT_MAX_OPS);
    assert_eq!(config.max_stack, DEFAULT_MAX_STACK);
    assert_eq!(config.max_vars, DEFAULT_MAX_VARS);
}

// --- Empty script tests ---

#[test]
fn test_empty_instructions() {
    let script = make_script(vec![], 0);
    let mut vm = ScriptVm::new(&script, default_config()).unwrap();
    vm.execute(&mut NoOpApi).unwrap();
    assert_eq!(vm.ops_executed(), 0);
}

#[test]
fn test_only_return() {
    let script = make_script(vec![IrInstruction::Return], 0);
    let mut vm = ScriptVm::new(&script, default_config()).unwrap();
    vm.execute(&mut NoOpApi).unwrap();
    assert_eq!(vm.ops_executed(), 1);
}

// --- Integration-style: multi-instruction sequences ---

#[test]
fn test_compute_and_branch() {
    let script = make_script(
        vec![
            IrInstruction::LoadConst(0, Value::Int(3)),
            IrInstruction::LoadConst(1, Value::Int(4)),
            IrInstruction::BinaryOp {
                op: BinaryOp::Add,
                dest: 2,
                lhs: 0,
                rhs: 1,
            },
            IrInstruction::LoadConst(3, Value::Int(5)),
            IrInstruction::BinaryOp {
                op: BinaryOp::Greater,
                dest: 4,
                lhs: 2,
                rhs: 3,
            },
            IrInstruction::Branch {
                condition: 4,
                true_label: 0,
                false_label: 1,
            },
            IrInstruction::Label(0),
            IrInstruction::LoadConst(5, Value::String("big".into())),
            IrInstruction::Return,
            IrInstruction::Label(1),
            IrInstruction::LoadConst(5, Value::String("small".into())),
            IrInstruction::Return,
        ],
        6,
    );
    let mut vm = ScriptVm::new(&script, default_config()).unwrap();
    vm.execute(&mut NoOpApi).unwrap();
    assert_eq!(vm.read_register(5), Some(&Value::String("big".into())));
}

#[test]
fn test_multiple_api_calls() {
    let script = make_script(
        vec![
            IrInstruction::LoadConst(0, Value::Entity(1)),
            IrInstruction::LoadConst(
                1,
                Value::Vec3 {
                    x: 1.0,
                    y: 2.0,
                    z: 3.0,
                },
            ),
            IrInstruction::Call {
                function: "set_position".into(),
                args: vec![0, 1],
                result: None,
            },
            IrInstruction::LoadConst(2, Value::String("boom".into())),
            IrInstruction::Call {
                function: "play_sound".into(),
                args: vec![2],
                result: None,
            },
            IrInstruction::Return,
        ],
        3,
    );
    let mut api = RecordingApi::default();
    let mut vm = ScriptVm::new(&script, default_config()).unwrap();
    vm.execute(&mut api).unwrap();
    assert_eq!(api.calls.len(), 2);
    assert_eq!(api.calls[0].0, "set_position");
    assert_eq!(api.calls[1].0, "play_sound");
}

#[test]
fn test_execute_resets_state_on_rerun() {
    let script = make_script(
        vec![
            IrInstruction::LoadConst(0, Value::Int(42)),
            IrInstruction::Return,
        ],
        1,
    );
    let mut vm = ScriptVm::new(&script, default_config()).unwrap();
    vm.execute(&mut NoOpApi).unwrap();
    assert_eq!(vm.ops_executed(), 2);

    vm.execute(&mut NoOpApi).unwrap();
    assert_eq!(vm.ops_executed(), 2);
}
