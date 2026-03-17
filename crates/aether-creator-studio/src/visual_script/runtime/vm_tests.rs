//! Tests for the visual script VM: instructions, arithmetic, control flow, calls.

use std::collections::HashMap;

use super::*;
use crate::visual_script::compiler::{BinaryOp, CompiledScript, IrInstruction};
use crate::visual_script::runtime::engine_api::{NoOpApi, RecordingApi};
use crate::visual_script::runtime::error::RuntimeError;
use crate::visual_script::types::Value;

/// Helper to build a CompiledScript from raw instructions.
pub(super) fn make_script(
    instructions: Vec<IrInstruction>,
    register_count: u32,
) -> CompiledScript {
    CompiledScript {
        instructions,
        node_instruction_map: HashMap::new(),
        register_count,
        wasm_bytes: vec![],
    }
}

pub(super) fn default_config() -> VmConfig {
    VmConfig::default()
}

// --- Instruction tests ---

#[test]
fn test_load_const_int() {
    let script = make_script(
        vec![IrInstruction::LoadConst(0, Value::Int(42)), IrInstruction::Return],
        1,
    );
    let mut vm = ScriptVm::new(&script, default_config()).unwrap();
    vm.execute(&mut NoOpApi).unwrap();
    assert_eq!(vm.read_register(0), Some(&Value::Int(42)));
}

#[test]
fn test_load_const_float() {
    let script = make_script(
        vec![IrInstruction::LoadConst(0, Value::Float(3.14)), IrInstruction::Return],
        1,
    );
    let mut vm = ScriptVm::new(&script, default_config()).unwrap();
    vm.execute(&mut NoOpApi).unwrap();
    assert_eq!(vm.read_register(0), Some(&Value::Float(3.14)));
}

#[test]
fn test_load_const_string() {
    let script = make_script(
        vec![
            IrInstruction::LoadConst(0, Value::String("hello".into())),
            IrInstruction::Return,
        ],
        1,
    );
    let mut vm = ScriptVm::new(&script, default_config()).unwrap();
    vm.execute(&mut NoOpApi).unwrap();
    assert_eq!(vm.read_register(0), Some(&Value::String("hello".into())));
}

#[test]
fn test_load_const_bool() {
    let script = make_script(
        vec![IrInstruction::LoadConst(0, Value::Bool(true)), IrInstruction::Return],
        1,
    );
    let mut vm = ScriptVm::new(&script, default_config()).unwrap();
    vm.execute(&mut NoOpApi).unwrap();
    assert_eq!(vm.read_register(0), Some(&Value::Bool(true)));
}

#[test]
fn test_load_const_vec3() {
    let v = Value::Vec3 { x: 1.0, y: 2.0, z: 3.0 };
    let script = make_script(
        vec![IrInstruction::LoadConst(0, v.clone()), IrInstruction::Return],
        1,
    );
    let mut vm = ScriptVm::new(&script, default_config()).unwrap();
    vm.execute(&mut NoOpApi).unwrap();
    assert_eq!(vm.read_register(0), Some(&v));
}

#[test]
fn test_load_const_entity() {
    let script = make_script(
        vec![IrInstruction::LoadConst(0, Value::Entity(99)), IrInstruction::Return],
        1,
    );
    let mut vm = ScriptVm::new(&script, default_config()).unwrap();
    vm.execute(&mut NoOpApi).unwrap();
    assert_eq!(vm.read_register(0), Some(&Value::Entity(99)));
}

// --- Binary op tests ---

#[test]
fn test_add_ints() {
    let script = make_script(
        vec![
            IrInstruction::LoadConst(0, Value::Int(3)),
            IrInstruction::LoadConst(1, Value::Int(4)),
            IrInstruction::BinaryOp { op: BinaryOp::Add, dest: 2, lhs: 0, rhs: 1 },
            IrInstruction::Return,
        ],
        3,
    );
    let mut vm = ScriptVm::new(&script, default_config()).unwrap();
    vm.execute(&mut NoOpApi).unwrap();
    assert_eq!(vm.read_register(2), Some(&Value::Int(7)));
}

