use std::cell::RefCell;
use std::rc::Rc;

use boa_engine::JsData;
use boa_gc::{Finalize, Trace};

use crate::dom::DomTree;

mod creation;
mod domimpl;
mod events;
mod mutation;
mod properties;
mod register;
mod traversal;
mod validation;

// ---------------------------------------------------------------------------
// JsDocument — singleton global `document` object backed by DomTree
// ---------------------------------------------------------------------------

#[derive(Debug, Trace, Finalize, JsData)]
pub(crate) struct JsDocument {
    #[unsafe_ignore_trace]
    pub(crate) tree: Rc<RefCell<DomTree>>,
}

// Re-export public API so that `super::document::X` paths continue to work
// for all other modules in the bindings crate.
pub(crate) use events::document_dispatch_event_public;
pub(crate) use properties::{add_document_properties_to_element, create_blank_xml_document};
pub(crate) use register::{register_document, register_domimplementation};
