use std::cell::RefCell;
use std::rc::Rc;

use boa_engine::{
    js_string,
    native_function::NativeFunction,
    object::{FunctionObjectBuilder, ObjectInitializer},
    property::PropertyDescriptor,
    Context, JsError, JsObject, JsResult, JsValue,
};

use crate::dom::{DomTree, NodeData, NodeId};
use crate::js::realm_state;

use super::super::element::get_or_create_js_element;

// ---------------------------------------------------------------------------
// Live HTMLCollection creation (for children)
// ---------------------------------------------------------------------------

/// Create a live HTMLCollection backed by the given node's element children.
/// The returned object is a JS Proxy that intercepts numeric + named property access
/// and delegates to the DOM tree for live results.
///
/// Uses a pre-built factory function (from register_collections) to avoid
/// calling context.eval() which triggers a Boa environment bug.
pub(crate) fn create_live_htmlcollection(
    parent_id: NodeId,
    tree: Rc<RefCell<DomTree>>,
    context: &mut Context,
) -> JsResult<JsObject> {
    let backing = ObjectInitializer::new(context).build();

    // Set prototype to HTMLCollection.prototype
    if let Some(p) = realm_state::htmlcollection_proto(context) {
        backing.set_prototype(Some(p));
    }

    let realm = context.realm().clone();

    // item(index) method
    let tree_for_item = tree.clone();
    let item_fn = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let index = args.first().map(|v| v.to_number(ctx)).transpose()?.unwrap_or(0.0) as i64;

            if index < 0 {
                return Ok(JsValue::null());
            }

            let tree_ref = tree_for_item.borrow();
            let element_children = tree_ref.element_children(parent_id);
            match element_children.get(index as usize) {
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
    backing.define_property_or_throw(
        js_string!("__braille_item"),
        PropertyDescriptor::builder()
            .value(item_fn.to_js_function(&realm))
            .writable(true)
            .configurable(true)
            .enumerable(false)
            .build(),
        context,
    )?;

    // namedItem(name) method
    let tree_for_named = tree.clone();
    let named_item_fn = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let name = args
                .first()
                .map(|v| v.to_string(ctx))
                .transpose()?
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_default();

            let tree_ref = tree_for_named.borrow();
            let element_children = tree_ref.element_children(parent_id);

            for &child_id in &element_children {
                let node = tree_ref.get_node(child_id);
                if let NodeData::Element {
                    ref attributes,
                    ref namespace,
                    ..
                } = node.data
                {
                    let is_html = namespace == "http://www.w3.org/1999/xhtml";

                    if let Some(id_val) = attributes
                        .iter()
                        .find(|a| a.local_name == "id")
                        .map(|a| a.value.as_str())
                    {
                        if id_val == name {
                            let tree_clone = tree_for_named.clone();
                            drop(tree_ref);
                            let js_obj = get_or_create_js_element(child_id, tree_clone, ctx)?;
                            return Ok(js_obj.into());
                        }
                    }
                    if is_html {
                        if let Some(name_val) = attributes
                            .iter()
                            .find(|a| a.local_name == "name")
                            .map(|a| a.value.as_str())
                        {
                            if name_val == name {
                                let tree_clone = tree_for_named.clone();
                                drop(tree_ref);
                                let js_obj = get_or_create_js_element(child_id, tree_clone, ctx)?;
                                return Ok(js_obj.into());
                            }
                        }
                    }
                }
            }
            Ok(JsValue::null())
        })
    };
    backing.define_property_or_throw(
        js_string!("__braille_namedItem"),
        PropertyDescriptor::builder()
            .value(named_item_fn.to_js_function(&realm))
            .writable(true)
            .configurable(true)
            .enumerable(false)
            .build(),
        context,
    )?;

    // Native functions for proxy handler traps

    // Length getter
    let tree_for_len = tree.clone();
    let length_fn = unsafe {
        NativeFunction::from_closure(move |_this, _args, _ctx| {
            let tree_ref = tree_for_len.borrow();
            let children = tree_ref.element_children(parent_id);
            Ok(JsValue::from(children.len() as i32))
        })
    };

    // getChild(index)
    let tree_for_get = tree.clone();
    let get_child_fn = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let index = args.first().map(|v| v.to_number(ctx)).transpose()?.unwrap_or(0.0) as i64;

            if index < 0 {
                return Ok(JsValue::undefined());
            }

            let tree_ref = tree_for_get.borrow();
            let children = tree_ref.element_children(parent_id);
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

    // getNamed(name) for proxy
    let tree_for_named2 = tree.clone();
    let get_named_fn = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let name = args
                .first()
                .map(|v| v.to_string(ctx))
                .transpose()?
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_default();

            let tree_ref = tree_for_named2.borrow();
            let element_children = tree_ref.element_children(parent_id);

            for &child_id in &element_children {
                let node = tree_ref.get_node(child_id);
                if let NodeData::Element {
                    ref attributes,
                    ref namespace,
                    ..
                } = node.data
                {
                    let is_html = namespace == "http://www.w3.org/1999/xhtml";

                    if let Some(id_val) = attributes
                        .iter()
                        .find(|a| a.local_name == "id")
                        .map(|a| a.value.as_str())
                    {
                        if id_val == name {
                            let tree_clone = tree_for_named2.clone();
                            drop(tree_ref);
                            let js_obj = get_or_create_js_element(child_id, tree_clone, ctx)?;
                            return Ok(js_obj.into());
                        }
                    }
                    if is_html {
                        if let Some(name_val) = attributes
                            .iter()
                            .find(|a| a.local_name == "name")
                            .map(|a| a.value.as_str())
                        {
                            if name_val == name {
                                let tree_clone = tree_for_named2.clone();
                                drop(tree_ref);
                                let js_obj = get_or_create_js_element(child_id, tree_clone, ctx)?;
                                return Ok(js_obj.into());
                            }
                        }
                    }
                }
            }
            Ok(JsValue::undefined())
        })
    };

    // getNamedKeys()
    let tree_for_keys = tree.clone();
    let get_named_keys_fn = unsafe {
        NativeFunction::from_closure(move |_this, _args, _ctx| {
            let tree_ref = tree_for_keys.borrow();
            let element_children = tree_ref.element_children(parent_id);
            let mut names = Vec::new();

            for &child_id in &element_children {
                let node = tree_ref.get_node(child_id);
                if let NodeData::Element {
                    ref attributes,
                    ref namespace,
                    ..
                } = node.data
                {
                    let is_html = namespace == "http://www.w3.org/1999/xhtml";

                    if let Some(id_val) = attributes
                        .iter()
                        .find(|a| a.local_name == "id")
                        .map(|a| a.value.clone())
                    {
                        if !id_val.is_empty() && !names.contains(&id_val) {
                            names.push(id_val);
                        }
                    }
                    if is_html {
                        if let Some(name_val) = attributes
                            .iter()
                            .find(|a| a.local_name == "name")
                            .map(|a| a.value.clone())
                        {
                            if !name_val.is_empty() && !names.contains(&name_val) {
                                names.push(name_val);
                            }
                        }
                    }
                }
            }
            Ok(JsValue::from(js_string!(names.join("\x00"))))
        })
    };

    // Call the pre-built factory function: factory(backing, getLength, getChild, getNamed, getNamedKeys)
    let factory = realm_state::hc_proxy_factory(context).expect("HTMLCollection proxy factory not initialized");

    let length_js = length_fn.to_js_function(&realm);
    let get_child_js = get_child_fn.to_js_function(&realm);
    let get_named_js = get_named_fn.to_js_function(&realm);
    let get_named_keys_js = get_named_keys_fn.to_js_function(&realm);

    let result = factory.call(
        &JsValue::undefined(),
        &[
            backing.into(),
            length_js.into(),
            get_child_js.into(),
            get_named_js.into(),
            get_named_keys_js.into(),
        ],
        context,
    )?;

    let proxy_obj = result
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("failed to create HTMLCollection proxy").into()))?
        .clone();

    Ok(proxy_obj)
}

