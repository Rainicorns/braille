use boa_engine::{
    class::ClassBuilder, js_string, native_function::NativeFunction, property::Attribute, Context, JsError, JsResult,
    JsValue,
};

use super::element::get_or_create_js_element;
use crate::dom::NodeData;

/// Register all attribute methods and properties on the Element class.
pub(crate) fn register_attributes(class: &mut ClassBuilder) -> JsResult<()> {
    // Register methods
    class.method(
        js_string!("getAttribute"),
        1,
        NativeFunction::from_fn_ptr(get_attribute_fn),
    );

    class.method(
        js_string!("setAttribute"),
        2,
        NativeFunction::from_fn_ptr(set_attribute_fn),
    );

    class.method(
        js_string!("removeAttribute"),
        1,
        NativeFunction::from_fn_ptr(remove_attribute_fn),
    );

    class.method(
        js_string!("hasAttribute"),
        1,
        NativeFunction::from_fn_ptr(has_attribute_fn),
    );

    class.method(
        js_string!("getAttributeNode"),
        1,
        NativeFunction::from_fn_ptr(get_attribute_node_fn),
    );

    class.method(
        js_string!("getAttributeNodeNS"),
        2,
        NativeFunction::from_fn_ptr(get_attribute_node_ns_fn),
    );

    class.method(
        js_string!("setAttributeNS"),
        3,
        NativeFunction::from_fn_ptr(set_attribute_ns_fn),
    );

    class.method(
        js_string!("getAttributeNS"),
        2,
        NativeFunction::from_fn_ptr(get_attribute_ns_fn),
    );

    class.method(
        js_string!("removeAttributeNS"),
        2,
        NativeFunction::from_fn_ptr(remove_attribute_ns_fn),
    );

    class.method(
        js_string!("hasAttributeNS"),
        2,
        NativeFunction::from_fn_ptr(has_attribute_ns_fn),
    );

    class.method(
        js_string!("hasAttributes"),
        0,
        NativeFunction::from_fn_ptr(has_attributes_fn),
    );

    class.method(
        js_string!("toggleAttribute"),
        1,
        NativeFunction::from_fn_ptr(toggle_attribute_fn),
    );

    class.method(
        js_string!("setAttributeNode"),
        1,
        NativeFunction::from_fn_ptr(set_attribute_node_fn),
    );

    class.method(
        js_string!("setAttributeNodeNS"),
        1,
        NativeFunction::from_fn_ptr(set_attribute_node_ns_fn),
    );

    class.method(
        js_string!("removeAttributeNode"),
        1,
        NativeFunction::from_fn_ptr(remove_attribute_node_fn),
    );

    class.method(
        js_string!("getAttributeNames"),
        0,
        NativeFunction::from_fn_ptr(get_attribute_names_fn),
    );

    // Register properties (id and className)
    let realm = class.context().realm().clone();

    let id_getter = NativeFunction::from_fn_ptr(get_id);
    let id_setter = NativeFunction::from_fn_ptr(set_id);

    class.accessor(
        js_string!("id"),
        Some(id_getter.to_js_function(&realm)),
        Some(id_setter.to_js_function(&realm)),
        Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    );

    let class_getter = NativeFunction::from_fn_ptr(get_class_name);
    let class_setter = NativeFunction::from_fn_ptr(set_class_name);

    class.accessor(
        js_string!("className"),
        Some(class_getter.to_js_function(&realm)),
        Some(class_setter.to_js_function(&realm)),
        Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    );

    Ok(())
}

/// Native implementation of element.getAttribute(name)
fn get_attribute_fn(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "getAttribute");
    let name = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    // Per spec, lowercase the name for HTML elements in HTML documents
    let tree = el.tree.borrow();
    let name = if tree.is_html_document() {
        name.to_ascii_lowercase()
    } else {
        name
    };
    match tree.get_attribute(el.node_id, &name) {
        Some(val) => Ok(JsValue::from(js_string!(val))),
        None => Ok(JsValue::null()),
    }
}

