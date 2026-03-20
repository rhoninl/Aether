//! Pre-built game logic templates for common visual scripting patterns.

use super::graph::NodeGraph;
use super::node::NodeKind;

/// Identifies a built-in template.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemplateKind {
    /// OnInteract -> Log
    OnInteractLog,
    /// OnEnter -> SetPosition
    OnEnterTeleport,
    /// OnTimer -> Log (periodic)
    TimerLog,
    /// OnStart -> SetVariable
    OnStartInitialize,
    /// OnInteract -> Branch -> Log (true) / Log (false)
    OnInteractBranch,
    /// OnCollision -> DestroyEntity
    OnCollisionDestroy,
}

impl TemplateKind {
    /// Human-readable name for the template.
    pub fn display_name(self) -> &'static str {
        match self {
            TemplateKind::OnInteractLog => "On Interact - Log",
            TemplateKind::OnEnterTeleport => "On Enter - Teleport",
            TemplateKind::TimerLog => "Timer - Log",
            TemplateKind::OnStartInitialize => "On Start - Initialize Variable",
            TemplateKind::OnInteractBranch => "On Interact - Branch",
            TemplateKind::OnCollisionDestroy => "On Collision - Destroy",
        }
    }

    /// Description of what this template does.
    pub fn description(self) -> &'static str {
        match self {
            TemplateKind::OnInteractLog => "Logs a message when a player interacts with an entity.",
            TemplateKind::OnEnterTeleport => {
                "Teleports the player to a position when they enter a trigger zone."
            }
            TemplateKind::TimerLog => "Logs a message periodically on a timer.",
            TemplateKind::OnStartInitialize => "Initializes a variable when the world starts.",
            TemplateKind::OnInteractBranch => "Branches based on a condition when interacted with.",
            TemplateKind::OnCollisionDestroy => "Destroys the other entity on collision.",
        }
    }
}

/// All available template kinds.
pub fn all_templates() -> Vec<TemplateKind> {
    vec![
        TemplateKind::OnInteractLog,
        TemplateKind::OnEnterTeleport,
        TemplateKind::TimerLog,
        TemplateKind::OnStartInitialize,
        TemplateKind::OnInteractBranch,
        TemplateKind::OnCollisionDestroy,
    ]
}

/// Instantiate a template, producing a ready-to-use NodeGraph.
pub fn instantiate_template(kind: TemplateKind) -> NodeGraph {
    let mut graph = NodeGraph::new(
        format!("template-{:?}", kind).to_lowercase(),
        kind.display_name(),
    );
    graph.description = kind.description().to_string();

    match kind {
        TemplateKind::OnInteractLog => build_on_interact_log(&mut graph),
        TemplateKind::OnEnterTeleport => build_on_enter_teleport(&mut graph),
        TemplateKind::TimerLog => build_timer_log(&mut graph),
        TemplateKind::OnStartInitialize => build_on_start_initialize(&mut graph),
        TemplateKind::OnInteractBranch => build_on_interact_branch(&mut graph),
        TemplateKind::OnCollisionDestroy => build_on_collision_destroy(&mut graph),
    }

    graph
}

fn build_on_interact_log(graph: &mut NodeGraph) {
    let event = graph.add_node_at(NodeKind::OnInteract, 0.0, 0.0).unwrap();
    let log = graph.add_node_at(NodeKind::Log, 250.0, 0.0).unwrap();

    let exec_out = graph
        .get_node(event)
        .unwrap()
        .find_output("exec")
        .unwrap()
        .id;
    let exec_in = graph.get_node(log).unwrap().find_input("exec").unwrap().id;
    graph.connect(event, exec_out, log, exec_in).unwrap();
}

