//! Demo application for the Aether Visual Script Editor.
//!
//! Opens the editor window pre-loaded with a sample graph demonstrating:
//! - Event nodes (OnStart, OnInteract)
//! - Flow control (Branch)
//! - Actions (Log, SetPosition)
//! - Math (Add)
//! - Conditions (Greater)
//! - Connections between nodes

use aether_creator_studio::visual_script::{NodeGraph, NodeKind};
use aether_visual_editor::VisualEditorApp;

/// Build a sample graph with several connected nodes.
fn build_sample_graph() -> NodeGraph {
    let mut graph = NodeGraph::new("demo", "Visual Editor Demo");
    graph.description = "A sample graph demonstrating the visual script editor.".to_string();

    // Event: On Start
    let on_start = graph.add_node_at(NodeKind::OnStart, 0.0, 0.0).unwrap();

    // Action: Log "Hello World"
    let log_hello = graph.add_node_at(NodeKind::Log, 300.0, 0.0).unwrap();

    // Connect OnStart -> Log
    let start_exec = graph
        .get_node(on_start)
        .unwrap()
        .find_output("exec")
        .unwrap()
        .id;
    let log_exec_in = graph
        .get_node(log_hello)
        .unwrap()
        .find_input("exec")
        .unwrap()
        .id;
    graph
        .connect(on_start, start_exec, log_hello, log_exec_in)
        .unwrap();

    // Event: On Interact
    let on_interact = graph
        .add_node_at(NodeKind::OnInteract, 0.0, 200.0)
        .unwrap();

    // Branch
    let branch = graph
        .add_node_at(NodeKind::Branch, 300.0, 200.0)
        .unwrap();

    // Connect OnInteract -> Branch
    let interact_exec = graph
        .get_node(on_interact)
        .unwrap()
        .find_output("exec")
        .unwrap()
        .id;
    let branch_exec_in = graph
        .get_node(branch)
        .unwrap()
        .find_input("exec")
        .unwrap()
        .id;
    graph
        .connect(on_interact, interact_exec, branch, branch_exec_in)
        .unwrap();

    // Greater node: compare something
    let greater = graph
        .add_node_at(NodeKind::Greater, 100.0, 350.0)
        .unwrap();

    // Add node
    let _add = graph.add_node_at(NodeKind::Add, 100.0, 500.0).unwrap();

    // Connect Greater.result -> Branch.condition
    let greater_out = graph
        .get_node(greater)
        .unwrap()
        .find_output("result")
        .unwrap()
        .id;
    let branch_cond = graph
        .get_node(branch)
        .unwrap()
        .find_input("condition")
        .unwrap()
        .id;
    graph
        .connect(greater, greater_out, branch, branch_cond)
        .unwrap();

    // Log for true branch
    let log_true = graph.add_node_at(NodeKind::Log, 600.0, 150.0).unwrap();

    // Log for false branch
    let log_false = graph.add_node_at(NodeKind::Log, 600.0, 300.0).unwrap();

    // Connect Branch.true -> log_true
    let branch_true = graph
        .get_node(branch)
        .unwrap()
        .find_output("true")
        .unwrap()
        .id;
    let lt_in = graph
        .get_node(log_true)
        .unwrap()
        .find_input("exec")
        .unwrap()
        .id;
    graph
        .connect(branch, branch_true, log_true, lt_in)
        .unwrap();

    // Connect Branch.false -> log_false
    let branch_false = graph
        .get_node(branch)
        .unwrap()
        .find_output("false")
        .unwrap()
        .id;
    let lf_in = graph
        .get_node(log_false)
        .unwrap()
        .find_input("exec")
        .unwrap()
        .id;
    graph
        .connect(branch, branch_false, log_false, lf_in)
        .unwrap();

    // SetPosition connected to log_true
    let set_pos = graph
        .add_node_at(NodeKind::SetPosition, 900.0, 150.0)
        .unwrap();
    let lt_out = graph
        .get_node(log_true)
        .unwrap()
        .find_output("exec")
        .unwrap()
        .id;
    let sp_in = graph
        .get_node(set_pos)
        .unwrap()
        .find_input("exec")
        .unwrap()
        .id;
    graph
        .connect(log_true, lt_out, set_pos, sp_in)
        .unwrap();

    // Connect the interact entity to SetPosition
    let interact_entity = graph
        .get_node(on_interact)
        .unwrap()
        .find_output("entity")
        .unwrap()
        .id;
    let sp_entity = graph
        .get_node(set_pos)
        .unwrap()
        .find_input("entity")
        .unwrap()
        .id;
    graph
        .connect(on_interact, interact_entity, set_pos, sp_entity)
        .unwrap();

    graph
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_title("Aether Visual Script Editor"),
        ..Default::default()
    };

    let sample_graph = build_sample_graph();

    eframe::run_native(
        "Aether Visual Script Editor",
        options,
        Box::new(move |cc| Ok(Box::new(VisualEditorApp::with_graph(cc, sample_graph)))),
    )
}
