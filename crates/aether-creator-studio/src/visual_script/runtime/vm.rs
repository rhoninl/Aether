//! Register-based virtual machine for executing compiled visual scripts.

use std::collections::HashMap;
use std::env;

use crate::visual_script::compiler::{BinaryOp, CompiledScript, IrInstruction, LabelId};
use crate::visual_script::types::Value;

use super::engine_api::EngineApi;
use super::error::RuntimeError;

/// Default maximum operations per execution.
const DEFAULT_MAX_OPS: u64 = 10_000;
/// Default maximum register file size.
const DEFAULT_MAX_STACK: u32 = 256;
/// Default maximum number of variables.
const DEFAULT_MAX_VARS: usize = 1024;

/// Environment variable name for max operations limit.
const ENV_MAX_OPS: &str = "AETHER_SCRIPT_MAX_OPS";
/// Environment variable name for max stack (register) limit.
const ENV_MAX_STACK: &str = "AETHER_SCRIPT_MAX_STACK";
/// Environment variable name for max variables limit.
const ENV_MAX_VARS: &str = "AETHER_SCRIPT_MAX_VARS";

/// Configuration for the script VM execution limits.
#[derive(Debug, Clone)]
pub struct VmConfig {
    pub max_ops: u64,
    pub max_stack: u32,
    pub max_vars: usize,
}

impl VmConfig {
    /// Load configuration from environment variables, falling back to defaults.
    pub fn from_env() -> Self {
        Self {
            max_ops: env::var(ENV_MAX_OPS)
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(DEFAULT_MAX_OPS),
            max_stack: env::var(ENV_MAX_STACK)
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(DEFAULT_MAX_STACK),
            max_vars: env::var(ENV_MAX_VARS)
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(DEFAULT_MAX_VARS),
        }
    }
}

impl Default for VmConfig {
    fn default() -> Self {
        Self {
            max_ops: DEFAULT_MAX_OPS,
            max_stack: DEFAULT_MAX_STACK,
            max_vars: DEFAULT_MAX_VARS,
        }
    }
}

/// The visual script virtual machine.
///
/// Executes `IrInstruction` sequences from a `CompiledScript` against an
/// `EngineApi` implementation. Maintains register state, variables, and
/// enforces sandboxing limits.
#[derive(Debug)]
pub struct ScriptVm {
    config: VmConfig,
    registers: Vec<Value>,
    variables: HashMap<String, Value>,
    label_map: HashMap<LabelId, usize>,
    instructions: Vec<IrInstruction>,
    pc: usize,
    ops_executed: u64,
    halted: bool,
}

impl ScriptVm {
    /// Create a new VM for the given compiled script.
    pub fn new(script: &CompiledScript, config: VmConfig) -> Result<Self, RuntimeError> {
        if script.register_count > config.max_stack {
            return Err(RuntimeError::StackOverflow {
                requested: script.register_count,
                limit: config.max_stack,
            });
        }

        let registers = vec![Value::None; script.register_count as usize];

        // Build label map by scanning instructions.
        let label_map = build_label_map(&script.instructions);

        Ok(Self {
            config,
            registers,
            variables: HashMap::new(),
            label_map,
            instructions: script.instructions.clone(),
            pc: 0,
            ops_executed: 0,
            halted: false,
        })
    }

    /// Hot-reload: replace the current script with a new one, resetting
    /// execution state but preserving variables.
    pub fn reload(&mut self, script: &CompiledScript) -> Result<(), RuntimeError> {
        if script.register_count > self.config.max_stack {
            return Err(RuntimeError::StackOverflow {
                requested: script.register_count,
                limit: self.config.max_stack,
            });
        }

        self.registers = vec![Value::None; script.register_count as usize];
        self.label_map = build_label_map(&script.instructions);
        self.instructions = script.instructions.clone();
        self.pc = 0;
        self.ops_executed = 0;
        self.halted = false;
        // Variables are intentionally preserved across reloads.
        Ok(())
    }