// ---------------------------------------------------------------------------
// Live HTMLCollection creation for getElementsByClassName
// ---------------------------------------------------------------------------

/// Collect all descendant elements (not `root` itself) that match ALL the given class names.
/// This is the tree-walk used by the live collection on every access.
fn collect_descendants_by_class(tree: &DomTree, root: NodeId, class_names: &[String]) -> Vec<NodeId> {
    // Per spec: if the token set is empty, return an empty collection
    if class_names.is_empty() {
        return Vec::new();
    }
    let mut results = Vec::new();
    collect_descendants_by_class_recursive(tree, root, class_names, &mut results, true);
    results
}

fn collect_descendants_by_class_recursive(
    tree: &DomTree,
    node_id: NodeId,
    class_names: &[String],
    results: &mut Vec<NodeId>,
    _is_root: bool,
) {
    // Iterative DFS — start with root's children to skip root itself
    let mut stack: Vec<NodeId> = tree.get_node(node_id).children.iter().copied().rev().collect();
    while let Some(current) = stack.pop() {
        let node = tree.get_node(current);
        if let NodeData::Element { ref attributes, .. } = node.data {
            if let Some(class_attr) = attributes
                .iter()
                .find(|a| a.local_name == "class")
                .map(|a| a.value.as_str())
            {
                let element_classes: Vec<&str> = class_attr.split_ascii_whitespace().collect();
                if class_names.iter().all(|cn| element_classes.contains(&cn.as_str())) {
                    results.push(current);
                }
            }
        }
        for &child_id in node.children.iter().rev() {
            stack.push(child_id);
        }
    }
}

