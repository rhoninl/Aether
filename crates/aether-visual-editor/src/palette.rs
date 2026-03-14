//! Node palette sidebar: categorized list of available nodes with search/filter.

use aether_creator_studio::visual_script::{all_templates, instantiate_template, NodeKind};
use egui::{Color32, RichText, Ui};

use crate::state::EditorState;

/// A category in the node palette.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaletteCategory {
    pub name: String,
    pub color: Color32,
    pub entries: Vec<PaletteEntry>,
}

/// A single entry in the node palette.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaletteEntry {
    pub display_name: String,
    pub kind: NodeKind,
}

/// Build the full palette of available nodes, organized by category.
pub fn build_palette() -> Vec<PaletteCategory> {
    vec![
        PaletteCategory {
            name: "Events".to_string(),
            color: Color32::from_rgb(180, 40, 40),
            entries: vec![
                PaletteEntry {
                    display_name: "On Start".to_string(),
                    kind: NodeKind::OnStart,
                },
                PaletteEntry {
                    display_name: "On Interact".to_string(),
                    kind: NodeKind::OnInteract,
                },
                PaletteEntry {
                    display_name: "On Enter".to_string(),
                    kind: NodeKind::OnEnter,
                },
                PaletteEntry {
                    display_name: "On Exit".to_string(),
                    kind: NodeKind::OnExit,
                },
                PaletteEntry {
                    display_name: "On Timer".to_string(),
                    kind: NodeKind::OnTimer { interval_ms: 1000 },
                },
                PaletteEntry {
                    display_name: "On Collision".to_string(),
                    kind: NodeKind::OnCollision,
                },
            ],
        },
        PaletteCategory {
            name: "Flow Control".to_string(),
            color: Color32::from_rgb(200, 120, 40),
            entries: vec![
                PaletteEntry {
                    display_name: "Branch".to_string(),
                    kind: NodeKind::Branch,
                },
                PaletteEntry {
                    display_name: "For Loop".to_string(),
                    kind: NodeKind::ForLoop,
                },
                PaletteEntry {
                    display_name: "Sequence".to_string(),
                    kind: NodeKind::Sequence { output_count: 2 },
                },
                PaletteEntry {
                    display_name: "Delay".to_string(),
                    kind: NodeKind::Delay { delay_ms: 1000 },
                },
            ],
        },
        PaletteCategory {
            name: "Actions".to_string(),
            color: Color32::from_rgb(40, 100, 200),
            entries: vec![
                PaletteEntry {
                    display_name: "Set Position".to_string(),
                    kind: NodeKind::SetPosition,
                },
                PaletteEntry {
                    display_name: "Set Rotation".to_string(),
                    kind: NodeKind::SetRotation,
                },
                PaletteEntry {
                    display_name: "Play Animation".to_string(),
                    kind: NodeKind::PlayAnimation,
                },
                PaletteEntry {
                    display_name: "Play Sound".to_string(),
                    kind: NodeKind::PlaySound,
                },
                PaletteEntry {
                    display_name: "Spawn Entity".to_string(),
                    kind: NodeKind::SpawnEntity,
                },
                PaletteEntry {
                    display_name: "Destroy Entity".to_string(),
                    kind: NodeKind::DestroyEntity,
                },
                PaletteEntry {
                    display_name: "Send Message".to_string(),
                    kind: NodeKind::SendMessage,
                },
                PaletteEntry {
                    display_name: "Log".to_string(),
                    kind: NodeKind::Log,
                },
            ],
        },
        PaletteCategory {
            name: "Variables".to_string(),
            color: Color32::from_rgb(40, 160, 40),
            entries: vec![
                PaletteEntry {
                    display_name: "Get Variable".to_string(),
                    kind: NodeKind::GetVariable {
                        var_name: "myVar".to_string(),
                    },
                },
                PaletteEntry {
                    display_name: "Set Variable".to_string(),
                    kind: NodeKind::SetVariable {
                        var_name: "myVar".to_string(),
                    },
                },
            ],
        },
        PaletteCategory {
            name: "Math".to_string(),
            color: Color32::from_rgb(140, 40, 180),
            entries: vec![
                PaletteEntry {
                    display_name: "Add".to_string(),
                    kind: NodeKind::Add,
                },
                PaletteEntry {
                    display_name: "Subtract".to_string(),
                    kind: NodeKind::Subtract,
                },
                PaletteEntry {
                    display_name: "Multiply".to_string(),
                    kind: NodeKind::Multiply,
                },
                PaletteEntry {
                    display_name: "Divide".to_string(),
                    kind: NodeKind::Divide,
                },
                PaletteEntry {
                    display_name: "Clamp".to_string(),
                    kind: NodeKind::Clamp,
                },
                PaletteEntry {
                    display_name: "Lerp".to_string(),
                    kind: NodeKind::Lerp,
                },
                PaletteEntry {
                    display_name: "Random Range".to_string(),
                    kind: NodeKind::RandomRange,
                },
            ],
        },
        PaletteCategory {
            name: "Conditions".to_string(),
            color: Color32::from_rgb(200, 200, 40),
            entries: vec![
                PaletteEntry {
                    display_name: "Equal".to_string(),
                    kind: NodeKind::Equal,
                },
                PaletteEntry {
                    display_name: "Not Equal".to_string(),
                    kind: NodeKind::NotEqual,
                },
                PaletteEntry {
                    display_name: "Greater".to_string(),
                    kind: NodeKind::Greater,
                },
                PaletteEntry {
                    display_name: "Less".to_string(),
                    kind: NodeKind::Less,
                },
                PaletteEntry {
                    display_name: "And".to_string(),
                    kind: NodeKind::And,
                },
                PaletteEntry {
                    display_name: "Or".to_string(),
                    kind: NodeKind::Or,
                },
                PaletteEntry {
                    display_name: "Not".to_string(),
                    kind: NodeKind::Not,
                },
            ],
        },
    ]
}

