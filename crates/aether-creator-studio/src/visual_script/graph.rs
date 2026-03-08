//! Node graph container: manages nodes, connections, and serialization.

use std::collections::HashMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use super::node::{Node, NodeId, NodeKind, PortDirection, PortId};
use super::types::DataType;

/// Maximum number of nodes allowed per graph.
const MAX_NODES_PER_GRAPH: usize = 1000;

/// Maximum number of connections allowed per graph.
const MAX_CONNECTIONS_PER_GRAPH: usize = 5000;

/// Unique identifier for a connection.
pub type ConnectionId = u64;

/// A connection between two ports on different nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Connection {
    pub id: ConnectionId,
    pub from_node: NodeId,
    pub from_port: PortId,
    pub to_node: NodeId,
    pub to_port: PortId,
}

/// Errors that can occur during graph operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphError {
    NodeNotFound(NodeId),
    PortNotFound(PortId),
    ConnectionNotFound(ConnectionId),
    MaxNodesReached,
    MaxConnectionsReached,
    SelfConnection,
    InvalidDirection,
    TypeMismatch {
        from_type: String,
        to_type: String,
    },
    InputAlreadyConnected(PortId),
    DuplicateConnection,
}

impl fmt::Display for GraphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GraphError::NodeNotFound(id) => write!(f, "node not found: {id}"),
            GraphError::PortNotFound(id) => write!(f, "port not found: {id}"),
            GraphError::ConnectionNotFound(id) => write!(f, "connection not found: {id}"),
            GraphError::MaxNodesReached => {
                write!(f, "maximum nodes reached ({MAX_NODES_PER_GRAPH})")
            }
            GraphError::MaxConnectionsReached => {
                write!(f, "maximum connections reached ({MAX_CONNECTIONS_PER_GRAPH})")
            }
            GraphError::SelfConnection => write!(f, "cannot connect a node to itself"),
            GraphError::InvalidDirection => {
                write!(f, "connection must go from output to input")
            }
            GraphError::TypeMismatch { from_type, to_type } => {
                write!(f, "type mismatch: {from_type} -> {to_type}")
            }
            GraphError::InputAlreadyConnected(id) => {
                write!(f, "input port {id} already has a connection")
            }
            GraphError::DuplicateConnection => write!(f, "connection already exists"),
        }
    }
}

impl std::error::Error for GraphError {}

/// The main node graph data structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeGraph {
    pub id: String,
    pub name: String,
    pub description: String,
    nodes: HashMap<NodeId, Node>,
    connections: Vec<Connection>,
    next_node_id: NodeId,
    next_port_id: PortId,
    next_connection_id: ConnectionId,
}

