use super::Engine;

impl Engine {
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
}