/// Create a live HTMLCollection for getElementsByClassName.
/// Re-walks the subtree on every access to provide live behavior.
pub(crate) fn create_live_htmlcollection_by_class(
    root_id: NodeId,
    tree: Rc<RefCell<DomTree>>,
    class_arg: String,
    context: &mut Context,
) -> JsResult<JsObject> {
    // Per spec, split the argument on ASCII whitespace to get list of class names.
    // If empty or all whitespace, return empty collection.
    let class_names: Vec<String> = class_arg.split_ascii_whitespace().map(|s| s.to_string()).collect();

    let backing = ObjectInitializer::new(context).build();

    // Set prototype to HTMLCollection.prototype
    if let Some(p) = realm_state::htmlcollection_proto(context) {
        backing.set_prototype(Some(p));
    }

    let realm = context.realm().clone();

    // item(index) method
    let class_names_item = class_names.clone();
    let tree_for_item = tree.clone();
    let item_fn = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let index = args.first().map(|v| v.to_number(ctx)).transpose()?.unwrap_or(0.0) as i64;
            if index < 0 {
                return Ok(JsValue::null());
            }
            let tree_ref = tree_for_item.borrow();
            let matches = collect_descendants_by_class(&tree_ref, root_id, &class_names_item);
            match matches.get(index as usize) {
                Some(&nid) => {
                    let tc = tree_for_item.clone();
                    drop(tree_ref);
                    let js_obj = get_or_create_js_element(nid, tc, ctx)?;
                    Ok(js_obj.into())
                }
                None => Ok(JsValue::null()),
            }
        })
    };
    backing.define_property_or_throw(
        js_string!("__braille_item"),
        PropertyDescriptor::builder()
            .value(item_fn.to_js_function(&realm))
            .writable(true)
            .configurable(true)
            .enumerable(false)
            .build(),
        context,
    )?;

    // namedItem(name) method
    let class_names_named = class_names.clone();
    let tree_for_named = tree.clone();
    let named_item_fn = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let name = args
                .first()
                .map(|v| v.to_string(ctx))
                .transpose()?
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_default();
            let tree_ref = tree_for_named.borrow();
            let matches = collect_descendants_by_class(&tree_ref, root_id, &class_names_named);
            for &nid in &matches {
                let node = tree_ref.get_node(nid);
                if let NodeData::Element {
                    ref attributes,
                    ref namespace,
                    ..
                } = node.data
                {
                    let is_html = namespace == "http://www.w3.org/1999/xhtml";
                    if let Some(id_val) = attributes
                        .iter()
                        .find(|a| a.local_name == "id")
                        .map(|a| a.value.as_str())
                    {
                        if id_val == name {
                            let tc = tree_for_named.clone();
                            drop(tree_ref);
                            return Ok(get_or_create_js_element(nid, tc, ctx)?.into());
                        }
                    }
                    if is_html {
                        if let Some(name_val) = attributes
                            .iter()
                            .find(|a| a.local_name == "name")
                            .map(|a| a.value.as_str())
                        {
                            if name_val == name {
                                let tc = tree_for_named.clone();
                                drop(tree_ref);
                                return Ok(get_or_create_js_element(nid, tc, ctx)?.into());
                            }
                        }
                    }
                }
            }
            Ok(JsValue::null())
        })
    };
    backing.define_property_or_throw(
        js_string!("__braille_namedItem"),
        PropertyDescriptor::builder()
            .value(named_item_fn.to_js_function(&realm))
            .writable(true)
            .configurable(true)
            .enumerable(false)
            .build(),
        context,
    )?;

    // Length getter
    let class_names_len = class_names.clone();
    let tree_for_len = tree.clone();
    let length_fn = unsafe {
        NativeFunction::from_closure(move |_this, _args, _ctx| {
            let tree_ref = tree_for_len.borrow();
            let matches = collect_descendants_by_class(&tree_ref, root_id, &class_names_len);
            Ok(JsValue::from(matches.len() as i32))
        })
    };

    // getChild(index)
    let class_names_get = class_names.clone();
    let tree_for_get = tree.clone();
    let get_child_fn = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let index = args.first().map(|v| v.to_number(ctx)).transpose()?.unwrap_or(0.0) as i64;
            if index < 0 {
                return Ok(JsValue::undefined());
            }
            let tree_ref = tree_for_get.borrow();
            let matches = collect_descendants_by_class(&tree_ref, root_id, &class_names_get);
            match matches.get(index as usize) {
                Some(&nid) => {
                    let tc = tree_for_get.clone();
                    drop(tree_ref);
                    Ok(get_or_create_js_element(nid, tc, ctx)?.into())
                }
                None => Ok(JsValue::undefined()),
            }
        })
    };

    // getNamed(name) for proxy
    let class_names_named2 = class_names.clone();
    let tree_for_named2 = tree.clone();
    let get_named_fn = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let name = args
                .first()
                .map(|v| v.to_string(ctx))
                .transpose()?
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_default();
            let tree_ref = tree_for_named2.borrow();
            let matches = collect_descendants_by_class(&tree_ref, root_id, &class_names_named2);
            for &nid in &matches {
                let node = tree_ref.get_node(nid);
                if let NodeData::Element {
                    ref attributes,
                    ref namespace,
                    ..
                } = node.data
                {
                    let is_html = namespace == "http://www.w3.org/1999/xhtml";
                    if let Some(id_val) = attributes
                        .iter()
                        .find(|a| a.local_name == "id")
                        .map(|a| a.value.as_str())
                    {
                        if id_val == name {
                            let tc = tree_for_named2.clone();
                            drop(tree_ref);
                            return Ok(get_or_create_js_element(nid, tc, ctx)?.into());
                        }
                    }
                    if is_html {
                        if let Some(name_val) = attributes
                            .iter()
                            .find(|a| a.local_name == "name")
                            .map(|a| a.value.as_str())
                        {
                            if name_val == name {
                                let tc = tree_for_named2.clone();
                                drop(tree_ref);
                                return Ok(get_or_create_js_element(nid, tc, ctx)?.into());
                            }
                        }
                    }
                }
            }
            Ok(JsValue::undefined())
        })
    };

    // getNamedKeys()
    let class_names_keys = class_names.clone();
    let tree_for_keys = tree.clone();
    let get_named_keys_fn = unsafe {
        NativeFunction::from_closure(move |_this, _args, _ctx| {
            let tree_ref = tree_for_keys.borrow();
            let matches = collect_descendants_by_class(&tree_ref, root_id, &class_names_keys);
            let mut names = Vec::new();
            for &nid in &matches {
                let node = tree_ref.get_node(nid);
                if let NodeData::Element {
                    ref attributes,
                    ref namespace,
                    ..
                } = node.data
                {
                    let is_html = namespace == "http://www.w3.org/1999/xhtml";
                    if let Some(id_val) = attributes
                        .iter()
                        .find(|a| a.local_name == "id")
                        .map(|a| a.value.clone())
                    {
                        if !id_val.is_empty() && !names.contains(&id_val) {
                            names.push(id_val);
                        }
                    }
                    if is_html {
                        if let Some(name_val) = attributes
                            .iter()
                            .find(|a| a.local_name == "name")
                            .map(|a| a.value.clone())
                        {
                            if !name_val.is_empty() && !names.contains(&name_val) {
                                names.push(name_val);
                            }
                        }
                    }
                }
            }
            Ok(JsValue::from(js_string!(names.join("\x00"))))
        })
    };

    // Call factory
    let factory = realm_state::hc_proxy_factory(context).expect("HTMLCollection proxy factory not initialized");

    let length_js = length_fn.to_js_function(&realm);
    let get_child_js = get_child_fn.to_js_function(&realm);
    let get_named_js = get_named_fn.to_js_function(&realm);
    let get_named_keys_js = get_named_keys_fn.to_js_function(&realm);

    let result = factory.call(
        &JsValue::undefined(),
        &[
            backing.into(),
            length_js.into(),
            get_child_js.into(),
            get_named_js.into(),
            get_named_keys_js.into(),
        ],
        context,
    )?;

    let proxy_obj = result
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("failed to create HTMLCollection proxy").into()))?
        .clone();

    Ok(proxy_obj)
}