impl NodeGraph {
    /// Create a new empty graph.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: String::new(),
            nodes: HashMap::new(),
            connections: Vec::new(),
            next_node_id: 1,
            next_port_id: 1,
            next_connection_id: 1,
        }
    }

    /// Add a node of the given kind. Returns the assigned NodeId.
    pub fn add_node(&mut self, kind: NodeKind) -> Result<NodeId, GraphError> {
        if self.nodes.len() >= MAX_NODES_PER_GRAPH {
            return Err(GraphError::MaxNodesReached);
        }

        let node_id = self.next_node_id;
        self.next_node_id += 1;

        let node = Node::new(node_id, kind, self.next_port_id);
        // Advance port id past all ports created
        let port_count = node.inputs.len() + node.outputs.len();
        self.next_port_id += port_count as u64;

        self.nodes.insert(node_id, node);
        Ok(node_id)
    }

    /// Add a node at a specific position. Returns the assigned NodeId.
    pub fn add_node_at(
        &mut self,
        kind: NodeKind,
        x: f32,
        y: f32,
    ) -> Result<NodeId, GraphError> {
        let id = self.add_node(kind)?;
        if let Some(node) = self.nodes.get_mut(&id) {
            node.position = (x, y);
        }
        Ok(id)
    }

    /// Remove a node and all its connections. Returns the removed node.
    pub fn remove_node(&mut self, node_id: NodeId) -> Result<Node, GraphError> {
        let node = self
            .nodes
            .remove(&node_id)
            .ok_or(GraphError::NodeNotFound(node_id))?;

        // Remove all connections involving this node
        self.connections
            .retain(|c| c.from_node != node_id && c.to_node != node_id);

        Ok(node)
    }

    /// Get a reference to a node.
    pub fn get_node(&self, node_id: NodeId) -> Option<&Node> {
        self.nodes.get(&node_id)
    }

    /// Get a mutable reference to a node.
    pub fn get_node_mut(&mut self, node_id: NodeId) -> Option<&mut Node> {
        self.nodes.get_mut(&node_id)
    }

    /// Iterate over all nodes.
    pub fn nodes(&self) -> impl Iterator<Item = &Node> {
        self.nodes.values()
    }

    /// Number of nodes in the graph.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// All connections in the graph.
    pub fn connections(&self) -> &[Connection] {
        &self.connections
    }

    /// Number of connections in the graph.
    pub fn connection_count(&self) -> usize {
        self.connections.len()
    }

    /// Find which node owns a given port id.
    pub fn find_port_owner(&self, port_id: PortId) -> Option<NodeId> {
        for (node_id, node) in &self.nodes {
            if node.find_port(port_id).is_some() {
                return Some(*node_id);
            }
        }
        None
    }

    /// Connect an output port to an input port.
    pub fn connect(
        &mut self,
        from_node: NodeId,
        from_port: PortId,
        to_node: NodeId,
        to_port: PortId,
    ) -> Result<ConnectionId, GraphError> {
        if self.connections.len() >= MAX_CONNECTIONS_PER_GRAPH {
            return Err(GraphError::MaxConnectionsReached);
        }

        // Prevent self-connection
        if from_node == to_node {
            return Err(GraphError::SelfConnection);
        }

        // Validate nodes exist
        let from = self
            .nodes
            .get(&from_node)
            .ok_or(GraphError::NodeNotFound(from_node))?;
        let to = self
            .nodes
            .get(&to_node)
            .ok_or(GraphError::NodeNotFound(to_node))?;

        // Validate ports exist and directions are correct
        let from_p = from
            .find_port(from_port)
            .ok_or(GraphError::PortNotFound(from_port))?;
        let to_p = to
            .find_port(to_port)
            .ok_or(GraphError::PortNotFound(to_port))?;

        if from_p.direction != PortDirection::Output || to_p.direction != PortDirection::Input {
            return Err(GraphError::InvalidDirection);
        }

        // Type check
        if !from_p.data_type.is_compatible_with(to_p.data_type) {
            return Err(GraphError::TypeMismatch {
                from_type: from_p.data_type.to_string(),
                to_type: to_p.data_type.to_string(),
            });
        }

        // Check duplicate
        let duplicate = self.connections.iter().any(|c| {
            c.from_node == from_node
                && c.from_port == from_port
                && c.to_node == to_node
                && c.to_port == to_port
        });
        if duplicate {
            return Err(GraphError::DuplicateConnection);
        }

        // Check single-input constraint
        let already_connected = self.connections.iter().any(|c| c.to_port == to_port);
        if already_connected {
            return Err(GraphError::InputAlreadyConnected(to_port));
        }

        let conn_id = self.next_connection_id;
        self.next_connection_id += 1;

        self.connections.push(Connection {
            id: conn_id,
            from_node,
            from_port,
            to_node,
            to_port,
        });

        Ok(conn_id)
    }

    /// Remove a connection by id.
    pub fn disconnect(&mut self, connection_id: ConnectionId) -> Result<Connection, GraphError> {
        let pos = self
            .connections
            .iter()
            .position(|c| c.id == connection_id)
            .ok_or(GraphError::ConnectionNotFound(connection_id))?;
        Ok(self.connections.remove(pos))
    }

    /// Get all connections from a specific node.
    pub fn connections_from(&self, node_id: NodeId) -> Vec<&Connection> {
        self.connections
            .iter()
            .filter(|c| c.from_node == node_id)
            .collect()
    }

    /// Get all connections to a specific node.
    pub fn connections_to(&self, node_id: NodeId) -> Vec<&Connection> {
        self.connections
            .iter()
            .filter(|c| c.to_node == node_id)
            .collect()
    }

    /// Get all connections to a specific input port.
    pub fn connections_to_port(&self, port_id: PortId) -> Vec<&Connection> {
        self.connections
            .iter()
            .filter(|c| c.to_port == port_id)
            .collect()
    }

    /// Get all connections from a specific output port.
    pub fn connections_from_port(&self, port_id: PortId) -> Vec<&Connection> {
        self.connections
            .iter()
            .filter(|c| c.from_port == port_id)
            .collect()
    }

    /// Get all event nodes in the graph.
    pub fn event_nodes(&self) -> Vec<&Node> {
        self.nodes.values().filter(|n| n.kind.is_event()).collect()
    }

    /// Get all node IDs.
    pub fn node_ids(&self) -> Vec<NodeId> {
        self.nodes.keys().copied().collect()
    }

    /// Clear the entire graph.
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.connections.clear();
    }

    /// Get the DataType of a port by looking up its owner node.
    pub fn port_data_type(&self, port_id: PortId) -> Option<DataType> {
        for node in self.nodes.values() {
            if let Some(port) = node.find_port(port_id) {
                return Some(port.data_type);
            }
        }
        None
    }
}

