use std::sync::{Arc, Mutex};
use std::collections::VecDeque;
use web_time::{Duration, Instant};
use crate::agent::Direction;

/// Unique identifier for events
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EventId(u64);

static EVENT_COUNTER: Mutex<u64> = Mutex::new(0);

impl EventId {
    pub fn new() -> Self {
        let mut counter = EVENT_COUNTER.lock().unwrap();
        let id = *counter;
        *counter += 1;
        EventId(id)
    }
}

/// Status of an event
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EventStatus {
    Pending,
    Processing,
    Completed,
    Failed(String),
}

/// Types of events that can occur in the game
#[derive(Clone, Debug)]
pub enum Event {
    /// Agent movement event
    AgentMove {
        agent_id: u32,
        direction: Direction,
    },
    /// Delay event (wait for N ticks)
    Delay {
        ticks: u32,
    },
}

/// A scheduled event with its execution time
#[derive(Clone, Debug)]
pub struct ScheduledEvent {
    pub id: EventId,
    pub event: Event,
    pub execute_at: Instant,
    pub status: EventStatus,
}

impl ScheduledEvent {
    pub fn new(event: Event, delay: Duration) -> Self {
        Self {
            id: EventId::new(),
            event,
            execute_at: Instant::now() + delay,
            status: EventStatus::Pending,
        }
    }

    pub fn immediate(event: Event) -> Self {
        Self::new(event, Duration::from_millis(0))
    }

    pub fn is_ready(&self) -> bool {
        Instant::now() >= self.execute_at
    }
}

/// Event queue that manages all pending events
#[derive(Clone)]
pub struct EventQueue {
    events: Arc<Mutex<VecDeque<ScheduledEvent>>>,
    completed: Arc<Mutex<Vec<(EventId, Result<(), String>)>>>,
}

impl EventQueue {
    pub fn new() -> Self {
        Self {
            events: Arc::new(Mutex::new(VecDeque::new())),
            completed: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Submit an event to be executed after a delay
    pub fn submit(&self, event: Event, delay: Duration) -> EventId {
        let scheduled = ScheduledEvent::new(event, delay);
        let id = scheduled.id;

        let mut queue = self.events.lock().unwrap();
        queue.push_back(scheduled);

        id
    }

    /// Submit an event to be executed immediately
    pub fn submit_immediate(&self, event: Event) -> EventId {
        self.submit(event, Duration::from_millis(0))
    }

    /// Submit multiple events as a sequence (each waits for previous to complete)
    /// Returns a SequenceId that can be used to cancel all events in the sequence
    pub fn submit_sequence(&self, events: Vec<Event>, delay_between: Duration) -> Vec<EventId> {
        let mut ids = Vec::new();
        let mut current_delay = Duration::from_millis(0);

        for event in events {
            let id = self.submit(event, current_delay);
            ids.push(id);
            current_delay += delay_between;
        }

        ids
    }

    /// Cancel events by their IDs
    pub fn cancel_events(&self, event_ids: &[EventId]) {
        let mut queue = self.events.lock().unwrap();

        // Remove all matching events
        queue.retain(|event| !event_ids.contains(&event.id));
    }

    /// Cancel all pending events for a specific agent
    pub fn cancel_agent_events(&self, agent_id: u32) {
        let mut queue = self.events.lock().unwrap();

        // Remove all events for this agent
        queue.retain(|event| {
            match &event.event {
                Event::AgentMove { agent_id: id, .. } => *id != agent_id,
                _ => true, // Keep other event types
            }
        });
    }

    /// Get the next ready event to process
    pub fn pop_ready(&self) -> Option<ScheduledEvent> {
        let mut queue = self.events.lock().unwrap();

        // Find first ready event
        if let Some(pos) = queue.iter().position(|e| e.is_ready() && e.status == EventStatus::Pending) {
            if let Some(mut event) = queue.remove(pos) {
                event.status = EventStatus::Processing;
                // Put it back at the end while processing
                queue.push_back(event.clone());
                return Some(event);
            }
        }

        None
    }

    /// Mark an event as completed
    pub fn complete(&self, id: EventId, result: Result<(), String>) {
        let mut queue = self.events.lock().unwrap();

        // Remove from queue
        if let Some(pos) = queue.iter().position(|e| e.id == id) {
            queue.remove(pos);
        }

        // Add to completed
        let mut completed = self.completed.lock().unwrap();
        completed.push((id, result));
    }

    /// Check if an event has completed
    pub fn is_completed(&self, id: EventId) -> Option<Result<(), String>> {
        let completed = self.completed.lock().unwrap();
        completed.iter()
            .find(|(event_id, _)| *event_id == id)
            .map(|(_, result)| result.clone())
    }

    /// Get count of pending events
    pub fn pending_count(&self) -> usize {
        self.events.lock().unwrap().len()
    }

    /// Clear all events
    pub fn clear(&self) {
        self.events.lock().unwrap().clear();
        self.completed.lock().unwrap().clear();
    }
}

impl Default for EventQueue {
    fn default() -> Self {
        Self::new()
    }
}

/// Tracks a tool execution that is waiting for events to complete
#[derive(Clone, Debug)]
pub struct PendingToolExecution {
    pub tool_call_id: String,
    pub tool_name: String,
    pub initial_result: String,
    pub event_ids: Vec<EventId>,
    pub created_at: Instant,
}

impl PendingToolExecution {
    pub fn new(
        tool_call_id: String,
        tool_name: String,
        initial_result: String,
        event_ids: Vec<EventId>,
    ) -> Self {
        Self {
            tool_call_id,
            tool_name,
            initial_result,
            event_ids,
            created_at: Instant::now(),
        }
    }

    /// Check if all events for this tool execution have completed or been cancelled
    pub fn is_complete(&self, event_queue: &EventQueue) -> bool {
        let queue = event_queue.events.lock().unwrap();

        // Check if any of our events are still in the queue
        let any_pending = queue.iter().any(|scheduled| {
            self.event_ids.contains(&scheduled.id)
        });

        !any_pending
    }

    /// Get results of all events
    pub fn get_event_results(&self, event_queue: &EventQueue) -> Vec<(EventId, Result<(), String>)> {
        let completed = event_queue.completed.lock().unwrap();

        self.event_ids.iter()
            .filter_map(|event_id| {
                completed.iter()
                    .find(|(id, _)| id == event_id)
                    .map(|(id, result)| (*id, result.clone()))
            })
            .collect()
    }
}
