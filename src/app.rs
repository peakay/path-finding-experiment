use crate::agent::{Agent, LogEntry};
use crate::animation::AnimationController;
use crate::board::{BoardInput, BoardRenderer};
use crate::editor::{EditorState, EditorUI};
use crate::events::EventQueue;
use crate::map::{GridMap, TileKind};
use crate::map_type::MapType;
use crate::rendering::*;
use crate::tool_execution::ToolExecutionManager;
use crate::ui::{AgentPanel, TileInfoPanel};
use eframe::egui;
use std::sync::{Arc, Mutex};
use web_time::{Duration, Instant};

// Game tick rate: 500ms per tick (2 ticks per second)
const TICK_RATE: Duration = Duration::from_millis(500);

pub struct MyApp {
    // Map + rendering state
    board_dim: usize,
    selected_cell: Option<(usize, usize)>,
    selected_tile: Option<(usize, usize)>, // Separate from selected_cell for tile info
    map: GridMap,
    current_map_type: MapType,
    pending_map_change: Option<MapType>, // Defer map changes to avoid blocking UI
    tree_tex: Option<egui::TextureHandle>,

    // Agent state (single agent)
    agent: Agent,
    agent_selected: bool,
    agent_instruction: String,
    selected_model: String,

    // Log callback queue from async operations
    log_callbacks: Arc<Mutex<Vec<(u32, LogEntry)>>>,

    // Event queue for tick-based processing
    event_queue: EventQueue,

    // Tool execution manager
    tool_execution_manager: ToolExecutionManager,

    // Agent execution control
    agent_running: bool, // True if agent is in continuous execution loop
    should_continue_execution: bool, // Set to true when tool result is added
    llm_active: bool,    // True when waiting for LLM response or streaming tokens

    // LLM status callback
    llm_status_callback: Arc<Mutex<bool>>, // Shared flag for LLM activity status

    // Tick timing
    last_tick: Instant,
    accumulated_time: Duration,

    // OpenRouter API key
    openrouter_api_key: String,

    // Animation controller
    animation_controller: AnimationController,

    // Map editor state
    editor_state: EditorState,
}

impl MyApp {
    /// Load OpenRouter API key from localStorage
    fn load_api_key_from_storage() -> Option<String> {
        web_sys::window()
            .and_then(|window| window.local_storage().ok())
            .flatten()
            .and_then(|storage| storage.get_item("openrouter_api_key").ok())
            .flatten()
    }

    /// Save OpenRouter API key to localStorage
    fn save_api_key_to_storage(api_key: &str) {
        if let Some(window) = web_sys::window() {
            if let Ok(Some(storage)) = window.local_storage() {
                let _ = storage.set_item("openrouter_api_key", api_key);
            }
        }
    }

    pub fn new(cc: &eframe::CreationContext<'_>, openrouter_api_key: String) -> Self {
        // Load API key from localStorage if available, otherwise use provided key
        let api_key = Self::load_api_key_from_storage().unwrap_or(openrouter_api_key);
        // Prepare sprites
        let tree_image = generate_tree_sprite(48);
        let tree_tex = Some(cc.egui_ctx.load_texture(
            "tree_sprite",
            tree_image,
            egui::TextureOptions::LINEAR,
        ));

        let initial_map = MapType::LakeTrees.create_map(24, 24).unwrap_or_else(|e| {
            eprintln!("Failed to load initial map: {}", e);
            GridMap::new(24, 24, TileKind::Grass)
        });

        let mut editor_state = EditorState::new(24, 24);
        editor_state.initialize_from_map(&initial_map);

        Self {
            board_dim: 24,
            selected_cell: None,
            selected_tile: None,
            map: initial_map,
            current_map_type: MapType::LakeTrees,
            pending_map_change: None,
            tree_tex,
            agent: Agent::new(1, "Agent-1", 6, 10),
            agent_selected: false,
            agent_instruction: String::new(),
            selected_model: "x-ai/grok-4-fast".to_string(),
            log_callbacks: Arc::new(Mutex::new(Vec::new())),
            event_queue: EventQueue::new(),
            tool_execution_manager: ToolExecutionManager::new(TICK_RATE),
            agent_running: false,
            should_continue_execution: false,
            llm_active: false,
            llm_status_callback: Arc::new(Mutex::new(false)),
            last_tick: Instant::now(),
            accumulated_time: Duration::from_secs(0),
            openrouter_api_key: api_key,
            animation_controller: AnimationController::new(),
            editor_state,
        }
    }