/// Native implementation of element.setAttribute(name, value)
fn set_attribute_fn(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "setAttribute");
    let name = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    // Validate the attribute name per spec
    if !crate::dom::is_valid_attribute_name(&name) {
        let exc = super::create_dom_exception(ctx, "InvalidCharacterError", "String contains an invalid character", 5)?;
        return Err(JsError::from_opaque(exc.into()));
    }

    let value = args
        .get(1)
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    // Per spec, lowercase the name for HTML elements in HTML documents
    let name = if el.tree.borrow().is_html_document() {
        name.to_ascii_lowercase()
    } else {
        name
    };
    super::mutation_observer::set_attribute_with_observer(ctx, &el.tree, el.node_id, &name, &value);
    // Compile inline event handler if this is an on* attribute
    if name.starts_with("on") && name.len() > 2 {
        let tree_ptr = std::rc::Rc::as_ptr(&el.tree) as usize;
        let node_id = el.node_id;
        super::on_event::compile_inline_event_handler(tree_ptr, node_id, &name[2..], &value, ctx);
    }
    Ok(JsValue::undefined())
}

/// Native implementation of element.removeAttribute(name)
fn remove_attribute_fn(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "removeAttribute");
    let name = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    // Per spec, lowercase the name for HTML elements in HTML documents
    let name = if el.tree.borrow().is_html_document() {
        name.to_ascii_lowercase()
    } else {
        name
    };
    super::mutation_observer::remove_attribute_with_observer(ctx, &el.tree, el.node_id, &name);
    // Clean up shared attr_node_cache and set ownerElement to null on cached Attr
    {
        let cache = crate::js::realm_state::attr_node_cache(ctx);
        let tree_ptr = std::rc::Rc::as_ptr(&el.tree) as usize;
        let cache_key = (tree_ptr, el.node_id, name.clone());
        let removed_nid = cache.borrow_mut().remove(&cache_key);
        if let Some(attr_nid) = removed_nid {
            if let Ok(attr_js) = get_or_create_js_element(attr_nid, el.tree.clone(), ctx) {
                let _ = attr_js.define_property_or_throw(
                    js_string!("ownerElement"),
                    boa_engine::property::PropertyDescriptor::builder()
                        .value(JsValue::null())
                        .writable(false)
                        .configurable(true)
                        .enumerable(true)
                        .build(),
                    ctx,
                );
            }
        }
    }
    // Clear inline event handler if this is an on* attribute
    if name.starts_with("on") && name.len() > 2 {
        if let Some(interned) = super::on_event::intern_event_name(&name[2..]) {
            let tree_ptr = std::rc::Rc::as_ptr(&el.tree) as usize;
            super::on_event::set_on_event_handler(tree_ptr, el.node_id, interned, None, ctx);
        }
    }
    Ok(JsValue::undefined())
}

/// Native implementation of element.hasAttribute(name)
fn has_attribute_fn(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "hasAttribute");
    let name = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    // Per spec, lowercase the name for HTML elements in HTML documents
    let tree = el.tree.borrow();
    let name = if tree.is_html_document() {
        name.to_ascii_lowercase()
    } else {
        name
    };
    let has_attr = tree.has_attribute(el.node_id, &name);
    Ok(JsValue::from(has_attr))
}

