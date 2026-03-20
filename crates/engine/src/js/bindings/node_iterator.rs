use std::cell::Cell;
use std::cell::RefCell;
use std::rc::Rc;

use boa_engine::{
    js_string,
    native_function::NativeFunction,
    object::ObjectInitializer,
    property::PropertyDescriptor,
    Context, JsData, JsError, JsNativeError, JsResult, JsValue,
};
use boa_gc::{Finalize, Trace};

use crate::dom::{DomTree, NodeId};

use super::element::get_or_create_js_element;
use super::tree_walker::{dom_node_type, FILTER_ACCEPT, FILTER_SKIP};

// ---------------------------------------------------------------------------
// JsNodeIterator — native data for NodeIterator instances
// ---------------------------------------------------------------------------

#[derive(Debug, Trace, Finalize, JsData)]
pub(crate) struct JsNodeIterator {
    #[unsafe_ignore_trace]
    tree: Rc<RefCell<DomTree>>,
    #[unsafe_ignore_trace]
    root: NodeId,
    #[unsafe_ignore_trace]
    reference_node: Cell<NodeId>,
    #[unsafe_ignore_trace]
    pointer_before_reference: Cell<bool>,
    what_to_show: u32,
    filter: JsValue,
    #[unsafe_ignore_trace]
    active: Cell<bool>,
}

// ---------------------------------------------------------------------------
// node_filter — run the iterator's filter on a node
// ---------------------------------------------------------------------------

fn node_filter(iter_obj: &boa_engine::JsObject, node_id: NodeId, ctx: &mut Context) -> JsResult<u16> {
    let (tree, what_to_show, filter_val, active) = {
        let ni = iter_obj.downcast_ref::<JsNodeIterator>().unwrap();
        (ni.tree.clone(), ni.what_to_show, ni.filter.clone(), ni.active.get())
    };

    // Check active flag (recursive filter detection)
    if active {
        let exc = super::create_dom_exception(ctx, "InvalidStateError", "NodeIterator filter is active", 11)?;
        return Err(JsError::from_opaque(JsValue::from(exc)));
    }

    // Check whatToShow bitmask
    let node_type = {
        let t = tree.borrow();
        dom_node_type(&t.get_node(node_id).data)
    };
    if (1u32 << (node_type - 1)) & what_to_show == 0 {
        return Ok(FILTER_SKIP);
    }

    // If filter is null/undefined, accept
    if filter_val.is_null() || filter_val.is_undefined() {
        return Ok(FILTER_ACCEPT);
    }

    // Set active = true
    {
        let ni = iter_obj.downcast_ref::<JsNodeIterator>().unwrap();
        ni.active.set(true);
    }

    // Create JS node and call filter
    let node_js = get_or_create_js_element(node_id, tree, ctx)?;
    let result = (|| -> JsResult<u16> {
        if let Some(filter_obj) = filter_val.as_object() {
            if filter_obj.is_callable() {
                let res = filter_obj.call(&JsValue::undefined(), &[JsValue::from(node_js)], ctx)?;
                return Ok(res.to_u32(ctx)? as u16);
            }
            let accept_node = filter_obj.get(js_string!("acceptNode"), ctx)?;
            let accept_fn = accept_node.as_object().ok_or_else(|| {
                JsError::from_native(
                    JsNativeError::typ().with_message("NodeIterator filter.acceptNode is not callable"),
                )
            })?;
            if !accept_fn.is_callable() {
                return Err(JsError::from_native(
                    JsNativeError::typ().with_message("NodeIterator filter.acceptNode is not callable"),
                ));
            }
            let res = accept_fn.call(&JsValue::from(filter_obj.clone()), &[JsValue::from(node_js)], ctx)?;
            Ok(res.to_u32(ctx)? as u16)
        } else {
            Err(JsError::from_native(
                JsNativeError::typ().with_message("NodeIterator filter is not an object or function"),
            ))
        }
    })();

    // Unset active (even on error)
    {
        let ni = iter_obj.downcast_ref::<JsNodeIterator>().unwrap();
        ni.active.set(false);
    }

    result
}

// ---------------------------------------------------------------------------
// Document order traversal helpers
// ---------------------------------------------------------------------------

/// Next node in document order within the subtree rooted at `root`.
fn next_in_document_order(node_id: NodeId, root: NodeId, tree: &DomTree) -> Option<NodeId> {
    let node = tree.get_node(node_id);

    // First child
    if let Some(&first) = node.children.first() {
        return Some(first);
    }

    // Next sibling, or walk up
    let mut current = node_id;
    loop {
        if current == root {
            return None;
        }
        let n = tree.get_node(current);
        if let Some(parent_id) = n.parent {
            let parent = tree.get_node(parent_id);
            let idx = parent.children.iter().position(|&c| c == current).unwrap();
            if idx + 1 < parent.children.len() {
                return Some(parent.children[idx + 1]);
            }
            current = parent_id;
        } else {
            return None;
        }
    }
}