    /// Process one game tick - processes all ready events
    fn process_tick(&mut self) {
        use crate::events::Event;

        // Process all ready events in this tick
        while let Some(scheduled_event) = self.event_queue.pop_ready() {
            // Extract agent_id before consuming event
            let agent_id_for_cancel = match &scheduled_event.event {
                Event::AgentMove { agent_id, .. } => Some(*agent_id),
                _ => None,
            };

            let result = match scheduled_event.event {
                Event::AgentMove {
                    agent_id,
                    direction,
                } => {
                    if agent_id == self.agent.id {
                        let result = self.agent.execute_move_step(direction, &mut self.map);

                        // Update selected tile to follow agent if movement succeeded
                        if result.is_ok() {
                            self.selected_tile = Some(self.agent.pos());
                        }

                        result
                    } else {
                        Err(format!("Unknown agent id: {}", agent_id))
                    }
                }
                Event::Delay { .. } => {
                    // Delay events just complete successfully
                    Ok(())
                }
            };

            // Check if we should abort and cancel remaining events
            if let Err(ref err_msg) = result {
                if err_msg.starts_with("ABORT:") {
                    // Cancel all remaining events for this agent
                    if let Some(agent_id) = agent_id_for_cancel {
                        self.event_queue.cancel_agent_events(agent_id);
                        self.agent.log(LogEntry::Info(
                            "Remaining movement events cancelled".to_string(),
                        ));
                    }
                }
            }

            self.event_queue.complete(scheduled_event.id, result);
        }
    }

    /// Process accumulated time and run ticks
    fn process_ticks(&mut self) {
        let now = Instant::now();
        let delta = now.duration_since(self.last_tick);
        self.accumulated_time += delta;
        self.last_tick = now;

        // Process ticks at fixed rate
        while self.accumulated_time >= TICK_RATE {
            self.process_tick();
            self.accumulated_time -= TICK_RATE;
        }

        // Process pending map change (deferred to avoid blocking UI)
        if let Some(new_map_type) = self.pending_map_change.take() {
            self.current_map_type = new_map_type;
            match new_map_type.create_map(self.board_dim, self.board_dim) {
                Ok(new_map) => {
                    self.map = new_map;
                    // Update board dimensions to match the loaded map
                    self.board_dim = self.map.width().max(self.map.height());
                    // Initialize editor state with new map metadata
                    self.editor_state.initialize_from_map(&self.map);
                    // Update editor state's target dimensions to match new map
                    self.editor_state.set_target_dimensions(self.map.width(), self.map.height());
                    // Clear selection when changing maps to prevent hover issues
                    self.selected_cell = None;
                    self.selected_tile = None;
                    // Clear agent trail when changing maps
                    self.agent.clear_movement_history();
                    self.agent.log(LogEntry::Info(
                        "Map changed - agent trail cleared".to_string(),
                    ));
                }
                Err(e) => {
                    self.agent.log(LogEntry::Error(
                        format!("Failed to load map: {}", e),
                    ));
                }
            }
        }
    }

}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Update animation
        self.animation_controller.update();

        // Process fixed-rate ticks
        self.process_ticks();

        // Drain log callbacks from async operations
        let pending_logs: Vec<(u32, LogEntry)> = {
            let mut g = self.log_callbacks.lock().unwrap();
            g.drain(..).collect()
        };
        for (agent_id, log_entry) in pending_logs {
            if agent_id == self.agent.id {
                self.agent.log(log_entry);
            }
        }

        // Process tool callbacks through the execution manager
        if self.tool_execution_manager.process_tool_callbacks(
            &mut self.agent,
            &mut self.map,
            &self.event_queue,
            self.agent_running,
        ) {
            self.should_continue_execution = true;
        }

        // Complete pending tool executions when their events are done
        if self
            .tool_execution_manager
            .process_pending_executions(&mut self.agent, &self.event_queue)
        {
            self.should_continue_execution = true;
        }

        // Check for ESC key to cancel execution
        ctx.input(|i| {
            if i.key_pressed(egui::Key::Escape) && self.agent_running {
                self.agent_running = false;
                self.should_continue_execution = false;

                // Clear LLM status indicator immediately
                if let Ok(mut status) = self.llm_status_callback.lock() {
                    *status = false;
                }

                // Clear any pending tool callbacks that haven't been processed yet
                self.tool_execution_manager.clear_callbacks();

                // Clear any pending log callbacks
                if let Ok(mut logs) = self.log_callbacks.lock() {
                    logs.clear();
                }

                self.agent.log(LogEntry::Info(
                    "WARN: Execution cancelled by user (ESC)".to_string(),
                ));
            }
        });

        // Continue agent execution if tool result was just added
        if self.should_continue_execution
            && self.agent_running
            && !self.tool_execution_manager.has_pending_executions()
            && self.event_queue.pending_count() == 0
        {
            self.should_continue_execution = false;

            // Continue with empty instruction (agent will use chat history)
            let api_key = self.openrouter_api_key.clone();
            let tool_callbacks = self.tool_execution_manager.get_tool_callbacks();
            let log_callbacks = self.log_callbacks.clone();

            self.agent.execute_instruction(
                String::new(), // Empty instruction - continue from chat history
                api_key,
                self.selected_model.clone(),
                &self.map,
                tool_callbacks,
                log_callbacks,
                self.llm_status_callback.clone(),
            );
        }

        // Request repaint if there are pending events, tool executions, or LLM activity
        if self.event_queue.pending_count() > 0
            || self.tool_execution_manager.has_pending_executions()
            || self.agent_running
            || *self.llm_status_callback.lock().unwrap()
        {
            ctx.request_repaint_after(Duration::from_millis(50));
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::none().inner_margin(0.0))
            .show(ctx, |ui| {
                let avail = ui.available_size();
                let left_w = (avail.x * 0.3333).max(200.0);
                let right_w = (avail.x - left_w).max(100.0);
                ui.horizontal(|ui| {
                    // Left: Agent panel (1/3 width, full height)
                    ui.allocate_ui(egui::vec2(left_w, avail.y), |ui| {
                        self.draw_agent_panel(ui);
                    });

                    // Right: Grid (remaining width, full height)
                    ui.allocate_ui(egui::vec2(right_w, avail.y), |ui| {
                        self.draw_grid_panel(ui);
                    });
                });
            });
    }
}

