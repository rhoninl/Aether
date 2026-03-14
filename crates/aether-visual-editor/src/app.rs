//! Main editor application: ties all components together as an eframe::App.

use aether_creator_studio::visual_script::{DataType, NodeGraph};
use egui::{CentralPanel, Color32, Pos2, Rect, SidePanel, TopBottomPanel};

use crate::canvas::{draw_grid, snap_to_grid};
use crate::connection_renderer::{draw_connections, draw_pending_connection};
use crate::interaction::{handle_canvas_interaction, InteractionState};
use crate::minimap::draw_minimap;
use crate::node_renderer::{draw_node, port_screen_pos};
use crate::palette::draw_palette;
use crate::properties::draw_properties;
use crate::state::EditorState;
use crate::toolbar::draw_toolbar;

/// Left panel width.
const LEFT_PANEL_WIDTH: f32 = 220.0;

/// Right panel width.
const RIGHT_PANEL_WIDTH: f32 = 260.0;

/// Status bar height.
const STATUS_BAR_HEIGHT: f32 = 24.0;

/// The main visual script editor application.
pub struct VisualEditorApp {
    pub state: EditorState,
    interaction: InteractionState,
}

impl VisualEditorApp {
    /// Create a new editor with a default empty graph.
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            state: EditorState::new(),
            interaction: InteractionState::default(),
        }
    }

    /// Create a new editor pre-loaded with the given graph.
    pub fn with_graph(_cc: &eframe::CreationContext<'_>, graph: NodeGraph) -> Self {
        Self {
            state: EditorState::with_graph(graph),
            interaction: InteractionState::default(),
        }
    }
}