impl Default for NodeGraph {
    fn default() -> Self {
        Self::new("default", "Untitled")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_graph() -> NodeGraph {
        NodeGraph::new("test-1", "Test Graph")
    }

    // Basic graph tests

    #[test]
    fn test_new_graph() {
        let g = make_graph();
        assert_eq!(g.id, "test-1");
        assert_eq!(g.name, "Test Graph");
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.connection_count(), 0);
    }

    #[test]
    fn test_default_graph() {
        let g = NodeGraph::default();
        assert_eq!(g.id, "default");
        assert_eq!(g.name, "Untitled");
    }

    // Node management

    #[test]
    fn test_add_node() {
        let mut g = make_graph();
        let id = g.add_node(NodeKind::OnStart).unwrap();
        assert_eq!(g.node_count(), 1);
        assert!(g.get_node(id).is_some());
    }

    #[test]
    fn test_add_node_at_position() {
        let mut g = make_graph();
        let id = g.add_node_at(NodeKind::Add, 100.0, 200.0).unwrap();
        let node = g.get_node(id).unwrap();
        assert_eq!(node.position, (100.0, 200.0));
    }

    #[test]
    fn test_remove_node() {
        let mut g = make_graph();
        let id = g.add_node(NodeKind::OnStart).unwrap();
        let removed = g.remove_node(id).unwrap();
        assert_eq!(removed.id, id);
        assert_eq!(g.node_count(), 0);
    }

    #[test]
    fn test_remove_nonexistent_node() {
        let mut g = make_graph();
        assert!(matches!(
            g.remove_node(999),
            Err(GraphError::NodeNotFound(999))
        ));
    }

    #[test]
    fn test_remove_node_cleans_connections() {
        let mut g = make_graph();
        let event_id = g.add_node(NodeKind::OnStart).unwrap();
        let log_id = g.add_node(NodeKind::Log).unwrap();

        let event_out = g.get_node(event_id).unwrap().find_output("exec").unwrap().id;
        let log_in = g.get_node(log_id).unwrap().find_input("exec").unwrap().id;

        g.connect(event_id, event_out, log_id, log_in).unwrap();
        assert_eq!(g.connection_count(), 1);

        g.remove_node(event_id).unwrap();
        assert_eq!(g.connection_count(), 0);
    }

    #[test]
    fn test_node_ids() {
        let mut g = make_graph();
        let id1 = g.add_node(NodeKind::OnStart).unwrap();
        let id2 = g.add_node(NodeKind::Log).unwrap();
        let ids = g.node_ids();
        assert!(ids.contains(&id1));
        assert!(ids.contains(&id2));
    }

