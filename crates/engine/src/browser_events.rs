use braille_wire::{BrowserEvent, BrowserEventKind};

/// Manages the queue of browser events that need agent attention.
pub struct BrowserEventQueue {
    events: Vec<BrowserEvent>,
    next_id: u64,
}

impl Default for BrowserEventQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl BrowserEventQueue {
    pub fn new() -> Self {
        Self {
            events: Vec::new(),
            next_id: 1,
        }
    }

    /// Queue a new browser event and return its ID.
    pub fn push(&mut self, kind: BrowserEventKind) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.events.push(BrowserEvent {
            id,
            kind,
            timestamp_ms: 0,
        });
        id
    }

    /// Take all pending events, clearing the queue.
    pub fn drain(&mut self) -> Vec<BrowserEvent> {
        std::mem::take(&mut self.events)
    }

    /// Number of pending events.
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Whether the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Remove a specific event by ID (for dismiss).
    pub fn remove(&mut self, id: u64) -> bool {
        if let Some(pos) = self.events.iter().position(|e| e.id == id) {
            self.events.remove(pos);
            true
        } else {
            false
        }
    }

    /// Peek at all pending events without draining.
    pub fn peek(&self) -> &[BrowserEvent] {
        &self.events
    }
}