// Split UI rendering into separate methods
impl MyApp {
    fn draw_agent_panel(&mut self, ui: &mut egui::Ui) {
        egui::Frame::default()
            .fill(egui::Color32::from_rgb(240, 235, 255))
            .inner_margin(egui::Margin::same(12.0))
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    ui.heading("Agent Panel");
                    if self.agent_selected {
                        self.draw_agent_controls(ui);
                    } else {
                        ui.label("Click the agent on the map to configure.");
                    }

                    // Show tile info if a tile is selected
                    if self.selected_tile.is_some() {
                        self.draw_tile_info(ui);
                    }

                    ui.separator();
                    ui.heading("Activity Log");
                    self.draw_activity_log(ui);
                });
            });
    }

    fn draw_agent_controls(&mut self, ui: &mut egui::Ui) {
        let is_processing =
            AgentPanel::is_processing(&self.event_queue, &self.tool_execution_manager);
        let is_llm_active = *self.llm_status_callback.lock().unwrap();

        ui.label("Name");
        ui.add_enabled_ui(!is_processing, |ui| {
            ui.text_edit_singleline(&mut self.agent.name);
        });
        ui.add_space(8.0);

        // Model selection
        ui.label("Model");
        ui.add_enabled_ui(!is_processing, |ui| {
            egui::ComboBox::from_id_source("model_selector")
                .selected_text(&self.selected_model)
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut self.selected_model,
                        "anthropic/claude-haiku-4.5".to_string(),
                        "Claude Haiku 4.5",
                    );
                    ui.selectable_value(
                        &mut self.selected_model,
                        "anthropic/claude-sonnet-4.5".to_string(),
                        "Claude Sonnet 4.5",
                    );
                    ui.selectable_value(
                        &mut self.selected_model,
                        "x-ai/grok-4-fast".to_string(),
                        "Grok 4 Fast",
                    );
                    ui.selectable_value(
                        &mut self.selected_model,
                        "x-ai/grok-code-fast-1".to_string(),
                        "Grok Code Fast",
                    );
                });
        });
        ui.add_space(8.0);

        // History messages limit
        ui.label("Max History Messages");
        ui.add_enabled_ui(!is_processing, |ui| {
            let mut max_history = self.agent.max_history_messages() as i32;
            if ui
                .add(egui::Slider::new(&mut max_history, 1..=50).text("messages"))
                .changed()
            {
                self.agent.set_max_history_messages(max_history as usize);
            }
            ui.add_space(2.0);
            ui.label(
                egui::RichText::new("Controls how many recent messages are sent to the LLM")
                    .small()
                    .color(egui::Color32::from_gray(120)),
            );
        });
        ui.add_space(8.0);

        // Tool toggles
        self.draw_tool_toggles(ui, is_processing);

        ui.label("Instruction");

        let mut should_submit = false;

        ui.add_enabled_ui(!is_processing, |ui| {
            let text_edit = egui::TextEdit::multiline(&mut self.agent_instruction)
                .desired_rows(6)
                .desired_width(ui.available_width());
            let response = ui.add(text_edit);

            // Check for Shift+Enter shortcut
            if response.has_focus() {
                ui.input(|i| {
                    if i.key_pressed(egui::Key::Enter) && i.modifiers.shift {
                        should_submit = true;
                    }
                });
            }

            ui.add_space(8.0);
            if ui
                .add_sized(
                    [ui.available_width(), 0.0],
                    egui::Button::new("Run Instruction"),
                )
                .clicked()
            {
                should_submit = true;
            }
        });

        // Show status indicators under the button
        if is_llm_active {
            AgentPanel::draw_thinking_status(ui, &self.animation_controller);
        }

        if is_processing {
            AgentPanel::draw_processing_status(ui, &self.event_queue, &self.animation_controller);
        }

        if should_submit && !is_processing {
            let api_key = self.openrouter_api_key.clone();
            let instruction = self.agent_instruction.clone();
            let tool_callbacks = self.tool_execution_manager.get_tool_callbacks();
            let log_callbacks = self.log_callbacks.clone();

            // Start agent execution loop
            self.agent_running = true;

            // Agent executes instruction internally
            self.agent.execute_instruction(
                instruction,
                api_key,
                self.selected_model.clone(),
                &self.map,
                tool_callbacks,
                log_callbacks,
                self.llm_status_callback.clone(),
            );
        }
    }

    fn draw_tool_toggles(&mut self, ui: &mut egui::Ui, is_processing: bool) {
        ui.label("Available Tools");
        ui.add_enabled_ui(!is_processing, |ui| {
            let tool_names: Vec<String> = self
                .agent
                .get_all_tools()
                .iter()
                .map(|t| t.function.name.clone())
                .collect();

            for tool_name in tool_names {
                let mut is_enabled = self.agent.is_tool_enabled(&tool_name);
                if ui.checkbox(&mut is_enabled, &tool_name).changed() {
                    if is_enabled {
                        self.agent.enable_tool(&tool_name);
                    } else {
                        self.agent.disable_tool(&tool_name);
                    }
                }
                ui.add_space(2.0);
                // Get description from the tool
                if let Some(tool) = self
                    .agent
                    .get_all_tools()
                    .iter()
                    .find(|t| t.function.name == tool_name)
                {
                    ui.label(
                        egui::RichText::new(&tool.function.description)
                            .small()
                            .color(egui::Color32::from_gray(120)),
                    );
                }
                ui.add_space(4.0);
            }
        });
        ui.add_space(8.0);
    }

    fn draw_tile_info(&mut self, ui: &mut egui::Ui) {
        TileInfoPanel::draw(ui, self.selected_tile, &self.map, &self.agent);
    }

    fn draw_activity_log(&mut self, ui: &mut egui::Ui) {
        AgentPanel::draw_activity_log(ui, &self.agent);
    }

    fn draw_grid_panel(&mut self, ui: &mut egui::Ui) {
        egui::Frame::default()
            .fill(egui::Color32::WHITE)
            .inner_margin(egui::Margin::same(12.0))
            .show(ui, |ui| {
                ui.vertical(|ui| {
            // Top controls: API key, map selector, and edit mode toggle
            ui.horizontal(|ui| {
                ui.label("API Key:");
                let mut api_key_changed = false;
                if ui.text_edit_singleline(&mut self.openrouter_api_key).changed() {
                    api_key_changed = true;
                }
                if api_key_changed {
                    Self::save_api_key_to_storage(&self.openrouter_api_key);
                }

                ui.separator();
                ui.label("Map:");
                let mut selected_map = self.current_map_type;
                egui::ComboBox::from_id_source("map_selector")
                    .selected_text(selected_map.name())
                    .show_ui(ui, |ui| {
                        for map_type in MapType::all() {
                            ui.selectable_value(&mut selected_map, map_type, map_type.name());
                        }
                    });

                if selected_map != self.current_map_type {
                    // Defer expensive map creation to avoid blocking UI
                    self.pending_map_change = Some(selected_map);
                }

                ui.separator();
                EditorUI::draw_edit_mode_toggle(ui, &mut self.editor_state);
            });

            // Tile palette when in edit mode
            if self.editor_state.edit_mode {
                if let Some(new_board_dim) = EditorUI::draw_edit_controls(
                    ui,
                    &mut self.editor_state,
                    &mut self.map,
                    &mut self.agent,
                ) {
                    self.board_dim = new_board_dim;
                    // Clear selection when map is resized to prevent hover issues
                    self.selected_cell = None;
                    self.selected_tile = None;
                }
            } else {
                ui.add_space(8.0);
            }

            ui.heading("Game Board");
            let avail_r = ui.available_size();
            let board_side = avail_r.x.min(avail_r.y).max(100.0);
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(board_side, board_side), egui::Sense::click());
            let painter = ui.painter();

            // Render the board
            BoardRenderer::render(
                &painter,
                rect,
                &self.map,
                &self.agent,
                self.selected_cell,
                self.tree_tex.as_ref(),
            );

            // Handle input
            BoardInput::handle_input(
                ui,
                rect,
                board_side,
                self.board_dim,
                &response,
                &mut self.editor_state,
                &mut self.map,
                &mut self.agent,
                &mut self.agent_selected,
                &mut self.selected_cell,
                &mut self.selected_tile,
            );
                });
            });
    }
}
