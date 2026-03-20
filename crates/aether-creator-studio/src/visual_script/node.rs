//! Node and port definitions for the visual scripting graph.

use serde::{Deserialize, Serialize};

use super::types::{DataType, Value};

/// Unique identifier for a node within a graph.
pub type NodeId = u64;

/// Unique identifier for a port within a node.
pub type PortId = u64;

/// Direction of a port on a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PortDirection {
    Input,
    Output,
}

/// A port (input or output) on a node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Port {
    pub id: PortId,
    pub name: String,
    pub data_type: DataType,
    pub direction: PortDirection,
    pub default_value: Option<Value>,
}

impl Port {
    pub fn new_input(id: PortId, name: impl Into<String>, data_type: DataType) -> Self {
        Self {
            id,
            name: name.into(),
            data_type,
            direction: PortDirection::Input,
            default_value: None,
        }
    }

    pub fn new_output(id: PortId, name: impl Into<String>, data_type: DataType) -> Self {
        Self {
            id,
            name: name.into(),
            data_type,
            direction: PortDirection::Output,
            default_value: None,
        }
    }

    pub fn with_default(mut self, value: Value) -> Self {
        self.default_value = Some(value);
        self
    }
}

/// The kind of node -- determines its behavior and ports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeKind {
    // Events
    OnInteract,
    OnEnter,
    OnExit,
    OnTimer { interval_ms: u64 },
    OnStart,
    OnCollision,

    // Flow control
    Branch,
    ForLoop,
    Sequence { output_count: u32 },
    Delay { delay_ms: u64 },

    // Actions
    SetPosition,
    SetRotation,
    PlayAnimation,
    PlaySound,
    SpawnEntity,
    DestroyEntity,
    SendMessage,
    Log,

    // Variables
    GetVariable { var_name: String },
    SetVariable { var_name: String },

    // Math
    Add,
    Subtract,
    Multiply,
    Divide,
    Clamp,
    Lerp,
    RandomRange,

    // Comparison / Logic
    Equal,
    NotEqual,
    Greater,
    Less,
    And,
    Or,
    Not,
}

impl NodeKind {
    /// Returns a human-readable display name.
    pub fn display_name(&self) -> &str {
        match self {
            NodeKind::OnInteract => "On Interact",
            NodeKind::OnEnter => "On Enter",
            NodeKind::OnExit => "On Exit",
            NodeKind::OnTimer { .. } => "On Timer",
            NodeKind::OnStart => "On Start",
            NodeKind::OnCollision => "On Collision",
            NodeKind::Branch => "Branch",
            NodeKind::ForLoop => "For Loop",
            NodeKind::Sequence { .. } => "Sequence",
            NodeKind::Delay { .. } => "Delay",
            NodeKind::SetPosition => "Set Position",
            NodeKind::SetRotation => "Set Rotation",
            NodeKind::PlayAnimation => "Play Animation",
            NodeKind::PlaySound => "Play Sound",
            NodeKind::SpawnEntity => "Spawn Entity",
            NodeKind::DestroyEntity => "Destroy Entity",
            NodeKind::SendMessage => "Send Message",
            NodeKind::Log => "Log",
            NodeKind::GetVariable { .. } => "Get Variable",
            NodeKind::SetVariable { .. } => "Set Variable",
            NodeKind::Add => "Add",
            NodeKind::Subtract => "Subtract",
            NodeKind::Multiply => "Multiply",
            NodeKind::Divide => "Divide",
            NodeKind::Clamp => "Clamp",
            NodeKind::Lerp => "Lerp",
            NodeKind::RandomRange => "Random Range",
            NodeKind::Equal => "Equal",
            NodeKind::NotEqual => "Not Equal",
            NodeKind::Greater => "Greater",
            NodeKind::Less => "Less",
            NodeKind::And => "And",
            NodeKind::Or => "Or",
            NodeKind::Not => "Not",
        }
    }

