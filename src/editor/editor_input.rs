use crate::agent::{Agent, LogEntry};
use crate::editor::EditorState;
use crate::map::GridMap;

/// Handles input events for editor mode
pub struct EditorInput;

impl EditorInput {
    /// Handle board input during edit mode
    pub fn handle_edit_input(
        editor_state: &mut EditorState,
        map: &mut GridMap,
        agent: &mut Agent,
        pressed: bool,
        _down: bool,
        col: usize,
        row: usize,
    ) {
        if editor_state.placing_agent {
            // Agent placement mode - only react to press (not drag)
            if pressed {
                agent.x = col;
                agent.y = row;
                agent.log(LogEntry::Info(format!(
                    "Agent moved to position ({}, {})",
                    col, row
                )));
                editor_state.exit_placement_mode();
            }
        }
        // Note: Tile painting during dragging is now handled in BoardInput::handle_input
    }

    /// Handle board input during play mode
    pub fn handle_play_input(
        agent: &mut Agent,
        agent_selected: &mut bool,
        pressed: bool,
        col: usize,
        row: usize,
    ) {
        // Check if agent was clicked
        if row == agent.y && col == agent.x {
            *agent_selected = true;
        } else if pressed {
            *agent_selected = false;
        }
    }
}
