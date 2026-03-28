use super::Engine;

impl Engine {
    /// Take the pending navigation URL if JS set location.href.
    pub fn take_pending_navigation(&mut self) -> Option<String> {
        self.runtime.as_ref().and_then(|rt| rt.take_pending_navigation())
    }

    /// Returns true if there are pending fetch requests that need to be serviced.
    pub fn has_pending_fetches(&self) -> bool {
        if let Some(runtime) = &self.runtime {
            runtime.has_pending_fetches()
        } else {
            false
        }
    }

    /// Returns true if there are pending timers.
    pub fn has_pending_timers(&self) -> bool {
        if let Some(runtime) = &self.runtime {
            runtime.has_pending_timers()
        } else {
            false
        }
    }

    /// Returns all pending fetch requests as serializable DTOs.
    pub fn pending_fetches(&self) -> Vec<braille_wire::FetchRequest> {
        if let Some(runtime) = &self.runtime {
            runtime.pending_fetches()
        } else {
            Vec::new()
        }
    }

    /// Resolve a pending fetch with a response.
    pub fn resolve_fetch(&mut self, id: u64, response: &braille_wire::FetchResponseData) {
        let runtime = self.runtime.as_mut().expect("resolve_fetch: no runtime loaded");
        runtime.resolve_fetch(id, response);
    }

    /// Reject a pending fetch with an error message.
    pub fn reject_fetch(&mut self, id: u64, error: &str) {
        let runtime = self.runtime.as_mut().expect("reject_fetch: no runtime loaded");
        runtime.reject_fetch(id, error);
    }

    /// Returns true if there are pending worker operations.
    pub fn has_pending_workers(&self) -> bool {
        if let Some(runtime) = &self.runtime {
            let state = runtime.state.borrow();
            !state.pending_worker_spawns.is_empty()
                || !state.pending_worker_messages.is_empty()
                || !state.pending_worker_terminates.is_empty()
        } else {
            false
        }
    }

    /// Drain pending worker spawn requests. Returns vec of (url,).
    pub fn drain_pending_worker_spawns(&mut self) -> Vec<(String,)> {
        if let Some(runtime) = &self.runtime {
            let mut state = runtime.state.borrow_mut();
            state
                .pending_worker_spawns
                .drain(..)
                .map(|s| (s.url,))
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Drain pending worker messages. Returns vec of (worker_id, data).
    pub fn drain_pending_worker_messages(&mut self) -> Vec<(u64, String)> {
        if let Some(runtime) = &self.runtime {
            let mut state = runtime.state.borrow_mut();
            state
                .pending_worker_messages
                .drain(..)
                .map(|m| (m.worker_id, m.data))
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Drain pending worker terminates. Returns vec of worker_ids.
    pub fn drain_pending_worker_terminates(&mut self) -> Vec<u64> {
        if let Some(runtime) = &self.runtime {
            let mut state = runtime.state.borrow_mut();
            state
                .pending_worker_terminates
                .drain(..)
                .map(|t| t.worker_id)
                .collect()
        } else {
            Vec::new()
        }
    }
}
