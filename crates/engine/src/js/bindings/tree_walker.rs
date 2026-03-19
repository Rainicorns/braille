use std::cell::{Cell, RefCell};
use std::rc::Rc;

use boa_engine::{
    js_string,
    native_function::NativeFunction,
    object::ObjectInitializer,
    property::Attribute,
    Context, JsData, JsObject, JsResult, JsValue,
};
use boa_gc::{Finalize, Trace};

use crate::dom::{DomTree, NodeData, NodeId};

use super::element::get_or_create_js_element;

const FILTER_ACCEPT: u16 = 1;
const FILTER_REJECT: u16 = 2;
const FILTER_SKIP: u16 = 3;

/// Native data for TreeWalker instances.
#[derive(Debug, Trace, Finalize, JsData)]
pub(crate) struct JsTreeWalker {
    #[unsafe_ignore_trace]
    tree: Rc<RefCell<DomTree>>,
    #[unsafe_ignore_trace]
    root: NodeId,
    #[unsafe_ignore_trace]
    current_node: Cell<NodeId>,
    what_to_show: u32,
    filter: JsValue,
    #[unsafe_ignore_trace]
    active: Cell<bool>,
}

// ---------------------------------------------------------------------------
// Helper: compute DOM nodeType from NodeData
// ---------------------------------------------------------------------------

fn dom_node_type(data: &NodeData) -> u32 {
    match data {
        NodeData::Element { .. } => 1,
        NodeData::Attr { .. } => 2,
        NodeData::Text { .. } => 3,
        NodeData::CDATASection { .. } => 4,
        NodeData::ProcessingInstruction { .. } => 7,
        NodeData::Comment { .. } => 8,
        NodeData::Document => 9,
        NodeData::Doctype { .. } => 10,
        NodeData::DocumentFragment | NodeData::ShadowRoot { .. } => 11,
    }
}

// ---------------------------------------------------------------------------
// node_filter — run the TreeWalker's filter on a node
// ---------------------------------------------------------------------------

fn node_filter(
    tw_obj: &JsObject,
    node_id: NodeId,
    ctx: &mut Context,
) -> JsResult<u16> {
    let (tree, what_to_show, filter_val, active) = {
        let tw = tw_obj.downcast_ref::<JsTreeWalker>().unwrap();
        (tw.tree.clone(), tw.what_to_show, tw.filter.clone(), tw.active.get())
    };

    // 1. Check active flag
    if active {
        let exc = super::create_dom_exception(ctx, "InvalidStateError", "TreeWalker filter is active", 11)?;
        return Err(boa_engine::JsError::from_opaque(JsValue::from(exc)));
    }

    // 2. Check whatToShow bitmask
    let node_type = {
        let t = tree.borrow();
        dom_node_type(&t.get_node(node_id).data)
    };
    if (1u32 << (node_type - 1)) & what_to_show == 0 {
        return Ok(FILTER_SKIP);
    }

    // 3. If filter is null/undefined, accept
    if filter_val.is_null() || filter_val.is_undefined() {
        return Ok(FILTER_ACCEPT);
    }

    // 4. Set active = true
    {
        let tw = tw_obj.downcast_ref::<JsTreeWalker>().unwrap();
        tw.active.set(true);
    }

    // 5. Create JS node and call filter
    let node_js = get_or_create_js_element(node_id, tree, ctx)?;
    let result = (|| -> JsResult<u16> {
        if let Some(filter_obj) = filter_val.as_object() {
            if filter_obj.is_callable() {
                // filter is a function
                let res = filter_obj.call(&JsValue::undefined(), &[JsValue::from(node_js)], ctx)?;
                return Ok(res.to_u32(ctx)? as u16);
            }
            // filter is an object with acceptNode method
            let accept_node = filter_obj.get(js_string!("acceptNode"), ctx)?;
            let accept_fn = accept_node.as_object().ok_or_else(|| {
                boa_engine::JsError::from_native(
                    boa_engine::JsNativeError::typ()
                        .with_message("TreeWalker filter object must have a callable acceptNode"),
                )
            })?;
            if !accept_fn.is_callable() {
                return Err(boa_engine::JsError::from_native(
                    boa_engine::JsNativeError::typ()
                        .with_message("TreeWalker filter.acceptNode is not callable"),
                ));
            }
            let res = accept_fn.call(&JsValue::from(filter_obj.clone()), &[JsValue::from(node_js)], ctx)?;
            Ok(res.to_u32(ctx)? as u16)
        } else {
            Err(boa_engine::JsError::from_native(
                boa_engine::JsNativeError::typ().with_message("TreeWalker filter is not an object or function"),
            ))
        }
    })();

    // 6. Unset active (even on error)
    {
        let tw = tw_obj.downcast_ref::<JsTreeWalker>().unwrap();
        tw.active.set(false);
    }

    result
}

