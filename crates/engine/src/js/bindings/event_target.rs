use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use boa_engine::JsObject;

use crate::dom::NodeId;

// ---------------------------------------------------------------------------
// ListenerEntry — one registered event listener
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub(crate) struct ListenerEntry {
    pub(crate) event_type: String,
    pub(crate) callback: JsObject,
    pub(crate) capture: bool,
    pub(crate) once: bool,
}

// ---------------------------------------------------------------------------
// ListenerMap — NodeId -> Vec<ListenerEntry>
// ---------------------------------------------------------------------------

pub(crate) type ListenerMap = HashMap<NodeId, Vec<ListenerEntry>>;

// ---------------------------------------------------------------------------
// Thread-local storage for the listener map.
// This allows NativeFunction callbacks (addEventListener, removeEventListener)
// to access the listener map without needing a reference to JsRuntime.
// ---------------------------------------------------------------------------

thread_local! {
    pub(crate) static EVENT_LISTENERS: RefCell<Option<Rc<RefCell<ListenerMap>>>> = const { RefCell::new(None) };
}