#[test]
fn test_add_floats() {
    let script = make_script(
        vec![
            IrInstruction::LoadConst(0, Value::Float(1.5)),
            IrInstruction::LoadConst(1, Value::Float(2.5)),
            IrInstruction::BinaryOp { op: BinaryOp::Add, dest: 2, lhs: 0, rhs: 1 },
            IrInstruction::Return,
        ],
        3,
    );
    let mut vm = ScriptVm::new(&script, default_config()).unwrap();
    vm.execute(&mut NoOpApi).unwrap();
    assert_eq!(vm.read_register(2), Some(&Value::Float(4.0)));
}

#[test]
fn test_add_int_float_coercion() {
    let script = make_script(
        vec![
            IrInstruction::LoadConst(0, Value::Int(2)),
            IrInstruction::LoadConst(1, Value::Float(3.5)),
            IrInstruction::BinaryOp { op: BinaryOp::Add, dest: 2, lhs: 0, rhs: 1 },
            IrInstruction::Return,
        ],
        3,
    );
    let mut vm = ScriptVm::new(&script, default_config()).unwrap();
    vm.execute(&mut NoOpApi).unwrap();
    assert_eq!(vm.read_register(2), Some(&Value::Float(5.5)));
}

#[test]
fn test_subtract() {
    let script = make_script(
        vec![
            IrInstruction::LoadConst(0, Value::Int(10)),
            IrInstruction::LoadConst(1, Value::Int(3)),
            IrInstruction::BinaryOp { op: BinaryOp::Subtract, dest: 2, lhs: 0, rhs: 1 },
            IrInstruction::Return,
        ],
        3,
    );
    let mut vm = ScriptVm::new(&script, default_config()).unwrap();
    vm.execute(&mut NoOpApi).unwrap();
    assert_eq!(vm.read_register(2), Some(&Value::Int(7)));
}

#[test]
fn test_multiply() {
    let script = make_script(
        vec![
            IrInstruction::LoadConst(0, Value::Int(5)),
            IrInstruction::LoadConst(1, Value::Int(6)),
            IrInstruction::BinaryOp { op: BinaryOp::Multiply, dest: 2, lhs: 0, rhs: 1 },
            IrInstruction::Return,
        ],
        3,
    );
    let mut vm = ScriptVm::new(&script, default_config()).unwrap();
    vm.execute(&mut NoOpApi).unwrap();
    assert_eq!(vm.read_register(2), Some(&Value::Int(30)));
}

#[test]
fn test_divide() {
    let script = make_script(
        vec![
            IrInstruction::LoadConst(0, Value::Int(10)),
            IrInstruction::LoadConst(1, Value::Int(2)),
            IrInstruction::BinaryOp { op: BinaryOp::Divide, dest: 2, lhs: 0, rhs: 1 },
            IrInstruction::Return,
        ],
        3,
    );
    let mut vm = ScriptVm::new(&script, default_config()).unwrap();
    vm.execute(&mut NoOpApi).unwrap();
    assert_eq!(vm.read_register(2), Some(&Value::Int(5)));
}

#[test]
fn test_divide_by_zero_int() {
    let script = make_script(
        vec![
            IrInstruction::LoadConst(0, Value::Int(10)),
            IrInstruction::LoadConst(1, Value::Int(0)),
            IrInstruction::BinaryOp { op: BinaryOp::Divide, dest: 2, lhs: 0, rhs: 1 },
            IrInstruction::Return,
        ],
        3,
    );
    let mut vm = ScriptVm::new(&script, default_config()).unwrap();
    let err = vm.execute(&mut NoOpApi).unwrap_err();
    assert_eq!(err, RuntimeError::DivisionByZero);
}

#[test]
fn test_divide_by_zero_float() {
    let script = make_script(
        vec![
            IrInstruction::LoadConst(0, Value::Float(10.0)),
            IrInstruction::LoadConst(1, Value::Float(0.0)),
            IrInstruction::BinaryOp { op: BinaryOp::Divide, dest: 2, lhs: 0, rhs: 1 },
            IrInstruction::Return,
        ],
        3,
    );
    let mut vm = ScriptVm::new(&script, default_config()).unwrap();
    let err = vm.execute(&mut NoOpApi).unwrap_err();
    assert_eq!(err, RuntimeError::DivisionByZero);
}

