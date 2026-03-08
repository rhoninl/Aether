//! Automatic node layout engine using a simplified Sugiyama layered layout.

use std::collections::{HashMap, HashSet, VecDeque};

use super::graph::NodeGraph;
use super::node::NodeId;
use super::types::DataType;

/// Horizontal spacing between node layers (pixels).
const DEFAULT_NODE_SPACING_X: f32 = 250.0;

/// Vertical spacing between nodes within a layer (pixels).
const DEFAULT_NODE_SPACING_Y: f32 = 100.0;

/// Configuration for the layout engine.
#[derive(Debug, Clone, Copy)]
pub struct LayoutConfig {
    pub spacing_x: f32,
    pub spacing_y: f32,
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            spacing_x: DEFAULT_NODE_SPACING_X,
            spacing_y: DEFAULT_NODE_SPACING_Y,
        }
    }
}

/// Computed layout: maps each node id to its (x, y) position.
pub type LayoutResult = HashMap<NodeId, (f32, f32)>;

/// Compute a layered layout for the given node graph.
///
/// Algorithm:
/// 1. Assign layers via BFS from event/source nodes.
/// 2. Order nodes within each layer using a barycenter heuristic.
/// 3. Assign (x, y) coordinates based on layer and position within layer.
pub fn compute_layout(graph: &NodeGraph, config: &LayoutConfig) -> LayoutResult {
    let layers = assign_layers(graph);
    let ordered = order_within_layers(graph, &layers);
    assign_positions(&ordered, config)
}

/// Apply a computed layout to the graph, updating each node's position.
pub fn apply_layout(graph: &mut NodeGraph, layout: &LayoutResult) {
    for (&node_id, &(x, y)) in layout {
        if let Some(node) = graph.get_node_mut(node_id) {
            node.position = (x, y);
        }
    }
}

/// Assign each node to a layer (depth) via BFS from source nodes (events or nodes
/// with no incoming flow connections).
fn assign_layers(graph: &NodeGraph) -> HashMap<NodeId, usize> {
    let mut layers: HashMap<NodeId, usize> = HashMap::new();

    // Build flow adjacency
    let mut flow_adjacency: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
    let mut has_incoming_flow: HashSet<NodeId> = HashSet::new();

    for conn in graph.connections() {
        let is_flow = graph
            .port_data_type(conn.from_port)
            .map(|dt| dt == DataType::Flow)
            .unwrap_or(false);
        if is_flow {
            flow_adjacency
                .entry(conn.from_node)
                .or_default()
                .push(conn.to_node);
            has_incoming_flow.insert(conn.to_node);
        }
    }

    // Source nodes: event nodes or flow nodes with no incoming flow
    let mut sources: Vec<NodeId> = Vec::new();
    for node in graph.nodes() {
        let has_flow_ports = node
            .inputs
            .iter()
            .chain(node.outputs.iter())
            .any(|p| p.data_type == DataType::Flow);

        if node.kind.is_event() || (has_flow_ports && !has_incoming_flow.contains(&node.id)) {
            sources.push(node.id);
        }
    }

    // BFS from sources
    let mut queue: VecDeque<NodeId> = VecDeque::new();
    for &src in &sources {
        layers.insert(src, 0);
        queue.push_back(src);
    }

    while let Some(node_id) = queue.pop_front() {
        let current_layer = *layers.get(&node_id).unwrap_or(&0);
        if let Some(neighbors) = flow_adjacency.get(&node_id) {
            for &next in neighbors {
                let new_layer = current_layer + 1;
                let existing = layers.get(&next).copied();
                if existing.map_or(true, |l| l < new_layer) {
                    layers.insert(next, new_layer);
                    queue.push_back(next);
                }
            }
        }
    }

    // Assign pure (data-only) nodes to the layer just before their first consumer
    for node in graph.nodes() {
        if !layers.contains_key(&node.id) {
            // Find the minimum layer of any node this feeds into
            let min_consumer_layer = graph
                .connections_from(node.id)
                .iter()
                .filter_map(|c| layers.get(&c.to_node))
                .min()
                .copied();

            let layer = min_consumer_layer.map_or(0, |l| l.saturating_sub(1));
            layers.insert(node.id, layer);
        }
    }

    layers
}