/// Previous node in document order within the subtree rooted at `root`.
fn previous_in_document_order(node_id: NodeId, root: NodeId, tree: &DomTree) -> Option<NodeId> {
    if node_id == root {
        return None;
    }

    let node = tree.get_node(node_id);

    if let Some(parent_id) = node.parent {
        let parent = tree.get_node(parent_id);
        let idx = parent.children.iter().position(|&c| c == node_id).unwrap();
        if idx > 0 {
            // Previous sibling's deepest last descendant
            let mut deepest = parent.children[idx - 1];
            loop {
                let n = tree.get_node(deepest);
                if let Some(&last) = n.children.last() {
                    deepest = last;
                } else {
                    return Some(deepest);
                }
            }
        }
        // Parent
        Some(parent_id)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// nextNode / previousNode
// ---------------------------------------------------------------------------

fn next_node(iter_obj: &boa_engine::JsObject, ctx: &mut Context) -> JsResult<JsValue> {
    let (tree, root, reference_node, pointer_before) = {
        let ni = iter_obj.downcast_ref::<JsNodeIterator>().unwrap();
        (ni.tree.clone(), ni.root, ni.reference_node.get(), ni.pointer_before_reference.get())
    };

    let mut node = reference_node;
    let mut before_node = pointer_before;

    loop {
        if !before_node {
            // Advance to next node in document order
            let next = {
                let t = tree.borrow();
                next_in_document_order(node, root, &t)
            };
            match next {
                Some(n) => node = n,
                None => return Ok(JsValue::null()),
            }
        } else {
            before_node = false;
        }

        let result = node_filter(iter_obj, node, ctx)?;
        if result == FILTER_ACCEPT {
            // Update reference
            let ni = iter_obj.downcast_ref::<JsNodeIterator>().unwrap();
            ni.reference_node.set(node);
            ni.pointer_before_reference.set(false);
            return get_or_create_js_element(node, tree, ctx).map(JsValue::from);
        }
        // For NodeIterator, SKIP and REJECT are the same — just continue
    }
}

fn previous_node(iter_obj: &boa_engine::JsObject, ctx: &mut Context) -> JsResult<JsValue> {
    let (tree, root, reference_node, pointer_before) = {
        let ni = iter_obj.downcast_ref::<JsNodeIterator>().unwrap();
        (ni.tree.clone(), ni.root, ni.reference_node.get(), ni.pointer_before_reference.get())
    };

    let mut node = reference_node;
    let mut before_node = pointer_before;

    loop {
        if before_node {
            // Go to previous node in document order
            let prev = {
                let t = tree.borrow();
                previous_in_document_order(node, root, &t)
            };
            match prev {
                Some(n) => node = n,
                None => return Ok(JsValue::null()),
            }
        } else {
            before_node = true;
        }

        let result = node_filter(iter_obj, node, ctx)?;
        if result == FILTER_ACCEPT {
            let ni = iter_obj.downcast_ref::<JsNodeIterator>().unwrap();
            ni.reference_node.set(node);
            ni.pointer_before_reference.set(true);
            return get_or_create_js_element(node, tree, ctx).map(JsValue::from);
        }
    }
}

// ---------------------------------------------------------------------------
// register_create_node_iterator — add document.createNodeIterator
// ---------------------------------------------------------------------------

pub(crate) fn register_create_node_iterator(
    doc_obj: &boa_engine::JsObject,
    tree: Rc<RefCell<DomTree>>,
    ctx: &mut Context,
) {
    let realm = ctx.realm().clone();
    let tree_for_closure = tree;

    let create_fn = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            // First arg: root node
            let root_val = args.first().ok_or_else(|| {
                JsError::from_native(JsNativeError::typ().with_message("createNodeIterator: root argument required"))
            })?;

            let root_obj = root_val.as_object().ok_or_else(|| {
                JsError::from_native(JsNativeError::typ().with_message("createNodeIterator: root must be a Node"))
            })?;

            let root_id = if let Some(el) = root_obj.downcast_ref::<super::element::JsElement>() {
                el.node_id
            } else if let Some(doc) = root_obj.downcast_ref::<super::document::JsDocument>() {
                doc.tree.borrow().document()
            } else {
                return Err(JsError::from_native(
                    JsNativeError::typ().with_message("createNodeIterator: root must be a Node"),
                ));
            };

            // whatToShow (default 0xFFFFFFFF)
            let what_to_show = if let Some(ws) = args.get(1) {
                if ws.is_undefined() {
                    0xFFFFFFFF_u32
                } else if ws.is_null() {
                    0_u32
                } else {
                    ws.to_u32(ctx)?
                }
            } else {
                0xFFFFFFFF_u32
            };

            // filter (default null)
            let filter = args
                .get(2)
                .cloned()
                .unwrap_or(JsValue::null());
            let filter = if filter.is_undefined() { JsValue::null() } else { filter };

            let ni_data = JsNodeIterator {
                tree: tree_for_closure.clone(),
                root: root_id,
                reference_node: Cell::new(root_id),
                pointer_before_reference: Cell::new(true),
                what_to_show,
                filter: filter.clone(),
                active: Cell::new(false),
            };

            let ni_obj = ObjectInitializer::with_native_data(ni_data, ctx).build();

            let tree_for_methods = tree_for_closure.clone();

            // root getter
            let root_tree = tree_for_methods.clone();
            let root_getter =
                NativeFunction::from_closure(move |this, _args, ctx| {
                    let obj = this.as_object().unwrap();
                    let root = obj.downcast_ref::<JsNodeIterator>().unwrap().root;
                    get_or_create_js_element(root, root_tree.clone(), ctx).map(JsValue::from)
                });

            // referenceNode getter
            let ref_tree = tree_for_methods.clone();
            let ref_getter =
                NativeFunction::from_closure(move |this, _args, ctx| {
                    let obj = this.as_object().unwrap();
                    let ref_node = obj.downcast_ref::<JsNodeIterator>().unwrap().reference_node.get();
                    get_or_create_js_element(ref_node, ref_tree.clone(), ctx).map(JsValue::from)
                });

            // pointerBeforeReferenceNode getter
            let pbr_getter = NativeFunction::from_fn_ptr(|this, _args, _ctx| {
                let obj = this.as_object().unwrap();
                let pbr = obj.downcast_ref::<JsNodeIterator>().unwrap().pointer_before_reference.get();
                Ok(JsValue::from(pbr))
            });

            // whatToShow getter
            let wts_getter = NativeFunction::from_fn_ptr(|this, _args, _ctx| {
                let obj = this.as_object().unwrap();
                let wts = obj.downcast_ref::<JsNodeIterator>().unwrap().what_to_show;
                Ok(JsValue::from(wts))
            });

            // filter getter
            let filter_getter = NativeFunction::from_fn_ptr(|this, _args, _ctx| {
                let obj = this.as_object().unwrap();
                let f = obj.downcast_ref::<JsNodeIterator>().unwrap().filter.clone();
                Ok(f)
            });

            let r = ctx.realm().clone();

            // Define readonly properties
            for (name, getter) in [
                ("root", root_getter.to_js_function(&r)),
                ("referenceNode", ref_getter.to_js_function(&r)),
                ("pointerBeforeReferenceNode", pbr_getter.to_js_function(&r)),
                ("whatToShow", wts_getter.to_js_function(&r)),
                ("filter", filter_getter.to_js_function(&r)),
            ] {
                ni_obj
                    .define_property_or_throw(
                        js_string!(name),
                        PropertyDescriptor::builder()
                            .get(getter)
                            .configurable(true)
                            .enumerable(true)
                            .build(),
                        ctx,
                    )
                    .unwrap();
            }

            // nextNode method
            let ni_for_next = ni_obj.clone();
            let next_fn =
                NativeFunction::from_closure(move |_this, _args, ctx| next_node(&ni_for_next, ctx));
            ni_obj
                .set(js_string!("nextNode"), next_fn.to_js_function(&r), false, ctx)
                .unwrap();

            // previousNode method
            let ni_for_prev = ni_obj.clone();
            let prev_fn =
                NativeFunction::from_closure(move |_this, _args, ctx| previous_node(&ni_for_prev, ctx));
            ni_obj
                .set(js_string!("previousNode"), prev_fn.to_js_function(&r), false, ctx)
                .unwrap();

            // detach (no-op per spec)
            let detach_fn = NativeFunction::from_fn_ptr(|_this, _args, _ctx| Ok(JsValue::undefined()));
            ni_obj
                .set(js_string!("detach"), detach_fn.to_js_function(&r), false, ctx)
                .unwrap();

            // Symbol.toStringTag = "NodeIterator"
            ni_obj
                .define_property_or_throw(
                    boa_engine::JsSymbol::to_string_tag(),
                    PropertyDescriptor::builder()
                        .value(js_string!("NodeIterator"))
                        .configurable(true)
                        .build(),
                    ctx,
                )
                .unwrap();

            Ok(JsValue::from(ni_obj))
        })
    };

    doc_obj
        .set(
            js_string!("createNodeIterator"),
            create_fn.to_js_function(&realm),
            false,
            ctx,
        )
        .unwrap();
}