// ---------------------------------------------------------------------------
// Traversal helpers
// ---------------------------------------------------------------------------

enum ChildType {
    First,
    Last,
}

enum SiblingType {
    Next,
    Previous,
}

/// DOM spec: "traverse children" algorithm
fn traverse_children(tw_obj: &JsObject, child_type: ChildType, ctx: &mut Context) -> JsResult<JsValue> {
    let (tree, root) = {
        let tw = tw_obj.downcast_ref::<JsTreeWalker>().unwrap();
        (tw.tree.clone(), tw.root)
    };

    let current = {
        let tw = tw_obj.downcast_ref::<JsTreeWalker>().unwrap();
        tw.current_node.get()
    };

    // Get first/last child of current
    let mut node = {
        let t = tree.borrow();
        match child_type {
            ChildType::First => t.first_child(current),
            ChildType::Last => t.last_child(current),
        }
    };

    'outer: while let Some(mut candidate) = node {
        let result = node_filter(tw_obj, candidate, ctx)?;
        if result == FILTER_ACCEPT {
            let tw = tw_obj.downcast_ref::<JsTreeWalker>().unwrap();
            tw.current_node.set(candidate);
            drop(tw);
            let js_el = get_or_create_js_element(candidate, tree, ctx)?;
            return Ok(JsValue::from(js_el));
        }
        // If SKIP (not REJECT), try to descend
        if result == FILTER_SKIP {
            let child = {
                let t = tree.borrow();
                match child_type {
                    ChildType::First => t.first_child(candidate),
                    ChildType::Last => t.last_child(candidate),
                }
            };
            if child.is_some() {
                node = child;
                continue;
            }
        }
        // REJECT or SKIP with no children: try sibling, then walk up
        loop {
            let sibling = {
                let t = tree.borrow();
                match child_type {
                    ChildType::First => t.next_sibling(candidate),
                    ChildType::Last => t.prev_sibling(candidate),
                }
            };
            if let Some(s) = sibling {
                node = Some(s);
                continue 'outer;
            }
            // Walk up to parent
            let parent = {
                let t = tree.borrow();
                t.get_node(candidate).parent
            };
            match parent {
                Some(p) if p != root && p != current => {
                    candidate = p;
                    // loop continues: try this parent's sibling
                }
                _ => {
                    node = None;
                    break;
                }
            }
        }
    }

    Ok(JsValue::null())
}

