//! Compilation pipeline: node graph -> IR instructions -> WASM bytecode.

use std::collections::HashMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use super::graph::NodeGraph;
use super::node::{NodeId, NodeKind};
use super::types::Value;
use super::validation::{topological_sort_flow, validate_graph};

/// Register index for the IR virtual machine.
pub type Register = u32;

/// Starting register for user allocations.
const REGISTER_BASE: Register = 0;

/// A label identifier in the IR.
pub type LabelId = u32;

/// Binary operation kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinaryOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    Equal,
    NotEqual,
    Greater,
    Less,
    And,
    Or,
}

/// An intermediate representation instruction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum IrInstruction {
    /// Load a constant value into a register.
    LoadConst(Register, Value),
    /// Perform a binary operation: dest = lhs op rhs.
    BinaryOp {
        op: BinaryOp,
        dest: Register,
        lhs: Register,
        rhs: Register,
    },
    /// Unary NOT: dest = !src.
    Not {
        dest: Register,
        src: Register,
    },
    /// Conditional branch.
    Branch {
        condition: Register,
        true_label: LabelId,
        false_label: LabelId,
    },
    /// Unconditional jump.
    Jump(LabelId),
    /// A jump target label.
    Label(LabelId),
    /// Call a built-in action function.
    Call {
        function: String,
        args: Vec<Register>,
        result: Option<Register>,
    },
    /// Nop (placeholder).
    Nop,
    /// Return from the current function.
    Return,
}

impl fmt::Display for IrInstruction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IrInstruction::LoadConst(reg, val) => write!(f, "r{reg} = {val}"),
            IrInstruction::BinaryOp { op, dest, lhs, rhs } => {
                write!(f, "r{dest} = r{lhs} {op:?} r{rhs}")
            }
            IrInstruction::Not { dest, src } => write!(f, "r{dest} = !r{src}"),
            IrInstruction::Branch {
                condition,
                true_label,
                false_label,
            } => write!(f, "branch r{condition} ? L{true_label} : L{false_label}"),
            IrInstruction::Jump(label) => write!(f, "jump L{label}"),
            IrInstruction::Label(id) => write!(f, "L{id}:"),
            IrInstruction::Call {
                function,
                args,
                result,
            } => {
                let args_str: Vec<String> = args.iter().map(|r| format!("r{r}")).collect();
                if let Some(res) = result {
                    write!(f, "r{res} = call {function}({})", args_str.join(", "))
                } else {
                    write!(f, "call {function}({})", args_str.join(", "))
                }
            }
            IrInstruction::Nop => write!(f, "nop"),
            IrInstruction::Return => write!(f, "return"),
        }
    }
}

/// Compiled output of the node graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledScript {
    /// The IR instructions.
    pub instructions: Vec<IrInstruction>,
    /// Mapping from node ID to the first instruction index for that node.
    pub node_instruction_map: HashMap<NodeId, usize>,
    /// Total registers used.
    pub register_count: u32,
    /// The WASM bytecode (simplified representation).
    pub wasm_bytes: Vec<u8>,
}

/// Errors from compilation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompileError {
    ValidationFailed(Vec<String>),
    CycleDetected,
    EmptyGraph,
    InternalError(String),
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CompileError::ValidationFailed(errs) => {
                write!(f, "validation failed: {}", errs.join("; "))
            }
            CompileError::CycleDetected => write!(f, "cycle detected in flow graph"),
            CompileError::EmptyGraph => write!(f, "graph is empty"),
            CompileError::InternalError(msg) => write!(f, "internal error: {msg}"),
        }
    }
}

impl std::error::Error for CompileError {}

/// State used during IR generation.
struct IrGenerator {
    instructions: Vec<IrInstruction>,
    node_instruction_map: HashMap<NodeId, usize>,
    next_register: Register,
    next_label: LabelId,
    /// Maps (node_id, output_port_name) -> register holding that value.
    output_registers: HashMap<(NodeId, String), Register>,
}

impl IrGenerator {
    fn new() -> Self {
        Self {
            instructions: Vec::new(),
            node_instruction_map: HashMap::new(),
            next_register: REGISTER_BASE,
            next_label: 0,
            output_registers: HashMap::new(),
        }
    }

