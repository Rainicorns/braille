//! The DOM bridge: Rust-backed DOM tree with JS wrappers.
//! Everything here runs inside a single IIFE with shared closure scope (_cache, EP, etc.).
//! Loading order matters — see the concatenation order in register_js_wrappers().
//!
//! Architecture: native Rust functions accept simple types (u32 nodeIds, Strings).
//! JS wrapper code on prototypes calls these native functions.
//! A node cache (JS-side Map) ensures identity: same NodeId → same JS object.
//!
//! Module responsibilities:
//!   element_prototype   — EP (the shared Node prototype) + element attribute methods
//!   element_events      — addEventListener, removeEventListener, dispatchEvent
//!   element_scroll      — getBoundingClientRect, scroll methods, geometry
//!   element_properties  — tagName, id, className, style, innerHTML, etc.
//!   form_bindings       — form property, submit(), validation
//!   label_bindings      — htmlFor, control, labels
//!   wrapper_and_dispatch — __w() wrapper factory, constructors, prototype wiring, CE lifecycle
//!   event_dispatch      — __dispatch() with capture/bubble, __adoptSubtree
//!   dom_mutation         — appendChild, removeChild, insertBefore
//!   global_document     — __makeDocumentLike, document methods, Range, DOMRect
//!   native_functions    — __n_* Rust functions for tree operations
//!   native_tree_ops     — __n_appendChild, __n_removeChild, etc.
//!   native_attributes   — __n_getAttribute, __n_setAttribute, etc.

use std::cell::RefCell;
use std::rc::Rc;

use rquickjs::Ctx;

use crate::dom::node::NodeData;
use crate::dom::tree::DomTree;
use crate::dom::NodeId;

use super::state::EngineState;

mod dom_mutation;
mod element_events;
mod element_properties;
mod element_prototype;
mod element_scroll;
mod event_dispatch;
mod form_bindings;
mod global_document;
mod label_bindings;
mod native_attributes;
mod native_functions;
mod native_tree_ops;
mod selection;
mod slot_assignment;
mod wrapper_and_dispatch;

#[cfg(test)]
mod tests;

// Thread-local for DomTree access from native functions.
thread_local! {
    static TREE: RefCell<Option<Rc<RefCell<DomTree>>>> = const { RefCell::new(None) };
    static STATE: RefCell<Option<Rc<RefCell<EngineState>>>> = const { RefCell::new(None) };
}

pub(crate) fn with_tree<F, R>(f: F) -> R
where
    F: FnOnce(&DomTree) -> R,
{
    TREE.with(|t| {
        let borrow = t.borrow();
        let tree_rc = borrow.as_ref().expect("DOM bridge tree not set");
        let tree = tree_rc.borrow();
        f(&tree)
    })
}

pub(crate) fn with_tree_mut<F, R>(f: F) -> R
where
    F: FnOnce(&mut DomTree) -> R,
{
    TREE.with(|t| {
        let borrow = t.borrow();
        let tree_rc = borrow.as_ref().expect("DOM bridge tree not set");
        let mut tree = tree_rc.borrow_mut();
        f(&mut tree)
    })
}

#[allow(dead_code)]
pub(crate) fn with_state<F, R>(f: F) -> R
where
    F: FnOnce(&EngineState) -> R,
{
    STATE.with(|s| {
        let borrow = s.borrow();
        let state_rc = borrow.as_ref().expect("DOM bridge state not set");
        let state = state_rc.borrow();
        f(&state)
    })
}

pub(crate) fn with_state_mut<F, R>(f: F) -> R
where
    F: FnOnce(&mut EngineState) -> R,
{
    STATE.with(|s| {
        let borrow = s.borrow();
        let state_rc = borrow.as_ref().expect("DOM bridge state not set");
        let mut state = state_rc.borrow_mut();
        f(&mut state)
    })
}

pub(crate) fn set_tree(tree: Rc<RefCell<DomTree>>) {
    TREE.with(|t| {
        *t.borrow_mut() = Some(tree);
    });
}

pub(crate) fn set_state(state: Rc<RefCell<EngineState>>) {
    STATE.with(|s| {
        *s.borrow_mut() = Some(state);
    });
}

/// Swap the tree thread-local, returning the previous value.
/// Used by BrailleContext for push/pop semantics.
pub(crate) fn swap_tree(new: Option<Rc<RefCell<DomTree>>>) -> Option<Rc<RefCell<DomTree>>> {
    TREE.with(|t| {
        let mut borrow = t.borrow_mut();
        let prev = borrow.take();
        *borrow = new;
        prev
    })
}

/// Swap the state thread-local, returning the previous value.
/// Used by BrailleContext for push/pop semantics.
pub(crate) fn swap_state(new: Option<Rc<RefCell<EngineState>>>) -> Option<Rc<RefCell<EngineState>>> {
    STATE.with(|s| {
        let mut borrow = s.borrow_mut();
        let prev = borrow.take();
        *borrow = new;
        prev
    })
}

/// Install the DOM bridge. Must be called once during runtime initialization.
pub fn install(ctx: &Ctx<'_>, tree: Rc<RefCell<DomTree>>, state: Rc<RefCell<EngineState>>) {
    set_tree(tree);
    set_state(state);

    native_functions::register_native_functions(ctx);
    register_js_wrappers(ctx);
}

