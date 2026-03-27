use std::cell::RefCell;
use std::rc::Rc;

use boa_engine::{
    js_string,
    native_function::NativeFunction,
    object::{FunctionObjectBuilder, ObjectInitializer},
    property::PropertyDescriptor,
    Context, JsError, JsObject, JsResult, JsValue,
};

use crate::dom::{DomTree, NodeId};
use crate::js::realm_state;

use super::super::element::get_or_create_js_element;

// ---------------------------------------------------------------------------
// Live NodeList creation (for childNodes)
// ---------------------------------------------------------------------------

/// Create a live NodeList backed by the given node's childNodes.
/// The returned object is a JS Proxy that intercepts numeric property access
/// and delegates to the DOM tree for live results.
///
/// Uses a pre-built factory function (from register_collections) to avoid
/// calling context.eval() which triggers a Boa environment bug when called
/// from within native function scopes.
pub(crate) fn create_live_nodelist(
    parent_id: NodeId,
    tree: Rc<RefCell<DomTree>>,
    context: &mut Context,
) -> JsResult<JsObject> {
    // Create a backing object that stores the native functions
    let backing = ObjectInitializer::new(context).build();

    // Set prototype to NodeList.prototype
    if let Some(p) = realm_state::nodelist_proto(context) {
        backing.set_prototype(Some(p));
    }

    // Define `item(index)` method on the backing object
    let tree_for_item = tree.clone();
    let item_fn = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let index = args.first().map(|v| v.to_number(ctx)).transpose()?.unwrap_or(0.0) as i64;

            if index < 0 {
                return Ok(JsValue::null());
            }

            let tree_ref = tree_for_item.borrow();
            let children = tree_ref.children(parent_id);
            match children.get(index as usize) {
                Some(&child_id) => {
                    let tree_clone = tree_for_item.clone();
                    drop(tree_ref);
                    let js_obj = get_or_create_js_element(child_id, tree_clone, ctx)?;
                    Ok(js_obj.into())
                }
                None => Ok(JsValue::null()),
            }
        })
    };
    let realm = context.realm().clone();
    backing.define_property_or_throw(
        js_string!("item"),
        PropertyDescriptor::builder()
            .value(item_fn.to_js_function(&realm))
            .writable(true)
            .configurable(true)
            .enumerable(false)
            .build(),
        context,
    )?;

    // Length getter (native function)
    let tree_for_len = tree.clone();
    let length_fn = unsafe {
        NativeFunction::from_closure(move |_this, _args, _ctx| {
            let tree_ref = tree_for_len.borrow();
            let children = tree_ref.children(parent_id);
            Ok(JsValue::from(children.len() as i32))
        })
    };

    // getChild(index) -> returns the JS element at index, or undefined
    let tree_for_get = tree.clone();
    let get_child_fn = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let index = args.first().map(|v| v.to_number(ctx)).transpose()?.unwrap_or(0.0) as i64;

            if index < 0 {
                return Ok(JsValue::undefined());
            }

            let tree_ref = tree_for_get.borrow();
            let children = tree_ref.children(parent_id);
            match children.get(index as usize) {
                Some(&child_id) => {
                    let tree_clone = tree_for_get.clone();
                    drop(tree_ref);
                    let js_obj = get_or_create_js_element(child_id, tree_clone, ctx)?;
                    Ok(js_obj.into())
                }
                None => Ok(JsValue::undefined()),
            }
        })
    };

    // Call the pre-built factory function: factory(backing, getLength, getChild)
    let factory = realm_state::nl_proxy_factory(context).expect("NodeList proxy factory not initialized");

    let length_js = length_fn.to_js_function(&realm);
    let get_child_js = get_child_fn.to_js_function(&realm);

    let result = factory.call(
        &JsValue::undefined(),
        &[backing.into(), length_js.into(), get_child_js.into()],
        context,
    )?;

    let proxy_obj = result
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("failed to create NodeList proxy").into()))?
        .clone();

    Ok(proxy_obj)
}

// ---------------------------------------------------------------------------
// Static NodeList creation (for querySelectorAll)
// ---------------------------------------------------------------------------

/// Create a static NodeList from a list of NodeIds.
/// The returned object is a plain object with pre-populated indices (no Proxy needed).
pub(crate) fn create_static_nodelist(
    node_ids: Vec<NodeId>,
    tree: Rc<RefCell<DomTree>>,
    context: &mut Context,
) -> JsResult<JsObject> {
    // Create a backing object
    let backing = ObjectInitializer::new(context).build();

    // Set prototype to NodeList.prototype
    if let Some(p) = realm_state::nodelist_proto(context) {
        backing.set_prototype(Some(p));
    }

    // Define length as a data property (static, non-enumerable like real NodeList)
    let len = node_ids.len();
    backing.define_property_or_throw(
        js_string!("length"),
        PropertyDescriptor::builder()
            .value(JsValue::from(len as i32))
            .writable(false)
            .configurable(true)
            .enumerable(false)
            .build(),
        context,
    )?;

    // Pre-populate numeric indices
    for (i, &node_id) in node_ids.iter().enumerate() {
        let js_obj = get_or_create_js_element(node_id, tree.clone(), context)?;
        backing.define_property_or_throw(
            js_string!(i.to_string()),
            PropertyDescriptor::builder()
                .value(js_obj)
                .writable(false)
                .configurable(true)
                .enumerable(true)
                .build(),
            context,
        )?;
    }

    // Define item() method
    let tree_for_item = tree.clone();
    let node_ids_for_item = node_ids.clone();
    let item_fn = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let index = args.first().map(|v| v.to_number(ctx)).transpose()?.unwrap_or(0.0) as i64;

            if index < 0 {
                return Ok(JsValue::null());
            }

            match node_ids_for_item.get(index as usize) {
                Some(&node_id) => {
                    let js_obj = get_or_create_js_element(node_id, tree_for_item.clone(), ctx)?;
                    Ok(js_obj.into())
                }
                None => Ok(JsValue::null()),
            }
        })
    };
    let realm = context.realm().clone();
    backing.define_property_or_throw(
        js_string!("item"),
        PropertyDescriptor::builder()
            .value(item_fn.to_js_function(&realm))
            .writable(true)
            .configurable(true)
            .enumerable(false)
            .build(),
        context,
    )?;

    Ok(backing)
}