    /// Execute the loaded script to completion (or until a limit is hit).
    pub fn execute(&mut self, api: &mut dyn EngineApi) -> Result<(), RuntimeError> {
        self.pc = 0;
        self.ops_executed = 0;
        self.halted = false;

        while self.pc < self.instructions.len() && !self.halted {
            self.ops_executed += 1;
            if self.ops_executed > self.config.max_ops {
                return Err(RuntimeError::ExecutionLimitExceeded {
                    limit: self.config.max_ops,
                });
            }

            let instr = self.instructions[self.pc].clone();
            self.step_instruction(&instr, api)?;
        }

        Ok(())
    }

    /// Execute a single instruction and advance the program counter.
    fn step_instruction(
        &mut self,
        instr: &IrInstruction,
        api: &mut dyn EngineApi,
    ) -> Result<(), RuntimeError> {
        match instr {
            IrInstruction::LoadConst(reg, val) => {
                self.set_register(*reg, val.clone())?;
                self.pc += 1;
            }

            IrInstruction::BinaryOp { op, dest, lhs, rhs } => {
                let left = self.get_register(*lhs)?;
                let right = self.get_register(*rhs)?;
                let result = execute_binary_op(*op, &left, &right)?;
                self.set_register(*dest, result)?;
                self.pc += 1;
            }

            IrInstruction::Not { dest, src } => {
                let val = self.get_register(*src)?;
                let result = execute_not(&val)?;
                self.set_register(*dest, result)?;
                self.pc += 1;
            }

            IrInstruction::Branch {
                condition,
                true_label,
                false_label,
            } => {
                let cond = self.get_register(*condition)?;
                let is_truthy = value_is_truthy(&cond);
                let target = if is_truthy { *true_label } else { *false_label };
                let target_pc = self
                    .label_map
                    .get(&target)
                    .ok_or(RuntimeError::LabelNotFound(target))?;
                self.pc = *target_pc;
            }

            IrInstruction::Jump(label) => {
                let target_pc = self
                    .label_map
                    .get(label)
                    .ok_or(RuntimeError::LabelNotFound(*label))?;
                self.pc = *target_pc;
            }

            IrInstruction::Label(_) => {
                // Labels are no-ops at runtime; just advance.
                self.pc += 1;
            }

            IrInstruction::Call {
                function,
                args,
                result,
            } => {
                let arg_values: Vec<Value> = args
                    .iter()
                    .map(|r| self.get_register(*r))
                    .collect::<Result<_, _>>()?;

                let ret = self.handle_call(function, &arg_values, api)?;

                if let Some(dest) = result {
                    self.set_register(*dest, ret)?;
                }
                self.pc += 1;
            }

            IrInstruction::Nop => {
                self.pc += 1;
            }

            IrInstruction::Return => {
                self.halted = true;
            }
        }

        Ok(())
    }

    /// Handle a function call, dispatching built-ins internally and
    /// forwarding unknown functions to the engine API.
    fn handle_call(
        &mut self,
        function: &str,
        args: &[Value],
        api: &mut dyn EngineApi,
    ) -> Result<Value, RuntimeError> {
        match function {
            "get_variable" => {
                // get_variable expects no args; the var name is implicit.
                // For the IR, we can't know the name here, so return None.
                // A more complete implementation would encode the name in the IR.
                Ok(Value::None)
            }

            "set_variable" => {
                // args[0] = var name (String), args[1] = value
                if args.len() >= 2 {
                    if let Value::String(name) = &args[0] {
                        if self.variables.len() >= self.config.max_vars
                            && !self.variables.contains_key(name)
                        {
                            return Err(RuntimeError::VariableLimitExceeded {
                                limit: self.config.max_vars,
                            });
                        }
                        self.variables.insert(name.clone(), args[1].clone());
                    }
                }
                Ok(Value::None)
            }

            "clamp" => {
                // args: [value, min, max]
                if args.len() >= 3 {
                    let val = value_to_f32(&args[0])?;
                    let min = value_to_f32(&args[1])?;
                    let max = value_to_f32(&args[2])?;
                    Ok(Value::Float(val.clamp(min, max)))
                } else {
                    Ok(Value::Float(0.0))
                }
            }

            "lerp" => {
                // args: [a, b, t]
                if args.len() >= 3 {
                    let a = value_to_f32(&args[0])?;
                    let b = value_to_f32(&args[1])?;
                    let t = value_to_f32(&args[2])?;
                    Ok(Value::Float(a + (b - a) * t))
                } else {
                    Ok(Value::Float(0.0))
                }
            }

            // All other functions are dispatched to the engine API.
            _ => api.call(function, args),
        }
    }