fn build_on_enter_teleport(graph: &mut NodeGraph) {
    let event = graph.add_node_at(NodeKind::OnEnter, 0.0, 0.0).unwrap();
    let set_pos = graph
        .add_node_at(NodeKind::SetPosition, 250.0, 0.0)
        .unwrap();

    let exec_out = graph
        .get_node(event)
        .unwrap()
        .find_output("exec")
        .unwrap()
        .id;
    let exec_in = graph
        .get_node(set_pos)
        .unwrap()
        .find_input("exec")
        .unwrap()
        .id;
    graph.connect(event, exec_out, set_pos, exec_in).unwrap();

    let entity_out = graph
        .get_node(event)
        .unwrap()
        .find_output("entity")
        .unwrap()
        .id;
    let entity_in = graph
        .get_node(set_pos)
        .unwrap()
        .find_input("entity")
        .unwrap()
        .id;
    graph
        .connect(event, entity_out, set_pos, entity_in)
        .unwrap();
}

fn build_timer_log(graph: &mut NodeGraph) {
    let event = graph
        .add_node_at(NodeKind::OnTimer { interval_ms: 5000 }, 0.0, 0.0)
        .unwrap();
    let log = graph.add_node_at(NodeKind::Log, 250.0, 0.0).unwrap();

    let exec_out = graph
        .get_node(event)
        .unwrap()
        .find_output("exec")
        .unwrap()
        .id;
    let exec_in = graph.get_node(log).unwrap().find_input("exec").unwrap().id;
    graph.connect(event, exec_out, log, exec_in).unwrap();
}

fn build_on_start_initialize(graph: &mut NodeGraph) {
    let event = graph.add_node_at(NodeKind::OnStart, 0.0, 0.0).unwrap();
    let set_var = graph
        .add_node_at(
            NodeKind::SetVariable {
                var_name: "score".into(),
            },
            250.0,
            0.0,
        )
        .unwrap();

    let exec_out = graph
        .get_node(event)
        .unwrap()
        .find_output("exec")
        .unwrap()
        .id;
    let exec_in = graph
        .get_node(set_var)
        .unwrap()
        .find_input("exec")
        .unwrap()
        .id;
    graph.connect(event, exec_out, set_var, exec_in).unwrap();
}

fn build_on_interact_branch(graph: &mut NodeGraph) {
    let event = graph.add_node_at(NodeKind::OnInteract, 0.0, 0.0).unwrap();
    let branch = graph.add_node_at(NodeKind::Branch, 250.0, 0.0).unwrap();
    let log_true = graph.add_node_at(NodeKind::Log, 500.0, -50.0).unwrap();
    let log_false = graph.add_node_at(NodeKind::Log, 500.0, 50.0).unwrap();

    let ev_exec = graph
        .get_node(event)
        .unwrap()
        .find_output("exec")
        .unwrap()
        .id;
    let br_in = graph
        .get_node(branch)
        .unwrap()
        .find_input("exec")
        .unwrap()
        .id;
    graph.connect(event, ev_exec, branch, br_in).unwrap();

    let br_true = graph
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
    graph.connect(branch, br_true, log_true, lt_in).unwrap();

    let br_false = graph
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
    graph.connect(branch, br_false, log_false, lf_in).unwrap();
}