    /// Returns true if this is an event node (entry point).
    pub fn is_event(&self) -> bool {
        matches!(
            self,
            NodeKind::OnInteract
                | NodeKind::OnEnter
                | NodeKind::OnExit
                | NodeKind::OnTimer { .. }
                | NodeKind::OnStart
                | NodeKind::OnCollision
        )
    }

    /// Returns true if this node is pure (no side effects, no flow ports).
    pub fn is_pure(&self) -> bool {
        matches!(
            self,
            NodeKind::Add
                | NodeKind::Subtract
                | NodeKind::Multiply
                | NodeKind::Divide
                | NodeKind::Clamp
                | NodeKind::Lerp
                | NodeKind::RandomRange
                | NodeKind::Equal
                | NodeKind::NotEqual
                | NodeKind::Greater
                | NodeKind::Less
                | NodeKind::And
                | NodeKind::Or
                | NodeKind::Not
                | NodeKind::GetVariable { .. }
        )
    }
}

/// Helper for building ports with auto-incrementing IDs.
struct PortBuilder {
    inputs: Vec<Port>,
    outputs: Vec<Port>,
    next_id: PortId,
}

impl PortBuilder {
    fn new(base_port_id: PortId) -> Self {
        Self {
            inputs: Vec::new(),
            outputs: Vec::new(),
            next_id: base_port_id,
        }
    }

    fn add_in(&mut self, name: &str, dt: DataType) {
        self.inputs.push(Port::new_input(self.next_id, name, dt));
        self.next_id += 1;
    }

    fn add_out(&mut self, name: impl Into<String>, dt: DataType) {
        self.outputs.push(Port::new_output(self.next_id, name, dt));
        self.next_id += 1;
    }

    fn finish(self) -> (Vec<Port>, Vec<Port>, PortId) {
        (self.inputs, self.outputs, self.next_id)
    }
}