/// Order nodes within each layer using a barycenter heuristic.
///
/// Returns layers as a Vec of Vec<NodeId>, where index is the layer number.
fn order_within_layers(
    graph: &NodeGraph,
    layer_map: &HashMap<NodeId, usize>,
) -> Vec<Vec<NodeId>> {
    if layer_map.is_empty() {
        return Vec::new();
    }

    let max_layer = layer_map.values().max().copied().unwrap_or(0);
    let mut layers: Vec<Vec<NodeId>> = vec![Vec::new(); max_layer + 1];

    for (&node_id, &layer) in layer_map {
        layers[layer].push(node_id);
    }

    // Barycenter ordering: for each layer (after the first), order nodes by
    // the average position of their predecessors in the previous layer.
    for layer_idx in 1..layers.len() {
        let prev_layer = &layers[layer_idx - 1];
        let prev_positions: HashMap<NodeId, usize> = prev_layer
            .iter()
            .enumerate()
            .map(|(pos, &id)| (id, pos))
            .collect();

        let mut scored: Vec<(NodeId, f32)> = layers[layer_idx]
            .iter()
            .map(|&node_id| {
                let incoming = graph.connections_to(node_id);
                let positions: Vec<f32> = incoming
                    .iter()
                    .filter_map(|c| prev_positions.get(&c.from_node))
                    .map(|&p| p as f32)
                    .collect();
                let barycenter = if positions.is_empty() {
                    0.0
                } else {
                    positions.iter().sum::<f32>() / positions.len() as f32
                };
                (node_id, barycenter)
            })
            .collect();

        scored.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        layers[layer_idx] = scored.into_iter().map(|(id, _)| id).collect();
    }

    // Sort first layer by node id for determinism
    if !layers.is_empty() {
        layers[0].sort();
    }

    layers
}

