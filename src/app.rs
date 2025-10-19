use crate::agent::{Agent, LogEntry};
use crate::events::{EventQueue, PendingToolExecution};
use crate::map::{GridMap, TileKind};
use crate::map_type::MapType;
use crate::rendering::*;
use eframe::egui;
use egui::{Painter, Rect};
use serde_json::Value;
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

    // Tool callback queue
    tool_callbacks: Arc<Mutex<Vec<(u32, String, Value)>>>,

    // Log callback queue from async operations
    log_callbacks: Arc<Mutex<Vec<(u32, LogEntry)>>>,

    // Event queue for tick-based processing
    event_queue: EventQueue,

    // Track pending tool executions waiting for events to complete
    pending_tool_executions: Vec<PendingToolExecution>,

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

    // Animation state for status indicators
    animation_frame: u64,
    last_animation_update: Instant,
}

impl MyApp {
    pub fn new(cc: &eframe::CreationContext<'_>, openrouter_api_key: String) -> Self {
        // Prepare sprites
        let tree_image = generate_tree_sprite(48);
        let tree_tex = Some(cc.egui_ctx.load_texture(
            "tree_sprite",
            tree_image,
            egui::TextureOptions::LINEAR,
        ));

        Self {
            board_dim: 24,
            selected_cell: None,
            selected_tile: None,
            map: GridMap::default_grass_trees_water(24, 24),
            current_map_type: MapType::LakeTrees,
            pending_map_change: None,
            tree_tex,
            agent: Agent::new(1, "Agent-1", 6, 10),
            agent_selected: false,
            agent_instruction: String::new(),
            selected_model: "anthropic/claude-haiku-4.5".to_string(),
            tool_callbacks: Arc::new(Mutex::new(Vec::new())),
            log_callbacks: Arc::new(Mutex::new(Vec::new())),
            event_queue: EventQueue::new(),
            pending_tool_executions: Vec::new(),
            agent_running: false,
            should_continue_execution: false,
            llm_active: false,
            llm_status_callback: Arc::new(Mutex::new(false)),
            last_tick: Instant::now(),
            accumulated_time: Duration::from_secs(0),
            openrouter_api_key,
            animation_frame: 0,
            last_animation_update: Instant::now(),
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
            self.map = new_map_type.create_map(self.board_dim, self.board_dim);
            // Clear agent trail when changing maps
            self.agent.log(LogEntry::Info(
                "Map changed - agent trail cleared".to_string(),
            ));
        }
    }

    fn render_board(&self, painter: &Painter, rect: Rect) {
        let n = self.board_dim as f32;
        let cell = rect.width() / n;

        // Background
        painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(240, 240, 240));