/// DOM spec: "traverse siblings" algorithm
/// https://dom.spec.whatwg.org/#concept-traverse-siblings
fn traverse_siblings(tw_obj: &JsObject, sibling_type: SiblingType, ctx: &mut Context) -> JsResult<JsValue> {
    let (tree, root) = {
        let tw = tw_obj.downcast_ref::<JsTreeWalker>().unwrap();
        (tw.tree.clone(), tw.root)
    };

    let mut node = {
        let tw = tw_obj.downcast_ref::<JsTreeWalker>().unwrap();
        tw.current_node.get()
    };

    if node == root {
        return Ok(JsValue::null());
    }

    'outer: loop {
        // Get first sibling to try
        let mut sibling = {
            let t = tree.borrow();
            match sibling_type {
                SiblingType::Next => t.next_sibling(node),
                SiblingType::Previous => t.prev_sibling(node),
            }
        };

        while let Some(sib) = sibling {
            node = sib;
            let result = node_filter(tw_obj, node, ctx)?;
            if result == FILTER_ACCEPT {
                let tw = tw_obj.downcast_ref::<JsTreeWalker>().unwrap();
                tw.current_node.set(node);
                drop(tw);
                let js_el = get_or_create_js_element(node, tree, ctx)?;
                return Ok(JsValue::from(js_el));
            }
            // SKIP: try to descend into first/last child
            if result == FILTER_SKIP {
                let child = {
                    let t = tree.borrow();
                    match sibling_type {
                        SiblingType::Next => t.first_child(node),
                        SiblingType::Previous => t.last_child(node),
                    }
                };
                if child.is_some() {
                    sibling = child;
                    continue;
                }
            }
            // REJECT or SKIP with no children: try next/prev sibling
            // Walk up until we find a sibling or reach the starting point
            loop {
                let next = {
                    let t = tree.borrow();
                    match sibling_type {
                        SiblingType::Next => t.next_sibling(node),
                        SiblingType::Previous => t.prev_sibling(node),
                    }
                };
                if next.is_some() {
                    sibling = next;
                    break;
                }
                // Walk up to parent
                let parent = {
                    let t = tree.borrow();
                    t.get_node(node).parent
                };
                match parent {
                    Some(p) if p != root => {
                        node = p;
                        let filter_result = node_filter(tw_obj, node, ctx)?;
                        if filter_result == FILTER_ACCEPT {
                            return Ok(JsValue::null());
                        }
                        // continue inner loop to try parent's sibling
                    }
                    _ => return Ok(JsValue::null()),
                }
            }
        }

        // No sibling at all for current node — walk to parent
        let parent = {
            let t = tree.borrow();
            t.get_node(node).parent
        };
        match parent {
            Some(p) if p != root => {
                node = p;
                let filter_result = node_filter(tw_obj, node, ctx)?;
                if filter_result == FILTER_ACCEPT {
                    return Ok(JsValue::null());
                }
                continue 'outer;
            }
            _ => return Ok(JsValue::null()),
        }
    }
}

// ---------------------------------------------------------------------------
// 7 traversal methods
// ---------------------------------------------------------------------------

fn tw_parent_node(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let tw_obj = this.as_object().unwrap().clone();
    let (tree, root) = {
        let tw = tw_obj.downcast_ref::<JsTreeWalker>().unwrap();
        (tw.tree.clone(), tw.root)
    };

    let mut node = {
        let tw = tw_obj.downcast_ref::<JsTreeWalker>().unwrap();
        tw.current_node.get()
    };

    loop {
        if node == root {
            return Ok(JsValue::null());
        }
        let parent = {
            let t = tree.borrow();
            t.get_node(node).parent
        };
        match parent {
            None => return Ok(JsValue::null()),
            Some(p) => {
                node = p;
                let result = node_filter(&tw_obj, node, ctx)?;
                if result == FILTER_ACCEPT {
                    let tw = tw_obj.downcast_ref::<JsTreeWalker>().unwrap();
                    tw.current_node.set(node);
                    drop(tw);
                    let js_el = get_or_create_js_element(node, tree, ctx)?;
                    return Ok(JsValue::from(js_el));
                }
            }
        }
    }
}

fn tw_first_child(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let tw_obj = this.as_object().unwrap().clone();
    traverse_children(&tw_obj, ChildType::First, ctx)
}

fn tw_last_child(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let tw_obj = this.as_object().unwrap().clone();
    traverse_children(&tw_obj, ChildType::Last, ctx)
}