/// Assign (x, y) positions based on layer and position within layer.
fn assign_positions(layers: &[Vec<NodeId>], config: &LayoutConfig) -> LayoutResult {
    let mut result = HashMap::new();

    for (layer_idx, layer) in layers.iter().enumerate() {
        let x = layer_idx as f32 * config.spacing_x;
        for (pos_idx, &node_id) in layer.iter().enumerate() {
            let y = pos_idx as f32 * config.spacing_y;
            result.insert(node_id, (x, y));
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::visual_script::graph::NodeGraph;
    use crate::visual_script::node::NodeKind;

    #[test]
    fn test_layout_config_default() {
        let config = LayoutConfig::default();
        assert_eq!(config.spacing_x, DEFAULT_NODE_SPACING_X);
        assert_eq!(config.spacing_y, DEFAULT_NODE_SPACING_Y);
    }

    #[test]
    fn test_empty_graph_layout() {
        let g = NodeGraph::new("test", "Empty");
        let config = LayoutConfig::default();
        let layout = compute_layout(&g, &config);
        assert!(layout.is_empty());
    }

    #[test]
    fn test_single_node_layout() {
        let mut g = NodeGraph::new("test", "Single");
        let id = g.add_node(NodeKind::OnStart).unwrap();
        let config = LayoutConfig::default();
        let layout = compute_layout(&g, &config);

        assert_eq!(layout.len(), 1);
        let (x, y) = layout[&id];
        assert_eq!(x, 0.0);
        assert_eq!(y, 0.0);
    }

    #[test]
    fn test_chain_layout_layers() {
        let mut g = NodeGraph::new("test", "Chain");
        let event = g.add_node(NodeKind::OnStart).unwrap();
        let log1 = g.add_node(NodeKind::Log).unwrap();
        let log2 = g.add_node(NodeKind::Log).unwrap();

        let ev_exec = g.get_node(event).unwrap().find_output("exec").unwrap().id;
        let l1_in = g.get_node(log1).unwrap().find_input("exec").unwrap().id;
        let l1_out = g.get_node(log1).unwrap().find_output("exec").unwrap().id;
        let l2_in = g.get_node(log2).unwrap().find_input("exec").unwrap().id;

        g.connect(event, ev_exec, log1, l1_in).unwrap();
        g.connect(log1, l1_out, log2, l2_in).unwrap();

        let config = LayoutConfig::default();
        let layout = compute_layout(&g, &config);

        // Event at layer 0, log1 at layer 1, log2 at layer 2
        let (ex, _) = layout[&event];
        let (l1x, _) = layout[&log1];
        let (l2x, _) = layout[&log2];

        assert!(ex < l1x, "event x ({ex}) should be < log1 x ({l1x})");
        assert!(l1x < l2x, "log1 x ({l1x}) should be < log2 x ({l2x})");
    }

    #[test]
    fn test_branch_layout_spacing() {
        let mut g = NodeGraph::new("test", "Branch");
        let event = g.add_node(NodeKind::OnStart).unwrap();
        let branch = g.add_node(NodeKind::Branch).unwrap();
        let log_t = g.add_node(NodeKind::Log).unwrap();
        let log_f = g.add_node(NodeKind::Log).unwrap();

        let ev_exec = g.get_node(event).unwrap().find_output("exec").unwrap().id;
        let br_in = g.get_node(branch).unwrap().find_input("exec").unwrap().id;
        g.connect(event, ev_exec, branch, br_in).unwrap();

        let br_true = g.get_node(branch).unwrap().find_output("true").unwrap().id;
        let br_false = g.get_node(branch).unwrap().find_output("false").unwrap().id;
        let lt_in = g.get_node(log_t).unwrap().find_input("exec").unwrap().id;
        let lf_in = g.get_node(log_f).unwrap().find_input("exec").unwrap().id;

        g.connect(branch, br_true, log_t, lt_in).unwrap();
        g.connect(branch, br_false, log_f, lf_in).unwrap();

        let config = LayoutConfig::default();
        let layout = compute_layout(&g, &config);

        // Both log nodes should be in the same layer (layer 2) but at different y positions
        let (lt_x, lt_y) = layout[&log_t];
        let (lf_x, lf_y) = layout[&log_f];

        assert_eq!(lt_x, lf_x, "branch outputs in same layer");
        assert!((lt_y - lf_y).abs() >= config.spacing_y, "vertical spacing");
    }

    #[test]
    fn test_pure_node_layout() {
        let mut g = NodeGraph::new("test", "Pure");
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

        let config = LayoutConfig::default();
        let layout = compute_layout(&g, &config);

        // Pure node (add) should be before its consumer (log)
        let (add_x, _) = layout[&add];
        let (log_x, _) = layout[&log];
        assert!(
            add_x <= log_x,
            "add x ({add_x}) should be <= log x ({log_x})"
        );
    }

    #[test]
    fn test_apply_layout() {
        let mut g = NodeGraph::new("test", "Apply");
        let id = g.add_node(NodeKind::OnStart).unwrap();

        let mut layout = HashMap::new();
        layout.insert(id, (100.0, 200.0));

        apply_layout(&mut g, &layout);
        assert_eq!(g.get_node(id).unwrap().position, (100.0, 200.0));
    }

    #[test]
    fn test_apply_layout_nonexistent_node() {
        let mut g = NodeGraph::new("test", "Apply");
        let mut layout = HashMap::new();
        layout.insert(999, (100.0, 200.0));
        // Should not panic
        apply_layout(&mut g, &layout);
    }

    #[test]
    fn test_custom_spacing() {
        let mut g = NodeGraph::new("test", "Custom");
        let event = g.add_node(NodeKind::OnStart).unwrap();
        let log = g.add_node(NodeKind::Log).unwrap();

        let ev_exec = g.get_node(event).unwrap().find_output("exec").unwrap().id;
        let log_in = g.get_node(log).unwrap().find_input("exec").unwrap().id;
        g.connect(event, ev_exec, log, log_in).unwrap();

        let config = LayoutConfig {
            spacing_x: 500.0,
            spacing_y: 200.0,
        };
        let layout = compute_layout(&g, &config);

        let (ev_x, _) = layout[&event];
        let (log_x, _) = layout[&log];
        assert_eq!(log_x - ev_x, 500.0);
    }

    #[test]
    fn test_layer_assignment_stability() {
        let mut g = NodeGraph::new("test", "Stability");
        let e = g.add_node(NodeKind::OnStart).unwrap();
        let l1 = g.add_node(NodeKind::Log).unwrap();
        let l2 = g.add_node(NodeKind::Log).unwrap();
        let l3 = g.add_node(NodeKind::Log).unwrap();

        let e_exec = g.get_node(e).unwrap().find_output("exec").unwrap().id;
        let l1_in = g.get_node(l1).unwrap().find_input("exec").unwrap().id;
        let l1_out = g.get_node(l1).unwrap().find_output("exec").unwrap().id;
        let l2_in = g.get_node(l2).unwrap().find_input("exec").unwrap().id;
        let l2_out = g.get_node(l2).unwrap().find_output("exec").unwrap().id;
        let l3_in = g.get_node(l3).unwrap().find_input("exec").unwrap().id;

        g.connect(e, e_exec, l1, l1_in).unwrap();
        g.connect(l1, l1_out, l2, l2_in).unwrap();
        g.connect(l2, l2_out, l3, l3_in).unwrap();

        let config = LayoutConfig::default();
        // Run twice and check same results
        let layout1 = compute_layout(&g, &config);
        let layout2 = compute_layout(&g, &config);

        for (&id, &pos1) in &layout1 {
            let pos2 = layout2[&id];
            assert_eq!(pos1, pos2, "layout should be deterministic for node {id}");
        }
    }

    #[test]
    fn test_assign_positions_empty_layers() {
        let layers: Vec<Vec<NodeId>> = Vec::new();
        let config = LayoutConfig::default();
        let result = assign_positions(&layers, &config);
        assert!(result.is_empty());
    }

    #[test]
    fn test_multiple_events_layout() {
        let mut g = NodeGraph::new("test", "Multi");
        let e1 = g.add_node(NodeKind::OnStart).unwrap();
        let e2 = g.add_node(NodeKind::OnInteract).unwrap();

        let config = LayoutConfig::default();
        let layout = compute_layout(&g, &config);

        // Both events should be in layer 0
        let (e1x, _) = layout[&e1];
        let (e2x, _) = layout[&e2];
        assert_eq!(e1x, 0.0);
        assert_eq!(e2x, 0.0);
    }
}