// ---------------------------------------------------------------------------
// Live HTMLCollection creation for getElementsByTagName
// ---------------------------------------------------------------------------

/// Collect all descendant elements (not `root` itself) that match the tag name.
/// For HTML documents: HTML-namespace elements match case-insensitively via ASCII lowercase
/// of the *qualified name* (tagName). Non-HTML-namespace elements match the qualified name
/// exactly (case-sensitive).
fn collect_descendants_by_tag(tree: &DomTree, root: NodeId, tag_name: &str) -> Vec<NodeId> {
    let mut results = Vec::new();
    collect_descendants_by_tag_recursive(tree, root, tag_name, &mut results, true);
    results
}

fn collect_descendants_by_tag_recursive(
    tree: &DomTree,
    node_id: NodeId,
    search_tag: &str,
    results: &mut Vec<NodeId>,
    _is_root: bool,
) {
    // Iterative DFS — start with root's children to skip root itself
    let mut stack: Vec<NodeId> = tree.get_node(node_id).children.iter().copied().rev().collect();
    while let Some(current) = stack.pop() {
        let node = tree.get_node(current);
        if let NodeData::Element {
            ref tag_name,
            ref namespace,
            ..
        } = node.data
        {
            if search_tag == "*" {
                results.push(current);
            } else if tree.is_html_document() {
                // Per spec for HTML documents:
                // - HTML-namespace elements: compare search_tag ASCII-lowercased against
                //   the element's qualified name AS-IS (not lowercased)
                // - Non-HTML-namespace elements: compare search_tag AS-IS (exact match)
                let is_html_ns = namespace == "http://www.w3.org/1999/xhtml";
                if is_html_ns {
                    if search_tag.to_ascii_lowercase() == *tag_name {
                        results.push(current);
                    }
                } else if search_tag == tag_name {
                    results.push(current);
                }
            } else {
                // XML document: exact match for all namespaces
                if search_tag == tag_name {
                    results.push(current);
                }
            }
        }
        for &child_id in node.children.iter().rev() {
            stack.push(child_id);
        }
    }
}

