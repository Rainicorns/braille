//! Native DOM bindings connecting JS objects to the Rust DomTree.
//!
//! Architecture: native Rust functions accept simple types (u32 nodeIds, Strings).
//! JS wrapper code on prototypes calls these native functions.
//! A node cache (JS-side Map) ensures identity: same NodeId → same JS object.

use std::cell::RefCell;
use std::rc::Rc;

use rquickjs::Ctx;

use crate::dom::node::NodeData;
use crate::dom::tree::DomTree;
use crate::dom::NodeId;

use super::state::EngineState;

mod element_prototype;
mod form_bindings;
mod label_bindings;
mod native_functions;
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

/// Install the DOM bridge. Must be called once during runtime initialization.
pub fn install(ctx: &Ctx<'_>, tree: Rc<RefCell<DomTree>>, state: Rc<RefCell<EngineState>>) {
    set_tree(tree);
    set_state(state);

    native_functions::register_native_functions(ctx);
    register_js_wrappers(ctx);
}

fn register_js_wrappers(ctx: &Ctx<'_>) {
    // Build the JS initialization as a single IIFE from separate chunks.
    // Each chunk is returned by a dedicated function for maintainability.
    let js = [
        "(function() {\n",
        "var _cache = {};\n",
        "var _listeners = {};\n",
        "var _captureKeys = {};\n",
        "var _bubbleKeys = {};\n",
        "var _winListeners = {};\n",
        "var _winCapture = {};\n",
        "var _docCapture = {};\n",
        "var EP = {};\n",
        element_prototype::element_prototype_js(),
        form_bindings::form_bindings_js(),
        label_bindings::label_bindings_js(),
        wrapper_and_dispatch::wrapper_and_dispatch_js(),
        "\n})();\n",
    ].concat();
    // Debug: dump JS around line 2002
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
            ..
        } => {
            let attrs: Vec<crate::dom::node::DomAttribute> = attributes.clone();
            dst.create_element_ns(tag_name, attrs, namespace)
        }
        NodeData::Text { content } => dst.create_text(content),
        NodeData::Comment { content } => dst.create_comment(content),
        _ => return,
    };
    dst.append_child(dst_parent_id, new_id);

    let children: Vec<NodeId> = src_node.children.clone();
    for &child_id in &children {
        import_node_recursive(dst, src, child_id, new_id);
    }
}