/// Build the default ports for a given NodeKind.
///
/// Port IDs are assigned sequentially starting from `base_port_id`.
/// Returns `(inputs, outputs, next_available_port_id)`.
pub fn build_ports(kind: &NodeKind, base_port_id: PortId) -> (Vec<Port>, Vec<Port>, PortId) {
    let mut b = PortBuilder::new(base_port_id);

    match kind {
        // Events: no inputs, flow + context outputs
        NodeKind::OnInteract => {
            b.add_out("exec", DataType::Flow);
            b.add_out("entity", DataType::Entity);
        }
        NodeKind::OnEnter => {
            b.add_out("exec", DataType::Flow);
            b.add_out("entity", DataType::Entity);
        }
        NodeKind::OnExit => {
            b.add_out("exec", DataType::Flow);
            b.add_out("entity", DataType::Entity);
        }
        NodeKind::OnTimer { .. } => {
            b.add_out("exec", DataType::Flow);
        }
        NodeKind::OnStart => {
            b.add_out("exec", DataType::Flow);
        }
        NodeKind::OnCollision => {
            b.add_out("exec", DataType::Flow);
            b.add_out("self_entity", DataType::Entity);
            b.add_out("other_entity", DataType::Entity);
        }

        // Flow control
        NodeKind::Branch => {
            b.add_in("exec", DataType::Flow);
            b.add_in("condition", DataType::Bool);
            b.add_out("true", DataType::Flow);
            b.add_out("false", DataType::Flow);
        }
        NodeKind::ForLoop => {
            b.add_in("exec", DataType::Flow);
            b.add_in("start", DataType::Int);
            b.add_in("end", DataType::Int);
            b.add_out("body", DataType::Flow);
            b.add_out("index", DataType::Int);
            b.add_out("done", DataType::Flow);
        }
        NodeKind::Sequence { output_count } => {
            b.add_in("exec", DataType::Flow);
            for i in 0..*output_count {
                b.add_out(format!("out_{i}"), DataType::Flow);
            }
        }
        NodeKind::Delay { .. } => {
            b.add_in("exec", DataType::Flow);
            b.add_out("exec", DataType::Flow);
        }

        // Actions
        NodeKind::SetPosition => {
            b.add_in("exec", DataType::Flow);
            b.add_in("entity", DataType::Entity);
            b.add_in("position", DataType::Vec3);
            b.add_out("exec", DataType::Flow);
        }
        NodeKind::SetRotation => {
            b.add_in("exec", DataType::Flow);
            b.add_in("entity", DataType::Entity);
            b.add_in("rotation", DataType::Vec3);
            b.add_out("exec", DataType::Flow);
        }
        NodeKind::PlayAnimation => {
            b.add_in("exec", DataType::Flow);
            b.add_in("entity", DataType::Entity);
            b.add_in("animation", DataType::String);
            b.add_out("exec", DataType::Flow);
        }
        NodeKind::PlaySound => {
            b.add_in("exec", DataType::Flow);
            b.add_in("sound", DataType::String);
            b.add_out("exec", DataType::Flow);
        }
        NodeKind::SpawnEntity => {
            b.add_in("exec", DataType::Flow);
            b.add_in("template", DataType::String);
            b.add_in("position", DataType::Vec3);
            b.add_out("exec", DataType::Flow);
            b.add_out("spawned", DataType::Entity);
        }
        NodeKind::DestroyEntity => {
            b.add_in("exec", DataType::Flow);
            b.add_in("entity", DataType::Entity);
            b.add_out("exec", DataType::Flow);
        }
        NodeKind::SendMessage => {
            b.add_in("exec", DataType::Flow);
            b.add_in("channel", DataType::String);
            b.add_in("message", DataType::String);
            b.add_out("exec", DataType::Flow);
        }
        NodeKind::Log => {
            b.add_in("exec", DataType::Flow);
            b.add_in("message", DataType::Any);
            b.add_out("exec", DataType::Flow);
        }

        // Variables
        NodeKind::GetVariable { .. } => {
            b.add_out("value", DataType::Any);
        }
        NodeKind::SetVariable { .. } => {
            b.add_in("exec", DataType::Flow);
            b.add_in("value", DataType::Any);
            b.add_out("exec", DataType::Flow);
        }

        // Math binary ops
        NodeKind::Add | NodeKind::Subtract | NodeKind::Multiply | NodeKind::Divide => {
            b.add_in("a", DataType::Float);
            b.add_in("b", DataType::Float);
            b.add_out("result", DataType::Float);
        }
        NodeKind::Clamp => {
            b.add_in("value", DataType::Float);
            b.add_in("min", DataType::Float);
            b.add_in("max", DataType::Float);
            b.add_out("result", DataType::Float);
        }
        NodeKind::Lerp => {
            b.add_in("a", DataType::Float);
            b.add_in("b", DataType::Float);
            b.add_in("t", DataType::Float);
            b.add_out("result", DataType::Float);
        }
        NodeKind::RandomRange => {
            b.add_in("min", DataType::Float);
            b.add_in("max", DataType::Float);
            b.add_out("result", DataType::Float);
        }

        // Comparison
        NodeKind::Equal | NodeKind::NotEqual => {
            b.add_in("a", DataType::Any);
            b.add_in("b", DataType::Any);
            b.add_out("result", DataType::Bool);
        }
        NodeKind::Greater | NodeKind::Less => {
            b.add_in("a", DataType::Float);
            b.add_in("b", DataType::Float);
            b.add_out("result", DataType::Bool);
        }

        // Logic
        NodeKind::And | NodeKind::Or => {
            b.add_in("a", DataType::Bool);
            b.add_in("b", DataType::Bool);
            b.add_out("result", DataType::Bool);
        }
        NodeKind::Not => {
            b.add_in("value", DataType::Bool);
            b.add_out("result", DataType::Bool);
        }
    }

    b.finish()
}

/// A node in the visual script graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: NodeId,
    pub kind: NodeKind,
    pub position: (f32, f32),
    pub inputs: Vec<Port>,
    pub outputs: Vec<Port>,
    /// Optional user-provided comment/label.
    pub comment: Option<String>,
}

