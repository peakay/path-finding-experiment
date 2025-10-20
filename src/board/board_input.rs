use crate::agent::Agent;
use crate::board::BoardRenderer;
use crate::editor::{EditorInput, EditorState};
use crate::map::GridMap;
use eframe::egui;
use egui::Rect;

/// Handles input for the game board
pub struct BoardInput;

impl BoardInput {
    /// Handle board input - dispatches to edit or play mode handlers
    pub fn handle_input(
        ui: &mut egui::Ui,
        rect: Rect,
        board_side: f32,
        board_dim: usize,
        response: &egui::Response,
        editor_state: &mut EditorState,
        map: &mut GridMap,
        agent: &mut Agent,
        agent_selected: &mut bool,
        selected_cell: &mut Option<(usize, usize)>,
        selected_tile: &mut Option<(usize, usize)>,
    ) {
        let (pressed, released, pointer_pos) = ui.input(|i| {
            (
                i.pointer.any_pressed(),
                i.pointer.any_released(),
                i.pointer.interact_pos(),
            )
        });

        // Track drag state for edit mode
        if editor_state.edit_mode && !editor_state.placing_agent {
            ui.ctx().data_mut(|data| {
                let drag_key = egui::Id::new("edit_drag_state");
                let mut is_dragging = data.get_temp::<bool>(drag_key).unwrap_or(false);

                if pressed && response.hovered() {
                    is_dragging = true;
                } else if released {
                    is_dragging = false;
                }

                data.insert_temp(drag_key, is_dragging);

                // Handle painting during drag
                if is_dragging {
                    if let Some(pos) = pointer_pos {
                        if let Some((r, c)) = BoardRenderer::screen_to_grid(pos, rect, board_side, board_dim) {
                            map.set(c, r, editor_state.selected_edit_tile);
                        }
                    }
                }
            });
        }

        // Handle hover and click interactions
        if response.hovered() {
            if let Some(pos) = pointer_pos {
                if let Some((r, c)) = BoardRenderer::screen_to_grid(pos, rect, board_side, board_dim)
                {
                    // Update selection highlight in both modes
                    *selected_cell = Some((r, c));
                    *selected_tile = Some((c, r));

                    if editor_state.edit_mode {
                        EditorInput::handle_edit_input(
                            editor_state,
                            map,
                            agent,
                            pressed,
                            false, // Don't pass down state since we handle dragging separately
                            c,
                            r,
                        );
                    } else {
                        EditorInput::handle_play_input(agent, agent_selected, pressed, c, r);
                    }
                }
            }
        }
    }
}
