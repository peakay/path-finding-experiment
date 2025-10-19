use crate::map::{GridMap, TileKind};
use crate::openrouter::{Function, Message, OpenRouterEvent, Tool, open_router_event_stream};
use futures::stream::StreamExt;
use serde_json::{Value, json};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use web_time::{Duration, Instant};

/// Types of log entries for structured display
#[derive(Clone, Debug)]
pub enum LogEntry {
    /// User instruction to the agent
    UserInstruction(String),
    /// Agent's reasoning or response content
    AgentThinking(String),
    /// Tool call initiated (generic display)
    ToolCall { name: String, args: String },
    /// Rich tool proposal (for tools with custom UI rendering)
    ToolProposal {
        name: String,
        data: serde_json::Value,
    },
    /// Tool execution result
    ToolResult {
        name: String,
        success: bool,
        message: String,
    },
    /// Movement step
    Movement {
        direction: String,
        position: (usize, usize),
    },
    /// Error message
    Error(String),
    /// General info message
    Info(String),
}

/// Direction for agent movement
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

impl Direction {
    /// Parse direction from string
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            "up" => Ok(Direction::Up),
            "down" => Ok(Direction::Down),
            "left" => Ok(Direction::Left),
            "right" => Ok(Direction::Right),
            _ => Err(format!("Invalid direction: {}", s)),
        }
    }

    /// Get the string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            Direction::Up => "up",
            Direction::Down => "down",
            Direction::Left => "left",
            Direction::Right => "right",
        }
    }

    /// Apply direction to position, returning new position
    pub fn apply(&self, x: i32, y: i32, map_width: i32, map_height: i32) -> (i32, i32) {
        match self {
            Direction::Up => (x, y.saturating_sub(1)),
            Direction::Down => (x, (y + 1).min(map_height - 1)),
            Direction::Left => (x.saturating_sub(1), y),
            Direction::Right => ((x + 1).min(map_width - 1), y),
        }
    }
}

/// Result of a movement step
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MovementStatus {
    Completed,        // All moves done
    StepSuccess,      // One step succeeded, more to go
    BlockedByTerrain, // Hit obstacle
    OutOfBounds,      // Hit map edge
}

#[derive(Clone, Debug)]
pub struct Agent {
    // Identity
    pub id: u32,
    pub name: String,

    // Position
    pub x: usize,
    pub y: usize,

    // LLM interaction
    chat_history: Vec<Message>,
    logs: Vec<LogEntry>,

    // Movement state
    pending_moves: Vec<Direction>,
    movement_active: bool,
    next_step_at: Option<Instant>,
    current_target: Option<(usize, usize)>, // Optional target for hint generation
    movement_step_index: usize, // Current step number (0-based) in the movement sequence
    total_movement_steps: usize, // Total steps in current movement sequence

    // Configuration
    max_history_messages: usize, // Maximum number of chat history messages to send to LLM
    enabled_tools: HashSet<String>, // Set of enabled tool names

    // Tools
    tool_registry: Vec<Tool>,
}

impl Agent {
    pub fn new(id: u32, name: impl Into<String>, x: usize, y: usize) -> Self {
        let mut agent = Self {
            id,
            name: name.into(),
            x,
            y,
            chat_history: Vec::new(),
            logs: Vec::new(),
            pending_moves: Vec::new(),
            movement_active: false,
            next_step_at: None,
            current_target: None,
            movement_step_index: 0,
            total_movement_steps: 0,
            max_history_messages: 50, // Default to last 50 messages
            enabled_tools: HashSet::new(),
            tool_registry: Vec::new(),
        };

        // Register default tools
        agent.register_default_tools();
        agent
    }

    pub fn pos(&self) -> (usize, usize) {
        (self.x, self.y)
    }

    pub fn set_pos(&mut self, x: usize, y: usize) {
        self.x = x;
        self.y = y;
    }

    /// Get agent logs
    pub fn get_logs(&self) -> &[LogEntry] {
        &self.logs
    }

    /// Add a log entry
    pub fn log(&mut self, entry: LogEntry) {
        web_sys::console::log_1(&format!("{:?}", entry).into());
        self.logs.push(entry);
    }

    /// Add a simple info log
    pub fn log_info(&mut self, message: impl Into<String>) {
        self.log(LogEntry::Info(message.into()));
    }

    /// Get chat history
    pub fn get_chat_history(&self) -> &[Message] {
        &self.chat_history
    }

    /// Clear chat history
    pub fn clear_chat_history(&mut self) {
        self.chat_history.clear();
    }