#[test]
fn test_equal_true() {
    let script = make_script(
        vec![
            IrInstruction::LoadConst(0, Value::Int(5)),
            IrInstruction::LoadConst(1, Value::Int(5)),
            IrInstruction::BinaryOp { op: BinaryOp::Equal, dest: 2, lhs: 0, rhs: 1 },
            IrInstruction::Return,
        ],
        3,
    );
    let mut vm = ScriptVm::new(&script, default_config()).unwrap();
    vm.execute(&mut NoOpApi).unwrap();
    assert_eq!(vm.read_register(2), Some(&Value::Bool(true)));
}

#[test]
fn test_equal_false() {
    let script = make_script(
        vec![
            IrInstruction::LoadConst(0, Value::Int(5)),
            IrInstruction::LoadConst(1, Value::Int(6)),
            IrInstruction::BinaryOp { op: BinaryOp::Equal, dest: 2, lhs: 0, rhs: 1 },
            IrInstruction::Return,
        ],
        3,
    );
    let mut vm = ScriptVm::new(&script, default_config()).unwrap();
    vm.execute(&mut NoOpApi).unwrap();
    assert_eq!(vm.read_register(2), Some(&Value::Bool(false)));
}

#[test]
fn test_not_equal() {
    let script = make_script(
        vec![
            IrInstruction::LoadConst(0, Value::Int(5)),
            IrInstruction::LoadConst(1, Value::Int(6)),
            IrInstruction::BinaryOp { op: BinaryOp::NotEqual, dest: 2, lhs: 0, rhs: 1 },
            IrInstruction::Return,
        ],
        3,
    );
    let mut vm = ScriptVm::new(&script, default_config()).unwrap();
    vm.execute(&mut NoOpApi).unwrap();
    assert_eq!(vm.read_register(2), Some(&Value::Bool(true)));
}

#[test]
fn test_greater() {
    let script = make_script(
        vec![
            IrInstruction::LoadConst(0, Value::Int(10)),
            IrInstruction::LoadConst(1, Value::Int(5)),
            IrInstruction::BinaryOp { op: BinaryOp::Greater, dest: 2, lhs: 0, rhs: 1 },
            IrInstruction::Return,
        ],
        3,
    );
    let mut vm = ScriptVm::new(&script, default_config()).unwrap();
    vm.execute(&mut NoOpApi).unwrap();
    assert_eq!(vm.read_register(2), Some(&Value::Bool(true)));
}

#[test]
fn test_less() {
    let script = make_script(
        vec![
            IrInstruction::LoadConst(0, Value::Int(3)),
            IrInstruction::LoadConst(1, Value::Int(5)),
            IrInstruction::BinaryOp { op: BinaryOp::Less, dest: 2, lhs: 0, rhs: 1 },
            IrInstruction::Return,
        ],
        3,
    );
    let mut vm = ScriptVm::new(&script, default_config()).unwrap();
    vm.execute(&mut NoOpApi).unwrap();
    assert_eq!(vm.read_register(2), Some(&Value::Bool(true)));
}

#[test]
fn test_and_both_true() {
    let script = make_script(
        vec![
            IrInstruction::LoadConst(0, Value::Bool(true)),
            IrInstruction::LoadConst(1, Value::Bool(true)),
            IrInstruction::BinaryOp { op: BinaryOp::And, dest: 2, lhs: 0, rhs: 1 },
            IrInstruction::Return,
        ],
        3,
    );
    let mut vm = ScriptVm::new(&script, default_config()).unwrap();
    vm.execute(&mut NoOpApi).unwrap();
    assert_eq!(vm.read_register(2), Some(&Value::Bool(true)));
}