fn tw_next_sibling(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let tw_obj = this.as_object().unwrap().clone();
    traverse_siblings(&tw_obj, SiblingType::Next, ctx)
}

fn tw_previous_sibling(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let tw_obj = this.as_object().unwrap().clone();
    traverse_siblings(&tw_obj, SiblingType::Previous, ctx)
}

fn tw_next_node(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let tw_obj = this.as_object().unwrap().clone();
    let (tree, root) = {
        let tw = tw_obj.downcast_ref::<JsTreeWalker>().unwrap();
        (tw.tree.clone(), tw.root)
    };

    let mut node = {
        let tw = tw_obj.downcast_ref::<JsTreeWalker>().unwrap();
        tw.current_node.get()
    };

    let mut result = FILTER_ACCEPT;

    loop {
        // Try first child (unless last filter was REJECT)
        if result != FILTER_REJECT {
            let first_child = {
                let t = tree.borrow();
                t.first_child(node)
            };
            if let Some(child) = first_child {
                node = child;
                result = node_filter(&tw_obj, node, ctx)?;
                if result == FILTER_ACCEPT {
                    let tw = tw_obj.downcast_ref::<JsTreeWalker>().unwrap();
                    tw.current_node.set(node);
                    drop(tw);
                    let js_el = get_or_create_js_element(node, tree, ctx)?;
                    return Ok(JsValue::from(js_el));
                }
                continue;
            }
        }

        // Try following (next sibling, or ancestor's next sibling)
        let mut candidate = node;
        loop {
            if candidate == root {
                return Ok(JsValue::null());
            }
            let next_sib = {
                let t = tree.borrow();
                t.next_sibling(candidate)
            };
            if let Some(sib) = next_sib {
                node = sib;
                result = node_filter(&tw_obj, node, ctx)?;
                if result == FILTER_ACCEPT {
                    let tw = tw_obj.downcast_ref::<JsTreeWalker>().unwrap();
                    tw.current_node.set(node);
                    drop(tw);
                    let js_el = get_or_create_js_element(node, tree, ctx)?;
                    return Ok(JsValue::from(js_el));
                }
                break; // continue outer loop with this node
            }
            // No next sibling — go to parent
            candidate = {
                let t = tree.borrow();
                match t.get_node(candidate).parent {
                    Some(p) => p,
                    None => return Ok(JsValue::null()),
                }
            };
        }
    }
}

fn tw_previous_node(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let tw_obj = this.as_object().unwrap().clone();
    let (tree, root) = {
        let tw = tw_obj.downcast_ref::<JsTreeWalker>().unwrap();
        (tw.tree.clone(), tw.root)
    };

    let mut node = {
        let tw = tw_obj.downcast_ref::<JsTreeWalker>().unwrap();
        tw.current_node.get()
    };

    loop {
        if node == root {
            return Ok(JsValue::null());
        }

        // Try previous sibling
        let prev_sib = {
            let t = tree.borrow();
            t.prev_sibling(node)
        };

        if let Some(mut sib) = prev_sib {
            // Descend to last most descendant (unless REJECT)
            let mut result = node_filter(&tw_obj, sib, ctx)?;
            loop {
                if result == FILTER_REJECT {
                    break;
                }
                let last_child = {
                    let t = tree.borrow();
                    t.last_child(sib)
                };
                match last_child {
                    Some(lc) => {
                        sib = lc;
                        result = node_filter(&tw_obj, sib, ctx)?;
                    }
                    None => break,
                }
            }
            if result == FILTER_ACCEPT {
                let tw = tw_obj.downcast_ref::<JsTreeWalker>().unwrap();
                tw.current_node.set(sib);
                drop(tw);
                let js_el = get_or_create_js_element(sib, tree, ctx)?;
                return Ok(JsValue::from(js_el));
            }
            node = sib;
            continue;
        }

        // No previous sibling — try parent
        let parent = {
            let t = tree.borrow();
            t.get_node(node).parent
        };
        match parent {
            Some(p) if p != root => {
                node = p;
                // If parent is root, we'd catch it at top of loop
            }
            Some(p) if p == root => {
                // parent is root: filter root, if ACCEPT return it
                let result = node_filter(&tw_obj, p, ctx)?;
                if result == FILTER_ACCEPT {
                    let tw = tw_obj.downcast_ref::<JsTreeWalker>().unwrap();
                    tw.current_node.set(p);
                    drop(tw);
                    let js_el = get_or_create_js_element(p, tree, ctx)?;
                    return Ok(JsValue::from(js_el));
                }
                return Ok(JsValue::null());
            }
            _ => return Ok(JsValue::null()),
        }

        let result = node_filter(&tw_obj, node, ctx)?;
        if result == FILTER_ACCEPT {
            let tw = tw_obj.downcast_ref::<JsTreeWalker>().unwrap();
            tw.current_node.set(node);
            drop(tw);
            let js_el = get_or_create_js_element(node, tree, ctx)?;
            return Ok(JsValue::from(js_el));
        }
    }
}