impl eframe::App for VisualEditorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Dark theme
        ctx.set_visuals(egui::Visuals::dark());

        // Toolbar at top
        TopBottomPanel::top("toolbar").show(ctx, |ui| {
            draw_toolbar(ui, &mut self.state);
        });

        // Status bar at bottom
        TopBottomPanel::bottom("status_bar")
            .exact_height(STATUS_BAR_HEIGHT)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if let Some(status) = &self.state.status {
                        let color = if status.is_error {
                            Color32::from_rgb(255, 80, 80)
                        } else {
                            Color32::from_rgb(180, 255, 180)
                        };
                        ui.colored_label(color, &status.text);
                    }
                });
            });

        // Left panel: Node Palette
        SidePanel::left("palette_panel")
            .default_width(LEFT_PANEL_WIDTH)
            .show(ctx, |ui| {
                let selected_kind = draw_palette(ui, &mut self.state);
                if let Some(kind) = selected_kind {
                    // Add node at center of viewport
                    let center_canvas = self.state.view.screen_to_canvas(
                        Pos2::new(400.0, 300.0),
                        Pos2::ZERO,
                    );
                    let snapped = snap_to_grid(center_canvas);
                    let _ = self.state.add_node_at(kind, snapped.x, snapped.y);
                }
            });

        // Right panel: Properties
        if self.state.show_properties {
            SidePanel::right("properties_panel")
                .default_width(RIGHT_PANEL_WIDTH)
                .show(ctx, |ui| {
                    draw_properties(ui, &mut self.state);
                });
        }

        // Central panel: Canvas
        CentralPanel::default().show(ctx, |ui| {
            let (response, painter) = ui.allocate_painter(
                ui.available_size(),
                egui::Sense::click_and_drag(),
            );

            let viewport = response.rect;

            // Draw grid background
            draw_grid(&painter, &self.state.view, viewport);

            // Draw connections
            draw_connections(
                &painter,
                &self.state.graph,
                &self.state.view,
                viewport.min,
            );

            // Draw pending connection
            if let Some(pending) = &self.interaction.pending_connection {
                let from_node = self.state.graph.get_node(pending.from_node);
                if let Some(from_node) = from_node {
                    let from_screen = port_screen_pos(
                        from_node,
                        pending.from_port,
                        &self.state.view,
                        viewport.min,
                    );
                    if let Some(from_screen) = from_screen {
                        let data_type = from_node
                            .find_port(pending.from_port)
                            .map(|p| p.data_type)
                            .unwrap_or(DataType::Any);
                        draw_pending_connection(
                            &painter,
                            from_screen,
                            pending.current_screen_pos,
                            data_type,
                        );
                    }
                }
            }

            // Draw nodes
            let selected_nodes = self.state.selection.nodes.clone();
            for node in self.state.graph.nodes() {
                let is_selected = selected_nodes.contains(&node.id);
                draw_node(
                    &painter,
                    node,
                    &self.state.view,
                    viewport.min,
                    is_selected,
                );
            }

            // Draw box selection
            if let (Some(start), Some(current)) = (
                self.interaction.box_select_start,
                self.interaction.box_select_current,
            ) {
                let select_rect = Rect::from_two_pos(start, current);
                painter.rect_filled(
                    select_rect,
                    egui::CornerRadius::ZERO,
                    Color32::from_rgba_premultiplied(100, 150, 255, 30),
                );
                painter.rect_stroke(
                    select_rect,
                    egui::CornerRadius::ZERO,
                    egui::Stroke::new(1.0, Color32::from_rgb(100, 150, 255)),
                    egui::StrokeKind::Middle,
                );
            }

            // Draw minimap
            if self.state.show_minimap {
                draw_minimap(
                    &painter,
                    &self.state.graph,
                    &self.state.view,
                    viewport,
                );
            }

            // Handle interactions
            handle_canvas_interaction(
                &response,
                &mut self.state,
                &mut self.interaction,
                viewport,
            );

            // Right-click context menu
            response.context_menu(|ui| {
                if let Some(hover_pos) = ui.ctx().input(|i| i.pointer.hover_pos()) {
                    let canvas_pos =
                        self.state.view.screen_to_canvas(hover_pos, viewport.min);
                    let snapped = snap_to_grid(canvas_pos);

                    ui.menu_button("Add Event", |ui| {
                        for (name, kind) in [
                            ("On Start", aether_creator_studio::visual_script::NodeKind::OnStart),
                            ("On Interact", aether_creator_studio::visual_script::NodeKind::OnInteract),
                            ("On Enter", aether_creator_studio::visual_script::NodeKind::OnEnter),
                            ("On Exit", aether_creator_studio::visual_script::NodeKind::OnExit),
                            ("On Collision", aether_creator_studio::visual_script::NodeKind::OnCollision),
                        ] {
                            if ui.button(name).clicked() {
                                let _ = self.state.add_node_at(kind, snapped.x, snapped.y);
                                ui.close_menu();
                            }
                        }
                    });

                    ui.menu_button("Add Action", |ui| {
                        for (name, kind) in [
                            ("Log", aether_creator_studio::visual_script::NodeKind::Log),
                            ("Set Position", aether_creator_studio::visual_script::NodeKind::SetPosition),
                            ("Play Sound", aether_creator_studio::visual_script::NodeKind::PlaySound),
                            ("Spawn Entity", aether_creator_studio::visual_script::NodeKind::SpawnEntity),
                            ("Destroy Entity", aether_creator_studio::visual_script::NodeKind::DestroyEntity),
                        ] {
                            if ui.button(name).clicked() {
                                let _ = self.state.add_node_at(kind, snapped.x, snapped.y);
                                ui.close_menu();
                            }
                        }
                    });

                    ui.menu_button("Add Flow", |ui| {
                        for (name, kind) in [
                            ("Branch", aether_creator_studio::visual_script::NodeKind::Branch),
                            ("For Loop", aether_creator_studio::visual_script::NodeKind::ForLoop),
                            ("Delay", aether_creator_studio::visual_script::NodeKind::Delay { delay_ms: 1000 }),
                        ] {
                            if ui.button(name).clicked() {
                                let _ = self.state.add_node_at(kind, snapped.x, snapped.y);
                                ui.close_menu();
                            }
                        }
                    });

                    ui.menu_button("Add Math", |ui| {
                        for (name, kind) in [
                            ("Add", aether_creator_studio::visual_script::NodeKind::Add),
                            ("Subtract", aether_creator_studio::visual_script::NodeKind::Subtract),
                            ("Multiply", aether_creator_studio::visual_script::NodeKind::Multiply),
                            ("Divide", aether_creator_studio::visual_script::NodeKind::Divide),
                        ] {
                            if ui.button(name).clicked() {
                                let _ = self.state.add_node_at(kind, snapped.x, snapped.y);
                                ui.close_menu();
                            }
                        }
                    });

                    ui.separator();

                    if !self.state.selection.nodes.is_empty() {
                        if ui.button("Delete Selected").clicked() {
                            self.state.delete_selected();
                            ui.close_menu();
                        }
                    }
                }
            });
        });
    }
}