/// Native implementation of element.getAttributeNode(name)
/// Returns an Attr node for the named attribute, or null if not found.
/// Uses the shared attr_node_cache from RealmState for identity.
fn get_attribute_node_fn(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
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
fn get_attribute_node_ns_fn(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
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

/// Native implementation of element.setAttributeNS(namespace, qualifiedName, value)
fn set_attribute_ns_fn(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "setAttributeNS");

    let ns_val = args.first().cloned().unwrap_or(JsValue::null());
    let namespace = if ns_val.is_null() || ns_val.is_undefined() {
        String::new()
    } else {
        ns_val.to_string(ctx)?.to_std_string_escaped()
    };

    let qualified_name = args
        .get(1)
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    // Validate the qualified name for attribute names
    if let Some(colon_pos) = qualified_name.find(':') {
        let prefix_part = &qualified_name[..colon_pos];
        let local_part = &qualified_name[colon_pos + 1..];
        let invalid_prefix =
            prefix_part.is_empty() || prefix_part.contains(['\0', '\t', '\n', '\x0C', '\r', ' ', '/', '>']);
        if invalid_prefix || !crate::dom::is_valid_attribute_name(local_part) {
            let exc =
                super::create_dom_exception(ctx, "InvalidCharacterError", "String contains an invalid character", 5)?;
            return Err(JsError::from_opaque(exc.into()));
        }
    } else if !crate::dom::is_valid_attribute_name(&qualified_name) {
        let exc = super::create_dom_exception(ctx, "InvalidCharacterError", "String contains an invalid character", 5)?;
        return Err(JsError::from_opaque(exc.into()));
    }

    // Namespace validation per spec (https://dom.spec.whatwg.org/#validate-and-extract)
    {
        let has_prefix = qualified_name.contains(':');
        let prefix = if has_prefix {
            qualified_name.split(':').next().unwrap_or("")
        } else {
            ""
        };
        let xml_ns = "http://www.w3.org/XML/1998/namespace";
        let xmlns_ns = "http://www.w3.org/2000/xmlns/";

        // 1. If prefix is present but namespace is empty → NamespaceError
        if has_prefix && namespace.is_empty() {
            let exc = super::create_dom_exception(ctx, "NamespaceError", "Namespace must not be empty when prefix is used", 14)?;
            return Err(JsError::from_opaque(exc.into()));
        }
        // 2. If prefix is "xml" and namespace is not the XML namespace → NamespaceError
        if prefix == "xml" && namespace != xml_ns {
            let exc = super::create_dom_exception(ctx, "NamespaceError", "The xml prefix requires the XML namespace", 14)?;
            return Err(JsError::from_opaque(exc.into()));
        }
        // 3. If prefix is "xmlns" or qualifiedName is "xmlns", namespace must be XMLNS
        if (prefix == "xmlns" || qualified_name == "xmlns") && namespace != xmlns_ns {
            let exc = super::create_dom_exception(ctx, "NamespaceError", "The xmlns prefix/name requires the XMLNS namespace", 14)?;
            return Err(JsError::from_opaque(exc.into()));
        }
        // 4. If namespace is XMLNS, prefix must be "xmlns" or qualifiedName must be "xmlns"
        if namespace == xmlns_ns && prefix != "xmlns" && qualified_name != "xmlns" {
            let exc = super::create_dom_exception(ctx, "NamespaceError", "XMLNS namespace requires xmlns prefix or name", 14)?;
            return Err(JsError::from_opaque(exc.into()));
        }
    }

    let value = args
        .get(2)
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    super::mutation_observer::set_attribute_ns_with_observer(
        ctx,
        &el.tree,
        el.node_id,
        &namespace,
        &qualified_name,
        &value,
    );
    Ok(JsValue::undefined())
}

/// Native implementation of element.getAttributeNS(namespace, localName)
fn get_attribute_ns_fn(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "getAttributeNS");

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

    let tree = el.tree.borrow();
    match tree.get_attribute_ns(el.node_id, &namespace, &local_name) {
        Some(val) => Ok(JsValue::from(js_string!(val))),
        None => Ok(JsValue::null()),
    }
}

/// Native implementation of element.removeAttributeNS(namespace, localName)
fn remove_attribute_ns_fn(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "removeAttributeNS");

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

    // Get the qualified name before removing (for cache cleanup)
    let qname_for_cache = {
        let t = el.tree.borrow();
        let node = t.get_node(el.node_id);
        match &node.data {
            NodeData::Element { attributes, .. } => attributes
                .iter()
                .find(|a| a.matches_ns(&namespace, &local_name))
                .map(|a| a.qualified_name()),
            _ => None,
        }
    };
    super::mutation_observer::remove_attribute_ns_with_observer(ctx, &el.tree, el.node_id, &namespace, &local_name);
    // Clean up shared attr_node_cache
    if let Some(qname) = qname_for_cache {
        let cache = crate::js::realm_state::attr_node_cache(ctx);
        let tree_ptr = std::rc::Rc::as_ptr(&el.tree) as usize;
        cache.borrow_mut().remove(&(tree_ptr, el.node_id, qname));
    }
    Ok(JsValue::undefined())
}