// ---------------------------------------------------------------------------
// Property getters/setters
// ---------------------------------------------------------------------------

fn tw_get_root(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this.as_object().unwrap();
    let tw = obj.downcast_ref::<JsTreeWalker>().unwrap();
    let root_id = tw.root;
    let tree = tw.tree.clone();
    drop(tw);
    let js_el = get_or_create_js_element(root_id, tree, ctx)?;
    Ok(JsValue::from(js_el))
}

fn tw_get_current_node(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this.as_object().unwrap();
    let tw = obj.downcast_ref::<JsTreeWalker>().unwrap();
    let current = tw.current_node.get();
    let tree = tw.tree.clone();
    drop(tw);
    let js_el = get_or_create_js_element(current, tree, ctx)?;
    Ok(JsValue::from(js_el))
}

fn tw_set_current_node(this: &JsValue, args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    let node_val = args.first().ok_or_else(|| {
        boa_engine::JsError::from_native(
            boa_engine::JsNativeError::typ().with_message("TreeWalker.currentNode setter: value required"),
        )
    })?;
    let node_obj = node_val.as_object().ok_or_else(|| {
        boa_engine::JsError::from_native(
            boa_engine::JsNativeError::typ().with_message("TreeWalker.currentNode setter: not a Node"),
        )
    })?;
    let node_id = {
        let el = node_obj.downcast_ref::<super::element::JsElement>().ok_or_else(|| {
            boa_engine::JsError::from_native(
                boa_engine::JsNativeError::typ().with_message("TreeWalker.currentNode setter: not a Node"),
            )
        })?;
        el.node_id
    };

    let obj = this.as_object().unwrap();
    let tw = obj.downcast_ref::<JsTreeWalker>().unwrap();
    tw.current_node.set(node_id);
    Ok(JsValue::undefined())
}

fn tw_get_what_to_show(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this.as_object().unwrap();
    let tw = obj.downcast_ref::<JsTreeWalker>().unwrap();
    Ok(JsValue::from(tw.what_to_show))
}

