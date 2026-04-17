//! Integration tests for DSL↔visual-graph round-tripping.

use aether_behavior_dsl::{ast_to_graph, graph_to_ast, modules_structurally_equal, parse};

const GOLDENS_THAT_ROUND_TRIP: &[(&str, &str)] = &[
    ("g01_patrol", include_str!("golden/g01_patrol.beh")),
    ("g04_enemy_melee", include_str!("golden/g04_enemy_melee.beh")),
    ("g09_companion", include_str!("golden/g09_companion.beh")),
    ("g10_boss_phase", include_str!("golden/g10_boss_phase.beh")),
];

#[test]
fn selected_goldens_round_trip_through_visual_graph() {
    // The spec requires at least 3 of the 10; we verify 4 for margin.
    for (name, src) in GOLDENS_THAT_ROUND_TRIP {
        let ast = parse(src).unwrap_or_else(|e| panic!("{}: parse failed: {:?}", name, e));
        let graph = ast_to_graph(&ast);
        let back =
            graph_to_ast(&graph).unwrap_or_else(|e| panic!("{}: graph_to_ast failed: {:?}", name, e));
        assert!(
            modules_structurally_equal(&ast, &back),
            "{}: module not structurally equal after round-trip",
            name
        );
    }
}

#[test]
fn visual_graph_is_json_serializable() {
    let src = include_str!("golden/g01_patrol.beh");
    let ast = parse(src).unwrap();
    let graph = ast_to_graph(&ast);
    let json = serde_json::to_string(&graph).expect("serde_json");
    let back: aether_behavior_dsl::VisualGraph = serde_json::from_str(&json).expect("roundtrip");
    let back_ast = graph_to_ast(&back).unwrap();
    assert!(modules_structurally_equal(&ast, &back_ast));
}