    /// Get the value in a register.
    fn get_register(&self, index: u32) -> Result<Value, RuntimeError> {
        self.registers
            .get(index as usize)
            .cloned()
            .ok_or(RuntimeError::RegisterOutOfBounds {
                index,
                count: self.registers.len() as u32,
            })
    }

    /// Set the value in a register.
    fn set_register(&mut self, index: u32, value: Value) -> Result<(), RuntimeError> {
        if (index as usize) >= self.registers.len() {
            return Err(RuntimeError::RegisterOutOfBounds {
                index,
                count: self.registers.len() as u32,
            });
        }
        self.registers[index as usize] = value;
        Ok(())
    }

    /// Read the current value of a variable.
    pub fn get_variable(&self, name: &str) -> Option<&Value> {
        self.variables.get(name)
    }

    /// Set a variable directly (for pre-loading state before execution).
    pub fn set_variable(&mut self, name: String, value: Value) -> Result<(), RuntimeError> {
        if self.variables.len() >= self.config.max_vars
            && !self.variables.contains_key(&name)
        {
            return Err(RuntimeError::VariableLimitExceeded {
                limit: self.config.max_vars,
            });
        }
        self.variables.insert(name, value);
        Ok(())
    }

    /// Get the number of operations executed in the last run.
    pub fn ops_executed(&self) -> u64 {
        self.ops_executed
    }

    /// Read a register value (for testing / inspection).
    pub fn read_register(&self, index: u32) -> Option<&Value> {
        self.registers.get(index as usize)
    }
}

/// Build a mapping from label ID to instruction index.
fn build_label_map(instructions: &[IrInstruction]) -> HashMap<LabelId, usize> {
    let mut map = HashMap::new();
    for (i, instr) in instructions.iter().enumerate() {
        if let IrInstruction::Label(id) = instr {
            map.insert(*id, i);
        }
    }
    map
}

/// Determine if a value is truthy.
fn value_is_truthy(val: &Value) -> bool {
    match val {
        Value::None => false,
        Value::Bool(b) => *b,
        Value::Int(n) => *n != 0,
        Value::Float(f) => *f != 0.0,
        Value::String(s) => !s.is_empty(),
        Value::Vec3 { x, y, z } => *x != 0.0 || *y != 0.0 || *z != 0.0,
        Value::Entity(id) => *id != 0,
    }
}

/// Convert a Value to f32 for arithmetic.
fn value_to_f32(val: &Value) -> Result<f32, RuntimeError> {
    match val {
        Value::Float(f) => Ok(*f),
        Value::Int(n) => Ok(*n as f32),
        _ => Err(RuntimeError::TypeError {
            expected: "Float or Int".into(),
            got: format!("{val:?}"),
        }),
    }
}

