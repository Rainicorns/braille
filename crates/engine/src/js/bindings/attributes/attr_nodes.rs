use boa_engine::{js_string, Context, JsError, JsResult, JsValue};

use super::super::element::get_or_create_js_element;
use crate::dom::NodeData;

/// Native implementation of element.getAttributeNode(name)
/// Returns an Attr node for the named attribute, or null if not found.
/// Uses the shared attr_node_cache from RealmState for identity.
pub(super) fn get_attribute_node_fn(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "getAttributeNode");
    let name = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    let tree = el.tree.clone();
    let el_id = el.node_id;
    let tree_ptr = std::rc::Rc::as_ptr(&tree) as usize;

    // Find the attribute on this element
    let attr_info = {
        let t = tree.borrow();
        let node = t.get_node(el_id);
        match &node.data {
            NodeData::Element { attributes, .. } => attributes
                .iter()
                .find(|a| a.qualified_name() == name || a.local_name == name)
                .map(|a| (a.qualified_name(), a.local_name.clone(), a.namespace.clone(), a.prefix.clone(), a.value.clone())),
            _ => None,
        }
    };

    match attr_info {
        Some((qname, local_name, namespace, prefix, attr_value)) => {
            let cache = crate::js::realm_state::attr_node_cache(ctx);
            let cache_key = (tree_ptr, el_id, qname.clone());

            // Check cache — if found, update its value and return
            if let Some(&cached_node_id) = cache.borrow().get(&cache_key) {
                // Update the Attr node's value in the tree in case it changed
                if let NodeData::Attr { ref mut value, .. } = tree.borrow_mut().get_node_mut(cached_node_id).data {
                    *value = attr_value;
                }
                let js_obj = get_or_create_js_element(cached_node_id, tree.clone(), ctx)?;
                // Ensure ownerElement is set
                let el_obj = get_or_create_js_element(el_id, tree, ctx)?;
                let _ = js_obj.define_property_or_throw(
                    js_string!("ownerElement"),
                    boa_engine::property::PropertyDescriptor::builder()
                        .value(JsValue::from(el_obj))
                        .writable(false)
                        .configurable(true)
                        .enumerable(true)
                        .build(),
                    ctx,
                );
                return Ok(js_obj.into());
            }

            // Not cached — create new Attr node
            let node_id = tree.borrow_mut().create_attr(&local_name, &namespace, &prefix, &attr_value);
            cache.borrow_mut().insert(cache_key, node_id);
            let js_obj = get_or_create_js_element(node_id, tree.clone(), ctx)?;
            // Set ownerElement
            let el_obj = get_or_create_js_element(el_id, tree, ctx)?;
            let _ = js_obj.define_property_or_throw(
                js_string!("ownerElement"),
                boa_engine::property::PropertyDescriptor::builder()
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
}

/// Native implementation of element.getAttributeNodeNS(namespace, localName)
/// Returns an Attr node for the named attribute, or null if not found.
/// Uses the shared attr_node_cache from RealmState for identity.
pub(super) fn get_attribute_node_ns_fn(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "getAttributeNodeNS");

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

    let tree = el.tree.clone();
    let el_id = el.node_id;
    let tree_ptr = std::rc::Rc::as_ptr(&tree) as usize;

    // Find the attribute on this element by namespace + localName
    let attr_info = {
        let t = tree.borrow();
        let node = t.get_node(el_id);
        match &node.data {
            NodeData::Element { attributes, .. } => attributes
                .iter()
                .find(|a| a.matches_ns(&namespace, &local_name))
                .map(|a| {
                    (
                        a.qualified_name(),
                        a.local_name.clone(),
                        a.namespace.clone(),
                        a.prefix.clone(),
                        a.value.clone(),
                    )
                }),
            _ => None,
        }
    };

    match attr_info {
        Some((qname, attr_local, attr_ns, attr_prefix, attr_value)) => {
            let cache = crate::js::realm_state::attr_node_cache(ctx);
            let cache_key = (tree_ptr, el_id, qname);

            // Check cache
            if let Some(&cached_node_id) = cache.borrow().get(&cache_key) {
                if let NodeData::Attr { ref mut value, .. } = tree.borrow_mut().get_node_mut(cached_node_id).data {
                    *value = attr_value;
                }
                let js_obj = get_or_create_js_element(cached_node_id, tree.clone(), ctx)?;
                let el_obj = get_or_create_js_element(el_id, tree, ctx)?;
                let _ = js_obj.define_property_or_throw(
                    js_string!("ownerElement"),
                    boa_engine::property::PropertyDescriptor::builder()
                        .value(JsValue::from(el_obj))
                        .writable(false)
                        .configurable(true)
                        .enumerable(true)
                        .build(),
                    ctx,
                );
                return Ok(js_obj.into());
            }

            // Not cached — create new Attr node
            let node_id = tree
                .borrow_mut()
                .create_attr(&attr_local, &attr_ns, &attr_prefix, &attr_value);
            cache.borrow_mut().insert(cache_key, node_id);
            let js_obj = get_or_create_js_element(node_id, tree.clone(), ctx)?;
            let el_obj = get_or_create_js_element(el_id, tree, ctx)?;
            let _ = js_obj.define_property_or_throw(
                js_string!("ownerElement"),
                boa_engine::property::PropertyDescriptor::builder()
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
}

/// Native implementation of element.setAttributeNode(attr)
/// Per spec: takes an Attr node, sets the attribute on the element, returns the old Attr (or null).
/// Uses the shared attr_node_cache from RealmState to maintain Attr identity.
pub(super) fn set_attribute_node_fn(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "setAttributeNode");

    let attr_val = args.first().cloned().unwrap_or(JsValue::undefined());
    let attr_obj = attr_val.as_object().ok_or_else(|| {
        JsError::from_opaque(JsValue::from(js_string!("setAttributeNode requires an Attr argument")))
    })?;

    // Check InUseAttributeError: if ownerElement is not null and not this element
    let owner = attr_obj.get(js_string!("ownerElement"), ctx)?;
    if !owner.is_null() && !owner.is_undefined() {
        if let Some(owner_obj) = owner.as_object() {
            let this_obj = get_or_create_js_element(el.node_id, el.tree.clone(), ctx)?;
            if !boa_engine::JsObject::equals(&owner_obj, &this_obj) {
                let exc =
                    super::super::create_dom_exception(ctx, "InUseAttributeError", "The attribute is already in use", 10)?;
                return Err(JsError::from_opaque(exc.into()));
            }
        }
    }

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

    let local_name_val = attr_obj.get(js_string!("localName"), ctx)?;
    let local_name = if local_name_val.is_null() || local_name_val.is_undefined() {
        name.clone()
    } else {
        local_name_val.to_string(ctx)?.to_std_string_escaped()
    };

    let tree = el.tree.clone();
    let el_id = el.node_id;
    let tree_ptr = std::rc::Rc::as_ptr(&tree) as usize;
    let cache = crate::js::realm_state::attr_node_cache(ctx);
    let cache_key = (tree_ptr, el_id, name.clone());

    // Find old Attr's cached node id (if any)
    let old_cached = cache.borrow().get(&cache_key).copied();

    // Find existing attribute info to return as old — match by NS if available, else by name
    let old_attr_info = {
        let t = tree.borrow();
        let node = t.get_node(el_id);
        match &node.data {
            NodeData::Element { attributes, .. } => {
                if !namespace.is_empty() {
                    attributes
                        .iter()
                        .find(|a| a.matches_ns(&namespace, &local_name))
                        .map(|a| (a.qualified_name(), a.local_name.clone(), a.namespace.clone(), a.prefix.clone(), a.value.clone()))
                } else {
                    attributes
                        .iter()
                        .find(|a| a.qualified_name() == name || a.local_name == name)
                        .map(|a| (a.qualified_name(), a.local_name.clone(), a.namespace.clone(), a.prefix.clone(), a.value.clone()))
                }
            }
            _ => None,
        }
    };

    // Use namespaced setter if the Attr has a namespace
    if !namespace.is_empty() {
        super::super::mutation_observer::set_attribute_ns_with_observer(ctx, &tree, el_id, &namespace, &name, &value);
    } else {
        super::super::mutation_observer::set_attribute_with_observer(ctx, &tree, el_id, &name, &value);
    }

    // Extract the new Attr's node_id from the JsObject (via downcast_ref)
    let new_node_id = attr_obj
        .downcast_ref::<super::super::element::JsElement>()
        .map(|js_el| js_el.node_id);

    // Update shared cache: point cache key to the new Attr's node_id
    if let Some(nid) = new_node_id {
        cache.borrow_mut().insert(cache_key.clone(), nid);
    }

    // Set ownerElement on new Attr
    let el_js = get_or_create_js_element(el_id, tree.clone(), ctx)?;
    let _ = attr_obj.define_property_or_throw(
        js_string!("ownerElement"),
        boa_engine::property::PropertyDescriptor::builder()
            .value(JsValue::from(el_js))
            .writable(false)
            .configurable(true)
            .enumerable(true)
            .build(),
        ctx,
    );

    // Return old Attr
    match old_attr_info {
        Some((qname, local, ns, prefix, old_val)) => {
            // If we have a cached old Attr node, return it (with ownerElement set to null)
            if let Some(old_nid) = old_cached {
                // Update old Attr node's value in tree
                if let NodeData::Attr { ref mut value, .. } = tree.borrow_mut().get_node_mut(old_nid).data {
                    *value = old_val;
                }
                let old_js = get_or_create_js_element(old_nid, tree.clone(), ctx)?;
                // Remove old cache entry if qname != new name
                if qname != name {
                    cache.borrow_mut().remove(&(tree_ptr, el_id, qname));
                }
                let _ = old_js.define_property_or_throw(
                    js_string!("ownerElement"),
                    boa_engine::property::PropertyDescriptor::builder()
                        .value(JsValue::null())
                        .writable(false)
                        .configurable(true)
                        .enumerable(true)
                        .build(),
                    ctx,
                );
                Ok(old_js.into())
            } else {
                // No cached Attr — create a new one for the return value
                let node_id = tree.borrow_mut().create_attr(&local, &ns, &prefix, &old_val);
                let js_obj = get_or_create_js_element(node_id, tree.clone(), ctx)?;
                Ok(js_obj.into())
            }
        }
        None => Ok(JsValue::null()),
    }
}

/// Native implementation of element.setAttributeNodeNS(attr)
/// Same as setAttributeNode but handles namespace.
/// Uses the shared attr_node_cache from RealmState to maintain Attr identity.
pub(super) fn set_attribute_node_ns_fn(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "setAttributeNodeNS");

    let attr_val = args.first().cloned().unwrap_or(JsValue::undefined());
    let attr_obj = attr_val.as_object().ok_or_else(|| {
        JsError::from_opaque(JsValue::from(js_string!("setAttributeNodeNS requires an Attr argument")))
    })?;

    // Check InUseAttributeError
    let owner = attr_obj.get(js_string!("ownerElement"), ctx)?;
    if !owner.is_null() && !owner.is_undefined() {
        if let Some(owner_obj) = owner.as_object() {
            let this_obj = get_or_create_js_element(el.node_id, el.tree.clone(), ctx)?;
            if !boa_engine::JsObject::equals(&owner_obj, &this_obj) {
                let exc =
                    super::super::create_dom_exception(ctx, "InUseAttributeError", "The attribute is already in use", 10)?;
                return Err(JsError::from_opaque(exc.into()));
            }
        }
    }

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

    let local_name = attr_obj
        .get(js_string!("localName"), ctx)?
        .to_string(ctx)?
        .to_std_string_escaped();

    let tree = el.tree.clone();
    let el_id = el.node_id;
    let tree_ptr = std::rc::Rc::as_ptr(&tree) as usize;
    let cache = crate::js::realm_state::attr_node_cache(ctx);

    // Find existing attribute to return as old
    let old_attr = {
        let t = tree.borrow();
        let node = t.get_node(el_id);
        match &node.data {
            NodeData::Element { attributes, .. } => {
                if namespace.is_empty() {
                    attributes
                        .iter()
                        .find(|a| a.qualified_name() == name || a.local_name == name)
                        .map(|a| (a.qualified_name(), a.local_name.clone(), a.namespace.clone(), a.prefix.clone(), a.value.clone()))
                } else {
                    attributes
                        .iter()
                        .find(|a| a.matches_ns(&namespace, &local_name))
                        .map(|a| (a.qualified_name(), a.local_name.clone(), a.namespace.clone(), a.prefix.clone(), a.value.clone()))
                }
            }
            _ => None,
        }
    };

    // Get old cached Attr node id before we modify cache
    let old_cache_key = old_attr.as_ref().map(|(qname, ..)| (tree_ptr, el_id, qname.clone()));
    let old_cached_nid = old_cache_key.as_ref().and_then(|k| cache.borrow().get(k).copied());

    if namespace.is_empty() {
        super::super::mutation_observer::set_attribute_with_observer(ctx, &tree, el_id, &name, &value);
    } else {
        super::super::mutation_observer::set_attribute_ns_with_observer(ctx, &tree, el_id, &namespace, &name, &value);
    }

    // Update shared cache with new Attr identity
    let new_node_id = attr_obj
        .downcast_ref::<super::super::element::JsElement>()
        .map(|js_el| js_el.node_id);
    let new_cache_key = (tree_ptr, el_id, name.clone());
    if let Some(nid) = new_node_id {
        cache.borrow_mut().insert(new_cache_key, nid);
    }

    // Set ownerElement on new Attr
    let el_js = get_or_create_js_element(el_id, tree.clone(), ctx)?;
    let _ = attr_obj.define_property_or_throw(
        js_string!("ownerElement"),
        boa_engine::property::PropertyDescriptor::builder()
            .value(JsValue::from(el_js))
            .writable(false)
            .configurable(true)
            .enumerable(true)
            .build(),
        ctx,
    );

    // Return old Attr
    match old_attr {
        Some((qname, local, ns, prefix, old_val)) => {
            if let Some(old_nid) = old_cached_nid {
                if let NodeData::Attr { ref mut value, .. } = tree.borrow_mut().get_node_mut(old_nid).data {
                    *value = old_val;
                }
                let old_js = get_or_create_js_element(old_nid, tree.clone(), ctx)?;
                // Remove old cache entry if different qname
                if qname != name {
                    cache.borrow_mut().remove(&(tree_ptr, el_id, qname));
                }
                let _ = old_js.define_property_or_throw(
                    js_string!("ownerElement"),
                    boa_engine::property::PropertyDescriptor::builder()
                        .value(JsValue::null())
                        .writable(false)
                        .configurable(true)
                        .enumerable(true)
                        .build(),
                    ctx,
                );
                Ok(old_js.into())
            } else {
                let node_id = tree.borrow_mut().create_attr(&local, &ns, &prefix, &old_val);
                let js_obj = get_or_create_js_element(node_id, tree.clone(), ctx)?;
                Ok(js_obj.into())
            }
        }
        None => Ok(JsValue::null()),
    }
}

/// Native implementation of element.removeAttributeNode(attr)
/// Per spec: removes the attribute matching the Attr node, returns the removed Attr.
/// Throws NotFoundError if not found.
/// Cleans up the shared attr_node_cache.
pub(super) fn remove_attribute_node_fn(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "removeAttributeNode");

    let attr_val = args.first().cloned().unwrap_or(JsValue::undefined());
    let attr_obj = attr_val.as_object().ok_or_else(|| {
        JsError::from_opaque(JsValue::from(js_string!("removeAttributeNode requires an Attr argument")))
    })?;

    let ns_val = attr_obj.get(js_string!("namespaceURI"), ctx)?;
    let namespace = if ns_val.is_null() || ns_val.is_undefined() {
        String::new()
    } else {
        ns_val.to_string(ctx)?.to_std_string_escaped()
    };
    let local_name = attr_obj
        .get(js_string!("localName"), ctx)?
        .to_string(ctx)?
        .to_std_string_escaped();
    let name = attr_obj
        .get(js_string!("name"), ctx)?
        .to_string(ctx)?
        .to_std_string_escaped();

    let tree = el.tree.clone();
    let el_id = el.node_id;
    let tree_ptr = std::rc::Rc::as_ptr(&tree) as usize;

    // Find the qualified name of the matching attribute
    let found_qname = {
        let t = tree.borrow();
        let node = t.get_node(el_id);
        match &node.data {
            NodeData::Element { attributes, .. } => {
                if !namespace.is_empty() {
                    attributes.iter().find(|a| a.matches_ns(&namespace, &local_name)).map(|a| a.qualified_name())
                } else {
                    attributes
                        .iter()
                        .find(|a| a.qualified_name() == name || a.local_name == local_name)
                        .map(|a| a.qualified_name())
                }
            }
            _ => None,
        }
    };

    let qname = match found_qname {
        Some(q) => q,
        None => {
            let exc = super::super::create_dom_exception(ctx, "NotFoundError", "The attribute was not found", 8)?;
            return Err(JsError::from_opaque(exc.into()));
        }
    };

    // Remove it
    if !namespace.is_empty() {
        super::super::mutation_observer::remove_attribute_ns_with_observer(ctx, &tree, el_id, &namespace, &local_name);
    } else {
        super::super::mutation_observer::remove_attribute_with_observer(ctx, &tree, el_id, &name);
    }

    // Clean up shared attr_node_cache
    let cache = crate::js::realm_state::attr_node_cache(ctx);
    cache.borrow_mut().remove(&(tree_ptr, el_id, qname));

    // Return the Attr node that was passed in (it's now detached)
    // Set ownerElement to null
    let _ = attr_obj.define_property_or_throw(
        js_string!("ownerElement"),
        boa_engine::property::PropertyDescriptor::builder()
            .value(JsValue::null())
            .writable(false)
            .configurable(true)
            .enumerable(true)
            .build(),
        ctx,
    );
    Ok(attr_val)
}
