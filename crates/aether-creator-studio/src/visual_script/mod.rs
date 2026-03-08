//! Visual scripting editor: node-based graph for authoring game logic.
//!
//! Provides a node graph data model, type system, validation, compilation
//! pipeline (graph -> IR -> WASM bytecode), automatic layout, and pre-built
//! game logic templates.

pub mod compiler;
pub mod graph;
pub mod layout;
pub mod node;
pub mod templates;
pub mod types;
pub mod validation;

// Re-export key types for convenient access.
pub use compiler::{compile, BinaryOp, CompileError, CompiledScript, IrInstruction};
pub use graph::{Connection, ConnectionId, GraphError, NodeGraph};
pub use layout::{apply_layout, compute_layout, LayoutConfig, LayoutResult};
pub use node::{build_ports, Node, NodeId, NodeKind, Port, PortDirection, PortId};
pub use templates::{all_templates, instantiate_template, TemplateKind};
pub use types::{DataType, Value};
pub use validation::{
    topological_sort_flow, validate_graph, Severity, ValidationDiagnostic, ValidationResult,
};