/// Execute a binary operation on two values.
fn execute_binary_op(op: BinaryOp, lhs: &Value, rhs: &Value) -> Result<Value, RuntimeError> {
    match op {
        BinaryOp::Add => numeric_op(lhs, rhs, |a, b| a + b, |a, b| a + b),
        BinaryOp::Subtract => numeric_op(lhs, rhs, |a, b| a - b, |a, b| a - b),
        BinaryOp::Multiply => numeric_op(lhs, rhs, |a, b| a * b, |a, b| a * b),
        BinaryOp::Divide => {
            // Check for division by zero.
            match rhs {
                Value::Int(0) => return Err(RuntimeError::DivisionByZero),
                Value::Float(f) if *f == 0.0 => return Err(RuntimeError::DivisionByZero),
                _ => {}
            }
            numeric_op(lhs, rhs, |a, b| a / b, |a, b| a / b)
        }
        BinaryOp::Equal => Ok(Value::Bool(values_equal(lhs, rhs))),
        BinaryOp::NotEqual => Ok(Value::Bool(!values_equal(lhs, rhs))),
        BinaryOp::Greater => comparison_op(lhs, rhs, |a, b| a > b, |a, b| a > b),
        BinaryOp::Less => comparison_op(lhs, rhs, |a, b| a < b, |a, b| a < b),
        BinaryOp::And => Ok(Value::Bool(value_is_truthy(lhs) && value_is_truthy(rhs))),
        BinaryOp::Or => Ok(Value::Bool(value_is_truthy(lhs) || value_is_truthy(rhs))),
    }
}

/// Perform a numeric operation, handling Int and Float types.
fn numeric_op(
    lhs: &Value,
    rhs: &Value,
    int_op: fn(i32, i32) -> i32,
    float_op: fn(f32, f32) -> f32,
) -> Result<Value, RuntimeError> {
    match (lhs, rhs) {
        (Value::Int(a), Value::Int(b)) => Ok(Value::Int(int_op(*a, *b))),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Float(float_op(*a, *b))),
        (Value::Int(a), Value::Float(b)) => Ok(Value::Float(float_op(*a as f32, *b))),
        (Value::Float(a), Value::Int(b)) => Ok(Value::Float(float_op(*a, *b as f32))),
        _ => Err(RuntimeError::TypeError {
            expected: "numeric".into(),
            got: format!("{lhs:?}, {rhs:?}"),
        }),
    }
}

/// Perform a comparison operation.
fn comparison_op(
    lhs: &Value,
    rhs: &Value,
    int_cmp: fn(i32, i32) -> bool,
    float_cmp: fn(f32, f32) -> bool,
) -> Result<Value, RuntimeError> {
    match (lhs, rhs) {
        (Value::Int(a), Value::Int(b)) => Ok(Value::Bool(int_cmp(*a, *b))),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Bool(float_cmp(*a, *b))),
        (Value::Int(a), Value::Float(b)) => Ok(Value::Bool(float_cmp(*a as f32, *b))),
        (Value::Float(a), Value::Int(b)) => Ok(Value::Bool(float_cmp(*a, *b as f32))),
        _ => Err(RuntimeError::TypeError {
            expected: "numeric".into(),
            got: format!("{lhs:?}, {rhs:?}"),
        }),
    }
}

/// Check equality between two values.
fn values_equal(lhs: &Value, rhs: &Value) -> bool {
    match (lhs, rhs) {
        (Value::None, Value::None) => true,
        (Value::Bool(a), Value::Bool(b)) => a == b,
        (Value::Int(a), Value::Int(b)) => a == b,
        (Value::Float(a), Value::Float(b)) => a == b,
        (Value::Int(a), Value::Float(b)) => (*a as f32) == *b,
        (Value::Float(a), Value::Int(b)) => *a == (*b as f32),
        (Value::String(a), Value::String(b)) => a == b,
        (Value::Entity(a), Value::Entity(b)) => a == b,
        (Value::Vec3 { x: x1, y: y1, z: z1 }, Value::Vec3 { x: x2, y: y2, z: z2 }) => {
            x1 == x2 && y1 == y2 && z1 == z2
        }
        _ => false,
    }
}

/// Execute a unary NOT operation.
fn execute_not(val: &Value) -> Result<Value, RuntimeError> {
    Ok(Value::Bool(!value_is_truthy(val)))
}

#[cfg(test)]
#[path = "vm_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "vm_integration_tests.rs"]
mod integration_tests;