    fn alloc_register(&mut self) -> Register {
        let r = self.next_register;
        self.next_register += 1;
        r
    }

    fn alloc_label(&mut self) -> LabelId {
        let l = self.next_label;
        self.next_label += 1;
        l
    }

    fn emit(&mut self, instr: IrInstruction) {
        self.instructions.push(instr);
    }

    fn current_offset(&self) -> usize {
        self.instructions.len()
    }

    /// Look up the register that holds the output of a connected input port.
    /// Returns the register if a connection was found and compiled, or None.
    fn resolve_input(
        &self,
        graph: &NodeGraph,
        node_id: NodeId,
        port_name: &str,
    ) -> Option<Register> {
        let node = graph.get_node(node_id)?;
        let port = node.find_input(port_name)?;
        let conns = graph.connections_to_port(port.id);
        let conn = conns.first()?;
        let from_node = graph.get_node(conn.from_node)?;
        let from_port = from_node.find_port(conn.from_port)?;
        self.output_registers
            .get(&(conn.from_node, from_port.name.clone()))
            .copied()
    }
}

/// Compile a node graph into IR instructions and WASM bytecode.
pub fn compile(graph: &NodeGraph) -> Result<CompiledScript, CompileError> {
    if graph.node_count() == 0 {
        return Err(CompileError::EmptyGraph);
    }

    // Validate first
    let validation = validate_graph(graph);
    if !validation.is_valid() {
        let errors: Vec<String> = validation
            .errors()
            .iter()
            .map(|d| d.message.clone())
            .collect();
        return Err(CompileError::ValidationFailed(errors));
    }

    // Topological sort
    let sorted = topological_sort_flow(graph).map_err(|_| CompileError::CycleDetected)?;

    let mut gen = IrGenerator::new();

    // First pass: compile pure (data-only) nodes that feed into the flow
    for node in graph.nodes() {
        if node.kind.is_pure() {
            compile_pure_node(graph, node.id, &mut gen);
        }
    }

    // Second pass: compile flow nodes in topological order
    for &node_id in &sorted {
        let node = graph
            .get_node(node_id)
            .ok_or_else(|| CompileError::InternalError("node disappeared".into()))?;

        gen.node_instruction_map
            .insert(node_id, gen.current_offset());

        compile_flow_node(graph, node_id, &node.kind.clone(), &mut gen);
    }

    // Emit final return
    gen.emit(IrInstruction::Return);

    let register_count = gen.next_register;
    let wasm_bytes = generate_wasm_stub(&gen.instructions);

    Ok(CompiledScript {
        instructions: gen.instructions,
        node_instruction_map: gen.node_instruction_map,
        register_count,
        wasm_bytes,
    })
}