/// Native implementation of element.hasAttributeNS(namespace, localName)
fn has_attribute_ns_fn(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "hasAttributeNS");

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

    let tree = el.tree.borrow();
    let has = tree.has_attribute_ns(el.node_id, &namespace, &local_name);
    Ok(JsValue::from(has))
}

/// Native implementation of element.hasAttributes()
fn has_attributes_fn(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "hasAttributes");

    let tree = el.tree.borrow();
    let has = tree.has_attributes(el.node_id);
    Ok(JsValue::from(has))
}

/// Native implementation of element.toggleAttribute(qualifiedName, force?)
fn toggle_attribute_fn(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "toggleAttribute");
    let name = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    // Validate the attribute name per spec
    if !crate::dom::is_valid_attribute_name(&name) {
        let exc = super::create_dom_exception(ctx, "InvalidCharacterError", "String contains an invalid character", 5)?;
        return Err(JsError::from_opaque(exc.into()));
    }

    // Per spec, lowercase the name for HTML elements in HTML documents
    let name = if el.tree.borrow().is_html_document() {
        name.to_ascii_lowercase()
    } else {
        name
    };

    let force_arg = args.get(1);
    let has_force = force_arg.is_some_and(|v| !v.is_undefined());

    let has_attr = el.tree.borrow().has_attribute(el.node_id, &name);

    if has_attr {
        if has_force && force_arg.unwrap().to_boolean() {
            // force=true and attribute exists: keep it, return true
            Ok(JsValue::from(true))
        } else if !has_force || !force_arg.unwrap().to_boolean() {
            // force missing or false: remove the attribute, return false
            super::mutation_observer::remove_attribute_with_observer(ctx, &el.tree, el.node_id, &name);
            Ok(JsValue::from(false))
        } else {
            Ok(JsValue::from(true))
        }
    } else if !has_force || force_arg.unwrap().to_boolean() {
        // force missing or true: add the attribute with empty value, return true
        super::mutation_observer::set_attribute_with_observer(ctx, &el.tree, el.node_id, &name, "");
        Ok(JsValue::from(true))
    } else {
        // force=false and attribute doesn't exist: return false
        Ok(JsValue::from(false))
    }
}

/// Native implementation of element.setAttributeNode(attr)
/// Per spec: takes an Attr node, sets the attribute on the element, returns the old Attr (or null).
/// Uses the shared attr_node_cache from RealmState to maintain Attr identity.
fn set_attribute_node_fn(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
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
                    super::create_dom_exception(ctx, "InUseAttributeError", "The attribute is already in use", 10)?;
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
        super::mutation_observer::set_attribute_ns_with_observer(ctx, &tree, el_id, &namespace, &name, &value);
    } else {
        super::mutation_observer::set_attribute_with_observer(ctx, &tree, el_id, &name, &value);
    }

    // Extract the new Attr's node_id from the JsObject (via downcast_ref)
    let new_node_id = attr_obj
        .downcast_ref::<super::element::JsElement>()
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
fn set_attribute_node_ns_fn(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
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
                    super::create_dom_exception(ctx, "InUseAttributeError", "The attribute is already in use", 10)?;
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
        super::mutation_observer::set_attribute_with_observer(ctx, &tree, el_id, &name, &value);
    } else {
        super::mutation_observer::set_attribute_ns_with_observer(ctx, &tree, el_id, &namespace, &name, &value);
    }

    // Update shared cache with new Attr identity
    let new_node_id = attr_obj
        .downcast_ref::<super::element::JsElement>()
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
fn remove_attribute_node_fn(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
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
            let exc = super::create_dom_exception(ctx, "NotFoundError", "The attribute was not found", 8)?;
            return Err(JsError::from_opaque(exc.into()));
        }
    };

    // Remove it
    if !namespace.is_empty() {
        super::mutation_observer::remove_attribute_ns_with_observer(ctx, &tree, el_id, &namespace, &local_name);
    } else {
        super::mutation_observer::remove_attribute_with_observer(ctx, &tree, el_id, &name);
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

/// Native implementation of element.getAttributeNames()
/// Returns a JS Array of qualified attribute names in order.
fn get_attribute_names_fn(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "getAttributeNames");

    let tree = el.tree.borrow();
    let node = tree.get_node(el.node_id);

    let names: Vec<JsValue> = match &node.data {
        NodeData::Element { attributes, .. } => attributes
            .iter()
            .map(|a| JsValue::from(js_string!(a.qualified_name())))
            .collect(),
        _ => Vec::new(),
    };

    let arr = boa_engine::object::builtins::JsArray::from_iter(names, ctx);
    Ok(arr.into())
}

