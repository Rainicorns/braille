//! Core types: RangeInner and JsRange.

use std::cell::Cell;
use std::cell::RefCell;
use std::rc::Rc;

use crate::dom::{DomTree, NodeId};

// ---------------------------------------------------------------------------
// RangeInner — shared boundary state for live range tracking
// ---------------------------------------------------------------------------

/// Shared interior state for a Range, referenced by both the JsRange (on the
/// JS object) and the live-range registry in RealmState. Using `Rc` + `Cell`
/// lets mutation hooks update boundaries without holding a borrow on the JS
/// object.
#[derive(Debug)]
pub(crate) struct RangeInner {
    pub(crate) start_node: Cell<NodeId>,
    pub(crate) start_offset: Cell<usize>,
    pub(crate) end_node: Cell<NodeId>,
    pub(crate) end_offset: Cell<usize>,
}

// ---------------------------------------------------------------------------
// JsRange — native data stored on the Range JsObject
// ---------------------------------------------------------------------------

#[derive(Debug, boa_engine::JsData, boa_gc::Trace, boa_gc::Finalize)]
pub(crate) struct JsRange {
    #[unsafe_ignore_trace]
    pub(super) tree: Rc<RefCell<DomTree>>,
    #[unsafe_ignore_trace]
    pub(super) inner: Rc<RangeInner>,
}