/// Compile a pure (data-only) node.
fn compile_pure_node(graph: &NodeGraph, node_id: NodeId, gen: &mut IrGenerator) {
    let node = match graph.get_node(node_id) {
        Some(n) => n,
        None => return,
    };

    match &node.kind {
        NodeKind::Add | NodeKind::Subtract | NodeKind::Multiply | NodeKind::Divide => {
            let a_reg = gen
                .resolve_input(graph, node_id, "a")
                .unwrap_or_else(|| {
                    let r = gen.alloc_register();
                    gen.emit(IrInstruction::LoadConst(r, Value::Float(0.0)));
                    r
                });
            let b_reg = gen
                .resolve_input(graph, node_id, "b")
                .unwrap_or_else(|| {
                    let r = gen.alloc_register();
                    gen.emit(IrInstruction::LoadConst(r, Value::Float(0.0)));
                    r
                });
            let dest = gen.alloc_register();

            let op = match &node.kind {
                NodeKind::Add => BinaryOp::Add,
                NodeKind::Subtract => BinaryOp::Subtract,
                NodeKind::Multiply => BinaryOp::Multiply,
                NodeKind::Divide => BinaryOp::Divide,
                _ => unreachable!(),
            };

            gen.emit(IrInstruction::BinaryOp {
                op,
                dest,
                lhs: a_reg,
                rhs: b_reg,
            });
            gen.output_registers
                .insert((node_id, "result".into()), dest);
        }

        NodeKind::Equal | NodeKind::NotEqual | NodeKind::Greater | NodeKind::Less => {
            let a_reg = gen
                .resolve_input(graph, node_id, "a")
                .unwrap_or_else(|| {
                    let r = gen.alloc_register();
                    gen.emit(IrInstruction::LoadConst(r, Value::Float(0.0)));
                    r
                });
            let b_reg = gen
                .resolve_input(graph, node_id, "b")
                .unwrap_or_else(|| {
                    let r = gen.alloc_register();
                    gen.emit(IrInstruction::LoadConst(r, Value::Float(0.0)));
                    r
                });
            let dest = gen.alloc_register();

            let op = match &node.kind {
                NodeKind::Equal => BinaryOp::Equal,
                NodeKind::NotEqual => BinaryOp::NotEqual,
                NodeKind::Greater => BinaryOp::Greater,
                NodeKind::Less => BinaryOp::Less,
                _ => unreachable!(),
            };

            gen.emit(IrInstruction::BinaryOp {
                op,
                dest,
                lhs: a_reg,
                rhs: b_reg,
            });
            gen.output_registers
                .insert((node_id, "result".into()), dest);
        }

        NodeKind::And | NodeKind::Or => {
            let a_reg = gen
                .resolve_input(graph, node_id, "a")
                .unwrap_or_else(|| {
                    let r = gen.alloc_register();
                    gen.emit(IrInstruction::LoadConst(r, Value::Bool(false)));
                    r
                });
            let b_reg = gen
                .resolve_input(graph, node_id, "b")
                .unwrap_or_else(|| {
                    let r = gen.alloc_register();
                    gen.emit(IrInstruction::LoadConst(r, Value::Bool(false)));
                    r
                });
            let dest = gen.alloc_register();

            let op = match &node.kind {
                NodeKind::And => BinaryOp::And,
                NodeKind::Or => BinaryOp::Or,
                _ => unreachable!(),
            };

            gen.emit(IrInstruction::BinaryOp {
                op,
                dest,
                lhs: a_reg,
                rhs: b_reg,
            });
            gen.output_registers
                .insert((node_id, "result".into()), dest);
        }

        NodeKind::Not => {
            let src_reg = gen
                .resolve_input(graph, node_id, "value")
                .unwrap_or_else(|| {
                    let r = gen.alloc_register();
                    gen.emit(IrInstruction::LoadConst(r, Value::Bool(false)));
                    r
                });
            let dest = gen.alloc_register();
            gen.emit(IrInstruction::Not {
                dest,
                src: src_reg,
            });
            gen.output_registers
                .insert((node_id, "result".into()), dest);
        }

        NodeKind::GetVariable { var_name } => {
            let dest = gen.alloc_register();
            gen.emit(IrInstruction::Call {
                function: "get_variable".into(),
                args: vec![],
                result: Some(dest),
            });
            // The variable name is baked into the call semantics
            let _name = var_name.clone();
            gen.output_registers
                .insert((node_id, "value".into()), dest);
        }

        NodeKind::Clamp => {
            let val_reg = gen
                .resolve_input(graph, node_id, "value")
                .unwrap_or_else(|| {
                    let r = gen.alloc_register();
                    gen.emit(IrInstruction::LoadConst(r, Value::Float(0.0)));
                    r
                });
            let min_reg = gen
                .resolve_input(graph, node_id, "min")
                .unwrap_or_else(|| {
                    let r = gen.alloc_register();
                    gen.emit(IrInstruction::LoadConst(r, Value::Float(0.0)));
                    r
                });
            let max_reg = gen
                .resolve_input(graph, node_id, "max")
                .unwrap_or_else(|| {
                    let r = gen.alloc_register();
                    gen.emit(IrInstruction::LoadConst(r, Value::Float(1.0)));
                    r
                });
            let dest = gen.alloc_register();
            gen.emit(IrInstruction::Call {
                function: "clamp".into(),
                args: vec![val_reg, min_reg, max_reg],
                result: Some(dest),
            });
            gen.output_registers
                .insert((node_id, "result".into()), dest);
        }

        NodeKind::Lerp => {
            let a_reg = gen
                .resolve_input(graph, node_id, "a")
                .unwrap_or_else(|| {
                    let r = gen.alloc_register();
                    gen.emit(IrInstruction::LoadConst(r, Value::Float(0.0)));
                    r
                });
            let b_reg = gen
                .resolve_input(graph, node_id, "b")
                .unwrap_or_else(|| {
                    let r = gen.alloc_register();
                    gen.emit(IrInstruction::LoadConst(r, Value::Float(1.0)));
                    r
                });
            let t_reg = gen
                .resolve_input(graph, node_id, "t")
                .unwrap_or_else(|| {
                    let r = gen.alloc_register();
                    gen.emit(IrInstruction::LoadConst(r, Value::Float(0.5)));
                    r
                });
            let dest = gen.alloc_register();
            gen.emit(IrInstruction::Call {
                function: "lerp".into(),
                args: vec![a_reg, b_reg, t_reg],
                result: Some(dest),
            });
            gen.output_registers
                .insert((node_id, "result".into()), dest);
        }

        NodeKind::RandomRange => {
            let min_reg = gen
                .resolve_input(graph, node_id, "min")
                .unwrap_or_else(|| {
                    let r = gen.alloc_register();
                    gen.emit(IrInstruction::LoadConst(r, Value::Float(0.0)));
                    r
                });
            let max_reg = gen
                .resolve_input(graph, node_id, "max")
                .unwrap_or_else(|| {
                    let r = gen.alloc_register();
                    gen.emit(IrInstruction::LoadConst(r, Value::Float(1.0)));
                    r
                });
            let dest = gen.alloc_register();
            gen.emit(IrInstruction::Call {
                function: "random_range".into(),
                args: vec![min_reg, max_reg],
                result: Some(dest),
            });
            gen.output_registers
                .insert((node_id, "result".into()), dest);
        }

        _ => {} // Other pure nodes not yet handled
    }
}