    /// Add assistant message with tool call to chat history
    pub fn add_assistant_tool_call(
        &mut self,
        tool_call_id: String,
        tool_name: String,
        tool_args: String,
    ) {
        use crate::openrouter::{ToolCall, ToolCallFunction};

        self.chat_history.push(Message {
            role: "assistant".into(),
            content: None,
            tool_calls: Some(vec![ToolCall {
                id: tool_call_id,
                r#type: "function".into(),
                function: ToolCallFunction {
                    name: tool_name,
                    arguments: tool_args,
                },
            }]),
            tool_call_id: None,
            name: None,
        });
    }

    /// Add tool result message to chat history
    pub fn add_tool_result(&mut self, tool_call_id: String, tool_name: String, result: String) {
        self.chat_history.push(Message {
            role: "tool".into(),
            content: Some(result),
            tool_calls: None,
            tool_call_id: Some(tool_call_id),
            name: Some(tool_name),
        });
    }

    /// Generate system prompt based on current context
    pub fn generate_system_prompt(&self, map: &GridMap) -> String {
        format!(
            "You are a tool-using agent named '{}' (ID: {}). \
            Decide on ONE tool call to best accomplish the user's instruction. \
            Return only a tool call with complete JSON arguments.\n\n\
            Current position: ({}, {})\n\
            Map dimensions: {}x{} (width x height)\n\
            Coordinate system: (0,0) is top-left corner\n\
            \n\
            TILE TRAVERSABILITY:\n\
            - TRAVERSABLE (you can move through): empty, grass, sand\n\
            - BLOCKING (you cannot move through): wall, water, tree\n\
            \n\
            Movement will fail if you try to move onto a blocking tile or outside the map boundaries.\n\n\
            IMPORTANT: Use 'get_map_state' tool to see the current map before planning movement.\n\
            For efficient path finding, use the 'area' parameter to focus on specific regions:\n\
            - Use 'area': {{\"x\": X, \"y\": Y}} to view a 7x7 area around coordinate (X,Y)\n\
            - Use 'visibility': N (1-10) to limit view distance from your position\n\
            - Smaller visibility values reduce complexity and speed up planning\n\
            - Focus on areas near your target or along your planned route\n\
            \n\
            Use 'think' tool to plan your path, especially when:\n\
            - Navigating around obstacles\n\
            - Planning multi-step routes\n\
            - Analyzing the map layout\n\
            The think tool helps you reason through complex navigation problems.\n\n\
            Use 'get_bearings' tool when you're stuck or need navigation help:\n\
            - Get information about obstacles and open directions\n\
            - Set a target coordinate for better navigation hints\n\
            - If you're stuck, try picking a different target coordinate!",
            self.name,
            self.id,
            self.x,
            self.y,
            map.width(),
            map.height()
        )
    }

    /// Generate JSON representation of map state with agent position marked
    fn map_state_json(&self, map: &GridMap) -> String {
        self.map_state_json_with_params(map, None, None)
    }

