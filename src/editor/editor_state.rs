use crate::map::TileKind;

/// State for map editing functionality
pub struct EditorState {
    /// Whether edit mode is currently active
    pub edit_mode: bool,

    /// Currently selected tile type for painting
    pub selected_edit_tile: TileKind,

    /// Target width for map resize operations
    pub edit_map_width: usize,

    /// Target height for map resize operations
    pub edit_map_height: usize,

    /// Whether the user is in agent placement mode
    pub placing_agent: bool,

    /// Map name for metadata
    pub map_name: String,

    /// Map description for metadata
    pub map_description: String,
}

impl EditorState {
    pub fn new(initial_map_width: usize, initial_map_height: usize) -> Self {
        Self {
            edit_mode: false,
            selected_edit_tile: TileKind::Grass,
            edit_map_width: initial_map_width,
            edit_map_height: initial_map_height,
            placing_agent: false,
            map_name: String::new(),
            map_description: String::new(),
        }
    }

    /// Toggle edit mode on/off
    pub fn toggle_edit_mode(&mut self) {
        self.edit_mode = !self.edit_mode;
    }

    /// Set the selected tile type for painting
    pub fn set_selected_tile(&mut self, tile: TileKind) {
        self.selected_edit_tile = tile;
        // When selecting a tile, exit agent placement mode
        self.placing_agent = false;
    }

    /// Toggle agent placement mode
    pub fn toggle_placing_agent(&mut self) {
        self.placing_agent = !self.placing_agent;
    }

    /// Exit agent placement mode (called after placing)
    pub fn exit_placement_mode(&mut self) {
        self.placing_agent = false;
    }

    /// Update target map dimensions
    pub fn set_target_dimensions(&mut self, width: usize, height: usize) {
        self.edit_map_width = width;
        self.edit_map_height = height;
    }

    /// Initialize metadata from existing map
    pub fn initialize_from_map(&mut self, map: &crate::map::GridMap) {
        if let Some(metadata) = &map.metadata {
            self.map_name = metadata.name.clone();
            self.map_description = metadata.description.clone();
        } else {
            self.map_name.clear();
            self.map_description.clear();
        }
    }
}