/// Filter palette entries by a search query.
pub fn filter_palette<'a>(
    categories: &'a [PaletteCategory],
    query: &str,
) -> Vec<&'a PaletteEntry> {
    if query.is_empty() {
        return Vec::new();
    }
    let lower_query = query.to_lowercase();
    let mut results = Vec::new();
    for cat in categories {
        for entry in &cat.entries {
            if entry.display_name.to_lowercase().contains(&lower_query) {
                results.push(entry);
            }
        }
    }
    results
}

/// Draw the palette sidebar. Returns the NodeKind to add if the user selected one.
pub fn draw_palette(ui: &mut Ui, state: &mut EditorState) -> Option<NodeKind> {
    let palette = build_palette();
    let mut selected_kind: Option<NodeKind> = None;

    ui.heading("Node Palette");
    ui.separator();

    // Search bar
    ui.horizontal(|ui| {
        ui.label("Search:");
        ui.text_edit_singleline(&mut state.palette_search);
    });
    ui.separator();

    // If searching, show filtered results
    if !state.palette_search.is_empty() {
        let results = filter_palette(&palette, &state.palette_search);
        if results.is_empty() {
            ui.label("No matches found.");
        } else {
            for entry in results {
                if ui.button(&entry.display_name).double_clicked() {
                    selected_kind = Some(entry.kind.clone());
                }
            }
        }
        ui.separator();
    }

    // Categorized list
    egui::ScrollArea::vertical().show(ui, |ui| {
        for category in &palette {
            ui.collapsing(
                RichText::new(&category.name).color(category.color).strong(),
                |ui| {
                    for entry in &category.entries {
                        if ui.button(&entry.display_name).double_clicked() {
                            selected_kind = Some(entry.kind.clone());
                        }
                    }
                },
            );
        }

        // Templates section
        ui.separator();
        ui.collapsing(
            RichText::new("Templates").color(Color32::from_rgb(200, 200, 200)).strong(),
            |ui| {
                for template_kind in all_templates() {
                    let name = template_kind.display_name();
                    let desc = template_kind.description();
                    if ui.button(name).on_hover_text(desc).clicked() {
                        let template_graph = instantiate_template(template_kind);
                        state.push_undo("Load template");
                        state.graph = template_graph;
                        state.selection.clear();
                        state.set_status(
                            &format!("Loaded template: {}", name),
                            false,
                        );
                    }
                }
            },
        );
    });

    selected_kind
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_palette_has_categories() {
        let palette = build_palette();
        assert_eq!(palette.len(), 6);
        assert_eq!(palette[0].name, "Events");
        assert_eq!(palette[1].name, "Flow Control");
        assert_eq!(palette[2].name, "Actions");
        assert_eq!(palette[3].name, "Variables");
        assert_eq!(palette[4].name, "Math");
        assert_eq!(palette[5].name, "Conditions");
    }

    #[test]
    fn test_build_palette_event_entries() {
        let palette = build_palette();
        let events = &palette[0];
        assert_eq!(events.entries.len(), 6);
        assert_eq!(events.entries[0].display_name, "On Start");
    }

    #[test]
    fn test_build_palette_all_entries_have_names() {
        let palette = build_palette();
        for cat in &palette {
            for entry in &cat.entries {
                assert!(!entry.display_name.is_empty());
            }
        }
    }

    #[test]
    fn test_filter_palette_empty_query() {
        let palette = build_palette();
        let results = filter_palette(&palette, "");
        assert!(results.is_empty());
    }

    #[test]
    fn test_filter_palette_exact_match() {
        let palette = build_palette();
        let results = filter_palette(&palette, "Branch");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].display_name, "Branch");
    }

    #[test]
    fn test_filter_palette_partial_match() {
        let palette = build_palette();
        let results = filter_palette(&palette, "on");
        // Should match "On Start", "On Interact", "On Enter", "On Exit", "On Timer",
        // "On Collision", "Set Position", "Set Rotation", "Play Animation", "Conditions"
        assert!(results.len() >= 5);
    }

    #[test]
    fn test_filter_palette_case_insensitive() {
        let palette = build_palette();
        let results_lower = filter_palette(&palette, "add");
        let results_upper = filter_palette(&palette, "ADD");
        assert_eq!(results_lower.len(), results_upper.len());
        assert!(!results_lower.is_empty());
    }

    #[test]
    fn test_filter_palette_no_match() {
        let palette = build_palette();
        let results = filter_palette(&palette, "xyznonexistent");
        assert!(results.is_empty());
    }

    #[test]
    fn test_filter_palette_math_category() {
        let palette = build_palette();
        let results = filter_palette(&palette, "Multiply");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].display_name, "Multiply");
    }

    #[test]
    fn test_palette_total_entries() {
        let palette = build_palette();
        let total: usize = palette.iter().map(|c| c.entries.len()).sum();
        // 6 events + 4 flow + 8 actions + 2 variables + 7 math + 7 conditions = 34
        assert_eq!(total, 34);
    }
}