        // Grid lines
        let line_color = egui::Color32::from_gray(180);
        let stroke = egui::Stroke {
            width: 1.0,
            color: line_color,
        };
        for i in 0..=self.board_dim {
            let x = rect.left() + (i as f32) * cell;
            let y = rect.top() + (i as f32) * cell;
            painter.line_segment(
                [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
                stroke,
            );
            painter.line_segment(
                [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
                stroke,
            );
        }

        // Paint tiles
        for y in 0..self.map.height() {
            for x in 0..self.map.width() {
                let x0 = rect.left() + (x as f32) * cell;
                let y0 = rect.top() + (y as f32) * cell;
                let rcell = egui::Rect::from_min_size(egui::pos2(x0, y0), egui::vec2(cell, cell));
                if let Some(kind) = self.map.get(x, y) {
                    match kind {
                        TileKind::Empty => {}
                        TileKind::Grass => draw_grass_tile(&painter, rcell),
                        TileKind::Water => draw_water_tile(&painter, rcell),
                        TileKind::Sand => draw_sand_tile(&painter, rcell),
                        TileKind::Wall => draw_wall_tile(&painter, rcell),
                        TileKind::Trail => {
                            painter.rect_filled(
                                rcell.shrink(4.0),
                                2.0,
                                egui::Color32::from_rgba_premultiplied(255, 200, 0, 100),
                            );
                        }
                        TileKind::Tree => {
                            if let Some(tex) = &self.tree_tex {
                                draw_tree_sprite(&painter, rcell, tex);
                            } else {
                                draw_grass_tile(&painter, rcell);
                            }
                        }
                        TileKind::Custom(code) => {
                            let r = ((code >> 16) & 0xFF) as u8;
                            let g = ((code >> 8) & 0xFF) as u8;
                            let b = (code & 0xFF) as u8;
                            painter.rect_filled(
                                rcell.shrink(2.0),
                                0.0,
                                egui::Color32::from_rgb(r, g, b),
                            );
                        }
                    }
                }
            }
        }

        // Selection highlight
        if let Some((sr, sc)) = self.selected_cell {
            let x0 = rect.left() + (sc as f32) * cell;
            let y0 = rect.top() + (sr as f32) * cell;
            let rcell = egui::Rect::from_min_size(egui::pos2(x0, y0), egui::vec2(cell, cell));
            painter.rect_stroke(
                rcell.shrink(1.0),
                0.0,
                egui::Stroke {
                    width: 2.0,
                    color: egui::Color32::YELLOW,
                },
            );
        }

        // Draw agent
        if self.agent.x < self.map.width() && self.agent.y < self.map.height() {
            let x0 = rect.left() + (self.agent.x as f32) * cell;
            let y0 = rect.top() + (self.agent.y as f32) * cell;
            let center = egui::pos2(x0 + cell * 0.5, y0 + cell * 0.6);
            painter.circle_filled(center, cell * 0.18, egui::Color32::from_rgb(230, 70, 50));
            painter.text(
                egui::pos2(center.x, y0 + cell * 0.15),
                egui::Align2::CENTER_CENTER,
                &self.agent.name,
                egui::FontId::proportional((cell * 0.32).max(10.0)),
                egui::Color32::BLACK,
            );
        }
    }

    fn handle_board_input(
        &mut self,
        ui: &mut egui::Ui,
        rect: Rect,
        board_side: f32,
        response: &egui::Response,
    ) {
        let (pressed, down, pointer_pos) = ui.input(|i| {
            (
                i.pointer.any_pressed(),
                i.pointer.any_down(),
                i.pointer.interact_pos(),
            )
        });

        if (pressed || down) && response.hovered() {
            if let Some(pos) = pointer_pos {
                let cell = board_side / (self.board_dim as f32);
                let rel_x = (pos.x - rect.left()).clamp(0.0, board_side - 0.001);
                let rel_y = (pos.y - rect.top()).clamp(0.0, board_side - 0.001);
                let c = (rel_x / cell).floor() as usize;
                let r = (rel_y / cell).floor() as usize;

                if r < self.board_dim && c < self.board_dim {
                    self.selected_cell = Some((r, c));
                    self.selected_tile = Some((c, r));

                    // Check if agent was clicked
                    if r == self.agent.y && c == self.agent.x {
                        self.agent_selected = true;
                    } else if pressed {
                        self.agent_selected = false;
                    }
                }
            }
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
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

        // Drain tool callbacks and dispatch to agent (only if execution is still active)
        let pending: Vec<(u32, String, Value)> = {
            let mut g = self.tool_callbacks.lock().unwrap();
            g.drain(..).collect()
        };
        for (agent_id, name, args) in pending {
            if agent_id == self.agent.id {
                // Skip processing tool callbacks if execution was cancelled
                if !self.agent_running {
                    self.agent.log(LogEntry::Info(
                        format!("TOOL: Discarded '{}' tool call (execution cancelled)", name)
                    ));
                    continue;
                }

                // Generate tool call ID
                let tool_call_id = format!(
                    "call_{}",
                    web_time::SystemTime::now()
                        .duration_since(web_time::SystemTime::UNIX_EPOCH)
                        .unwrap()
                        .as_nanos()
                );

                // Add assistant message with tool call to history
                let args_str = serde_json::to_string(&args).unwrap_or_default();
                self.agent
                    .add_assistant_tool_call(tool_call_id.clone(), name.clone(), args_str);

                match self.agent.handle_tool_call(&name, args, &mut self.map) {
                    Ok(result_msg) => {
                        // Check if tool generated any pending moves (events to submit)
                        let moves = self.agent.take_pending_moves();

                        if !moves.is_empty() {
                            // Tool needs to wait for events to complete
                            use crate::events::Event;

                            let events: Vec<Event> = moves
                                .into_iter()
                                .map(|direction| Event::AgentMove {
                                    agent_id: self.agent.id,
                                    direction,
                                })
                                .collect();

                            // Submit with 2-tick delay between moves (1 second @ 500ms/tick)
                            let event_ids = self.event_queue.submit_sequence(events, TICK_RATE * 2);

                            // Create pending tool execution to track this
                            // If result_msg is empty, it means "don't log initial result"
                            let initial_result = if result_msg.is_empty() {
                                String::new()
                            } else {
                                result_msg
                            };
                            let pending = PendingToolExecution::new(
                                tool_call_id,
                                name.clone(),
                                initial_result,
                                event_ids,
                            );
                            self.pending_tool_executions.push(pending);
                        } else {
                            // No events, tool completes immediately
                            // Only add result if it's not empty (don't log indicator)
                            if !result_msg.is_empty() {
                                self.agent
                                    .add_tool_result(tool_call_id, name.clone(), result_msg);
                            }

                            // Mark that we should continue execution after tool result
                            self.should_continue_execution = true;
                        }
                    }
                    Err(e) => {
                        // Add error as tool result immediately
                        self.agent.add_tool_result(
                            tool_call_id,
                            name.clone(),
                            format!("Error: {}", e),
                        );
                        self.agent
                            .log(LogEntry::Error(format!("Tool execution failed: {}", e)));

                        // Mark that we should continue execution after tool result
                        self.should_continue_execution = true;
                    }
                }
            }
        }

        // Complete pending tool executions when their events are done
        let mut completed_indices = Vec::new();
        for (idx, pending) in self.pending_tool_executions.iter().enumerate() {
            if pending.is_complete(&self.event_queue) {
                completed_indices.push(idx);
            }
        }

        // Process completed tool executions in reverse order to maintain indices
        for idx in completed_indices.into_iter().rev() {
            let pending = self.pending_tool_executions.remove(idx);

            // Get event results
            let event_results = pending.get_event_results(&self.event_queue);

            // Check logs for errors/cancellations
            let recent_logs: Vec<&LogEntry> = self.agent.get_logs().iter().rev().take(20).collect();

            let had_errors = recent_logs.iter().any(|entry| {
                matches!(entry, LogEntry::Error(msg) if msg.contains("Movement blocked") || msg.contains("ABORT"))
            });

            let cancelled = recent_logs.iter().any(
                |entry| matches!(entry, LogEntry::Info(msg) if msg.contains("events cancelled")),
            );

            // Check if any events failed
            let event_failures: Vec<String> = event_results
                .iter()
                .filter_map(|(_, result)| match result {
                    Err(e) => Some(e.clone()),
                    Ok(_) => None,
                })
                .collect();

            // Build comprehensive result message
            let result_msg = if had_errors || cancelled || !event_failures.is_empty() {
                let mut error_parts = Vec::new();

                if !event_failures.is_empty() {
                    error_parts.push(format!("Event failures: {}", event_failures.join(", ")));
                }

                let log_errors: Vec<String> = recent_logs
                    .iter()
                    .filter_map(|entry| match entry {
                        LogEntry::Error(msg) => Some(msg.clone()),
                        LogEntry::Info(msg) if msg.contains("cancelled") => Some(msg.clone()),
                        _ => None,
                    })
                    .collect();

                if !log_errors.is_empty() {
                    error_parts.push(format!("Details: {}", log_errors.join("; ")));
                }

                format!(
                    "{} (Errors: {})",
                    pending.initial_result,
                    error_parts.join(" | ")
                )
            } else {
                format!("{} (Completed successfully)", pending.initial_result)
            };

            self.agent
                .add_tool_result(pending.tool_call_id, pending.tool_name, result_msg);

            // Mark that we should continue execution after tool result
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
                if let Ok(mut callbacks) = self.tool_callbacks.lock() {
                    callbacks.clear();
                }

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
            && self.pending_tool_executions.is_empty()
            && self.event_queue.pending_count() == 0
        {
            self.should_continue_execution = false;

            // Continue with empty instruction (agent will use chat history)
            let api_key = self.openrouter_api_key.clone();
            let tool_callbacks = self.tool_callbacks.clone();
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
            || !self.pending_tool_executions.is_empty()
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
            self.event_queue.pending_count() > 0 || !self.pending_tool_executions.is_empty();
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
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("AI").size(16.0).strong());
                ui.label(
                    egui::RichText::new(&self.get_animated_thinking_text())
                        .color(egui::Color32::from_rgb(100, 150, 255))
                        .strong(),
                );
            });
            ui.add_space(4.0);
        }

        if is_processing {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("WORK").size(16.0).strong());
                ui.label(
                    egui::RichText::new(format!(
                        "Processing... ({} events)",
                        self.event_queue.pending_count()
                    ))
                    .color(egui::Color32::from_rgb(200, 100, 0))
                    .strong(),
                );
            });
            ui.add_space(4.0);
        }

        if should_submit && !is_processing {
            let api_key = self.openrouter_api_key.clone();
            let instruction = self.agent_instruction.clone();
            let tool_callbacks = self.tool_callbacks.clone();
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
        if let Some((tile_x, tile_y)) = self.selected_tile {
            ui.separator();
            ui.heading("Tile Info");

            egui::Frame::default()
                .fill(egui::Color32::from_rgb(245, 250, 255))
                .inner_margin(egui::Margin::same(8.0))
                .rounding(4.0)
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new(format!("Position: ({}, {})", tile_x, tile_y))
                            .strong()
                            .color(egui::Color32::from_rgb(50, 80, 120)),
                    );

                    if let Some(tile_kind) = self.map.get(tile_x, tile_y) {
                        ui.horizontal(|ui| {
                            ui.label("Type:");
                            ui.label(
                                egui::RichText::new(tile_kind.name())
                                    .color(egui::Color32::from_rgb(80, 120, 80)),
                            );
                        });

                        ui.horizontal(|ui| {
                            ui.label("Traversable:");
                            let (icon, color) = if tile_kind.is_traversable() {
                                ("YES", egui::Color32::from_rgb(50, 150, 50))
                            } else {
                                ("NO", egui::Color32::from_rgb(150, 50, 50))
                            };
                            ui.label(egui::RichText::new(icon).color(color).strong());
                        });

                        // Show if agent is on this tile
                        if tile_x == self.agent.x && tile_y == self.agent.y {
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new("AGENT").size(14.0).strong());
                                ui.label(
                                    egui::RichText::new(format!("{} is here", self.agent.name))
                                        .color(egui::Color32::from_rgb(200, 80, 50))
                                        .italics(),
                                );
                            });
                        }
                    }
                });
        }
    }

    fn draw_activity_log(&mut self, ui: &mut egui::Ui) {
        let scroll_area = egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .stick_to_bottom(true);

        scroll_area.show(ui, |ui| {
            for log_entry in self.agent.get_logs() {
                draw_log_entry(ui, log_entry);
            }
        });
    }

    /// Generate animated "Thinking..." text with elaborate effects
    fn get_animated_thinking_text(&mut self) -> String {
        let now = Instant::now();

        // Update animation frame every 150ms
        if now.duration_since(self.last_animation_update) >= Duration::from_millis(150) {
            self.animation_frame = self.animation_frame.wrapping_add(1);
            self.last_animation_update = now;
        }

        let frame = self.animation_frame;
        let cycle = (frame / 20) % 6; // Change animation every 20 frames (3 seconds)

        match cycle {
            0 => {
                // Spinning cursor animation
                let cursors = ["|", "/", "-", "\\"];
                let cursor_idx = (frame % 4) as usize;
                format!("Thinking{}  ", cursors[cursor_idx])
            }
            1 => {
                // Wave effect on letters
                let base = "Thinking...";
                let chars: Vec<char> = base.chars().collect();
                let mut result = String::new();
                for (i, &ch) in chars.iter().enumerate() {
                    let offset = (frame as i32 + i as i32 * 2) % 8;
                    if offset < 4 {
                        result.push(ch.to_ascii_uppercase());
                    } else {
                        result.push(ch.to_ascii_lowercase());
                    }
                }
                result
            }
            2 => {
                // Pulsing dots
                let dots = match frame % 4 {
                    0 => ".",
                    1 => "..",
                    2 => "...",
                    _ => "",
                };
                format!("Thinking{}", dots)
            }
            3 => {
                // Matrix-style random characters
                let base = "Thinking...";
                let chars: Vec<char> = base.chars().collect();
                let mut result = String::new();
                for &ch in &chars {
                    if (frame as usize + result.len()) % 3 == 0 {
                        // Replace with random ASCII char sometimes
                        let random_char = (b'A' + ((frame as u8 + result.len() as u8) % 26)) as char;
                        result.push(random_char);
                    } else {
                        result.push(ch);
                    }
                }
                result
            }
            4 => {
                // Breathing effect with spaces
                let spaces = match (frame / 3) % 6 {
                    0 | 5 => "  ",
                    1 | 4 => " ",
                    _ => "",
                };
                format!("{}Thinking...{}", spaces, spaces)
            }
            _ => {
                // Rainbow wave effect
                let base = "Thinking...";
                let chars: Vec<char> = base.chars().collect();
                let mut result = String::new();
                for (i, &ch) in chars.iter().enumerate() {
                    let wave = ((frame as f32 * 0.3 + i as f32 * 0.5).sin() + 1.0) * 0.5;
                    if wave > 0.7 {
                        result.push(ch.to_ascii_uppercase());
                    } else if wave > 0.3 {
                        result.push(ch);
                    } else {
                        result.push(' ');
                    }
                }
                result
            }
        }
    }

    fn draw_grid_panel(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            // Map selector
            ui.horizontal(|ui| {
                ui.label("Map:");
                let mut selected_map = self.current_map_type;
                egui::ComboBox::from_id_source("map_selector")
                    .selected_text(selected_map.name())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut selected_map,
                            MapType::EmptyGrass,
                            MapType::EmptyGrass.name(),
                        );
                        ui.selectable_value(
                            &mut selected_map,
                            MapType::MazeWalls,
                            MapType::MazeWalls.name(),
                        );
                        ui.selectable_value(
                            &mut selected_map,
                            MapType::DesertOasis,
                            MapType::DesertOasis.name(),
                        );
                        ui.selectable_value(
                            &mut selected_map,
                            MapType::LakeTrees,
                            MapType::LakeTrees.name(),
                        );
                    });

                if selected_map != self.current_map_type {
                    // Defer expensive map creation to avoid blocking UI
                    self.pending_map_change = Some(selected_map);
                }
            });
            ui.add_space(8.0);

            ui.heading("Game Board");
            let avail_r = ui.available_size();
            let board_side = avail_r.x.min(avail_r.y).max(100.0);
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(board_side, board_side), egui::Sense::click());
            let painter = ui.painter();

            self.render_board(&painter, rect);
            self.handle_board_input(ui, rect, board_side, &response);
        });
    }
}