/// Compile a flow (impure) node.
fn compile_flow_node(
    graph: &NodeGraph,
    node_id: NodeId,
    kind: &NodeKind,
    gen: &mut IrGenerator,
) {
    match kind {
        // Event nodes just emit a label (entry point)
        NodeKind::OnInteract
        | NodeKind::OnEnter
        | NodeKind::OnExit
        | NodeKind::OnTimer { .. }
        | NodeKind::OnStart
        | NodeKind::OnCollision => {
            gen.emit(IrInstruction::Nop); // Event entry point marker
        }

        NodeKind::Branch => {
            let cond_reg = gen
                .resolve_input(graph, node_id, "condition")
                .unwrap_or_else(|| {
                    let r = gen.alloc_register();
                    gen.emit(IrInstruction::LoadConst(r, Value::Bool(false)));
                    r
                });
            let true_label = gen.alloc_label();
            let false_label = gen.alloc_label();

            gen.emit(IrInstruction::Branch {
                condition: cond_reg,
                true_label,
                false_label,
            });
            gen.emit(IrInstruction::Label(true_label));
            gen.emit(IrInstruction::Label(false_label));
        }

        NodeKind::Delay { delay_ms } => {
            let delay_reg = gen.alloc_register();
            gen.emit(IrInstruction::LoadConst(
                delay_reg,
                Value::Int(*delay_ms as i32),
            ));
            gen.emit(IrInstruction::Call {
                function: "delay".into(),
                args: vec![delay_reg],
                result: None,
            });
        }

        NodeKind::Log => {
            let msg_reg = gen
                .resolve_input(graph, node_id, "message")
                .unwrap_or_else(|| {
                    let r = gen.alloc_register();
                    gen.emit(IrInstruction::LoadConst(
                        r,
                        Value::String("<empty>".into()),
                    ));
                    r
                });
            gen.emit(IrInstruction::Call {
                function: "log".into(),
                args: vec![msg_reg],
                result: None,
            });
        }

        NodeKind::SetPosition => {
            let entity_reg = gen.resolve_input(graph, node_id, "entity").unwrap_or_else(|| {
                let r = gen.alloc_register();
                gen.emit(IrInstruction::LoadConst(r, Value::Entity(0)));
                r
            });
            let pos_reg = gen.resolve_input(graph, node_id, "position").unwrap_or_else(|| {
                let r = gen.alloc_register();
                gen.emit(IrInstruction::LoadConst(
                    r,
                    Value::Vec3 {
                        x: 0.0,
                        y: 0.0,
                        z: 0.0,
                    },
                ));
                r
            });
            gen.emit(IrInstruction::Call {
                function: "set_position".into(),
                args: vec![entity_reg, pos_reg],
                result: None,
            });
        }

        NodeKind::SetRotation => {
            let entity_reg = gen.resolve_input(graph, node_id, "entity").unwrap_or_else(|| {
                let r = gen.alloc_register();
                gen.emit(IrInstruction::LoadConst(r, Value::Entity(0)));
                r
            });
            let rot_reg = gen.resolve_input(graph, node_id, "rotation").unwrap_or_else(|| {
                let r = gen.alloc_register();
                gen.emit(IrInstruction::LoadConst(
                    r,
                    Value::Vec3 {
                        x: 0.0,
                        y: 0.0,
                        z: 0.0,
                    },
                ));
                r
            });
            gen.emit(IrInstruction::Call {
                function: "set_rotation".into(),
                args: vec![entity_reg, rot_reg],
                result: None,
            });
        }

        NodeKind::PlayAnimation => {
            let entity_reg = gen.resolve_input(graph, node_id, "entity").unwrap_or_else(|| {
                let r = gen.alloc_register();
                gen.emit(IrInstruction::LoadConst(r, Value::Entity(0)));
                r
            });
            let anim_reg = gen.resolve_input(graph, node_id, "animation").unwrap_or_else(|| {
                let r = gen.alloc_register();
                gen.emit(IrInstruction::LoadConst(r, Value::String("idle".into())));
                r
            });
            gen.emit(IrInstruction::Call {
                function: "play_animation".into(),
                args: vec![entity_reg, anim_reg],
                result: None,
            });
        }

        NodeKind::PlaySound => {
            let sound_reg = gen.resolve_input(graph, node_id, "sound").unwrap_or_else(|| {
                let r = gen.alloc_register();
                gen.emit(IrInstruction::LoadConst(r, Value::String(String::new())));
                r
            });
            gen.emit(IrInstruction::Call {
                function: "play_sound".into(),
                args: vec![sound_reg],
                result: None,
            });
        }

        NodeKind::SpawnEntity => {
            let template_reg = gen.resolve_input(graph, node_id, "template").unwrap_or_else(|| {
                let r = gen.alloc_register();
                gen.emit(IrInstruction::LoadConst(r, Value::String(String::new())));
                r
            });
            let pos_reg = gen.resolve_input(graph, node_id, "position").unwrap_or_else(|| {
                let r = gen.alloc_register();
                gen.emit(IrInstruction::LoadConst(
                    r,
                    Value::Vec3 {
                        x: 0.0,
                        y: 0.0,
                        z: 0.0,
                    },
                ));
                r
            });
            let result_reg = gen.alloc_register();
            gen.emit(IrInstruction::Call {
                function: "spawn_entity".into(),
                args: vec![template_reg, pos_reg],
                result: Some(result_reg),
            });
            gen.output_registers
                .insert((node_id, "spawned".into()), result_reg);
        }

        NodeKind::DestroyEntity => {
            let entity_reg = gen.resolve_input(graph, node_id, "entity").unwrap_or_else(|| {
                let r = gen.alloc_register();
                gen.emit(IrInstruction::LoadConst(r, Value::Entity(0)));
                r
            });
            gen.emit(IrInstruction::Call {
                function: "destroy_entity".into(),
                args: vec![entity_reg],
                result: None,
            });
        }

        NodeKind::SendMessage => {
            let channel_reg = gen.resolve_input(graph, node_id, "channel").unwrap_or_else(|| {
                let r = gen.alloc_register();
                gen.emit(IrInstruction::LoadConst(r, Value::String(String::new())));
                r
            });
            let msg_reg = gen.resolve_input(graph, node_id, "message").unwrap_or_else(|| {
                let r = gen.alloc_register();
                gen.emit(IrInstruction::LoadConst(r, Value::String(String::new())));
                r
            });
            gen.emit(IrInstruction::Call {
                function: "send_message".into(),
                args: vec![channel_reg, msg_reg],
                result: None,
            });
        }

        NodeKind::SetVariable { var_name } => {
            let val_reg = gen
                .resolve_input(graph, node_id, "value")
                .unwrap_or_else(|| {
                    let r = gen.alloc_register();
                    gen.emit(IrInstruction::LoadConst(r, Value::None));
                    r
                });
            let name_reg = gen.alloc_register();
            gen.emit(IrInstruction::LoadConst(
                name_reg,
                Value::String(var_name.clone()),
            ));
            gen.emit(IrInstruction::Call {
                function: "set_variable".into(),
                args: vec![name_reg, val_reg],
                result: None,
            });
        }

        NodeKind::ForLoop => {
            let start_reg = gen.resolve_input(graph, node_id, "start").unwrap_or_else(|| {
                let r = gen.alloc_register();
                gen.emit(IrInstruction::LoadConst(r, Value::Int(0)));
                r
            });
            let end_reg = gen.resolve_input(graph, node_id, "end").unwrap_or_else(|| {
                let r = gen.alloc_register();
                gen.emit(IrInstruction::LoadConst(r, Value::Int(10)));
                r
            });
            let body_label = gen.alloc_label();
            let done_label = gen.alloc_label();
            let index_reg = gen.alloc_register();

            gen.emit(IrInstruction::LoadConst(index_reg, Value::Int(0)));
            gen.emit(IrInstruction::Label(body_label));
            gen.output_registers
                .insert((node_id, "index".into()), index_reg);

            // Emit comparison
            let cmp_reg = gen.alloc_register();
            gen.emit(IrInstruction::BinaryOp {
                op: BinaryOp::Less,
                dest: cmp_reg,
                lhs: index_reg,
                rhs: end_reg,
            });
            gen.emit(IrInstruction::Branch {
                condition: cmp_reg,
                true_label: body_label,
                false_label: done_label,
            });
            gen.emit(IrInstruction::Label(done_label));

            let _ = start_reg; // used for initialization in a full implementation
        }

        NodeKind::Sequence { output_count } => {
            // Just emit a nop; in a real implementation, each output would be a separate
            // execution path
            for _ in 0..*output_count {
                gen.emit(IrInstruction::Nop);
            }
        }

        // Pure nodes are handled separately
        _ => {}
    }
}