/// Native getter for element.id
fn get_id(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "id getter");

    let tree = el.tree.borrow();
    match tree.get_attribute(el.node_id, "id") {
        Some(val) => Ok(JsValue::from(js_string!(val))),
        None => Ok(JsValue::from(js_string!(""))),
    }
}

/// Native setter for element.id
fn set_id(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "id setter");
    let value = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    super::mutation_observer::set_attribute_with_observer(ctx, &el.tree, el.node_id, "id", &value);
    Ok(JsValue::undefined())
}

/// Native getter for element.className
fn get_class_name(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "className getter");

    let tree = el.tree.borrow();
    match tree.get_attribute(el.node_id, "class") {
        Some(val) => Ok(JsValue::from(js_string!(val))),
        None => Ok(JsValue::from(js_string!(""))),
    }
}

/// Native setter for element.className
fn set_class_name(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "className setter");
    let value = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    super::mutation_observer::set_attribute_with_observer(ctx, &el.tree, el.node_id, "class", &value);
    Ok(JsValue::undefined())
}

#[cfg(test)]
mod tests {
    use crate::dom::{DomTree, NodeData};
    use crate::js::JsRuntime;
    use std::cell::RefCell;
    use std::rc::Rc;

    // NOTE: These tests require register_attributes() to be called from element.rs
    // in the Element::init() method. Until that integration is complete, these tests
    // will fail because the attribute methods/properties won't be registered on
    // the Element class.

    /// Helper: build a DomTree with document > html > body > div#app
    fn make_test_tree() -> Rc<RefCell<DomTree>> {
        let tree = Rc::new(RefCell::new(DomTree::new()));
        {
            let mut t = tree.borrow_mut();
            let html = t.create_element("html");
            let body = t.create_element("body");
            let div = t.create_element("div");

            // Set id="app" on the div
            if let NodeData::Element { ref mut attributes, .. } = t.get_node_mut(div).data {
                attributes.push(crate::dom::node::DomAttribute::new("id", "app"));
            }

            let doc = t.document();
            t.append_child(doc, html);
            t.append_child(html, body);
            t.append_child(body, div);
        }
        tree
    }

    #[test]
    fn get_attribute_returns_value() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        let result = rt
            .eval(
                r#"
            var el = document.getElementById("app");
            el.getAttribute("id");
        "#,
            )
            .unwrap();