#[test]
fn test_and_one_false() {
    let script = make_script(
        vec![
            IrInstruction::LoadConst(0, Value::Bool(true)),
            IrInstruction::LoadConst(1, Value::Bool(false)),
            IrInstruction::BinaryOp { op: BinaryOp::And, dest: 2, lhs: 0, rhs: 1 },
            IrInstruction::Return,
        ],
        3,
    );
    let mut vm = ScriptVm::new(&script, default_config()).unwrap();
    vm.execute(&mut NoOpApi).unwrap();
    assert_eq!(vm.read_register(2), Some(&Value::Bool(false)));
}

#[test]
fn test_or() {
    let script = make_script(
        vec![
            IrInstruction::LoadConst(0, Value::Bool(false)),
            IrInstruction::LoadConst(1, Value::Bool(true)),
            IrInstruction::BinaryOp { op: BinaryOp::Or, dest: 2, lhs: 0, rhs: 1 },
            IrInstruction::Return,
        ],
        3,
    );
    let mut vm = ScriptVm::new(&script, default_config()).unwrap();
    vm.execute(&mut NoOpApi).unwrap();
    assert_eq!(vm.read_register(2), Some(&Value::Bool(true)));
}

#[test]
fn test_type_error_binary_op() {
    let script = make_script(
        vec![
            IrInstruction::LoadConst(0, Value::String("a".into())),
            IrInstruction::LoadConst(1, Value::Int(1)),
            IrInstruction::BinaryOp { op: BinaryOp::Add, dest: 2, lhs: 0, rhs: 1 },
            IrInstruction::Return,
        ],
        3,
    );
    let mut vm = ScriptVm::new(&script, default_config()).unwrap();
    let err = vm.execute(&mut NoOpApi).unwrap_err();
    assert!(matches!(err, RuntimeError::TypeError { .. }));
}

// --- NOT tests ---

#[test]
fn test_not_true() {
    let script = make_script(
        vec![
            IrInstruction::LoadConst(0, Value::Bool(true)),
            IrInstruction::Not { dest: 1, src: 0 },
            IrInstruction::Return,
        ],
        2,
    );
    let mut vm = ScriptVm::new(&script, default_config()).unwrap();
    vm.execute(&mut NoOpApi).unwrap();
    assert_eq!(vm.read_register(1), Some(&Value::Bool(false)));
}

#[test]
fn test_not_false() {
    let script = make_script(
        vec![
            IrInstruction::LoadConst(0, Value::Bool(false)),
            IrInstruction::Not { dest: 1, src: 0 },
            IrInstruction::Return,
        ],
        2,
    );
    let mut vm = ScriptVm::new(&script, default_config()).unwrap();
    vm.execute(&mut NoOpApi).unwrap();
    assert_eq!(vm.read_register(1), Some(&Value::Bool(true)));
}

#[test]
fn test_not_int_zero() {
    let script = make_script(
        vec![
            IrInstruction::LoadConst(0, Value::Int(0)),
            IrInstruction::Not { dest: 1, src: 0 },
            IrInstruction::Return,
        ],
        2,
    );
    let mut vm = ScriptVm::new(&script, default_config()).unwrap();
    vm.execute(&mut NoOpApi).unwrap();
    assert_eq!(vm.read_register(1), Some(&Value::Bool(true)));
}

#[test]
fn test_not_int_nonzero() {
    let script = make_script(
        vec![
            IrInstruction::LoadConst(0, Value::Int(42)),
            IrInstruction::Not { dest: 1, src: 0 },
            IrInstruction::Return,
        ],
        2,
    );
    let mut vm = ScriptVm::new(&script, default_config()).unwrap();
    vm.execute(&mut NoOpApi).unwrap();
    assert_eq!(vm.read_register(1), Some(&Value::Bool(false)));
}

// --- Control flow tests ---

#[test]
fn test_branch_true() {
    let script = make_script(
        vec![
            IrInstruction::LoadConst(0, Value::Bool(true)),
            IrInstruction::Branch { condition: 0, true_label: 0, false_label: 1 },
            IrInstruction::Label(0),
            IrInstruction::LoadConst(1, Value::Int(1)),
            IrInstruction::Return,
            IrInstruction::Label(1),
            IrInstruction::LoadConst(1, Value::Int(2)),
            IrInstruction::Return,
        ],
        2,
    );
    let mut vm = ScriptVm::new(&script, default_config()).unwrap();
    vm.execute(&mut NoOpApi).unwrap();
    assert_eq!(vm.read_register(1), Some(&Value::Int(1)));
}