/// Create a live HTMLCollection for getElementsByTagName.
/// Re-walks the subtree on every access to provide live behavior.
pub(crate) fn create_live_htmlcollection_by_tag(
    root_id: NodeId,
    tree: Rc<RefCell<DomTree>>,
    tag_name: String,
    context: &mut Context,
) -> JsResult<JsObject> {
    let backing = ObjectInitializer::new(context).build();

    if let Some(p) = realm_state::htmlcollection_proto(context) {
        backing.set_prototype(Some(p));
    }

    let realm = context.realm().clone();

    // item(index) method
    let tag_item = tag_name.clone();
    let tree_for_item = tree.clone();
    let item_fn = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let index = args.first().map(|v| v.to_number(ctx)).transpose()?.unwrap_or(0.0) as i64;
            if index < 0 {
                return Ok(JsValue::null());
            }
            let tree_ref = tree_for_item.borrow();
            let matches = collect_descendants_by_tag(&tree_ref, root_id, &tag_item);
            match matches.get(index as usize) {
                Some(&nid) => {
                    let tc = tree_for_item.clone();
                    drop(tree_ref);
                    Ok(get_or_create_js_element(nid, tc, ctx)?.into())
                }
                None => Ok(JsValue::null()),
            }
        })
    };
    backing.define_property_or_throw(
        js_string!("__braille_item"),
        PropertyDescriptor::builder()
            .value(item_fn.to_js_function(&realm))
            .writable(true)
            .configurable(true)
            .enumerable(false)
            .build(),
        context,
    )?;

    // namedItem(name) method
    let tag_named = tag_name.clone();
    let tree_for_named = tree.clone();
    let named_item_fn = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let name = args
                .first()
                .map(|v| v.to_string(ctx))
                .transpose()?
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_default();
            let tree_ref = tree_for_named.borrow();
            let matches = collect_descendants_by_tag(&tree_ref, root_id, &tag_named);
            for &nid in &matches {
                let node = tree_ref.get_node(nid);
                if let NodeData::Element {
                    ref attributes,
                    ref namespace,
                    ..
                } = node.data
                {
                    let is_html = namespace == "http://www.w3.org/1999/xhtml";
                    if let Some(id_val) = attributes
                        .iter()
                        .find(|a| a.local_name == "id")
                        .map(|a| a.value.as_str())
                    {
                        if id_val == name {
                            let tc = tree_for_named.clone();
                            drop(tree_ref);
                            return Ok(get_or_create_js_element(nid, tc, ctx)?.into());
                        }
                    }
                    if is_html {
                        if let Some(name_val) = attributes
                            .iter()
                            .find(|a| a.local_name == "name")
                            .map(|a| a.value.as_str())
                        {
                            if name_val == name {
                                let tc = tree_for_named.clone();
                                drop(tree_ref);
                                return Ok(get_or_create_js_element(nid, tc, ctx)?.into());
                            }
                        }
                    }
                }
            }
            Ok(JsValue::null())
        })
    };
    backing.define_property_or_throw(
        js_string!("__braille_namedItem"),
        PropertyDescriptor::builder()
            .value(named_item_fn.to_js_function(&realm))
            .writable(true)
            .configurable(true)
            .enumerable(false)
            .build(),
        context,
    )?;

    // Length getter
    let tag_len = tag_name.clone();
    let tree_for_len = tree.clone();
    let length_fn = unsafe {
        NativeFunction::from_closure(move |_this, _args, _ctx| {
            let tree_ref = tree_for_len.borrow();
            let matches = collect_descendants_by_tag(&tree_ref, root_id, &tag_len);
            Ok(JsValue::from(matches.len() as i32))
        })
    };

    // getChild(index)
    let tag_get = tag_name.clone();
    let tree_for_get = tree.clone();
    let get_child_fn = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let index = args.first().map(|v| v.to_number(ctx)).transpose()?.unwrap_or(0.0) as i64;
            if index < 0 {
                return Ok(JsValue::undefined());
            }
            let tree_ref = tree_for_get.borrow();
            let matches = collect_descendants_by_tag(&tree_ref, root_id, &tag_get);
            match matches.get(index as usize) {
                Some(&nid) => {
                    let tc = tree_for_get.clone();
                    drop(tree_ref);
                    Ok(get_or_create_js_element(nid, tc, ctx)?.into())
                }
                None => Ok(JsValue::undefined()),
            }
        })
    };

    // getNamed(name) for proxy
    let tag_named2 = tag_name.clone();
    let tree_for_named2 = tree.clone();
    let get_named_fn = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let name = args
                .first()
                .map(|v| v.to_string(ctx))
                .transpose()?
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_default();
            let tree_ref = tree_for_named2.borrow();
            let matches = collect_descendants_by_tag(&tree_ref, root_id, &tag_named2);
            for &nid in &matches {
                let node = tree_ref.get_node(nid);
                if let NodeData::Element {
                    ref attributes,
                    ref namespace,
                    ..
                } = node.data
                {
                    let is_html = namespace == "http://www.w3.org/1999/xhtml";
                    if let Some(id_val) = attributes
                        .iter()
                        .find(|a| a.local_name == "id")
                        .map(|a| a.value.as_str())
                    {
                        if id_val == name {
                            let tc = tree_for_named2.clone();
                            drop(tree_ref);
                            return Ok(get_or_create_js_element(nid, tc, ctx)?.into());
                        }
                    }
                    if is_html {
                        if let Some(name_val) = attributes
                            .iter()
                            .find(|a| a.local_name == "name")
                            .map(|a| a.value.as_str())
                        {
                            if name_val == name {
                                let tc = tree_for_named2.clone();
                                drop(tree_ref);
                                return Ok(get_or_create_js_element(nid, tc, ctx)?.into());
                            }
                        }
                    }
                }
            }
            Ok(JsValue::undefined())
        })
    };

    // getNamedKeys()
    let tag_keys = tag_name.clone();
    let tree_for_keys = tree.clone();
    let get_named_keys_fn = unsafe {
        NativeFunction::from_closure(move |_this, _args, _ctx| {
            let tree_ref = tree_for_keys.borrow();
            let matches = collect_descendants_by_tag(&tree_ref, root_id, &tag_keys);
            let mut names = Vec::new();
            for &nid in &matches {
                let node = tree_ref.get_node(nid);
                if let NodeData::Element {
                    ref attributes,
                    ref namespace,
                    ..
                } = node.data
                {
                    let is_html = namespace == "http://www.w3.org/1999/xhtml";
                    if let Some(id_val) = attributes
                        .iter()
                        .find(|a| a.local_name == "id")
                        .map(|a| a.value.clone())
                    {
                        if !id_val.is_empty() && !names.contains(&id_val) {
                            names.push(id_val);
                        }
                    }
                    if is_html {
                        if let Some(name_val) = attributes
                            .iter()
                            .find(|a| a.local_name == "name")
                            .map(|a| a.value.clone())
                        {
                            if !name_val.is_empty() && !names.contains(&name_val) {
                                names.push(name_val);
                            }
                        }
                    }
                }
            }
            Ok(JsValue::from(js_string!(names.join("\x00"))))
        })
    };

    // Call factory
    let factory = realm_state::hc_proxy_factory(context).expect("HTMLCollection proxy factory not initialized");

    let length_js = length_fn.to_js_function(&realm);
    let get_child_js = get_child_fn.to_js_function(&realm);
    let get_named_js = get_named_fn.to_js_function(&realm);
    let get_named_keys_js = get_named_keys_fn.to_js_function(&realm);

    let result = factory.call(
        &JsValue::undefined(),
        &[
            backing.into(),
            length_js.into(),
            get_child_js.into(),
            get_named_js.into(),
            get_named_keys_js.into(),
        ],
        context,
    )?;

    let proxy_obj = result
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("failed to create HTMLCollection proxy").into()))?
        .clone();

    Ok(proxy_obj)
}

