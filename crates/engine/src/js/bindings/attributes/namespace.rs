use boa_engine::{js_string, Context, JsError, JsResult, JsValue};

use crate::dom::NodeData;

/// Native implementation of element.setAttributeNS(namespace, qualifiedName, value)
pub(super) fn set_attribute_ns_fn(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
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
                super::super::create_dom_exception(ctx, "InvalidCharacterError", "String contains an invalid character", 5)?;
            return Err(JsError::from_opaque(exc.into()));
        }
    } else if !crate::dom::is_valid_attribute_name(&qualified_name) {
        let exc = super::super::create_dom_exception(ctx, "InvalidCharacterError", "String contains an invalid character", 5)?;
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
            let exc = super::super::create_dom_exception(ctx, "NamespaceError", "Namespace must not be empty when prefix is used", 14)?;
            return Err(JsError::from_opaque(exc.into()));
        }
        // 2. If prefix is "xml" and namespace is not the XML namespace → NamespaceError
        if prefix == "xml" && namespace != xml_ns {
            let exc = super::super::create_dom_exception(ctx, "NamespaceError", "The xml prefix requires the XML namespace", 14)?;
            return Err(JsError::from_opaque(exc.into()));
        }
        // 3. If prefix is "xmlns" or qualifiedName is "xmlns", namespace must be XMLNS
        if (prefix == "xmlns" || qualified_name == "xmlns") && namespace != xmlns_ns {
            let exc = super::super::create_dom_exception(ctx, "NamespaceError", "The xmlns prefix/name requires the XMLNS namespace", 14)?;
            return Err(JsError::from_opaque(exc.into()));
        }
        // 4. If namespace is XMLNS, prefix must be "xmlns" or qualifiedName must be "xmlns"
        if namespace == xmlns_ns && prefix != "xmlns" && qualified_name != "xmlns" {
            let exc = super::super::create_dom_exception(ctx, "NamespaceError", "XMLNS namespace requires xmlns prefix or name", 14)?;
            return Err(JsError::from_opaque(exc.into()));
        }
    }

    let value = args
        .get(2)
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    super::super::mutation_observer::set_attribute_ns_with_observer(
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
pub(super) fn get_attribute_ns_fn(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
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
pub(super) fn remove_attribute_ns_fn(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
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
    super::super::mutation_observer::remove_attribute_ns_with_observer(ctx, &el.tree, el.node_id, &namespace, &local_name);
    // Clean up shared attr_node_cache
    if let Some(qname) = qname_for_cache {
        let cache = crate::js::realm_state::attr_node_cache(ctx);
        let tree_ptr = std::rc::Rc::as_ptr(&el.tree) as usize;
        cache.borrow_mut().remove(&(tree_ptr, el.node_id, qname));
    }
    Ok(JsValue::undefined())
}

/// Native implementation of element.hasAttributeNS(namespace, localName)
pub(super) fn has_attribute_ns_fn(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
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