#[test]
fn test_branch_false() {
    let script = make_script(
        vec![
            IrInstruction::LoadConst(0, Value::Bool(false)),
            IrInstruction::Branch { condition: 0, true_label: 0, false_label: 1 },
            IrInstruction::Label(0),
            IrInstruction::LoadConst(1, Value::Int(1)),
            IrInstruction::Return,
            IrInstruction::Label(1),
            IrInstruction::LoadConst(1, Value::Int(2)),
            IrInstruction::Return,
        ],
        2,
    );
    let mut vm = ScriptVm::new(&script, default_config()).unwrap();
    vm.execute(&mut NoOpApi).unwrap();
    assert_eq!(vm.read_register(1), Some(&Value::Int(2)));
}

#[test]
fn test_jump() {
    let script = make_script(
        vec![
            IrInstruction::Jump(0),
            IrInstruction::LoadConst(0, Value::Int(999)),
            IrInstruction::Return,
            IrInstruction::Label(0),
            IrInstruction::LoadConst(0, Value::Int(42)),
            IrInstruction::Return,
        ],
        1,
    );
    let mut vm = ScriptVm::new(&script, default_config()).unwrap();
    vm.execute(&mut NoOpApi).unwrap();
    assert_eq!(vm.read_register(0), Some(&Value::Int(42)));
}

#[test]
fn test_label_not_found() {
    let script = make_script(
        vec![IrInstruction::Jump(99), IrInstruction::Return],
        0,
    );
    let mut vm = ScriptVm::new(&script, default_config()).unwrap();
    let err = vm.execute(&mut NoOpApi).unwrap_err();
    assert_eq!(err, RuntimeError::LabelNotFound(99));
}

#[test]
fn test_nop() {
    let script = make_script(
        vec![
            IrInstruction::Nop,
            IrInstruction::Nop,
            IrInstruction::LoadConst(0, Value::Int(1)),
            IrInstruction::Return,
        ],
        1,
    );
    let mut vm = ScriptVm::new(&script, default_config()).unwrap();
    vm.execute(&mut NoOpApi).unwrap();
    assert_eq!(vm.read_register(0), Some(&Value::Int(1)));
}

#[test]
fn test_return_halts_execution() {
    let script = make_script(
        vec![
            IrInstruction::LoadConst(0, Value::Int(1)),
            IrInstruction::Return,
            IrInstruction::LoadConst(0, Value::Int(2)),
        ],
        1,
    );
    let mut vm = ScriptVm::new(&script, default_config()).unwrap();
    vm.execute(&mut NoOpApi).unwrap();
    assert_eq!(vm.read_register(0), Some(&Value::Int(1)));
}

// --- Call tests ---

#[test]
fn test_call_dispatches_to_api() {
    let script = make_script(
        vec![
            IrInstruction::LoadConst(0, Value::String("hello".into())),
            IrInstruction::Call {
                function: "log".into(),
                args: vec![0],
                result: None,
            },
            IrInstruction::Return,
        ],
        1,
    );
    let mut api = RecordingApi::default();
    let mut vm = ScriptVm::new(&script, default_config()).unwrap();
    vm.execute(&mut api).unwrap();
    assert_eq!(api.calls.len(), 1);
    assert_eq!(api.calls[0].0, "log");
    assert_eq!(api.calls[0].1, vec![Value::String("hello".into())]);
}

#[test]
fn test_call_with_result() {
    let script = make_script(
        vec![
            IrInstruction::Call {
                function: "spawn".into(),
                args: vec![],
                result: Some(0),
            },
            IrInstruction::Return,
        ],
        1,
    );
    let mut vm = ScriptVm::new(&script, default_config()).unwrap();
    vm.execute(&mut NoOpApi).unwrap();
    assert_eq!(vm.read_register(0), Some(&Value::None));
}
