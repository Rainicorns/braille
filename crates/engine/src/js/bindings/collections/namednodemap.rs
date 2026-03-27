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

use super::super::element::{get_or_create_js_element, JsElement};

// ---------------------------------------------------------------------------
// Live NamedNodeMap creation (for element.attributes)
// ---------------------------------------------------------------------------

/// Create a live NamedNodeMap backed by the given element's attributes.
/// The returned object is a JS Proxy that intercepts numeric, named, and method access.
pub(crate) fn create_live_namednodemap(
    element_id: NodeId,
    tree: Rc<RefCell<DomTree>>,
    context: &mut Context,
) -> JsResult<JsObject> {
    let backing = ObjectInitializer::new(context).build();

    // Set prototype to NamedNodeMap.prototype
    if let Some(p) = realm_state::nnm_proto(context) {
        backing.set_prototype(Some(p));
    }

    let realm = context.realm().clone();

    // Shared Attr identity cache from RealmState — ensures the same Attr JsObject
    // is returned across getAttributeNode(), attributes.getNamedItem(), attributes[i], etc.
    let attr_node_map = realm_state::attr_node_cache(context);

    // item(index) — backing method
    let tree_for_item = tree.clone();
    let attr_map_item = attr_node_map.clone();
    let item_fn = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let index = args
                .first()
                .map(|v| v.to_number(ctx))
                .transpose()?
                .unwrap_or(0.0) as i64;

            if index < 0 {
                return Ok(JsValue::null());
            }

            let tree_ptr = Rc::as_ptr(&tree_for_item) as usize;
            let attr_info = {
                let t = tree_for_item.borrow();
                let node = t.get_node(element_id);
                match &node.data {
                    NodeData::Element { attributes, .. } => attributes.get(index as usize).map(|a| {
                        (
                            a.local_name.clone(),
                            a.namespace.clone(),
                            a.prefix.clone(),
                            a.value.clone(),
                            a.qualified_name(),
                        )
                    }),
                    _ => None,
                }
            };

            match attr_info {
                Some((local, ns, prefix, value, qname)) => {
                    let key = (tree_ptr, element_id, qname);
                    let node_id = {
                        let map = attr_map_item.borrow();
                        map.get(&key).copied()
                    };
                    let node_id = match node_id {
                        Some(id) => {
                            // Update the cached Attr node's value
                            if let NodeData::Attr {
                                value: ref mut v, ..
                            } = tree_for_item.borrow_mut().get_node_mut(id).data
                            {
                                *v = value;
                            }
                            id
                        }
                        None => {
                            let id = tree_for_item.borrow_mut().create_attr(&local, &ns, &prefix, &value);
                            attr_map_item.borrow_mut().insert(key, id);
                            id
                        }
                    };

                    // Get or create JS object for the Attr node
                    let js_obj = get_or_create_js_element(node_id, tree_for_item.clone(), ctx)?;
                    // Set ownerElement to the element
                    let el_obj = get_or_create_js_element(element_id, tree_for_item.clone(), ctx)?;
                    let _ = js_obj.define_property_or_throw(
                        js_string!("ownerElement"),
                        PropertyDescriptor::builder()
                            .value(JsValue::from(el_obj))
                            .writable(false)
                            .configurable(true)
                            .enumerable(true)
                            .build(),
                        ctx,
                    );
                    Ok(js_obj.into())
                }
                None => Ok(JsValue::null()),
            }
        })
    };
    backing.define_property_or_throw(
        js_string!("__braille_nnm_item"),
        PropertyDescriptor::builder()
            .value(item_fn.to_js_function(&realm))
            .writable(true)
            .configurable(true)
            .enumerable(false)
            .build(),
        context,
    )?;

    // getNamedItem(name) — backing method
    let tree_for_gni = tree.clone();
    let attr_map_gni = attr_node_map.clone();
    let get_named_item_fn = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let name = args
                .first()
                .map(|v| v.to_string(ctx))
                .transpose()?
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_default();

            let tree_ptr = Rc::as_ptr(&tree_for_gni) as usize;
            let attr_info = {
                let t = tree_for_gni.borrow();
                let node = t.get_node(element_id);
                match &node.data {
                    NodeData::Element { attributes, .. } => attributes
                        .iter()
                        .find(|a| a.qualified_name() == name || a.local_name == name)
                        .map(|a| {
                            (
                                a.local_name.clone(),
                                a.namespace.clone(),
                                a.prefix.clone(),
                                a.value.clone(),
                                a.qualified_name(),
                            )
                        }),
                    _ => None,
                }
            };

            match attr_info {
                Some((local, ns, prefix, value, qname)) => {
                    let key = (tree_ptr, element_id, qname);
                    let node_id = {
                        let map = attr_map_gni.borrow();
                        map.get(&key).copied()
                    };
                    let node_id = match node_id {
                        Some(id) => {
                            if let NodeData::Attr {
                                value: ref mut v, ..
                            } = tree_for_gni.borrow_mut().get_node_mut(id).data
                            {
                                *v = value;
                            }
                            id
                        }
                        None => {
                            let id = tree_for_gni.borrow_mut().create_attr(&local, &ns, &prefix, &value);
                            attr_map_gni.borrow_mut().insert(key, id);
                            id
                        }
                    };

                    let js_obj = get_or_create_js_element(node_id, tree_for_gni.clone(), ctx)?;
                    let el_obj = get_or_create_js_element(element_id, tree_for_gni.clone(), ctx)?;
                    let _ = js_obj.define_property_or_throw(
                        js_string!("ownerElement"),
                        PropertyDescriptor::builder()
                            .value(JsValue::from(el_obj))
                            .writable(false)
                            .configurable(true)
                            .enumerable(true)
                            .build(),
                        ctx,
                    );
                    Ok(js_obj.into())
                }
                None => Ok(JsValue::null()),
            }
        })
    };
    backing.define_property_or_throw(
        js_string!("__braille_nnm_getNamedItem"),
        PropertyDescriptor::builder()
            .value(get_named_item_fn.to_js_function(&realm))
            .writable(true)
            .configurable(true)
            .enumerable(false)
            .build(),
        context,
    )?;

    // getNamedItemNS(ns, localName) — backing method
    let tree_for_gnins = tree.clone();
    let attr_map_gnins = attr_node_map.clone();
    let get_named_item_ns_fn = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let ns_val = args.first().cloned().unwrap_or(JsValue::null());
            let namespace = if ns_val.is_null() || ns_val.is_undefined() {
                String::new()
            } else {
                ns_val.to_string(ctx)?.to_std_string_escaped()
            };

            let local_name = args
                .get(1)
                .map(|v| v.to_string(ctx))
                .transpose()?
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_default();

            let tree_ptr = Rc::as_ptr(&tree_for_gnins) as usize;
            let attr_info = {
                let t = tree_for_gnins.borrow();
                let node = t.get_node(element_id);
                match &node.data {
                    NodeData::Element { attributes, .. } => attributes
                        .iter()
                        .find(|a| a.matches_ns(&namespace, &local_name))
                        .map(|a| {
                            (
                                a.local_name.clone(),
                                a.namespace.clone(),
                                a.prefix.clone(),
                                a.value.clone(),
                                a.qualified_name(),
                            )
                        }),
                    _ => None,
                }
            };

            match attr_info {
                Some((local, ns, prefix, value, qname)) => {
                    let key = (tree_ptr, element_id, qname);
                    let node_id = {
                        let map = attr_map_gnins.borrow();
                        map.get(&key).copied()
                    };
                    let node_id = match node_id {
                        Some(id) => {
                            if let NodeData::Attr {
                                value: ref mut v, ..
                            } = tree_for_gnins.borrow_mut().get_node_mut(id).data
                            {
                                *v = value;
                            }
                            id
                        }
                        None => {
                            let id = tree_for_gnins.borrow_mut().create_attr(&local, &ns, &prefix, &value);
                            attr_map_gnins.borrow_mut().insert(key, id);
                            id
                        }
                    };

                    let js_obj = get_or_create_js_element(node_id, tree_for_gnins.clone(), ctx)?;
                    let el_obj = get_or_create_js_element(element_id, tree_for_gnins.clone(), ctx)?;
                    let _ = js_obj.define_property_or_throw(
                        js_string!("ownerElement"),
                        PropertyDescriptor::builder()
                            .value(JsValue::from(el_obj))
                            .writable(false)
                            .configurable(true)
                            .enumerable(true)
                            .build(),
                        ctx,
                    );
                    Ok(js_obj.into())
                }
                None => Ok(JsValue::null()),
            }
        })
    };
    backing.define_property_or_throw(
        js_string!("__braille_nnm_getNamedItemNS"),
        PropertyDescriptor::builder()
            .value(get_named_item_ns_fn.to_js_function(&realm))
            .writable(true)
            .configurable(true)
            .enumerable(false)
            .build(),
        context,
    )?;

    // setNamedItem(attr) — backing method
    let tree_for_sni = tree.clone();
    let attr_map_sni = attr_node_map.clone();
    let set_named_item_fn = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let attr_val = args.first().cloned().unwrap_or(JsValue::undefined());
            let attr_obj = attr_val.as_object().ok_or_else(|| {
                JsError::from_opaque(JsValue::from(js_string!("setNamedItem requires an Attr argument")))
            })?;

            let name = attr_obj
                .get(js_string!("name"), ctx)?
                .to_string(ctx)?
                .to_std_string_escaped();
            let value = attr_obj
                .get(js_string!("value"), ctx)?
                .to_string(ctx)?
                .to_std_string_escaped();

            // Check InUseAttributeError
            let owner = attr_obj.get(js_string!("ownerElement"), ctx)?;
            if !owner.is_null() && !owner.is_undefined() {
                if let Some(owner_obj) = owner.as_object() {
                    let el_obj = get_or_create_js_element(element_id, tree_for_sni.clone(), ctx)?;
                    if !JsObject::equals(&owner_obj, &el_obj) {
                        let exc = super::super::create_dom_exception(
                            ctx,
                            "InUseAttributeError",
                            "The attribute is already in use",
                            10,
                        )?;
                        return Err(JsError::from_opaque(exc.into()));
                    }
                }
            }

            let tree_ptr = Rc::as_ptr(&tree_for_sni) as usize;

            // Find existing old Attr's qualified name (to look up in attr_node_map)
            let old_qname = {
                let t = tree_for_sni.borrow();
                let node = t.get_node(element_id);
                match &node.data {
                    NodeData::Element { attributes, .. } => attributes
                        .iter()
                        .find(|a| a.qualified_name() == name || a.local_name == name)
                        .map(|a| a.qualified_name()),
                    _ => None,
                }
            };

            // Get old Attr JS object from the map (if cached)
            let old_result = if let Some(ref oq) = old_qname {
                let old_key = (tree_ptr, element_id, oq.clone());
                let old_node_id = attr_map_sni.borrow().get(&old_key).copied();
                if let Some(nid) = old_node_id {
                    let js = get_or_create_js_element(nid, tree_for_sni.clone(), ctx)?;
                    // Set ownerElement to null on old attr
                    let _ = js.define_property_or_throw(
                        js_string!("ownerElement"),
                        PropertyDescriptor::builder()
                            .value(JsValue::null())
                            .writable(false)
                            .configurable(true)
                            .enumerable(true)
                            .build(),
                        ctx,
                    );
                    // Remove old from map
                    attr_map_sni.borrow_mut().remove(&old_key);
                    Some(js)
                } else {
                    None
                }
            } else {
                None
            };

            // Set the attribute
            super::super::mutation_observer::set_attribute_with_observer(
                ctx,
                &tree_for_sni,
                element_id,
                &name,
                &value,
            );

            // Register the new attr's node_id in the map (if the passed-in attr is a JsElement)
            if let Some(el_data) = attr_obj.downcast_ref::<JsElement>() {
                let attr_node_id = el_data.node_id;
                let new_key = (tree_ptr, element_id, name.clone());
                attr_map_sni.borrow_mut().insert(new_key, attr_node_id);
            }

            // Set ownerElement on the passed-in attr
            let el_obj = get_or_create_js_element(element_id, tree_for_sni.clone(), ctx)?;
            let _ = attr_obj.define_property_or_throw(
                js_string!("ownerElement"),
                PropertyDescriptor::builder()
                    .value(JsValue::from(el_obj))
                    .writable(false)
                    .configurable(true)
                    .enumerable(true)
                    .build(),
                ctx,
            );

            match old_result {
                Some(js) => Ok(js.into()),
                None => Ok(JsValue::null()),
            }
        })
    };
    backing.define_property_or_throw(
        js_string!("__braille_nnm_setNamedItem"),
        PropertyDescriptor::builder()
            .value(set_named_item_fn.to_js_function(&realm))
            .writable(true)
            .configurable(true)
            .enumerable(false)
            .build(),
        context,
    )?;

    // setNamedItemNS(attr) — same as setNamedItem but uses namespace
    let tree_for_snins = tree.clone();
    let attr_map_snins = attr_node_map.clone();
    let set_named_item_ns_fn = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let attr_val = args.first().cloned().unwrap_or(JsValue::undefined());
            let attr_obj = attr_val.as_object().ok_or_else(|| {
                JsError::from_opaque(JsValue::from(js_string!("setNamedItemNS requires an Attr argument")))
            })?;

            let name = attr_obj
                .get(js_string!("name"), ctx)?
                .to_string(ctx)?
                .to_std_string_escaped();
            let value = attr_obj
                .get(js_string!("value"), ctx)?
                .to_string(ctx)?
                .to_std_string_escaped();

            let ns_val = attr_obj.get(js_string!("namespaceURI"), ctx)?;
            let namespace = if ns_val.is_null() || ns_val.is_undefined() {
                String::new()
            } else {
                ns_val.to_string(ctx)?.to_std_string_escaped()
            };

            // Check InUseAttributeError
            let owner = attr_obj.get(js_string!("ownerElement"), ctx)?;
            if !owner.is_null() && !owner.is_undefined() {
                if let Some(owner_obj) = owner.as_object() {
                    let el_obj = get_or_create_js_element(element_id, tree_for_snins.clone(), ctx)?;
                    if !JsObject::equals(&owner_obj, &el_obj) {
                        let exc = super::super::create_dom_exception(
                            ctx,
                            "InUseAttributeError",
                            "The attribute is already in use",
                            10,
                        )?;
                        return Err(JsError::from_opaque(exc.into()));
                    }
                }
            }

            let tree_ptr = Rc::as_ptr(&tree_for_snins) as usize;

            // Find existing old Attr's qualified name
            let local_name_val = attr_obj.get(js_string!("localName"), ctx)?;
            let local_name = local_name_val.to_string(ctx)?.to_std_string_escaped();

            let old_qname = {
                let t = tree_for_snins.borrow();
                let node = t.get_node(element_id);
                match &node.data {
                    NodeData::Element { attributes, .. } => {
                        if namespace.is_empty() {
                            attributes
                                .iter()
                                .find(|a| a.qualified_name() == name || a.local_name == name)
                                .map(|a| a.qualified_name())
                        } else {
                            attributes
                                .iter()
                                .find(|a| a.matches_ns(&namespace, &local_name))
                                .map(|a| a.qualified_name())
                        }
                    }
                    _ => None,
                }
            };

            // Get old Attr JS object from map
            let old_result = if let Some(ref oq) = old_qname {
                let old_key = (tree_ptr, element_id, oq.clone());
                let old_node_id = attr_map_snins.borrow().get(&old_key).copied();
                if let Some(nid) = old_node_id {
                    let js = get_or_create_js_element(nid, tree_for_snins.clone(), ctx)?;
                    let _ = js.define_property_or_throw(
                        js_string!("ownerElement"),
                        PropertyDescriptor::builder()
                            .value(JsValue::null())
                            .writable(false)
                            .configurable(true)
                            .enumerable(true)
                            .build(),
                        ctx,
                    );
                    attr_map_snins.borrow_mut().remove(&old_key);
                    Some(js)
                } else {
                    None
                }
            } else {
                None
            };

            // Set the attribute with namespace
            if namespace.is_empty() {
                super::super::mutation_observer::set_attribute_with_observer(
                    ctx,
                    &tree_for_snins,
                    element_id,
                    &name,
                    &value,
                );
            } else {
                super::super::mutation_observer::set_attribute_ns_with_observer(
                    ctx,
                    &tree_for_snins,
                    element_id,
                    &namespace,
                    &name,
                    &value,
                );
            }

            // Register the new attr's node_id in the map
            if let Some(el_data) = attr_obj.downcast_ref::<JsElement>() {
                let attr_node_id = el_data.node_id;
                let new_key = (tree_ptr, element_id, name.clone());
                attr_map_snins.borrow_mut().insert(new_key, attr_node_id);
            }

            // Set ownerElement on the passed-in attr
            let el_obj = get_or_create_js_element(element_id, tree_for_snins.clone(), ctx)?;
            let _ = attr_obj.define_property_or_throw(
                js_string!("ownerElement"),
                PropertyDescriptor::builder()
                    .value(JsValue::from(el_obj))
                    .writable(false)
                    .configurable(true)
                    .enumerable(true)
                    .build(),
                ctx,
            );

            match old_result {
                Some(js) => Ok(js.into()),
                None => Ok(JsValue::null()),
            }
        })
    };
    backing.define_property_or_throw(
        js_string!("__braille_nnm_setNamedItemNS"),
        PropertyDescriptor::builder()
            .value(set_named_item_ns_fn.to_js_function(&realm))
            .writable(true)
            .configurable(true)
            .enumerable(false)
            .build(),
        context,
    )?;

    // removeNamedItem(name) — backing method
    let tree_for_rni = tree.clone();
    let attr_map_rni = attr_node_map.clone();
    let remove_named_item_fn = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let name = args
                .first()
                .map(|v| v.to_string(ctx))
                .transpose()?
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_default();

            let tree_ptr = Rc::as_ptr(&tree_for_rni) as usize;

            // Find the attribute's qualified name
            let attr_qname = {
                let t = tree_for_rni.borrow();
                let node = t.get_node(element_id);
                match &node.data {
                    NodeData::Element { attributes, .. } => attributes
                        .iter()
                        .find(|a| a.qualified_name() == name || a.local_name == name)
                        .map(|a| a.qualified_name()),
                    _ => None,
                }
            };

            match attr_qname {
                Some(qname) => {
                    // Look up cached Attr JS object
                    let key = (tree_ptr, element_id, qname);
                    let cached_node_id = attr_map_rni.borrow().get(&key).copied();
                    let result_js = if let Some(nid) = cached_node_id {
                        let js = get_or_create_js_element(nid, tree_for_rni.clone(), ctx)?;
                        attr_map_rni.borrow_mut().remove(&key);
                        js
                    } else {
                        // Create a new Attr for the return value
                        let attr_info = {
                            let t = tree_for_rni.borrow();
                            let node = t.get_node(element_id);
                            match &node.data {
                                NodeData::Element { attributes, .. } => attributes
                                    .iter()
                                    .find(|a| a.qualified_name() == name || a.local_name == name)
                                    .map(|a| {
                                        (
                                            a.local_name.clone(),
                                            a.namespace.clone(),
                                            a.prefix.clone(),
                                            a.value.clone(),
                                        )
                                    }),
                                _ => None,
                            }
                        };
                        if let Some((local, ns, prefix, value)) = attr_info {
                            let nid = tree_for_rni.borrow_mut().create_attr(&local, &ns, &prefix, &value);
                            get_or_create_js_element(nid, tree_for_rni.clone(), ctx)?
                        } else {
                            // Should not happen, but fallback
                            let exc = super::super::create_dom_exception(
                                ctx,
                                "NotFoundError",
                                "The attribute was not found",
                                8,
                            )?;
                            return Err(JsError::from_opaque(exc.into()));
                        }
                    };

                    // Remove the attribute from the element
                    super::super::mutation_observer::remove_attribute_with_observer(
                        ctx,
                        &tree_for_rni,
                        element_id,
                        &name,
                    );

                    // Set ownerElement to null on returned attr
                    let _ = result_js.define_property_or_throw(
                        js_string!("ownerElement"),
                        PropertyDescriptor::builder()
                            .value(JsValue::null())
                            .writable(false)
                            .configurable(true)
                            .enumerable(true)
                            .build(),
                        ctx,
                    );

                    Ok(result_js.into())
                }
                None => {
                    let exc = super::super::create_dom_exception(ctx, "NotFoundError", "The attribute was not found", 8)?;
                    Err(JsError::from_opaque(exc.into()))
                }
            }
        })
    };
    backing.define_property_or_throw(
        js_string!("__braille_nnm_removeNamedItem"),
        PropertyDescriptor::builder()
            .value(remove_named_item_fn.to_js_function(&realm))
            .writable(true)
            .configurable(true)
            .enumerable(false)
            .build(),
        context,
    )?;

    // removeNamedItemNS(ns, localName) — backing method
    let tree_for_rnins = tree.clone();
    let attr_map_rnins = attr_node_map.clone();
    let remove_named_item_ns_fn = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let ns_val = args.first().cloned().unwrap_or(JsValue::null());
            let namespace = if ns_val.is_null() || ns_val.is_undefined() {
                String::new()
            } else {
                ns_val.to_string(ctx)?.to_std_string_escaped()
            };

            let local_name = args
                .get(1)
                .map(|v| v.to_string(ctx))
                .transpose()?
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_default();

            let tree_ptr = Rc::as_ptr(&tree_for_rnins) as usize;

            // Find the attribute's qualified name
            let attr_qname = {
                let t = tree_for_rnins.borrow();
                let node = t.get_node(element_id);
                match &node.data {
                    NodeData::Element { attributes, .. } => attributes
                        .iter()
                        .find(|a| a.matches_ns(&namespace, &local_name))
                        .map(|a| a.qualified_name()),
                    _ => None,
                }
            };

            match attr_qname {
                Some(qname) => {
                    let key = (tree_ptr, element_id, qname);
                    let cached_node_id = attr_map_rnins.borrow().get(&key).copied();
                    let result_js = if let Some(nid) = cached_node_id {
                        let js = get_or_create_js_element(nid, tree_for_rnins.clone(), ctx)?;
                        attr_map_rnins.borrow_mut().remove(&key);
                        js
                    } else {
                        let attr_info = {
                            let t = tree_for_rnins.borrow();
                            let node = t.get_node(element_id);
                            match &node.data {
                                NodeData::Element { attributes, .. } => attributes
                                    .iter()
                                    .find(|a| a.matches_ns(&namespace, &local_name))
                                    .map(|a| {
                                        (
                                            a.local_name.clone(),
                                            a.namespace.clone(),
                                            a.prefix.clone(),
                                            a.value.clone(),
                                        )
                                    }),
                                _ => None,
                            }
                        };
                        if let Some((local, ns, prefix, value)) = attr_info {
                            let nid = tree_for_rnins.borrow_mut().create_attr(&local, &ns, &prefix, &value);
                            get_or_create_js_element(nid, tree_for_rnins.clone(), ctx)?
                        } else {
                            let exc = super::super::create_dom_exception(
                                ctx,
                                "NotFoundError",
                                "The attribute was not found",
                                8,
                            )?;
                            return Err(JsError::from_opaque(exc.into()));
                        }
                    };

                    super::super::mutation_observer::remove_attribute_ns_with_observer(
                        ctx,
                        &tree_for_rnins,
                        element_id,
                        &namespace,
                        &local_name,
                    );

                    let _ = result_js.define_property_or_throw(
                        js_string!("ownerElement"),
                        PropertyDescriptor::builder()
                            .value(JsValue::null())
                            .writable(false)
                            .configurable(true)
                            .enumerable(true)
                            .build(),
                        ctx,
                    );

                    Ok(result_js.into())
                }
                None => {
                    let exc = super::super::create_dom_exception(ctx, "NotFoundError", "The attribute was not found", 8)?;
                    Err(JsError::from_opaque(exc.into()))
                }
            }
        })
    };
    backing.define_property_or_throw(
        js_string!("__braille_nnm_removeNamedItemNS"),
        PropertyDescriptor::builder()
            .value(remove_named_item_ns_fn.to_js_function(&realm))
            .writable(true)
            .configurable(true)
            .enumerable(false)
            .build(),
        context,
    )?;

    // Length getter for proxy
    let tree_for_len = tree.clone();
    let length_fn = unsafe {
        NativeFunction::from_closure(move |_this, _args, _ctx| {
            let t = tree_for_len.borrow();
            let node = t.get_node(element_id);
            let len = match &node.data {
                NodeData::Element { attributes, .. } => attributes.len(),
                _ => 0,
            };
            Ok(JsValue::from(len as i32))
        })
    };

    // getChild(index) for proxy — returns Attr or undefined
    let tree_for_get = tree.clone();
    let attr_map_get = attr_node_map.clone();
    let get_child_fn = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let index = args
                .first()
                .map(|v| v.to_number(ctx))
                .transpose()?
                .unwrap_or(0.0) as i64;

            if index < 0 {
                return Ok(JsValue::undefined());
            }

            let tree_ptr = Rc::as_ptr(&tree_for_get) as usize;
            let attr_info = {
                let t = tree_for_get.borrow();
                let node = t.get_node(element_id);
                match &node.data {
                    NodeData::Element { attributes, .. } => attributes.get(index as usize).map(|a| {
                        (
                            a.local_name.clone(),
                            a.namespace.clone(),
                            a.prefix.clone(),
                            a.value.clone(),
                            a.qualified_name(),
                        )
                    }),
                    _ => None,
                }
            };

            match attr_info {
                Some((local, ns, prefix, value, qname)) => {
                    let key = (tree_ptr, element_id, qname);
                    let node_id = {
                        let map = attr_map_get.borrow();
                        map.get(&key).copied()
                    };
                    let node_id = match node_id {
                        Some(id) => {
                            if let NodeData::Attr {
                                value: ref mut v, ..
                            } = tree_for_get.borrow_mut().get_node_mut(id).data
                            {
                                *v = value;
                            }
                            id
                        }
                        None => {
                            let id = tree_for_get.borrow_mut().create_attr(&local, &ns, &prefix, &value);
                            attr_map_get.borrow_mut().insert(key, id);
                            id
                        }
                    };

                    let js_obj = get_or_create_js_element(node_id, tree_for_get.clone(), ctx)?;
                    let el_obj = get_or_create_js_element(element_id, tree_for_get.clone(), ctx)?;
                    let _ = js_obj.define_property_or_throw(
                        js_string!("ownerElement"),
                        PropertyDescriptor::builder()
                            .value(JsValue::from(el_obj))
                            .writable(false)
                            .configurable(true)
                            .enumerable(true)
                            .build(),
                        ctx,
                    );
                    Ok(js_obj.into())
                }
                None => Ok(JsValue::undefined()),
            }
        })
    };

    // getNamed(name) for proxy — attribute access by qualified name
    let tree_for_named = tree.clone();
    let attr_map_named = attr_node_map.clone();
    let get_named_fn = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let name = args
                .first()
                .map(|v| v.to_string(ctx))
                .transpose()?
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_default();

            let tree_ptr = Rc::as_ptr(&tree_for_named) as usize;
            let attr_info = {
                let t = tree_for_named.borrow();
                let node = t.get_node(element_id);
                match &node.data {
                    NodeData::Element { attributes, .. } => attributes
                        .iter()
                        .find(|a| a.qualified_name() == name || a.local_name == name)
                        .map(|a| {
                            (
                                a.local_name.clone(),
                                a.namespace.clone(),
                                a.prefix.clone(),
                                a.value.clone(),
                                a.qualified_name(),
                            )
                        }),
                    _ => None,
                }
            };

            match attr_info {
                Some((local, ns, prefix, value, qname)) => {
                    let key = (tree_ptr, element_id, qname);
                    let node_id = {
                        let map = attr_map_named.borrow();
                        map.get(&key).copied()
                    };
                    let node_id = match node_id {
                        Some(id) => {
                            if let NodeData::Attr {
                                value: ref mut v, ..
                            } = tree_for_named.borrow_mut().get_node_mut(id).data
                            {
                                *v = value;
                            }
                            id
                        }
                        None => {
                            let id = tree_for_named.borrow_mut().create_attr(&local, &ns, &prefix, &value);
                            attr_map_named.borrow_mut().insert(key, id);
                            id
                        }
                    };

                    let js_obj = get_or_create_js_element(node_id, tree_for_named.clone(), ctx)?;
                    let el_obj = get_or_create_js_element(element_id, tree_for_named.clone(), ctx)?;
                    let _ = js_obj.define_property_or_throw(
                        js_string!("ownerElement"),
                        PropertyDescriptor::builder()
                            .value(JsValue::from(el_obj))
                            .writable(false)
                            .configurable(true)
                            .enumerable(true)
                            .build(),
                        ctx,
                    );
                    Ok(js_obj.into())
                }
                None => Ok(JsValue::undefined()),
            }
        })
    };

    // getNamedKeys() for proxy — returns null-separated unique qualified names
    let tree_for_keys = tree.clone();
    let get_named_keys_fn = unsafe {
        NativeFunction::from_closure(move |_this, _args, _ctx| {
            let t = tree_for_keys.borrow();
            let node = t.get_node(element_id);
            match &node.data {
                NodeData::Element { attributes, .. } => {
                    let mut seen = Vec::new();
                    for a in attributes {
                        let qn = a.qualified_name();
                        if !seen.contains(&qn) {
                            seen.push(qn);
                        }
                    }
                    Ok(JsValue::from(js_string!(seen.join("\x00"))))
                }
                _ => Ok(JsValue::from(js_string!(""))),
            }
        })
    };

    // Call the pre-built factory function
    let factory = realm_state::nnm_proxy_factory(context).expect("NamedNodeMap proxy factory not initialized");

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
        .ok_or_else(|| JsError::from_opaque(js_string!("failed to create NamedNodeMap proxy").into()))?
        .clone();

    Ok(proxy_obj)
}