    /// Generate JSON representation of map state with optional area and visibility parameters
    fn map_state_json_with_params(&self, map: &GridMap, area: Option<(usize, usize)>, visibility: Option<usize>) -> String {
        let visibility = visibility.unwrap_or(5).clamp(1, 10);

        // Determine the view bounds
        let (view_x, view_y, view_width, view_height) = if let Some((center_x, center_y)) = area {
            // Area mode: show 7x7 area around center point
            let half_size = 3; // 3 tiles in each direction from center
            let start_x = (center_x as i32 - half_size).max(0) as usize;
            let start_y = (center_y as i32 - half_size).max(0) as usize;
            let end_x = ((center_x as i32 + half_size + 1) as usize).min(map.width());
            let end_y = ((center_y as i32 + half_size + 1) as usize).min(map.height());
            (start_x, start_y, end_x - start_x, end_y - start_y)
        } else {
            // Visibility mode: show area within visibility distance from agent
            let start_x = (self.x as i32 - visibility as i32).max(0) as usize;
            let start_y = (self.y as i32 - visibility as i32).max(0) as usize;
            let end_x = ((self.x as i32 + visibility as i32 + 1) as usize).min(map.width());
            let end_y = ((self.y as i32 + visibility as i32 + 1) as usize).min(map.height());
            (start_x, start_y, end_x - start_x, end_y - start_y)
        };

        let mut rows: Vec<Vec<String>> = Vec::with_capacity(view_height);
        for y in view_y..(view_y + view_height) {
            let mut row: Vec<String> = Vec::with_capacity(view_width);
            for x in view_x..(view_x + view_width) {
                // Mark agent's position with "@" prefix
                if x == self.x && y == self.y {
                    let tile_name = map.get(x, y).map(|tile| tile.name()).unwrap_or("empty");
                    row.push(format!("@{}", tile_name));
                } else {
                    let tile_name = map.get(x, y).map(|tile| tile.name()).unwrap_or("empty");
                    row.push(tile_name.to_string());
                }
            }
            rows.push(row);
        }

        // Create ASCII minimap for better readability
        let mut minimap = String::new();
        minimap.push_str(&format!("Map View ({}x{} area at ({}, {}))\n",
            view_width, view_height, view_x, view_y));
        minimap.push_str(&format!("Agent at ({}, {}) in full map ({}x{})\n",
            self.x, self.y, map.width(), map.height()));

        if let Some((area_x, area_y)) = area {
            minimap.push_str(&format!("Area center: ({}, {})\n", area_x, area_y));
        }
        minimap.push_str(&format!("Visibility used: {}\n\n", visibility));

        // Create the visual map
        for y in view_y..(view_y + view_height) {
            for x in view_x..(view_x + view_width) {
                if x == self.x && y == self.y {
                    minimap.push('@');
                } else {
                    let tile_char = match map.get(x, y) {
                        Some(TileKind::Empty) => '.',
                        Some(TileKind::Grass) => ',',
                        Some(TileKind::Water) => '~',
                        Some(TileKind::Sand) => 's',
                        Some(TileKind::Wall) => '#',
                        Some(TileKind::Trail) => '*',
                        Some(TileKind::Tree) => 'T',
                        Some(TileKind::Custom(_)) => '?',
                        None => ' ',
                    };
                    minimap.push(tile_char);
                }
            }
            minimap.push('\n');
        }

        // Add legend
        minimap.push_str("\nLegend: @=agent, .=empty, ,=grass, ~=water, #=wall, T=tree, s=sand, *=trail\n");

        let mut result = json!({
            "view_bounds": {
                "x": view_x,
                "y": view_y,
                "width": view_width,
                "height": view_height
            },
            "full_map_size": {
                "width": map.width(),
                "height": map.height()
            },
            "tiles": rows,
            "agent_position": {"x": self.x, "y": self.y}
        });

        // Add area info if specified
        if let Some((area_x, area_y)) = area {
            result["area_center"] = json!({"x": area_x, "y": area_y});
        }

        // Add visibility info
        result["visibility_used"] = json!(visibility);

        // Store minimap separately for UI display (not sent to LLM)
        // The UI will reconstruct this from the JSON data if needed
        result["_minimap"] = json!(minimap); // Prefix with underscore to indicate UI-only

        serde_json::to_string(&result).unwrap_or_else(|_| "{}".to_string())
    }