        let value = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(value, "app");
    }

    #[test]
    fn get_attribute_returns_null_for_missing() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        let result = rt
            .eval(
                r#"
            var el = document.getElementById("app");
            el.getAttribute("nonexistent");
        "#,
            )
            .unwrap();

        assert!(result.is_null());
    }

    #[test]
    fn set_attribute_creates_new_attribute() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        rt.eval(
            r#"
            var el = document.getElementById("app");
            el.setAttribute("data-x", "hello");
        "#,
        )
        .unwrap();

        // Verify via DomTree
        let t = tree.borrow();
        let div_id = 3; // div#app
        assert_eq!(t.get_attribute(div_id, "data-x"), Some("hello".to_string()));
    }

    #[test]
    fn set_attribute_then_get_attribute() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        let result = rt
            .eval(
                r#"
            var el = document.getElementById("app");
            el.setAttribute("data-x", "hello");
            el.getAttribute("data-x");
        "#,
            )
            .unwrap();

        let value = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(value, "hello");
    }

    #[test]
    fn remove_attribute_removes_attribute() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        rt.eval(
            r#"
            var el = document.getElementById("app");
            el.removeAttribute("id");
        "#,
        )
        .unwrap();

        // Verify via DomTree
        let t = tree.borrow();
        let div_id = 3; // div#app
        assert_eq!(t.get_attribute(div_id, "id"), None);
    }

    #[test]
    fn has_attribute_returns_true_for_existing() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        let result = rt
            .eval(
                r#"
            var el = document.getElementById("app");
            el.hasAttribute("id");
        "#,
            )
            .unwrap();

        assert_eq!(result.as_boolean(), Some(true));
    }

    #[test]
    fn has_attribute_returns_false_for_missing() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        let result = rt
            .eval(
                r#"
            var el = document.getElementById("app");
            el.hasAttribute("nonexistent");
        "#,
            )
            .unwrap();

        assert_eq!(result.as_boolean(), Some(false));
    }

    #[test]
    fn id_getter_returns_id_attribute() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        let result = rt
            .eval(
                r#"
            var el = document.getElementById("app");
            el.id;
        "#,
            )
            .unwrap();

        let value = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(value, "app");
    }

    #[test]
    fn id_setter_updates_id_attribute() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        rt.eval(
            r#"
            var el = document.getElementById("app");
            el.id = "newId";
        "#,
        )
        .unwrap();

        // Verify via DomTree
        let t = tree.borrow();
        let div_id = 3; // div#app
        assert_eq!(t.get_attribute(div_id, "id"), Some("newId".to_string()));
    }

    #[test]
    fn id_setter_then_getter() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        let result = rt
            .eval(
                r#"
            var el = document.getElementById("app");
            el.id = "newId";
            el.id;
        "#,
            )
            .unwrap();

        let value = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(value, "newId");
    }

    #[test]
    fn class_name_getter_returns_class_attribute() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        rt.eval(
            r#"
            var el = document.getElementById("app");
            el.setAttribute("class", "container");
        "#,
        )
        .unwrap();

        let result = rt
            .eval(
                r#"
            var el = document.getElementById("app");
            el.className;
        "#,
            )
            .unwrap();

        let value = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(value, "container");
    }

    #[test]
    fn class_name_setter_updates_class_attribute() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        rt.eval(
            r#"
            var el = document.getElementById("app");
            el.className = "wrapper";
        "#,
        )
        .unwrap();

        // Verify via DomTree
        let t = tree.borrow();
        let div_id = 3; // div#app
        assert_eq!(t.get_attribute(div_id, "class"), Some("wrapper".to_string()));
    }

    #[test]
    fn class_name_getter_returns_empty_string_for_missing() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        let result = rt
            .eval(
                r#"
            var el = document.getElementById("app");
            el.className;
        "#,
            )
            .unwrap();

        let value = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(value, "");
    }

    #[test]
    fn id_getter_returns_empty_string_for_missing() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        rt.eval(
            r#"
            var el = document.getElementById("app");
            el.removeAttribute("id");
        "#,
        )
        .unwrap();

        let result = rt
            .eval(
                r#"
            var el = document.createElement("div");
            el.id;
        "#,
            )
            .unwrap();

        let value = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(value, "");
    }

    #[test]
    fn attribute_workflow_integration() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        rt.eval(
            r#"
            var el = document.getElementById("app");

            // Initially has id, no class
            var hasId = el.hasAttribute("id");
            var hasClass = el.hasAttribute("class");

            // Set class via setAttribute
            el.setAttribute("class", "container");

            // Set data-value via setAttribute
            el.setAttribute("data-value", "123");

            // Update id via property
            el.id = "main";

            // Update class via property
            el.className = "wrapper";

            // Remove data-value
            el.removeAttribute("data-value");
        "#,
        )
        .unwrap();

        // Verify the final state via DomTree
        let t = tree.borrow();
        let div_id = 3; // div#app
        assert_eq!(t.get_attribute(div_id, "id"), Some("main".to_string()));
        assert_eq!(t.get_attribute(div_id, "class"), Some("wrapper".to_string()));
        assert_eq!(t.get_attribute(div_id, "data-value"), None);
    }

    #[test]
    fn set_attribute_ns_then_read_attributes_array() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        let result = rt
            .eval(
                r#"
            var el = document.createElement("foo");
            el.setAttributeNS("http://www.w3.org/XML/1998/namespace", "a:bb", "pass");
            var attr = el.attributes[0];
            attr ? attr.value : "NO_ATTR";
        "#,
            )
            .unwrap();

        let value = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(value, "pass");
    }
}
