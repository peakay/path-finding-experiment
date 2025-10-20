use crate::agent::Agent;
use crate::editor::{EditorOperations, EditorState};
use crate::map::{GridMap, TileKind};
use eframe::egui;

/// UI rendering for editor mode
pub struct EditorUI;

impl EditorUI {
    /// Draw the edit mode controls (shown when edit mode is active)
    /// Returns the new board_dim if the map was resized, None otherwise
    pub fn draw_edit_controls(
        ui: &mut egui::Ui,
        editor_state: &mut EditorState,
        map: &mut GridMap,
        agent: &mut Agent,
    ) -> Option<usize> {
        let mut new_board_dim = None;
        ui.add_space(4.0);

        // Map metadata controls
        ui.label("Map Metadata:");
        ui.horizontal(|ui| {
            ui.label("Name:");
            ui.text_edit_singleline(&mut editor_state.map_name);
        });
        ui.horizontal(|ui| {
            ui.label("Description:");
            ui.text_edit_singleline(&mut editor_state.map_description);
        });

        ui.add_space(8.0);

        // Map dimensions controls
        ui.label("Map Size:");
        ui.horizontal(|ui| {
            ui.label("Width:");
            let mut width_str = editor_state.edit_map_width.to_string();
            if ui.text_edit_singleline(&mut width_str).changed() {
                if let Ok(w) = width_str.parse::<usize>() {
                    if w > 0 && w <= 100 {
                        // Reasonable limits
                        editor_state.edit_map_width = w;
                    }
                }
            }

            ui.label("Height:");
            let mut height_str = editor_state.edit_map_height.to_string();
            if ui.text_edit_singleline(&mut height_str).changed() {
                if let Ok(h) = height_str.parse::<usize>() {
                    if h > 0 && h <= 100 {
                        // Reasonable limits
                        editor_state.edit_map_height = h;
                    }
                }
            }

            if ui.button("Resize Map").clicked() {
                let (resized_map, board_dim) = EditorOperations::resize_map(
                    map,
                    editor_state.edit_map_width,
                    editor_state.edit_map_height,
                    agent,
                );
                *map = resized_map;
                new_board_dim = Some(board_dim);
            }
        });

        ui.add_space(4.0);

        // Agent placement mode
        ui.checkbox(&mut editor_state.placing_agent, "Place Agent");
        if editor_state.placing_agent {
            ui.label(
                egui::RichText::new("Click on the map to move the agent")
                    .small()
                    .italics(),
            );
        }

        ui.add_space(4.0);

        // Tile palette
        ui.label("Tile Palette:");
        ui.horizontal_wrapped(|ui| {
            let tile_types = [
                (TileKind::Empty, "Empty"),
                (TileKind::Grass, "Grass"),
                (TileKind::Sand, "Sand"),
                (TileKind::Water, "Water"),
                (TileKind::Wall, "Wall"),
                (TileKind::Tree, "Tree"),
            ];

            for (tile_kind, label) in tile_types {
                let selected = std::mem::discriminant(&editor_state.selected_edit_tile)
                    == std::mem::discriminant(&tile_kind);
                let response = ui.selectable_label(selected, label);
                if response.clicked() {
                    editor_state.set_selected_tile(tile_kind);
                }
            }
        });

        ui.add_space(4.0);

        // Map operations
        ui.horizontal(|ui| {
            if ui.button("Fill All").clicked() {
                EditorOperations::fill_all(map, editor_state.selected_edit_tile);
            }
            if ui.button("Copy JSON").clicked() {
                EditorOperations::copy_map_to_clipboard(map, editor_state, agent);
            }
        });

        ui.add_space(8.0);

        new_board_dim
    }

    /// Draw the edit mode toggle checkbox
    pub fn draw_edit_mode_toggle(ui: &mut egui::Ui, editor_state: &mut EditorState) {
        ui.checkbox(&mut editor_state.edit_mode, "Edit Mode");
    }
}