    /// Register default tools for this agent
    fn register_default_tools(&mut self) {
        // Get map state tool
        self.tool_registry.push(Tool {
            type_: "function".into(),
            function: Function {
                name: "get_map_state".into(),
                description: "Get a view of the map state. Use 'area' parameter to focus on a specific region and 'visibility' to control how much you can see. Your position is marked with '@' prefix. Use smaller areas for path finding to reduce complexity.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "area": {
                            "type": "object",
                            "description": "Center point (x, y) to focus the map view around. Shows a 7x7 area around this point.",
                            "properties": {
                                "x": {"type": "integer", "description": "X coordinate of area center"},
                                "y": {"type": "integer", "description": "Y coordinate of area center"}
                            }
                        },
                        "visibility": {
                            "type": "integer",
                            "description": "How far you can see from your current position (1-10). Smaller values show less area but are faster for planning.",
                            "minimum": 1,
                            "maximum": 10,
                            "default": 5
                        }
                    },
                    "required": []
                }),
            }
        });
        self.enabled_tools.insert("get_map_state".to_string());

        // Thinking tool - no-op, used for planning
        self.tool_registry.push(Tool {
            type_: "function".into(),
            function: Function {
                name: "think".into(),
                description: "Think out loud about your planning and reasoning. Use this to work through path planning, obstacle avoidance, or multi-step strategies before taking action.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "thoughts": {
                            "type": "string",
                            "description": "Your internal reasoning, planning, or analysis"
                        }
                    },
                    "required": ["thoughts"]
                }),
            }
        });
        self.enabled_tools.insert("think".to_string());

        // Movement tool
        self.tool_registry.push(Tool {
            type_: "function".into(),
            function: Function {
                name: "move_agent".into(),
                description: "Move the agent by executing up to 5 steps from ['up','down','left','right']. Optionally specify a target coordinate to receive helpful hints if blocked.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "agent_id": {"type":"integer"},
                        "steps": {
                            "type": "array",
                            "items": {"type": "string", "enum": ["up","down","left","right"]},
                            "minItems": 1,
                            "maxItems": 5
                        },
                        "target": {
                            "type": "object",
                            "description": "Optional target destination coordinate (x, y) - used to provide navigation hints if you get blocked",
                            "properties": {
                                "x": {"type": "integer"},
                                "y": {"type": "integer"}
                            }
                        }
                    },
                    "required": ["agent_id","steps"]
                }),
            }
        });
        self.enabled_tools.insert("move_agent".to_string());

        // Get position tool
        self.tool_registry.push(Tool {
            type_: "function".into(),
            function: Function {
                name: "get_position".into(),
                description: "Get the agent's current position coordinates (x, y).".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }),
            },
        });
        self.enabled_tools.insert("get_position".to_string());

        // Get available directions tool
        self.tool_registry.push(Tool {
            type_: "function".into(),
            function: Function {
                name: "get_available_directions".into(),
                description: "Get the list of valid movement directions from the current position. Returns directions that lead to traversable tiles.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }),
            }
        });
        self.enabled_tools
            .insert("get_available_directions".to_string());

        // Get bearings tool - provides navigation hints when blocked
        self.tool_registry.push(Tool {
            type_: "function".into(),
            function: Function {
                name: "get_bearings".into(),
                description: "Get navigation bearings and hints when you're blocked or need to navigate to a target. Provides information about obstacles, open directions, and distance to target.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "target": {
                            "type": "object",
                            "description": "Optional target destination coordinate (x, y) for navigation hints",
                            "properties": {
                                "x": {"type": "integer"},
                                "y": {"type": "integer"}
                            }
                        }
                    },
                    "required": []
                }),
            }
        });
        self.enabled_tools.insert("get_bearings".to_string());
    }

    /// Register a custom tool
    pub fn register_tool(&mut self, tool: Tool) {
        self.tool_registry.push(tool);
    }

    /// Get all enabled tools
    pub fn get_tools(&self) -> Vec<Tool> {
        self.tool_registry
            .iter()
            .filter(|tool| self.enabled_tools.contains(&tool.function.name))
            .cloned()
            .collect()
    }

    /// Get all registered tools (including disabled ones)
    pub fn get_all_tools(&self) -> &[Tool] {
        &self.tool_registry
    }

    /// Enable a tool by name
    pub fn enable_tool(&mut self, name: &str) -> bool {
        if self.tool_registry.iter().any(|t| t.function.name == name) {
            self.enabled_tools.insert(name.to_string());
            true
        } else {
            false
        }
    }

    /// Disable a tool by name
    pub fn disable_tool(&mut self, name: &str) -> bool {
        self.enabled_tools.remove(name)
    }

    /// Check if a tool is enabled
    pub fn is_tool_enabled(&self, name: &str) -> bool {
        self.enabled_tools.contains(name)
    }

    /// Get list of enabled tool names
    pub fn get_enabled_tools(&self) -> Vec<String> {
        self.enabled_tools.iter().cloned().collect()
    }

    /// Execute an instruction via LLM
    pub fn execute_instruction(
        &mut self,
        instruction: String,
        api_key: String,
        model: String,
        map: &GridMap,
        tool_callback: Arc<Mutex<Vec<(u32, String, Value)>>>,
        log_callback: Arc<Mutex<Vec<(u32, LogEntry)>>>,
        llm_status_callback: Arc<Mutex<bool>>,
    ) {
        let system_prompt = self.generate_system_prompt(map);

        // Check if this is a continuation (empty instruction) or new instruction
        let is_continuation = instruction.is_empty();

        if !is_continuation {
            // Log the user instruction
            self.log(LogEntry::UserInstruction(instruction.clone()));

            // Add to chat history
            self.chat_history.push(Message {
                role: "user".into(),
                content: Some(instruction.clone()),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            });
        }

        let agent_id = self.id;
        let tools = self.get_tools();

        // Build messages with history
        let mut messages = vec![Message {
            role: "system".into(),
            content: Some(system_prompt),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }];

        // Add last N chat history messages (includes tool results from previous turns)
        let start_idx = self
            .chat_history
            .len()
            .saturating_sub(self.max_history_messages);
        for msg in &self.chat_history[start_idx..] {
            messages.push(msg.clone());
        }

        // Spawn async task for streaming
        wasm_bindgen_futures::spawn_local(async move {
            // Set LLM active flag
            if let Ok(mut status) = llm_status_callback.lock() {
                *status = true;
            }

            let mut name_buf: Option<String> = None;
            let mut args_buf = String::new();
            let mut content_buf = String::new();
            let mut stream = open_router_event_stream(api_key, model, messages, None, Some(tools));

            while let Some(evt) = stream.next().await {
                match evt {
                    Ok(OpenRouterEvent::Content(c)) => {
                        web_sys::console::log_1(&format!("Content: {}", c).into());
                        content_buf.push_str(&c);
                    }
                    Ok(OpenRouterEvent::ToolCallDelta {
                        name,
                        arguments_delta,
                    }) => {
                        web_sys::console::log_1(
                            &format!(
                                "ToolCallDelta - name: {:?}, args: {:?}",
                                name, arguments_delta
                            )
                            .into(),
                        );
                        if let Some(n) = name {
                            name_buf = Some(n);
                        }
                        if let Some(a) = arguments_delta {
                            args_buf.push_str(&a);
                        }
                    }
                    Err(e) => {
                        web_sys::console::log_1(&format!("Stream error: {}", e).into());
                        if let Ok(mut g) = log_callback.lock() {
                            g.push((agent_id, LogEntry::Error(format!("Stream error: {}", e))));
                        }
                    }
                }
            }

            // Log accumulated agent thinking content if any
            if !content_buf.is_empty() {
                if let Ok(mut g) = log_callback.lock() {
                    g.push((agent_id, LogEntry::AgentThinking(content_buf)));
                }
            }

            web_sys::console::log_1(
                &format!("Final name_buf: {:?}, args_buf: {}", name_buf, args_buf).into(),
            );

            if let Some(n) = name_buf {
                web_sys::console::log_1(
                    &format!("Parsing tool call: {} with args: {}", n, args_buf).into(),
                );
                let mut parsed = serde_json::from_str::<Value>(&args_buf).unwrap_or(Value::Null);
                web_sys::console::log_1(&format!("Parsed JSON: {:?}", parsed).into());

                // Inject agent_id if not present
                if let Value::Object(ref mut map) = parsed {
                    map.insert("agent_id".to_string(), Value::from(agent_id as u64));
                }

                web_sys::console::log_1(&format!("Final args with agent_id: {:?}", parsed).into());

                // Log the tool call - use rich proposal for tools that have custom UI
                if let Ok(mut g) = log_callback.lock() {
                    let log_entry = if n == "think" || n == "get_map_state" {
                        // Rich tool proposal for tools with custom UI
                        LogEntry::ToolProposal {
                            name: n.clone(),
                            data: parsed.clone(),
                        }
                    } else {
                        // Generic tool call display
                        LogEntry::ToolCall {
                            name: n.clone(),
                            args: serde_json::to_string_pretty(&parsed).unwrap_or_default(),
                        }
                    };
                    g.push((agent_id, log_entry));
                }

                if let Ok(mut g) = tool_callback.lock() {
                    g.push((agent_id, n, parsed));
                }
            }

            // Clear LLM active flag when streaming completes
            if let Ok(mut status) = llm_status_callback.lock() {
                *status = false;
            }
        });
    }

    /// Handle a tool call for this agent
    pub fn handle_tool_call(
        &mut self,
        name: &str,
        args: Value,
        map: &mut GridMap,
    ) -> Result<String, String> {
        // Handle get_map_state tool (no agent_id required)
        if name == "get_map_state" {
            // Parse optional area and visibility parameters
            let area = args.get("area").and_then(|a| {
                let x = a.get("x")?.as_u64()? as usize;
                let y = a.get("y")?.as_u64()? as usize;
                Some((x, y))
            });

            let visibility = args.get("visibility")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize);

            let map_state = self.map_state_json_with_params(map, area, visibility);
            // Don't log here - the ToolProposal log entry will display the map fetch
            return Ok(map_state);
        }

        // Handle thinking tool (no agent_id required)
        if name == "think" {
            let thoughts = args
                .get("thoughts")
                .and_then(|v| v.as_str())
                .unwrap_or("(empty thoughts)");

            // Don't log here - the ToolProposal log entry will display the thoughts

            return Ok(format!("Recorded thoughts: {}", thoughts));
        }

        // Verify agent_id matches for other tools
        let agent_id_val = args
            .get("agent_id")
            .and_then(|v| v.as_u64())
            .ok_or("missing agent_id")? as u32;

        if agent_id_val != self.id {
            let err = format!(
                "agent id mismatch: expected {}, got {}",
                self.id, agent_id_val
            );
            self.log(LogEntry::ToolResult {
                name: name.to_string(),
                success: false,
                message: err.clone(),
            });
            return Err(err);
        }

        let result = match name {
            "move_agent" => self.handle_move_agent_tool(args, map),
            "get_position" => self.handle_get_position_tool(),
            "get_available_directions" => self.handle_get_available_directions_tool(map),
            "get_bearings" => self.handle_get_bearings_tool(args, map),
            _ => Err(format!("unknown tool: {}", name)),
        };

        // Log the result
        match &result {
            Ok(msg) => {
                self.log(LogEntry::ToolResult {
                    name: name.to_string(),
                    success: true,
                    message: msg.clone(),
                });
            }
            Err(e) => {
                self.log(LogEntry::ToolResult {
                    name: name.to_string(),
                    success: false,
                    message: e.clone(),
                });
            }
        }

        result
    }

    /// Execute a single move step (used by event system)
    /// Returns Err with "ABORT" prefix if movement should cancel remaining events
    pub fn execute_move_step(
        &mut self,
        direction: Direction,
        map: &mut GridMap,
    ) -> Result<(), String> {
        let (x, y) = (self.x as i32, self.y as i32);
        let w = map.width() as i32;
        let h = map.height() as i32;

        let (nx, ny) = direction.apply(x, y, w, h);

        // Check bounds
        if nx < 0 || ny < 0 || nx >= w || ny >= h {
            let err = "ABORT: Movement blocked - edge of map".to_string();
            self.log(LogEntry::Error("Movement blocked: edge of map".to_string()));
            return Err(err);
        }

        // Check traversability
        if !map.is_traversable(nx as usize, ny as usize) {
            let tile_type = map
                .get(nx as usize, ny as usize)
                .map(|t| t.name())
                .unwrap_or("unknown");
            let err = format!(
                "ABORT: Movement blocked by {} tile at ({}, {})",
                tile_type, nx, ny
            );
            self.log(LogEntry::Error(format!(
                "Movement blocked by {} tile at ({}, {})\n\
                Agent position: ({}, {})\n\
                Attempted move: {} to ({}, {})\n\
                Traversable tiles: empty, grass, sand, trail\n\
                Blocking tiles: wall, water, tree",
                tile_type,
                nx,
                ny,
                self.x,
                self.y,
                direction.as_str(),
                nx,
                ny
            )));
            return Err(err);
        }

        // Leave trail
        let _ = map.set(self.x, self.y, TileKind::Trail);

        // Move agent
        self.set_pos(nx as usize, ny as usize);
        self.log(LogEntry::Movement {
            direction: direction.as_str().to_string(),
            position: (self.x, self.y),
        });

        Ok(())
    }

    /// Handle the get_position tool
    fn handle_get_position_tool(&self) -> Result<String, String> {
        Ok(serde_json::to_string(&json!({
            "x": self.x,
            "y": self.y
        }))
        .unwrap_or_else(|_| format!("{{\"x\": {}, \"y\": {}}}", self.x, self.y)))
    }

    /// Handle the get_available_directions tool
    fn handle_get_available_directions_tool(&self, map: &GridMap) -> Result<String, String> {
        let mut valid_directions = Vec::new();

        for direction in [
            Direction::Up,
            Direction::Down,
            Direction::Left,
            Direction::Right,
        ] {
            let (nx, ny) = direction.apply(
                self.x as i32,
                self.y as i32,
                map.width() as i32,
                map.height() as i32,
            );

            // Check bounds and traversability
            if nx >= 0
                && ny >= 0
                && nx < map.width() as i32
                && ny < map.height() as i32
                && map.is_traversable(nx as usize, ny as usize)
            {
                valid_directions.push(direction.as_str().to_string());
            }
        }

        Ok(serde_json::to_string(&json!({
            "position": {"x": self.x, "y": self.y},
            "available_directions": valid_directions
        }))
        .unwrap_or_else(|_| {
            format!(
                "{{\"position\": {{\"x\": {}, \"y\": {}}}, \"available_directions\": {:?}}}",
                self.x, self.y, valid_directions
            )
        }))
    }

    /// Handle the get_bearings tool
    fn handle_get_bearings_tool(&mut self, args: Value, map: &GridMap) -> Result<String, String> {
        // Extract optional target
        let target = args.get("target").and_then(|t| {
            let x = t.get("x")?.as_u64()? as usize;
            let y = t.get("y")?.as_u64()? as usize;
            Some((x, y))
        });

        // Set current target for navigation hints
        self.current_target = target;

        let mut result = json!({
            "position": {"x": self.x, "y": self.y}
        });

        // Add target info if provided
        if let Some((tx, ty)) = target {
            let dx = tx as i32 - self.x as i32;
            let dy = ty as i32 - self.y as i32;
            result["target"] = json!({
                "x": tx,
                "y": ty,
                "distance": dx.abs() + dy.abs(),
                "delta_x": dx,
                "delta_y": dy
            });
        }

        // Check what's blocking/open in each cardinal direction
        let mut blocking_directions = Vec::new();
        let mut open_directions = Vec::new();

        for (dir, dir_name) in [
            (Direction::Up, "north"),
            (Direction::Down, "south"),
            (Direction::Left, "west"),
            (Direction::Right, "east"),
        ] {
            let (nx, ny) = dir.apply(
                self.x as i32,
                self.y as i32,
                map.width() as i32,
                map.height() as i32,
            );

            if nx < 0 || ny < 0 || nx >= map.width() as i32 || ny >= map.height() as i32 {
                blocking_directions.push(json!({
                    "direction": dir_name,
                    "reason": "map_edge"
                }));
            } else if !map.is_traversable(nx as usize, ny as usize) {
                let tile = map
                    .get(nx as usize, ny as usize)
                    .map(|t| t.name())
                    .unwrap_or("unknown");
                blocking_directions.push(json!({
                    "direction": dir_name,
                    "reason": "obstacle",
                    "tile": tile
                }));
            } else {
                open_directions.push(dir_name);
            }
        }

        result["blocking_directions"] = json!(blocking_directions);
        result["open_directions"] = json!(open_directions);

        // Add navigation advice
        let mut advice = Vec::new();

        if open_directions.is_empty() {
            advice.push("You're completely blocked! Try using 'get_map_state' to see the bigger picture.");
        } else {
            advice.push("You have open directions available.");
        }

        if target.is_some() && !open_directions.is_empty() {
            advice.push("If you're stuck, try picking a different target coordinate with the 'move_agent' tool.");
        } else if target.is_none() {
            advice.push("Consider specifying a target coordinate when moving to get better navigation hints.");
        }

        result["advice"] = json!(advice);

        Ok(serde_json::to_string(&result)
            .unwrap_or_else(|_| format!("{{\"position\": {{\"x\": {}, \"y\": {}}}}}", self.x, self.y)))
    }

    /// Handle the move_agent tool - returns directions for event submission
    fn handle_move_agent_tool(&mut self, args: Value, _map: &GridMap) -> Result<String, String> {
        let steps = args
            .get("steps")
            .and_then(|v| v.as_array())
            .ok_or("missing steps")?;

        if steps.is_empty() || steps.len() > 5 {
            return Err("steps must be 1..=5".into());
        }

        // Extract optional target
        self.current_target = args.get("target").and_then(|t| {
            let x = t.get("x")?.as_u64()? as usize;
            let y = t.get("y")?.as_u64()? as usize;
            Some((x, y))
        });

        // Parse directions
        let mut directions = Vec::new();
        for step in steps {
            let step_str = step.as_str().ok_or("step must be string")?;
            directions.push(Direction::from_str(step_str)?);
        }

        // Store for event submission (handled by caller)
        self.pending_moves = directions;
        self.movement_active = true;
        self.next_step_at = Some(Instant::now());
        self.total_movement_steps = self.pending_moves.len();
        self.movement_step_index = 0;

        let target_info = if let Some((tx, ty)) = self.current_target {
            format!(" towards target ({}, {})", tx, ty)
        } else {
            String::new()
        };

        self.log_info(format!(
            "Preparing {} movement steps{} starting at ({}, {})",
            self.pending_moves.len(),
            target_info,
            self.x,
            self.y
        ));

        Ok(String::new())
    }

    /// Get and clear pending moves (for event submission)
    pub fn take_pending_moves(&mut self) -> Vec<Direction> {
        self.movement_active = false;
        self.next_step_at = None;
        self.total_movement_steps = 0;
        self.movement_step_index = 0;
        std::mem::take(&mut self.pending_moves)
    }

    /// Check if agent is currently moving
    pub fn is_moving(&self) -> bool {
        self.movement_active
    }

    /// Get maximum history messages sent to LLM
    pub fn max_history_messages(&self) -> usize {
        self.max_history_messages
    }

    /// Set maximum history messages sent to LLM
    pub fn set_max_history_messages(&mut self, max: usize) {
        self.max_history_messages = max.max(1); // Minimum of 1
    }

    /// Generate navigation hint based on current position and target
    pub fn generate_navigation_hint(&self, map: &GridMap, had_errors: bool) -> Option<String> {
        if !had_errors {
            return None; // No hint needed if successful
        }

        let Some((target_x, target_y)) = self.current_target else {
            return None; // No target provided, can't give hints
        };

        let dx = target_x as i32 - self.x as i32;
        let dy = target_y as i32 - self.y as i32;

        // Check what's blocking in each cardinal direction
        let mut blocking_dirs = Vec::new();
        let mut open_dirs = Vec::new();

        for (dir, dir_name) in [
            (Direction::Up, "north"),
            (Direction::Down, "south"),
            (Direction::Left, "west"),
            (Direction::Right, "east"),
        ] {
            let (nx, ny) = dir.apply(
                self.x as i32,
                self.y as i32,
                map.width() as i32,
                map.height() as i32,
            );

            if nx < 0 || ny < 0 || nx >= map.width() as i32 || ny >= map.height() as i32 {
                blocking_dirs.push(format!("{} (map edge)", dir_name));
            } else if !map.is_traversable(nx as usize, ny as usize) {
                let tile = map
                    .get(nx as usize, ny as usize)
                    .map(|t| t.name())
                    .unwrap_or("unknown");
                blocking_dirs.push(format!("{} ({})", dir_name, tile));
            } else {
                open_dirs.push(dir_name);
            }
        }

        let mut hint = format!("NAVIGATION HINT: Target at ({}, {}). ", target_x, target_y);
        hint.push_str(&format!("You're at ({}, {}). ", self.x, self.y));
        hint.push_str(&format!(
            "Distance: {} steps ({} east/west, {} north/south). ",
            dx.abs() + dy.abs(),
            dx.abs(),
            dy.abs()
        ));

        if !blocking_dirs.is_empty() {
            hint.push_str(&format!("Blocked: {}. ", blocking_dirs.join(", ")));
        }

        if !open_dirs.is_empty() {
            hint.push_str(&format!("Open directions: {}. ", open_dirs.join(", ")));
            hint.push_str("Consider using 'think' tool to plan an alternate route.");
        }

        Some(hint)
    }

    /// Process one movement step (call this from update loop)
    pub fn process_movement_step(&mut self, map: &mut GridMap) -> MovementStatus {
        if !self.movement_active {
            return MovementStatus::Completed;
        }

        let now = Instant::now();
        if let Some(next_at) = self.next_step_at {
            if now < next_at {
                return MovementStatus::StepSuccess; // Still waiting
            }
        }

        if let Some(dir) = self.pending_moves.first().cloned() {
            let current_step = self.movement_step_index + 1;
            let (x, y) = (self.x as i32, self.y as i32);
            let w = map.width() as i32;
            let h = map.height() as i32;

            let (nx, ny) = dir.apply(x, y, w, h);

            // Check bounds
            if nx < 0 || ny < 0 || nx >= w || ny >= h {
                self.log(LogEntry::Error(format!(
                    "Movement aborted on step {} of {}: edge of map",
                    current_step, self.total_movement_steps
                )));
                self.movement_active = false;
                self.pending_moves.clear();
                self.next_step_at = None;
                return MovementStatus::OutOfBounds;
            }

            // Check traversability
            if !map.is_traversable(nx as usize, ny as usize) {
                let tile_type = map
                    .get(nx as usize, ny as usize)
                    .map(|t| t.name())
                    .unwrap_or("unknown");
                self.log(LogEntry::Error(format!(
                    "Movement aborted on step {} of {}: blocked by {} tile at ({}, {})\n\
                    Agent position: ({}, {})\n\
                    Attempted move: {} to ({}, {})\n\
                    Traversable tiles: empty, grass, sand, trail\n\
                    Blocking tiles: wall, water, tree",
                    current_step,
                    self.total_movement_steps,
                    tile_type,
                    nx,
                    ny,
                    self.x,
                    self.y,
                    dir.as_str(),
                    nx,
                    ny
                )));
                self.movement_active = false;
                self.pending_moves.clear();
                self.next_step_at = None;
                return MovementStatus::BlockedByTerrain;
            }

            // Leave trail
            let _ = map.set(self.x, self.y, TileKind::Trail);

            // Move agent
            self.set_pos(nx as usize, ny as usize);
            self.log(LogEntry::Movement {
                direction: dir.as_str().to_string(),
                position: (self.x, self.y),
            });

            // Remove completed step
            self.pending_moves.remove(0);
            self.movement_step_index += 1;

            // Check if done
            if self.pending_moves.is_empty() {
                self.movement_active = false;
                self.next_step_at = None;
                self.total_movement_steps = 0;
                self.movement_step_index = 0;
                self.log_info("Movement completed");
                return MovementStatus::Completed;
            } else {
                self.next_step_at = Some(Instant::now() + Duration::from_secs(1));
                return MovementStatus::StepSuccess;
            }
        } else {
            self.movement_active = false;
            self.next_step_at = None;
            return MovementStatus::Completed;
        }
    }
}