/// The ordered list of JS modules that make up the DOM bridge.
/// Each module contributes JS code that runs in a shared scope with access to
/// `_cache`, `_listeners`, `EP`, etc.
///
/// Plugins can customize this by calling `default_bridge_modules()`, removing/replacing
/// entries, or inserting new ones, then passing the result to `install_with_modules`.
#[allow(dead_code)]
pub struct BridgeModule {
    pub name: &'static str,
    pub js: fn() -> &'static str,
}

/// Returns the default set of bridge modules in load order.
pub fn default_bridge_modules() -> Vec<BridgeModule> {
    vec![
        BridgeModule { name: "element_prototype", js: element_prototype::element_prototype_js },
        BridgeModule { name: "element_events", js: element_events::element_events_js },
        BridgeModule { name: "element_scroll", js: element_scroll::element_scroll_js },
        BridgeModule { name: "element_properties", js: element_properties::element_properties_js },
        BridgeModule { name: "form_bindings", js: form_bindings::form_bindings_js },
        BridgeModule { name: "label_bindings", js: label_bindings::label_bindings_js },
        BridgeModule { name: "wrapper_factory", js: wrapper_and_dispatch::wrapper_factory_js },
        BridgeModule { name: "event_dispatch", js: event_dispatch::event_dispatch_js },
        BridgeModule { name: "dom_mutation", js: dom_mutation::dom_mutation_js },
        BridgeModule { name: "global_document", js: global_document::global_document_js },
        BridgeModule { name: "selection", js: selection::selection_js },
        BridgeModule { name: "slot_assignment", js: slot_assignment::slot_assignment_js },
        BridgeModule { name: "constructors_and_wiring", js: wrapper_and_dispatch::constructors_and_wiring_js },
    ]
}

/// Install the DOM bridge with a custom set of modules.
/// Allows plugins to replace or extend individual bridge modules.
#[allow(dead_code)]
pub fn install_with_modules(ctx: &Ctx<'_>, tree: Rc<RefCell<DomTree>>, state: Rc<RefCell<EngineState>>, modules: &[BridgeModule]) {
    set_tree(tree);
    set_state(state);
    native_functions::register_native_functions(ctx);
    register_js_wrappers_from_modules(ctx, modules);
}

fn register_js_wrappers(ctx: &Ctx<'_>) {
    let modules = default_bridge_modules();
    register_js_wrappers_from_modules(ctx, &modules);
}

fn register_js_wrappers_from_modules(ctx: &Ctx<'_>, modules: &[BridgeModule]) {
    let mut parts: Vec<&str> = Vec::with_capacity(modules.len() + 10);
    parts.push("(function() {\n");
    parts.push("var _cache = {};\n");
    parts.push("var _listeners = {};\n");
    parts.push("var _captureKeys = {};\n");
    parts.push("var _bubbleKeys = {};\n");
    parts.push("var _winListeners = {};\n");
    parts.push("var _winCapture = {};\n");
    parts.push("var _docCapture = {};\n");
    parts.push("var EP = {};\n");

    let module_sources: Vec<&str> = modules.iter().map(|m| (m.js)()).collect();
    for src in &module_sources {
        parts.push(src);
    }

    parts.push("\n})();\n");

    let js = parts.concat();
    ctx.eval::<(), _>(&*js).unwrap_or_else(|e| {
        let msg = match e {
            rquickjs::Error::Exception => {
                let exc = ctx.catch();
                if let Some(exc) = exc.as_exception() {
                    format!("{}: {}", exc.message().unwrap_or_default(), exc.stack().unwrap_or_default())
                } else {
                    format!("{exc:?}")
                }
            }
            other => format!("{other:?}"),
        };
        panic!("DOM bridge JS init failed: {msg}");
    });
}

/// Recursively copy a node from a source tree into a destination tree.
pub(crate) fn import_node_recursive(
    dst: &mut DomTree,
    src: &DomTree,
    src_node_id: NodeId,
    dst_parent_id: NodeId,
) {
    let src_node = src.get_node(src_node_id);
    let new_id = match &src_node.data {
        NodeData::Element {
            tag_name,
            attributes,
            namespace,
            prefix,
        } => {
            let attrs: Vec<crate::dom::node::DomAttribute> = attributes.clone();
            dst.create_element_ns_with_prefix(tag_name, attrs, namespace, prefix.as_deref())
        }
        NodeData::Text { content } => dst.create_text(content),
        NodeData::Comment { content } => dst.create_comment(content),
        NodeData::Doctype {
            name,
            public_id,
            system_id,
        } => dst.create_doctype(name, public_id, system_id),
        _ => return,
    };
    dst.append_child(dst_parent_id, new_id);

    // Import shadow root if the source node has one.
    let shadow_root_id = src_node.shadow_root;
    if let Some(src_shadow_id) = shadow_root_id {
        let mode = match &src.get_node(src_shadow_id).data {
            NodeData::ShadowRoot { mode, .. } => *mode,
            _ => unreachable!(),
        };
        let dst_shadow_id = dst.create_shadow_root(mode, new_id);
        let shadow_children: Vec<NodeId> = src.get_node(src_shadow_id).children.clone();
        for &child_id in &shadow_children {
            import_node_recursive(dst, src, child_id, dst_shadow_id);
        }
    }

    let children: Vec<NodeId> = src_node.children.clone();
    for &child_id in &children {
        import_node_recursive(dst, src, child_id, new_id);
    }
}
