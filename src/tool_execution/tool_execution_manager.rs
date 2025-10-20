use crate::agent::{Agent, LogEntry};
use crate::events::{Event, EventQueue, PendingToolExecution};
use crate::map::GridMap;
use serde_json::Value;
use std::sync::{Arc, Mutex};
use web_time::Duration;

/// Manages tool execution, callbacks, and event coordination
pub struct ToolExecutionManager {
    /// Queue for tool callbacks from async operations
    tool_callbacks: Arc<Mutex<Vec<(u32, String, Value)>>>,

    /// Track pending tool executions waiting for events to complete
    pending_tool_executions: Vec<PendingToolExecution>,

    /// Tick rate for event scheduling
    tick_rate: Duration,
}

impl ToolExecutionManager {
    pub fn new(tick_rate: Duration) -> Self {
        Self {
            tool_callbacks: Arc::new(Mutex::new(Vec::new())),
            pending_tool_executions: Vec::new(),
            tick_rate,
        }
    }

    /// Get a clone of the tool callbacks Arc for sharing with async tasks
    pub fn get_tool_callbacks(&self) -> Arc<Mutex<Vec<(u32, String, Value)>>> {
        self.tool_callbacks.clone()
    }

    /// Process tool callbacks and dispatch them to the agent
    /// Returns true if execution should continue (tool result was added)
    pub fn process_tool_callbacks(
        &mut self,
        agent: &mut Agent,
        map: &mut GridMap,
        event_queue: &EventQueue,
        agent_running: bool,
    ) -> bool {
        let mut should_continue = false;

        // Drain tool callbacks
        let pending: Vec<(u32, String, Value)> = {
            let mut g = self.tool_callbacks.lock().unwrap();
            g.drain(..).collect()
        };

        for (agent_id, name, args) in pending {
            if agent_id == agent.id {
                // Skip processing tool callbacks if execution was cancelled
                if !agent_running {
                    agent.log(LogEntry::Info(format!(
                        "TOOL: Discarded '{}' tool call (execution cancelled)",
                        name
                    )));
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
                agent.add_assistant_tool_call(tool_call_id.clone(), name.clone(), args_str);

                match agent.handle_tool_call(&name, args, map) {
                    Ok(result_msg) => {
                        // Check if tool generated any pending moves (events to submit)
                        let moves = agent.take_pending_moves();

                        if !moves.is_empty() {
                            // Tool needs to wait for events to complete
                            let events: Vec<Event> = moves
                                .into_iter()
                                .map(|direction| Event::AgentMove {
                                    agent_id: agent.id,
                                    direction,
                                })
                                .collect();

                            // Submit with 2-tick delay between moves
                            let event_ids =
                                event_queue.submit_sequence(events, self.tick_rate * 2);

                            // Create pending tool execution to track this
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
                            if !result_msg.is_empty() {
                                agent.add_tool_result(tool_call_id, name.clone(), result_msg);
                            }
                            should_continue = true;
                        }
                    }
                    Err(e) => {
                        // Add error as tool result immediately
                        agent.add_tool_result(
                            tool_call_id,
                            name.clone(),
                            format!("Error: {}", e),
                        );
                        agent.log(LogEntry::Error(format!("Tool execution failed: {}", e)));
                        should_continue = true;
                    }
                }
            }
        }

        should_continue
    }

    /// Process pending tool executions and complete those whose events are done
    /// Returns true if any executions completed and should continue
    pub fn process_pending_executions(
        &mut self,
        agent: &mut Agent,
        event_queue: &EventQueue,
    ) -> bool {
        let mut should_continue = false;
        let mut completed_indices = Vec::new();

        for (idx, pending) in self.pending_tool_executions.iter().enumerate() {
            if pending.is_complete(event_queue) {
                completed_indices.push(idx);
            }
        }

        // Process completed tool executions in reverse order to maintain indices
        for idx in completed_indices.into_iter().rev() {
            let pending = self.pending_tool_executions.remove(idx);

            // Get event results
            let event_results = pending.get_event_results(event_queue);

            // Check logs for errors/cancellations
            let recent_logs: Vec<&LogEntry> = agent.get_logs().iter().rev().take(20).collect();

            let had_errors = recent_logs.iter().any(|entry| {
                matches!(entry, LogEntry::Error(msg) if msg.contains("Movement blocked") || msg.contains("ABORT"))
            });

            let cancelled = recent_logs.iter().any(|entry| {
                matches!(entry, LogEntry::Info(msg) if msg.contains("events cancelled"))
            });

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

            agent.add_tool_result(pending.tool_call_id, pending.tool_name, result_msg);
            should_continue = true;
        }

        should_continue
    }

    /// Check if there are any pending tool executions
    pub fn has_pending_executions(&self) -> bool {
        !self.pending_tool_executions.is_empty()
    }

    /// Get the number of pending tool executions
    pub fn pending_count(&self) -> usize {
        self.pending_tool_executions.len()
    }

    /// Clear all pending callbacks (used when cancelling execution)
    pub fn clear_callbacks(&mut self) {
        if let Ok(mut callbacks) = self.tool_callbacks.lock() {
            callbacks.clear();
        }
    }
}