    #[test]
    fn test_get_node_mut() {
        let mut g = make_graph();
        let id = g.add_node(NodeKind::OnStart).unwrap();
        let node = g.get_node_mut(id).unwrap();
        node.comment = Some("hello".into());
        assert_eq!(g.get_node(id).unwrap().comment.as_deref(), Some("hello"));
    }

    #[test]
    fn test_event_nodes() {
        let mut g = make_graph();
        g.add_node(NodeKind::OnStart).unwrap();
        g.add_node(NodeKind::OnInteract).unwrap();
        g.add_node(NodeKind::Add).unwrap();
        assert_eq!(g.event_nodes().len(), 2);
    }

    // Connection tests

    #[test]
    fn test_connect_flow() {
        let mut g = make_graph();
        let event_id = g.add_node(NodeKind::OnStart).unwrap();
        let log_id = g.add_node(NodeKind::Log).unwrap();

        let event_out = g.get_node(event_id).unwrap().find_output("exec").unwrap().id;
        let log_in = g.get_node(log_id).unwrap().find_input("exec").unwrap().id;

        let conn_id = g.connect(event_id, event_out, log_id, log_in).unwrap();
        assert_eq!(g.connection_count(), 1);
        assert!(conn_id > 0);
    }

    #[test]
    fn test_connect_data() {
        let mut g = make_graph();
        let add_id = g.add_node(NodeKind::Add).unwrap();
        let log_id = g.add_node(NodeKind::Log).unwrap();

        let add_out = g.get_node(add_id).unwrap().find_output("result").unwrap().id;
        let log_msg = g.get_node(log_id).unwrap().find_input("message").unwrap().id;

        // Float -> Any should work
        g.connect(add_id, add_out, log_id, log_msg).unwrap();
        assert_eq!(g.connection_count(), 1);
    }

    #[test]
    fn test_connect_self_error() {
        let mut g = make_graph();
        let id = g.add_node(NodeKind::Add).unwrap();
        let a_port = g.get_node(id).unwrap().inputs[0].id;
        let out_port = g.get_node(id).unwrap().outputs[0].id;

        assert_eq!(
            g.connect(id, out_port, id, a_port),
            Err(GraphError::SelfConnection)
        );
    }

    #[test]
    fn test_connect_wrong_direction() {
        let mut g = make_graph();
        let add1 = g.add_node(NodeKind::Add).unwrap();
        let add2 = g.add_node(NodeKind::Add).unwrap();

        // Try connecting input -> input (wrong)
        let in1 = g.get_node(add1).unwrap().inputs[0].id;
        let in2 = g.get_node(add2).unwrap().inputs[0].id;

        assert_eq!(
            g.connect(add1, in1, add2, in2),
            Err(GraphError::InvalidDirection)
        );
    }

    #[test]
    fn test_connect_type_mismatch() {
        let mut g = make_graph();
        let on_start = g.add_node(NodeKind::OnStart).unwrap();
        let branch = g.add_node(NodeKind::Branch).unwrap();

        // Flow -> Bool should fail
        let exec_out = g.get_node(on_start).unwrap().find_output("exec").unwrap().id;
        let cond_in = g.get_node(branch).unwrap().find_input("condition").unwrap().id;

        let result = g.connect(on_start, exec_out, branch, cond_in);
        assert!(matches!(result, Err(GraphError::TypeMismatch { .. })));
    }

    #[test]
    fn test_connect_input_already_connected() {
        let mut g = make_graph();
        let event1 = g.add_node(NodeKind::OnStart).unwrap();
        let event2 = g.add_node(NodeKind::OnInteract).unwrap();
        let log_id = g.add_node(NodeKind::Log).unwrap();

        let exec_out1 = g.get_node(event1).unwrap().find_output("exec").unwrap().id;
        let exec_out2 = g.get_node(event2).unwrap().find_output("exec").unwrap().id;
        let log_in = g.get_node(log_id).unwrap().find_input("exec").unwrap().id;

        g.connect(event1, exec_out1, log_id, log_in).unwrap();
        assert_eq!(
            g.connect(event2, exec_out2, log_id, log_in),
            Err(GraphError::InputAlreadyConnected(log_in))
        );
    }

