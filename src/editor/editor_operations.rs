use crate::agent::{Agent, LogEntry};
use crate::editor::EditorState;
use crate::map::{GridMap, TileKind};
use serde::Serialize;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

/// Helper struct for serializing map with flattened metadata
#[derive(Serialize)]
struct MapJson {
    name: String,
    description: String,
    width: usize,
    height: usize,
    tiles: Vec<Vec<TileKind>>,
}

/// Map editing operations
pub struct EditorOperations;

impl EditorOperations {
    /// Resize the map to new dimensions, preserving existing tiles where possible
    /// Returns the new map and the new board dimension (max of width and height)
    pub fn resize_map(
        current_map: &GridMap,
        new_width: usize,
        new_height: usize,
        agent: &mut Agent,
    ) -> (GridMap, usize) {
        // Create new map with specified dimensions
        let mut new_map = GridMap::new(new_width, new_height, TileKind::Grass);

        // Copy as much of the old map as possible
        let copy_width = new_width.min(current_map.width());
        let copy_height = new_height.min(current_map.height());

        for y in 0..copy_height {
            for x in 0..copy_width {
                if let Some(tile) = current_map.get(x, y) {
                    let _ = new_map.set(x, y, *tile);
                }
            }
        }

        // Ensure agent stays within bounds
        if agent.x >= new_width {
            agent.x = new_width.saturating_sub(1);
        }
        if agent.y >= new_height {
            agent.y = new_height.saturating_sub(1);
        }

        agent.log(LogEntry::Info(format!(
            "Map resized to {}x{}",
            new_width, new_height
        )));

        let board_dim = new_width.max(new_height);
        (new_map, board_dim)
    }

    /// Fill entire map with a single tile type
    pub fn fill_all(map: &mut GridMap, tile: TileKind) {
        map.clear(tile);
    }

    /// Copy map JSON to clipboard
    pub fn copy_map_to_clipboard(map: &GridMap, editor_state: &EditorState, agent: &mut Agent) {
        let map_json = MapJson {
            name: editor_state.map_name.clone(),
            description: editor_state.map_description.clone(),
            width: map.width(),
            height: map.height(),
            tiles: map.tiles().clone(),
        };
        let json = serde_json::to_string_pretty(&map_json).unwrap_or_default();
        Self::copy_to_clipboard(&json);
        agent.log(LogEntry::Info(
            "Map JSON copied to clipboard!".to_string(),
        ));
    }

    /// Copy text to clipboard using web_sys
    fn copy_to_clipboard(text: &str) {
        if let Some(window) = web_sys::window() {
            if let Ok(navigator) = window.navigator().dyn_into::<web_sys::Navigator>() {
                if let Ok(clipboard) = navigator.clipboard().dyn_into::<web_sys::Clipboard>() {
                    let text = text.to_string();
                    wasm_bindgen_futures::spawn_local(async move {
                        let promise = clipboard.write_text(&text);
                        let _ = JsFuture::from(promise).await;
                    });
                }
            }
        }
    }
}
