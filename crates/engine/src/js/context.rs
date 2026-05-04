//! BrailleContext: the bridge between JS native functions and the DOM/state.
//!
//! Native functions (registered via rquickjs Function::new) need access to the
//! DomTree and EngineState. Currently this is provided via thread-locals.
//! BrailleContext wraps this pattern with an explicit struct, enabling:
//!
//! 1. Multiple engines on the same thread (context stack)
//! 2. Clear documentation of what native functions depend on
//! 3. Future migration to rquickjs userdata (eliminating thread-locals entirely)
//!
//! Migration path:
//!   Phase 1 (now): BrailleContext wraps thread-locals, provides push/pop API
//!   Phase 2 (future): Native functions receive context via rquickjs class data

use std::cell::RefCell;
use std::rc::Rc;

use crate::dom::tree::DomTree;
use super::state::EngineState;

/// The runtime context available to all native bridge functions.
/// Holds references to the DOM tree and engine state for the active page.
pub struct BrailleContext {
    pub tree: Rc<RefCell<DomTree>>,
    pub state: Rc<RefCell<EngineState>>,
}

impl BrailleContext {
    pub fn new(tree: Rc<RefCell<DomTree>>, state: Rc<RefCell<EngineState>>) -> Self {
        Self { tree, state }
    }

    /// Activate this context as the current one (sets thread-locals).
    /// Returns a guard that restores the previous context on drop.
    pub fn activate(&self) -> ContextGuard {
        let prev_tree = super::dom_bridge::swap_tree(Some(Rc::clone(&self.tree)));
        let prev_state = super::dom_bridge::swap_state(Some(Rc::clone(&self.state)));
        ContextGuard { prev_tree, prev_state }
    }
}

/// RAII guard that restores the previous context when dropped.
/// Enables nested engine usage (e.g., iframe engines) on the same thread.
pub struct ContextGuard {
    prev_tree: Option<Rc<RefCell<DomTree>>>,
    prev_state: Option<Rc<RefCell<EngineState>>>,
}

impl Drop for ContextGuard {
    fn drop(&mut self) {
        super::dom_bridge::swap_tree(self.prev_tree.take());
        super::dom_bridge::swap_state(self.prev_state.take());
    }
}
