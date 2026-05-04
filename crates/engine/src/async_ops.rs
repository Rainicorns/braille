use crate::js::state::{
    EngineState, PendingFetch, PendingFormSubmit, PendingSseConnect, PendingWorkerMessage,
    PendingWorkerSpawn, PendingWorkerTerminate, PendingWsClose, PendingWsConnect, PendingWsSend,
};

/// Trait abstracting async operation queuing.
///
/// The default implementation delegates to EngineState fields directly.
/// Alternative backends (Servo, plugin sidecars) can implement this trait
/// to intercept and route async operations through their own systems.
pub trait AsyncOperations {
    // -- Fetch --
    fn queue_fetch(&mut self, fetch: PendingFetch);
    fn take_pending_fetches(&mut self) -> Vec<PendingFetch>;
    fn has_pending_fetches(&self) -> bool;

    // -- Workers --
    fn queue_worker_spawn(&mut self, spawn: PendingWorkerSpawn);
    fn take_pending_worker_spawns(&mut self) -> Vec<PendingWorkerSpawn>;
    fn queue_worker_message(&mut self, msg: PendingWorkerMessage);
    fn take_pending_worker_messages(&mut self) -> Vec<PendingWorkerMessage>;
    fn queue_worker_terminate(&mut self, term: PendingWorkerTerminate);
    fn take_pending_worker_terminates(&mut self) -> Vec<PendingWorkerTerminate>;

    // -- WebSocket --
    fn queue_ws_connect(&mut self, conn: PendingWsConnect);
    fn take_pending_ws_connects(&mut self) -> Vec<PendingWsConnect>;
    fn queue_ws_send(&mut self, send: PendingWsSend);
    fn take_pending_ws_sends(&mut self) -> Vec<PendingWsSend>;
    fn queue_ws_close(&mut self, close: PendingWsClose);
    fn take_pending_ws_closes(&mut self) -> Vec<PendingWsClose>;

    // -- SSE --
    fn queue_sse_connect(&mut self, conn: PendingSseConnect);
    fn take_pending_sse_connects(&mut self) -> Vec<PendingSseConnect>;

    // -- Module/stylesheet fetches --
    fn queue_module_fetch(&mut self, url: String);
    fn take_pending_module_fetches(&mut self) -> Vec<String>;
    fn queue_stylesheet_fetch(&mut self, url: String);
    fn take_pending_stylesheet_fetches(&mut self) -> Vec<String>;

    // -- Navigation --
    fn set_pending_navigation(&mut self, url: String);
    fn take_pending_navigation(&mut self) -> Option<String>;

    // -- Form submission --
    fn set_pending_form_submit(&mut self, submit: PendingFormSubmit);
    fn take_pending_form_submit(&mut self) -> Option<PendingFormSubmit>;
}

/// Default implementation: delegates to EngineState fields.
impl AsyncOperations for EngineState {
    fn queue_fetch(&mut self, fetch: PendingFetch) {
        self.pending_fetches.push(fetch);
    }
    fn take_pending_fetches(&mut self) -> Vec<PendingFetch> {
        std::mem::take(&mut self.pending_fetches)
    }
    fn has_pending_fetches(&self) -> bool {
        !self.pending_fetches.is_empty()
    }

    fn queue_worker_spawn(&mut self, spawn: PendingWorkerSpawn) {
        self.pending_worker_spawns.push(spawn);
    }
    fn take_pending_worker_spawns(&mut self) -> Vec<PendingWorkerSpawn> {
        std::mem::take(&mut self.pending_worker_spawns)
    }
    fn queue_worker_message(&mut self, msg: PendingWorkerMessage) {
        self.pending_worker_messages.push(msg);
    }
    fn take_pending_worker_messages(&mut self) -> Vec<PendingWorkerMessage> {
        std::mem::take(&mut self.pending_worker_messages)
    }
    fn queue_worker_terminate(&mut self, term: PendingWorkerTerminate) {
        self.pending_worker_terminates.push(term);
    }
    fn take_pending_worker_terminates(&mut self) -> Vec<PendingWorkerTerminate> {
        std::mem::take(&mut self.pending_worker_terminates)
    }

    fn queue_ws_connect(&mut self, conn: PendingWsConnect) {
        self.pending_ws_connects.push(conn);
    }
    fn take_pending_ws_connects(&mut self) -> Vec<PendingWsConnect> {
        std::mem::take(&mut self.pending_ws_connects)
    }
    fn queue_ws_send(&mut self, send: PendingWsSend) {
        self.pending_ws_sends.push(send);
    }
    fn take_pending_ws_sends(&mut self) -> Vec<PendingWsSend> {
        std::mem::take(&mut self.pending_ws_sends)
    }
    fn queue_ws_close(&mut self, close: PendingWsClose) {
        self.pending_ws_closes.push(close);
    }
    fn take_pending_ws_closes(&mut self) -> Vec<PendingWsClose> {
        std::mem::take(&mut self.pending_ws_closes)
    }

    fn queue_sse_connect(&mut self, conn: PendingSseConnect) {
        self.pending_sse_connects.push(conn);
    }
    fn take_pending_sse_connects(&mut self) -> Vec<PendingSseConnect> {
        std::mem::take(&mut self.pending_sse_connects)
    }

    fn queue_module_fetch(&mut self, url: String) {
        self.pending_module_fetches.push(url);
    }
    fn take_pending_module_fetches(&mut self) -> Vec<String> {
        std::mem::take(&mut self.pending_module_fetches)
    }
    fn queue_stylesheet_fetch(&mut self, url: String) {
        self.pending_stylesheet_fetches.push(url);
    }
    fn take_pending_stylesheet_fetches(&mut self) -> Vec<String> {
        std::mem::take(&mut self.pending_stylesheet_fetches)
    }

    fn set_pending_navigation(&mut self, url: String) {
        self.pending_navigation = Some(url);
    }
    fn take_pending_navigation(&mut self) -> Option<String> {
        self.pending_navigation.take()
    }

    fn set_pending_form_submit(&mut self, submit: PendingFormSubmit) {
        self.pending_form_submit = Some(submit);
    }
    fn take_pending_form_submit(&mut self) -> Option<PendingFormSubmit> {
        self.pending_form_submit.take()
    }
}