fn build_on_collision_destroy(graph: &mut NodeGraph) {
    let event = graph.add_node_at(NodeKind::OnCollision, 0.0, 0.0).unwrap();
    let destroy = graph
        .add_node_at(NodeKind::DestroyEntity, 250.0, 0.0)
        .unwrap();

    let exec_out = graph
        .get_node(event)
        .unwrap()
        .find_output("exec")
        .unwrap()
        .id;
    let exec_in = graph
        .get_node(destroy)
        .unwrap()
        .find_input("exec")
        .unwrap()
        .id;
    graph.connect(event, exec_out, destroy, exec_in).unwrap();

    let other_entity = graph
        .get_node(event)
        .unwrap()
        .find_output("other_entity")
        .unwrap()
        .id;
    let entity_in = graph
        .get_node(destroy)
        .unwrap()
        .find_input("entity")
        .unwrap()
        .id;
    graph
        .connect(event, other_entity, destroy, entity_in)
        .unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::visual_script::compiler::compile;
    use crate::visual_script::validation::validate_graph;

    #[test]
    fn test_all_templates_listed() {
        let templates = all_templates();
        assert_eq!(templates.len(), 6);
    }

    #[test]
    fn test_template_display_names() {
        for kind in all_templates() {
            let name = kind.display_name();
            assert!(!name.is_empty(), "template {:?} has empty name", kind);
        }
    }

    #[test]
    fn test_template_descriptions() {
        for kind in all_templates() {
            let desc = kind.description();
            assert!(
                !desc.is_empty(),
                "template {:?} has empty description",
                kind
            );
        }
    }

    #[test]
    fn test_on_interact_log_template() {
        let g = instantiate_template(TemplateKind::OnInteractLog);
        assert_eq!(g.node_count(), 2);
        assert_eq!(g.connection_count(), 1);
        assert_eq!(g.event_nodes().len(), 1);
    }

    #[test]
    fn test_on_enter_teleport_template() {
        let g = instantiate_template(TemplateKind::OnEnterTeleport);
        assert_eq!(g.node_count(), 2);
        assert_eq!(g.connection_count(), 2); // exec + entity
        assert_eq!(g.event_nodes().len(), 1);
    }

    #[test]
    fn test_timer_log_template() {
        let g = instantiate_template(TemplateKind::TimerLog);
        assert_eq!(g.node_count(), 2);
        assert_eq!(g.connection_count(), 1);
    }

    #[test]
    fn test_on_start_initialize_template() {
        let g = instantiate_template(TemplateKind::OnStartInitialize);
        assert_eq!(g.node_count(), 2);
        assert_eq!(g.connection_count(), 1);
    }

    #[test]
    fn test_on_interact_branch_template() {
        let g = instantiate_template(TemplateKind::OnInteractBranch);
        assert_eq!(g.node_count(), 4); // event, branch, 2 logs
        assert_eq!(g.connection_count(), 3); // exec->branch, true->log, false->log
    }

    #[test]
    fn test_on_collision_destroy_template() {
        let g = instantiate_template(TemplateKind::OnCollisionDestroy);
        assert_eq!(g.node_count(), 2);
        assert_eq!(g.connection_count(), 2); // exec + entity
    }

    // Validate all templates produce valid graphs
    #[test]
    fn test_all_templates_validate() {
        for kind in all_templates() {
            let g = instantiate_template(kind);
            let result = validate_graph(&g);
            assert!(
                result.is_valid(),
                "template {:?} failed validation: {:?}",
                kind,
                result.diagnostics
            );
        }
    }

    // Compile all templates
    #[test]
    fn test_all_templates_compile() {
        for kind in all_templates() {
            let g = instantiate_template(kind);
            let result = compile(&g);
            assert!(
                result.is_ok(),
                "template {:?} failed compilation: {:?}",
                kind,
                result.err()
            );
        }
    }

    #[test]
    fn test_template_graph_has_description() {
        for kind in all_templates() {
            let g = instantiate_template(kind);
            assert!(
                !g.description.is_empty(),
                "template {:?} graph has no description",
                kind
            );
        }
    }

    #[test]
    fn test_template_positions_set() {
        let g = instantiate_template(TemplateKind::OnInteractLog);
        let nodes: Vec<_> = g.nodes().collect();
        // The event should be at (0, 0) and the log at (250, 0)
        let event = nodes.iter().find(|n| n.kind.is_event()).unwrap();
        assert_eq!(event.position, (0.0, 0.0));

        let action = nodes.iter().find(|n| !n.kind.is_event()).unwrap();
        assert_eq!(action.position, (250.0, 0.0));
    }

    #[test]
    fn test_branch_template_positions() {
        let g = instantiate_template(TemplateKind::OnInteractBranch);
        // 4 nodes at different positions
        let positions: Vec<_> = g.nodes().map(|n| n.position).collect();
        // Should have distinct positions
        let unique: std::collections::HashSet<_> = positions
            .iter()
            .map(|(x, y)| ((*x * 100.0) as i32, (*y * 100.0) as i32))
            .collect();
        assert!(
            unique.len() >= 3,
            "should have at least 3 distinct positions"
        );
    }
}