impl Node {
    /// Create a new node of the given kind at position (0, 0).
    /// Port IDs are assigned starting from `base_port_id`.
    pub fn new(id: NodeId, kind: NodeKind, base_port_id: PortId) -> Self {
        let (inputs, outputs, _) = build_ports(&kind, base_port_id);
        Self {
            id,
            kind,
            position: (0.0, 0.0),
            inputs,
            outputs,
            comment: None,
        }
    }

    /// Set the node position.
    pub fn with_position(mut self, x: f32, y: f32) -> Self {
        self.position = (x, y);
        self
    }

    /// Find an input port by name.
    pub fn find_input(&self, name: &str) -> Option<&Port> {
        self.inputs.iter().find(|p| p.name == name)
    }

    /// Find an output port by name.
    pub fn find_output(&self, name: &str) -> Option<&Port> {
        self.outputs.iter().find(|p| p.name == name)
    }

    /// Find any port (input or output) by its PortId.
    pub fn find_port(&self, port_id: PortId) -> Option<&Port> {
        self.inputs
            .iter()
            .chain(self.outputs.iter())
            .find(|p| p.id == port_id)
    }

    /// All port IDs belonging to this node.
    pub fn all_port_ids(&self) -> Vec<PortId> {
        self.inputs
            .iter()
            .chain(self.outputs.iter())
            .map(|p| p.id)
            .collect()
    }