// ---------------------------------------------------------------------------
// Live HTMLCollection creation for getElementsByTagNameNS
// ---------------------------------------------------------------------------

/// Collect all descendant elements (not `root` itself) that match the given namespace and local name.
/// `"*"` as namespace matches any namespace; `"*"` as local_name matches any local name.
/// An empty string namespace matches elements with null/empty namespace.
fn collect_descendants_by_tag_ns(tree: &DomTree, root: NodeId, namespace: &str, local_name: &str) -> Vec<NodeId> {
    let mut results = Vec::new();
    collect_descendants_by_tag_ns_recursive(tree, root, namespace, local_name, &mut results, true);
    results
}

fn collect_descendants_by_tag_ns_recursive(
    tree: &DomTree,
    node_id: NodeId,
    search_ns: &str,
    search_local: &str,
    results: &mut Vec<NodeId>,
    is_root: bool,
) {
    let node = tree.get_node(node_id);
    if !is_root {
        if let NodeData::Element {
            ref tag_name,
            ref namespace,
            ..
        } = node.data
        {
            // Extract local name from qualified name (strip prefix if present)
            let element_local = if let Some(colon_pos) = tag_name.find(':') {
                &tag_name[colon_pos + 1..]
            } else {
                tag_name.as_str()
            };

            let ns_matches = search_ns == "*" || search_ns == namespace;
            let local_matches = search_local == "*" || search_local == element_local;

            if ns_matches && local_matches {
                results.push(node_id);
            }
        }
    }
    let children: Vec<NodeId> = node.children.clone();
    for child_id in children {
        collect_descendants_by_tag_ns_recursive(tree, child_id, search_ns, search_local, results, false);
    }
}