/// Generate a minimal WASM module stub from IR instructions.
///
/// This produces a valid-ish WASM magic number and version header followed by
/// a custom section containing the instruction count. A real implementation
/// would produce actual WASM bytecode.
fn generate_wasm_stub(instructions: &[IrInstruction]) -> Vec<u8> {
    // WASM magic number: \0asm
    let mut bytes = vec![0x00, 0x61, 0x73, 0x6D];
    // WASM version 1
    bytes.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]);
    // Custom section (section id = 0)
    bytes.push(0x00);
    // Section name: "aether"
    let name = b"aether";
    let instr_count = instructions.len() as u32;
    let payload_len = 1 + name.len() + 4; // name length byte + name + u32
    bytes.push(payload_len as u8);
    bytes.push(name.len() as u8);
    bytes.extend_from_slice(name);
    bytes.extend_from_slice(&instr_count.to_le_bytes());

    bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::visual_script::graph::NodeGraph;
    use crate::visual_script::node::NodeKind;

    fn simple_event_log_graph() -> NodeGraph {
        let mut g = NodeGraph::new("test", "Test");
        let event = g.add_node(NodeKind::OnStart).unwrap();
        let log = g.add_node(NodeKind::Log).unwrap();

        let ev_exec = g.get_node(event).unwrap().find_output("exec").unwrap().id;
        let log_in = g.get_node(log).unwrap().find_input("exec").unwrap().id;
        g.connect(event, ev_exec, log, log_in).unwrap();

        g
    }

    #[test]
    fn test_compile_simple_graph() {
        let g = simple_event_log_graph();
        let result = compile(&g).unwrap();
        assert!(!result.instructions.is_empty());
        assert!(result.register_count > 0);
    }

    #[test]
    fn test_compile_empty_graph() {
        let g = NodeGraph::new("test", "Empty");
        let result = compile(&g);
        assert_eq!(result.unwrap_err(), CompileError::EmptyGraph);
    }

    #[test]
    fn test_compile_no_events() {
        let mut g = NodeGraph::new("test", "No Events");
        g.add_node(NodeKind::Log).unwrap();
        let result = compile(&g);
        assert!(matches!(result, Err(CompileError::ValidationFailed(_))));
    }

    #[test]
    fn test_compile_has_return() {
        let g = simple_event_log_graph();
        let result = compile(&g).unwrap();
        assert_eq!(
            *result.instructions.last().unwrap(),
            IrInstruction::Return
        );
    }

    #[test]
    fn test_compile_node_instruction_map() {
        let g = simple_event_log_graph();
        let result = compile(&g).unwrap();
        // All flow nodes should be in the map
        assert!(!result.node_instruction_map.is_empty());
    }

    #[test]
    fn test_compile_with_branch() {
        let mut g = NodeGraph::new("test", "Branch");
        let event = g.add_node(NodeKind::OnStart).unwrap();
        let branch = g.add_node(NodeKind::Branch).unwrap();
        let log = g.add_node(NodeKind::Log).unwrap();

        let ev_exec = g.get_node(event).unwrap().find_output("exec").unwrap().id;
        let br_in = g.get_node(branch).unwrap().find_input("exec").unwrap().id;
        let br_true = g.get_node(branch).unwrap().find_output("true").unwrap().id;
        let log_in = g.get_node(log).unwrap().find_input("exec").unwrap().id;

        g.connect(event, ev_exec, branch, br_in).unwrap();
        g.connect(branch, br_true, log, log_in).unwrap();

        let result = compile(&g).unwrap();
        assert!(!result.instructions.is_empty());

        // Should contain a Branch instruction
        let has_branch = result
            .instructions
            .iter()
            .any(|i| matches!(i, IrInstruction::Branch { .. }));
        assert!(has_branch);
    }

    #[test]
    fn test_compile_with_math() {
        let mut g = NodeGraph::new("test", "Math");
        let event = g.add_node(NodeKind::OnStart).unwrap();
        let add = g.add_node(NodeKind::Add).unwrap();
        let log = g.add_node(NodeKind::Log).unwrap();

        let ev_exec = g.get_node(event).unwrap().find_output("exec").unwrap().id;
        let log_in = g.get_node(log).unwrap().find_input("exec").unwrap().id;
        g.connect(event, ev_exec, log, log_in).unwrap();

        // Connect add.result -> log.message
        let add_out = g.get_node(add).unwrap().find_output("result").unwrap().id;
        let log_msg = g.get_node(log).unwrap().find_input("message").unwrap().id;
        g.connect(add, add_out, log, log_msg).unwrap();

        let result = compile(&g).unwrap();
        let has_binary = result
            .instructions
            .iter()
            .any(|i| matches!(i, IrInstruction::BinaryOp { .. }));
        assert!(has_binary);
    }

    #[test]
    fn test_compile_set_position() {
        let mut g = NodeGraph::new("test", "SetPos");
        let event = g.add_node(NodeKind::OnInteract).unwrap();
        let set_pos = g.add_node(NodeKind::SetPosition).unwrap();

        let ev_exec = g.get_node(event).unwrap().find_output("exec").unwrap().id;
        let sp_in = g.get_node(set_pos).unwrap().find_input("exec").unwrap().id;
        g.connect(event, ev_exec, set_pos, sp_in).unwrap();

        let ev_entity = g.get_node(event).unwrap().find_output("entity").unwrap().id;
        let sp_entity = g.get_node(set_pos).unwrap().find_input("entity").unwrap().id;
        g.connect(event, ev_entity, set_pos, sp_entity).unwrap();

        let result = compile(&g).unwrap();
        let has_call = result.instructions.iter().any(|i| matches!(
            i,
            IrInstruction::Call { function, .. } if function == "set_position"
        ));
        assert!(has_call);
    }

    #[test]
    fn test_compile_delay() {
        let mut g = NodeGraph::new("test", "Delay");
        let event = g.add_node(NodeKind::OnStart).unwrap();
        let delay = g.add_node(NodeKind::Delay { delay_ms: 500 }).unwrap();
        let log = g.add_node(NodeKind::Log).unwrap();

        let ev_exec = g.get_node(event).unwrap().find_output("exec").unwrap().id;
        let d_in = g.get_node(delay).unwrap().find_input("exec").unwrap().id;
        let d_out = g.get_node(delay).unwrap().find_output("exec").unwrap().id;
        let l_in = g.get_node(log).unwrap().find_input("exec").unwrap().id;

        g.connect(event, ev_exec, delay, d_in).unwrap();
        g.connect(delay, d_out, log, l_in).unwrap();

        let result = compile(&g).unwrap();
        let has_delay = result.instructions.iter().any(|i| matches!(
            i,
            IrInstruction::Call { function, .. } if function == "delay"
        ));
        assert!(has_delay);
    }

    #[test]
    fn test_wasm_stub_header() {
        let g = simple_event_log_graph();
        let result = compile(&g).unwrap();

        // WASM magic number
        assert_eq!(&result.wasm_bytes[0..4], &[0x00, 0x61, 0x73, 0x6D]);
        // WASM version 1
        assert_eq!(&result.wasm_bytes[4..8], &[0x01, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn test_ir_instruction_display() {
        assert_eq!(
            format!("{}", IrInstruction::LoadConst(0, Value::Int(42))),
            "r0 = 42"
        );
        assert_eq!(format!("{}", IrInstruction::Return), "return");
        assert_eq!(format!("{}", IrInstruction::Nop), "nop");
        assert_eq!(format!("{}", IrInstruction::Jump(5)), "jump L5");
        assert_eq!(format!("{}", IrInstruction::Label(3)), "L3:");

        let call = IrInstruction::Call {
            function: "log".into(),
            args: vec![0, 1],
            result: None,
        };
        assert!(format!("{call}").contains("log"));

        let call_result = IrInstruction::Call {
            function: "spawn".into(),
            args: vec![0],
            result: Some(2),
        };
        assert!(format!("{call_result}").contains("r2"));
    }

    #[test]
    fn test_compile_error_display() {
        assert!(format!("{}", CompileError::EmptyGraph).contains("empty"));
        assert!(format!("{}", CompileError::CycleDetected).contains("cycle"));
        assert!(
            format!(
                "{}",
                CompileError::ValidationFailed(vec!["bad".into()])
            )
            .contains("bad")
        );
        assert!(
            format!(
                "{}",
                CompileError::InternalError("oops".into())
            )
            .contains("oops")
        );
    }

    #[test]
    fn test_compile_spawn_entity() {
        let mut g = NodeGraph::new("test", "Spawn");
        let event = g.add_node(NodeKind::OnStart).unwrap();
        let spawn = g.add_node(NodeKind::SpawnEntity).unwrap();

        let ev_exec = g.get_node(event).unwrap().find_output("exec").unwrap().id;
        let sp_in = g.get_node(spawn).unwrap().find_input("exec").unwrap().id;
        g.connect(event, ev_exec, spawn, sp_in).unwrap();

        let result = compile(&g).unwrap();
        let has_spawn = result.instructions.iter().any(|i| matches!(
            i,
            IrInstruction::Call { function, .. } if function == "spawn_entity"
        ));
        assert!(has_spawn);
    }

    #[test]
    fn test_compile_not_node() {
        let mut g = NodeGraph::new("test", "Not");
        let event = g.add_node(NodeKind::OnStart).unwrap();
        let not_node = g.add_node(NodeKind::Not).unwrap();
        let branch = g.add_node(NodeKind::Branch).unwrap();
        let log = g.add_node(NodeKind::Log).unwrap();

        let ev_exec = g.get_node(event).unwrap().find_output("exec").unwrap().id;
        let br_in = g.get_node(branch).unwrap().find_input("exec").unwrap().id;
        g.connect(event, ev_exec, branch, br_in).unwrap();

        let not_out = g.get_node(not_node).unwrap().find_output("result").unwrap().id;
        let br_cond = g.get_node(branch).unwrap().find_input("condition").unwrap().id;
        g.connect(not_node, not_out, branch, br_cond).unwrap();

        let br_true = g.get_node(branch).unwrap().find_output("true").unwrap().id;
        let log_in = g.get_node(log).unwrap().find_input("exec").unwrap().id;
        g.connect(branch, br_true, log, log_in).unwrap();

        let result = compile(&g).unwrap();
        let has_not = result
            .instructions
            .iter()
            .any(|i| matches!(i, IrInstruction::Not { .. }));
        assert!(has_not);
    }

    #[test]
    fn test_compiled_script_serde() {
        let g = simple_event_log_graph();
        let compiled = compile(&g).unwrap();
        let json = serde_json::to_string(&compiled).unwrap();
        let parsed: CompiledScript = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.instructions.len(), compiled.instructions.len());
        assert_eq!(parsed.register_count, compiled.register_count);
    }
}