fn tw_get_filter(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this.as_object().unwrap();
    let tw = obj.downcast_ref::<JsTreeWalker>().unwrap();
    Ok(tw.filter.clone())
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/// Register `document.createTreeWalker(root, whatToShow?, filter?)`.
pub(crate) fn register_create_tree_walker(
    doc_obj: &boa_engine::JsObject,
    tree: Rc<RefCell<DomTree>>,
    ctx: &mut Context,
) {
    let tree_for_closure = tree;
    let create_tw = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            // root argument is required
            let root_val = args.first().ok_or_else(|| {
                boa_engine::JsError::from_native(
                    boa_engine::JsNativeError::typ()
                        .with_message("Failed to execute 'createTreeWalker': 1 argument required"),
                )
            })?;
            let root_obj = root_val.as_object().ok_or_else(|| {
                boa_engine::JsError::from_native(
                    boa_engine::JsNativeError::typ()
                        .with_message("Failed to execute 'createTreeWalker': parameter 1 is not a Node"),
                )
            })?;
            let root_id = {
                let el = root_obj
                    .downcast_ref::<super::element::JsElement>()
                    .ok_or_else(|| {
                        boa_engine::JsError::from_native(
                            boa_engine::JsNativeError::typ()
                                .with_message("createTreeWalker: root is not a Node"),
                        )
                    })?;
                el.node_id
            };

            // whatToShow (default: 0xFFFFFFFF = SHOW_ALL)
            let what_to_show = args
                .get(1)
                .filter(|v| !v.is_undefined())
                .map(|v| v.to_u32(ctx))
                .transpose()?
                .unwrap_or(0xFFFFFFFF);

            // filter (default: null) — normalize undefined to null per spec
            let filter = match args.get(2) {
                Some(v) if !v.is_undefined() => v.clone(),
                _ => JsValue::null(),
            };

            let tw = JsTreeWalker {
                tree: tree_for_closure.clone(),
                root: root_id,
                current_node: Cell::new(root_id),
                what_to_show,
                filter,
                active: Cell::new(false),
            };

            let realm = ctx.realm().clone();
            let tw_obj = ObjectInitializer::with_native_data(tw, ctx)
                .accessor(
                    js_string!("root"),
                    Some(NativeFunction::from_fn_ptr(tw_get_root).to_js_function(&realm)),
                    None,
                    Attribute::CONFIGURABLE | Attribute::ENUMERABLE,
                )
                .accessor(
                    js_string!("currentNode"),
                    Some(NativeFunction::from_fn_ptr(tw_get_current_node).to_js_function(&realm)),
                    Some(NativeFunction::from_fn_ptr(tw_set_current_node).to_js_function(&realm)),
                    Attribute::CONFIGURABLE | Attribute::ENUMERABLE,
                )
                .accessor(
                    js_string!("whatToShow"),
                    Some(NativeFunction::from_fn_ptr(tw_get_what_to_show).to_js_function(&realm)),
                    None,
                    Attribute::CONFIGURABLE | Attribute::ENUMERABLE,
                )
                .accessor(
                    js_string!("filter"),
                    Some(NativeFunction::from_fn_ptr(tw_get_filter).to_js_function(&realm)),
                    None,
                    Attribute::CONFIGURABLE | Attribute::ENUMERABLE,
                )
                .function(NativeFunction::from_fn_ptr(tw_parent_node), js_string!("parentNode"), 0)
                .function(NativeFunction::from_fn_ptr(tw_first_child), js_string!("firstChild"), 0)
                .function(NativeFunction::from_fn_ptr(tw_last_child), js_string!("lastChild"), 0)
                .function(NativeFunction::from_fn_ptr(tw_previous_sibling), js_string!("previousSibling"), 0)
                .function(NativeFunction::from_fn_ptr(tw_next_sibling), js_string!("nextSibling"), 0)
                .function(NativeFunction::from_fn_ptr(tw_previous_node), js_string!("previousNode"), 0)
                .function(NativeFunction::from_fn_ptr(tw_next_node), js_string!("nextNode"), 0)
                .build();

            // Set Symbol.toStringTag = "TreeWalker"
            tw_obj
                .set(
                    boa_engine::JsSymbol::to_string_tag(),
                    js_string!("TreeWalker"),
                    false,
                    ctx,
                )
                .expect("set Symbol.toStringTag on TreeWalker");

            Ok(JsValue::from(tw_obj))
        })
    };

    doc_obj
        .set(
            js_string!("createTreeWalker"),
            create_tw.to_js_function(ctx.realm()),
            false,
            ctx,
        )
        .expect("set createTreeWalker on document");
}