    #[test]
    fn test_connect_duplicate() {
        let mut g = make_graph();
        let event_id = g.add_node(NodeKind::OnStart).unwrap();
        let log_id = g.add_node(NodeKind::Log).unwrap();

        let exec_out = g.get_node(event_id).unwrap().find_output("exec").unwrap().id;
        let log_in = g.get_node(log_id).unwrap().find_input("exec").unwrap().id;

        g.connect(event_id, exec_out, log_id, log_in).unwrap();
        // Second connect should be caught by InputAlreadyConnected (same port)
        let result = g.connect(event_id, exec_out, log_id, log_in);
        assert!(result.is_err());
    }

    #[test]
    fn test_disconnect() {
        let mut g = make_graph();
        let event_id = g.add_node(NodeKind::OnStart).unwrap();
        let log_id = g.add_node(NodeKind::Log).unwrap();

        let exec_out = g.get_node(event_id).unwrap().find_output("exec").unwrap().id;
        let log_in = g.get_node(log_id).unwrap().find_input("exec").unwrap().id;

        let conn_id = g.connect(event_id, exec_out, log_id, log_in).unwrap();
        let removed = g.disconnect(conn_id).unwrap();
        assert_eq!(removed.id, conn_id);
        assert_eq!(g.connection_count(), 0);
    }

    #[test]
    fn test_disconnect_nonexistent() {
        let mut g = make_graph();
        assert!(matches!(
            g.disconnect(999),
            Err(GraphError::ConnectionNotFound(999))
        ));
    }

    #[test]
    fn test_connections_from() {
        let mut g = make_graph();
        let event_id = g.add_node(NodeKind::OnStart).unwrap();
        let log_id = g.add_node(NodeKind::Log).unwrap();

        let exec_out = g.get_node(event_id).unwrap().find_output("exec").unwrap().id;
        let log_in = g.get_node(log_id).unwrap().find_input("exec").unwrap().id;

        g.connect(event_id, exec_out, log_id, log_in).unwrap();

        assert_eq!(g.connections_from(event_id).len(), 1);
        assert_eq!(g.connections_from(log_id).len(), 0);
    }

    #[test]
    fn test_connections_to() {
        let mut g = make_graph();
        let event_id = g.add_node(NodeKind::OnStart).unwrap();
        let log_id = g.add_node(NodeKind::Log).unwrap();

        let exec_out = g.get_node(event_id).unwrap().find_output("exec").unwrap().id;
        let log_in = g.get_node(log_id).unwrap().find_input("exec").unwrap().id;

        g.connect(event_id, exec_out, log_id, log_in).unwrap();

        assert_eq!(g.connections_to(log_id).len(), 1);
        assert_eq!(g.connections_to(event_id).len(), 0);
    }

    #[test]
    fn test_connections_from_port() {
        let mut g = make_graph();
        let event_id = g.add_node(NodeKind::OnStart).unwrap();
        let log_id = g.add_node(NodeKind::Log).unwrap();

        let exec_out = g.get_node(event_id).unwrap().find_output("exec").unwrap().id;
        let log_in = g.get_node(log_id).unwrap().find_input("exec").unwrap().id;

        g.connect(event_id, exec_out, log_id, log_in).unwrap();

        assert_eq!(g.connections_from_port(exec_out).len(), 1);
        assert_eq!(g.connections_from_port(log_in).len(), 0);
    }

    #[test]
    fn test_connections_to_port() {
        let mut g = make_graph();
        let event_id = g.add_node(NodeKind::OnStart).unwrap();
        let log_id = g.add_node(NodeKind::Log).unwrap();

        let exec_out = g.get_node(event_id).unwrap().find_output("exec").unwrap().id;
        let log_in = g.get_node(log_id).unwrap().find_input("exec").unwrap().id;

        g.connect(event_id, exec_out, log_id, log_in).unwrap();

        assert_eq!(g.connections_to_port(log_in).len(), 1);
        assert_eq!(g.connections_to_port(exec_out).len(), 0);
    }