    /// Display name for this node.
    pub fn display_name(&self) -> &str {
        self.kind.display_name()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // NodeKind tests

    #[test]
    fn test_event_nodes_are_events() {
        assert!(NodeKind::OnInteract.is_event());
        assert!(NodeKind::OnEnter.is_event());
        assert!(NodeKind::OnExit.is_event());
        assert!(NodeKind::OnTimer { interval_ms: 1000 }.is_event());
        assert!(NodeKind::OnStart.is_event());
        assert!(NodeKind::OnCollision.is_event());
    }

    #[test]
    fn test_non_event_nodes() {
        assert!(!NodeKind::Branch.is_event());
        assert!(!NodeKind::Add.is_event());
        assert!(!NodeKind::SetPosition.is_event());
        assert!(!NodeKind::GetVariable {
            var_name: "x".into()
        }
        .is_event());
    }

    #[test]
    fn test_pure_nodes() {
        assert!(NodeKind::Add.is_pure());
        assert!(NodeKind::Subtract.is_pure());
        assert!(NodeKind::Multiply.is_pure());
        assert!(NodeKind::Divide.is_pure());
        assert!(NodeKind::Clamp.is_pure());
        assert!(NodeKind::Equal.is_pure());
        assert!(NodeKind::And.is_pure());
        assert!(NodeKind::Not.is_pure());
        assert!(NodeKind::GetVariable {
            var_name: "x".into()
        }
        .is_pure());
    }

    #[test]
    fn test_impure_nodes() {
        assert!(!NodeKind::SetPosition.is_pure());
        assert!(!NodeKind::Branch.is_pure());
        assert!(!NodeKind::OnInteract.is_pure());
        assert!(!NodeKind::SetVariable {
            var_name: "x".into()
        }
        .is_pure());
        assert!(!NodeKind::Log.is_pure());
    }

    #[test]
    fn test_display_names() {
        assert_eq!(NodeKind::OnInteract.display_name(), "On Interact");
        assert_eq!(NodeKind::Branch.display_name(), "Branch");
        assert_eq!(NodeKind::Add.display_name(), "Add");
        assert_eq!(NodeKind::SetPosition.display_name(), "Set Position");
        assert_eq!(NodeKind::Log.display_name(), "Log");
    }

    // Port building tests

    #[test]
    fn test_on_interact_ports() {
        let (inputs, outputs, _) = build_ports(&NodeKind::OnInteract, 0);
        assert!(inputs.is_empty());
        assert_eq!(outputs.len(), 2);
        assert_eq!(outputs[0].name, "exec");
        assert_eq!(outputs[0].data_type, DataType::Flow);
        assert_eq!(outputs[1].name, "entity");
        assert_eq!(outputs[1].data_type, DataType::Entity);
    }

    #[test]
    fn test_branch_ports() {
        let (inputs, outputs, _) = build_ports(&NodeKind::Branch, 0);
        assert_eq!(inputs.len(), 2);
        assert_eq!(inputs[0].name, "exec");
        assert_eq!(inputs[0].data_type, DataType::Flow);
        assert_eq!(inputs[1].name, "condition");
        assert_eq!(inputs[1].data_type, DataType::Bool);
        assert_eq!(outputs.len(), 2);
        assert_eq!(outputs[0].name, "true");
        assert_eq!(outputs[1].name, "false");
    }

    #[test]
    fn test_add_ports() {
        let (inputs, outputs, _) = build_ports(&NodeKind::Add, 0);
        assert_eq!(inputs.len(), 2);
        assert_eq!(inputs[0].name, "a");
        assert_eq!(inputs[0].data_type, DataType::Float);
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].name, "result");
    }

    #[test]
    fn test_sequence_ports() {
        let (inputs, outputs, _) = build_ports(&NodeKind::Sequence { output_count: 3 }, 0);
        assert_eq!(inputs.len(), 1);
        assert_eq!(outputs.len(), 3);
        assert_eq!(outputs[0].name, "out_0");
        assert_eq!(outputs[1].name, "out_1");
        assert_eq!(outputs[2].name, "out_2");
    }

    #[test]
    fn test_for_loop_ports() {
        let (inputs, outputs, _) = build_ports(&NodeKind::ForLoop, 0);
        assert_eq!(inputs.len(), 3); // exec, start, end
        assert_eq!(outputs.len(), 3); // body, index, done
        assert_eq!(outputs[1].name, "index");
        assert_eq!(outputs[1].data_type, DataType::Int);
    }

    #[test]
    fn test_set_position_ports() {
        let (inputs, outputs, _) = build_ports(&NodeKind::SetPosition, 0);
        assert_eq!(inputs.len(), 3); // exec, entity, position
        assert_eq!(inputs[1].data_type, DataType::Entity);
        assert_eq!(inputs[2].data_type, DataType::Vec3);
        assert_eq!(outputs.len(), 1); // exec
    }

    #[test]
    fn test_spawn_entity_ports() {
        let (inputs, outputs, _) = build_ports(&NodeKind::SpawnEntity, 0);
        assert_eq!(inputs.len(), 3); // exec, template, position
        assert_eq!(outputs.len(), 2); // exec, spawned
        assert_eq!(outputs[1].name, "spawned");
        assert_eq!(outputs[1].data_type, DataType::Entity);
    }

    #[test]
    fn test_get_variable_ports() {
        let (inputs, outputs, _) = build_ports(
            &NodeKind::GetVariable {
                var_name: "hp".into(),
            },
            0,
        );
        assert!(inputs.is_empty());
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].data_type, DataType::Any);
    }

    #[test]
    fn test_set_variable_ports() {
        let (inputs, outputs, _) = build_ports(
            &NodeKind::SetVariable {
                var_name: "hp".into(),
            },
            0,
        );
        assert_eq!(inputs.len(), 2); // exec, value
        assert_eq!(outputs.len(), 1); // exec
    }

    #[test]
    fn test_not_ports() {
        let (inputs, outputs, _) = build_ports(&NodeKind::Not, 0);
        assert_eq!(inputs.len(), 1);
        assert_eq!(inputs[0].data_type, DataType::Bool);
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].data_type, DataType::Bool);
    }

    #[test]
    fn test_clamp_ports() {
        let (inputs, outputs, _) = build_ports(&NodeKind::Clamp, 0);
        assert_eq!(inputs.len(), 3); // value, min, max
        assert_eq!(outputs.len(), 1); // result
    }

    #[test]
    fn test_port_id_sequencing() {
        let (inputs, outputs, next_id) = build_ports(&NodeKind::Branch, 10);
        assert_eq!(inputs[0].id, 10);
        assert_eq!(inputs[1].id, 11);
        assert_eq!(outputs[0].id, 12);
        assert_eq!(outputs[1].id, 13);
        assert_eq!(next_id, 14);
    }

    // Node tests

    #[test]
    fn test_node_creation() {
        let node = Node::new(1, NodeKind::Add, 100);
        assert_eq!(node.id, 1);
        assert_eq!(node.position, (0.0, 0.0));
        assert_eq!(node.inputs.len(), 2);
        assert_eq!(node.outputs.len(), 1);
    }

    #[test]
    fn test_node_with_position() {
        let node = Node::new(1, NodeKind::Add, 0).with_position(100.0, 200.0);
        assert_eq!(node.position, (100.0, 200.0));
    }

    #[test]
    fn test_find_input_by_name() {
        let node = Node::new(1, NodeKind::Branch, 0);
        let port = node.find_input("condition").unwrap();
        assert_eq!(port.data_type, DataType::Bool);
        assert!(node.find_input("nonexistent").is_none());
    }

    #[test]
    fn test_find_output_by_name() {
        let node = Node::new(1, NodeKind::Branch, 0);
        let port = node.find_output("true").unwrap();
        assert_eq!(port.data_type, DataType::Flow);
        assert!(node.find_output("nonexistent").is_none());
    }

    #[test]
    fn test_find_port_by_id() {
        let node = Node::new(1, NodeKind::Add, 10);
        assert!(node.find_port(10).is_some());
        assert!(node.find_port(11).is_some());
        assert!(node.find_port(12).is_some());
        assert!(node.find_port(99).is_none());
    }

    #[test]
    fn test_all_port_ids() {
        let node = Node::new(1, NodeKind::Branch, 0);
        let ids = node.all_port_ids();
        assert_eq!(ids.len(), 4); // 2 inputs + 2 outputs
    }

    #[test]
    fn test_node_display_name() {
        let node = Node::new(1, NodeKind::OnStart, 0);
        assert_eq!(node.display_name(), "On Start");
    }

    #[test]
    fn test_port_with_default() {
        let port = Port::new_input(0, "value", DataType::Float).with_default(Value::Float(1.0));
        assert_eq!(port.default_value, Some(Value::Float(1.0)));
    }

    #[test]
    fn test_node_serde_round_trip() {
        let node = Node::new(1, NodeKind::Branch, 0).with_position(50.0, 75.0);
        let json = serde_json::to_string(&node).unwrap();
        let parsed: Node = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, 1);
        assert_eq!(parsed.position, (50.0, 75.0));
        assert_eq!(parsed.inputs.len(), 2);
        assert_eq!(parsed.outputs.len(), 2);
    }

    #[test]
    fn test_on_collision_outputs() {
        let (inputs, outputs, _) = build_ports(&NodeKind::OnCollision, 0);
        assert!(inputs.is_empty());
        assert_eq!(outputs.len(), 3);
        assert_eq!(outputs[0].data_type, DataType::Flow);
        assert_eq!(outputs[1].name, "self_entity");
        assert_eq!(outputs[2].name, "other_entity");
    }

    #[test]
    fn test_log_ports() {
        let (inputs, outputs, _) = build_ports(&NodeKind::Log, 0);
        assert_eq!(inputs.len(), 2); // exec, message
        assert_eq!(inputs[1].data_type, DataType::Any);
        assert_eq!(outputs.len(), 1); // exec
    }

    #[test]
    fn test_lerp_ports() {
        let (inputs, outputs, _) = build_ports(&NodeKind::Lerp, 0);
        assert_eq!(inputs.len(), 3); // a, b, t
        assert_eq!(outputs.len(), 1);
    }

    #[test]
    fn test_random_range_ports() {
        let (inputs, outputs, _) = build_ports(&NodeKind::RandomRange, 0);
        assert_eq!(inputs.len(), 2); // min, max
        assert_eq!(outputs.len(), 1);
    }

    #[test]
    fn test_delay_ports() {
        let (inputs, outputs, _) = build_ports(&NodeKind::Delay { delay_ms: 500 }, 0);
        assert_eq!(inputs.len(), 1); // exec
        assert_eq!(outputs.len(), 1); // exec
    }
}