/// Create a live HTMLCollection for getElementsByTagNameNS.
/// Re-walks the subtree on every access to provide live behavior.
pub(crate) fn create_live_htmlcollection_by_tag_name_ns(
    root_id: NodeId,
    tree: Rc<RefCell<DomTree>>,
    namespace: String,
    local_name: String,
    context: &mut Context,
) -> JsResult<JsObject> {
    let backing = ObjectInitializer::new(context).build();

    if let Some(p) = realm_state::htmlcollection_proto(context) {
        backing.set_prototype(Some(p));
    }

    let realm = context.realm().clone();

    // item(index) method
    let ns_item = namespace.clone();
    let ln_item = local_name.clone();
    let tree_for_item = tree.clone();
    let item_fn = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let index = args.first().map(|v| v.to_number(ctx)).transpose()?.unwrap_or(0.0) as i64;
            if index < 0 {
                return Ok(JsValue::null());
            }
            let tree_ref = tree_for_item.borrow();
            let matches = collect_descendants_by_tag_ns(&tree_ref, root_id, &ns_item, &ln_item);
            match matches.get(index as usize) {
                Some(&nid) => {
                    let tc = tree_for_item.clone();
                    drop(tree_ref);
                    Ok(get_or_create_js_element(nid, tc, ctx)?.into())
                }
                None => Ok(JsValue::null()),
            }
        })
    };
    backing.define_property_or_throw(
        js_string!("__braille_item"),
        PropertyDescriptor::builder()
            .value(item_fn.to_js_function(&realm))
            .writable(true)
            .configurable(true)
            .enumerable(false)
            .build(),
        context,
    )?;

    // namedItem(name) method
    let ns_named = namespace.clone();
    let ln_named = local_name.clone();
    let tree_for_named = tree.clone();
    let named_item_fn = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let name = args
                .first()
                .map(|v| v.to_string(ctx))
                .transpose()?
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_default();
            let tree_ref = tree_for_named.borrow();
            let matches = collect_descendants_by_tag_ns(&tree_ref, root_id, &ns_named, &ln_named);
            for &nid in &matches {
                let node = tree_ref.get_node(nid);
                if let NodeData::Element {
                    ref attributes,
                    ref namespace,
                    ..
                } = node.data
                {
                    let is_html = namespace == "http://www.w3.org/1999/xhtml";
                    if let Some(id_val) = attributes
                        .iter()
                        .find(|a| a.local_name == "id")
                        .map(|a| a.value.as_str())
                    {
                        if id_val == name {
                            let tc = tree_for_named.clone();
                            drop(tree_ref);
                            return Ok(get_or_create_js_element(nid, tc, ctx)?.into());
                        }
                    }
                    if is_html {
                        if let Some(name_val) = attributes
                            .iter()
                            .find(|a| a.local_name == "name")
                            .map(|a| a.value.as_str())
                        {
                            if name_val == name {
                                let tc = tree_for_named.clone();
                                drop(tree_ref);
                                return Ok(get_or_create_js_element(nid, tc, ctx)?.into());
                            }
                        }
                    }
                }
            }
            Ok(JsValue::null())
        })
    };
    backing.define_property_or_throw(
        js_string!("__braille_namedItem"),
        PropertyDescriptor::builder()
            .value(named_item_fn.to_js_function(&realm))
            .writable(true)
            .configurable(true)
            .enumerable(false)
            .build(),
        context,
    )?;

    // Length getter
    let ns_len = namespace.clone();
    let ln_len = local_name.clone();
    let tree_for_len = tree.clone();
    let length_fn = unsafe {
        NativeFunction::from_closure(move |_this, _args, _ctx| {
            let tree_ref = tree_for_len.borrow();
            let matches = collect_descendants_by_tag_ns(&tree_ref, root_id, &ns_len, &ln_len);
            Ok(JsValue::from(matches.len() as i32))
        })
    };

    // getChild(index)
    let ns_get = namespace.clone();
    let ln_get = local_name.clone();
    let tree_for_get = tree.clone();
    let get_child_fn = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let index = args.first().map(|v| v.to_number(ctx)).transpose()?.unwrap_or(0.0) as i64;
            if index < 0 {
                return Ok(JsValue::undefined());
            }
            let tree_ref = tree_for_get.borrow();
            let matches = collect_descendants_by_tag_ns(&tree_ref, root_id, &ns_get, &ln_get);
            match matches.get(index as usize) {
                Some(&nid) => {
                    let tc = tree_for_get.clone();
                    drop(tree_ref);
                    Ok(get_or_create_js_element(nid, tc, ctx)?.into())
                }
                None => Ok(JsValue::undefined()),
            }
        })
    };

    // getNamed(name) for proxy
    let ns_named2 = namespace.clone();
    let ln_named2 = local_name.clone();
    let tree_for_named2 = tree.clone();
    let get_named_fn = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let name = args
                .first()
                .map(|v| v.to_string(ctx))
                .transpose()?
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_default();
            let tree_ref = tree_for_named2.borrow();
            let matches = collect_descendants_by_tag_ns(&tree_ref, root_id, &ns_named2, &ln_named2);
            for &nid in &matches {
                let node = tree_ref.get_node(nid);
                if let NodeData::Element {
                    ref attributes,
                    ref namespace,
                    ..
                } = node.data
                {
                    let is_html = namespace == "http://www.w3.org/1999/xhtml";
                    if let Some(id_val) = attributes
                        .iter()
                        .find(|a| a.local_name == "id")
                        .map(|a| a.value.as_str())
                    {
                        if id_val == name {
                            let tc = tree_for_named2.clone();
                            drop(tree_ref);
                            return Ok(get_or_create_js_element(nid, tc, ctx)?.into());
                        }
                    }
                    if is_html {
                        if let Some(name_val) = attributes
                            .iter()
                            .find(|a| a.local_name == "name")
                            .map(|a| a.value.as_str())
                        {
                            if name_val == name {
                                let tc = tree_for_named2.clone();
                                drop(tree_ref);
                                return Ok(get_or_create_js_element(nid, tc, ctx)?.into());
                            }
                        }
                    }
                }
            }
            Ok(JsValue::undefined())
        })
    };

    // getNamedKeys()
    let ns_keys = namespace.clone();
    let ln_keys = local_name.clone();
    let tree_for_keys = tree.clone();
    let get_named_keys_fn = unsafe {
        NativeFunction::from_closure(move |_this, _args, _ctx| {
            let tree_ref = tree_for_keys.borrow();
            let matches = collect_descendants_by_tag_ns(&tree_ref, root_id, &ns_keys, &ln_keys);
            let mut names = Vec::new();
            for &nid in &matches {
                let node = tree_ref.get_node(nid);
                if let NodeData::Element {
                    ref attributes,
                    ref namespace,
                    ..
                } = node.data
                {
                    let is_html = namespace == "http://www.w3.org/1999/xhtml";
                    if let Some(id_val) = attributes
                        .iter()
                        .find(|a| a.local_name == "id")
                        .map(|a| a.value.clone())
                    {
                        if !id_val.is_empty() && !names.contains(&id_val) {
                            names.push(id_val);
                        }
                    }
                    if is_html {
                        if let Some(name_val) = attributes
                            .iter()
                            .find(|a| a.local_name == "name")
                            .map(|a| a.value.clone())
                        {
                            if !name_val.is_empty() && !names.contains(&name_val) {
                                names.push(name_val);
                            }
                        }
                    }
                }
            }
            Ok(JsValue::from(js_string!(names.join("\x00"))))
        })
    };

    // Call factory
    let factory = realm_state::hc_proxy_factory(context).expect("HTMLCollection proxy factory not initialized");

    let length_js = length_fn.to_js_function(&realm);
    let get_child_js = get_child_fn.to_js_function(&realm);
    let get_named_js = get_named_fn.to_js_function(&realm);
    let get_named_keys_js = get_named_keys_fn.to_js_function(&realm);

    let result = factory.call(
        &JsValue::undefined(),
        &[
            backing.into(),
            length_js.into(),
            get_child_js.into(),
            get_named_js.into(),
            get_named_keys_js.into(),
        ],
        context,
    )?;

    let proxy_obj = result
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("failed to create HTMLCollection proxy").into()))?
        .clone();

    Ok(proxy_obj)
}