    #[test]
    fn test_find_port_owner() {
        let mut g = make_graph();
        let id = g.add_node(NodeKind::Add).unwrap();
        let port_id = g.get_node(id).unwrap().inputs[0].id;
        assert_eq!(g.find_port_owner(port_id), Some(id));
        assert_eq!(g.find_port_owner(9999), None);
    }

    #[test]
    fn test_port_data_type() {
        let mut g = make_graph();
        let id = g.add_node(NodeKind::Add).unwrap();
        let port_id = g.get_node(id).unwrap().outputs[0].id;
        assert_eq!(g.port_data_type(port_id), Some(DataType::Float));
        assert_eq!(g.port_data_type(9999), None);
    }

    #[test]
    fn test_clear_graph() {
        let mut g = make_graph();
        g.add_node(NodeKind::OnStart).unwrap();
        g.add_node(NodeKind::Log).unwrap();
        g.clear();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.connection_count(), 0);
    }

    #[test]
    fn test_int_to_float_connection_works() {
        let mut g = make_graph();
        let for_loop = g.add_node(NodeKind::ForLoop).unwrap();
        let add_id = g.add_node(NodeKind::Add).unwrap();

        // ForLoop.index (Int) -> Add.a (Float)
        let index_out = g.get_node(for_loop).unwrap().find_output("index").unwrap().id;
        let add_a = g.get_node(add_id).unwrap().find_input("a").unwrap().id;

        g.connect(for_loop, index_out, add_id, add_a).unwrap();
        assert_eq!(g.connection_count(), 1);
    }

    // Serialization

    #[test]
    fn test_graph_serde_round_trip() {
        let mut g = make_graph();
        let event_id = g.add_node(NodeKind::OnStart).unwrap();
        let log_id = g.add_node(NodeKind::Log).unwrap();

        let exec_out = g.get_node(event_id).unwrap().find_output("exec").unwrap().id;
        let log_in = g.get_node(log_id).unwrap().find_input("exec").unwrap().id;

        g.connect(event_id, exec_out, log_id, log_in).unwrap();

        let json = serde_json::to_string(&g).unwrap();
        let parsed: NodeGraph = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.id, "test-1");
        assert_eq!(parsed.name, "Test Graph");
        assert_eq!(parsed.node_count(), 2);
        assert_eq!(parsed.connection_count(), 1);
    }

    #[test]
    fn test_graph_error_display() {
        assert!(format!("{}", GraphError::NodeNotFound(1)).contains('1'));
        assert!(format!("{}", GraphError::PortNotFound(2)).contains('2'));
        assert!(format!("{}", GraphError::MaxNodesReached).contains("maximum"));
        assert!(format!("{}", GraphError::SelfConnection).contains("itself"));
        assert!(format!(
            "{}",
            GraphError::TypeMismatch {
                from_type: "Int".into(),
                to_type: "Bool".into()
            }
        )
        .contains("Int"));
    }

    // Edge case: multiple nodes iteration
    #[test]
    fn test_iterate_nodes() {
        let mut g = make_graph();
        g.add_node(NodeKind::OnStart).unwrap();
        g.add_node(NodeKind::Log).unwrap();
        g.add_node(NodeKind::Add).unwrap();

        let count = g.nodes().count();
        assert_eq!(count, 3);
    }

    #[test]
    fn test_node_not_found_for_connect() {
        let mut g = make_graph();
        let id = g.add_node(NodeKind::OnStart).unwrap();
        let port = g.get_node(id).unwrap().outputs[0].id;

        assert_eq!(
            g.connect(id, port, 999, 888),
            Err(GraphError::NodeNotFound(999))
        );
    }

    #[test]
    fn test_port_not_found_for_connect() {
        let mut g = make_graph();
        let id1 = g.add_node(NodeKind::OnStart).unwrap();
        let id2 = g.add_node(NodeKind::Log).unwrap();

        assert_eq!(
            g.connect(id1, 9999, id2, 8888),
            Err(GraphError::PortNotFound(9999))
        );
    }
}
