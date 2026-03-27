use boa_engine::{js_string, Context, JsError, JsResult, JsValue};

use super::super::element::get_or_create_js_element;
use crate::dom::NodeData;

/// Native implementation of element.getAttribute(name)
pub(super) fn get_attribute_fn(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
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
pub(super) fn set_attribute_fn(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "setAttribute");
    let name = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    // Validate the attribute name per spec
    if !crate::dom::is_valid_attribute_name(&name) {
        let exc = super::super::create_dom_exception(ctx, "InvalidCharacterError", "String contains an invalid character", 5)?;
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
    // Capture old value for attributeChangedCallback
    let old_value_for_ce = el.tree.borrow().get_attribute(el.node_id, &name).map(|s| s.to_string());
    super::super::mutation_observer::set_attribute_with_observer(ctx, &el.tree, el.node_id, &name, &value);
    // Custom element attributeChangedCallback
    super::super::custom_elements::invoke_attribute_changed_callback(
        &el.tree,
        el.node_id,
        &name,
        old_value_for_ce.as_deref(),
        Some(&value),
        ctx,
    );
    // Compile inline event handler if this is an on* attribute
    if name.starts_with("on") && name.len() > 2 {
        let tree_ptr = std::rc::Rc::as_ptr(&el.tree) as usize;
        let node_id = el.node_id;
        super::super::on_event::compile_inline_event_handler(tree_ptr, node_id, &name[2..], &value, ctx);
    }
    Ok(JsValue::undefined())
}

/// Native implementation of element.removeAttribute(name)
pub(super) fn remove_attribute_fn(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
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
    // Capture old value for attributeChangedCallback
    let old_value_for_ce = el.tree.borrow().get_attribute(el.node_id, &name).map(|s| s.to_string());
    super::super::mutation_observer::remove_attribute_with_observer(ctx, &el.tree, el.node_id, &name);
    // Custom element attributeChangedCallback
    if old_value_for_ce.is_some() {
        super::super::custom_elements::invoke_attribute_changed_callback(
            &el.tree,
            el.node_id,
            &name,
            old_value_for_ce.as_deref(),
            None,
            ctx,
        );
    }
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
        if let Some(interned) = super::super::on_event::intern_event_name(&name[2..]) {
            let tree_ptr = std::rc::Rc::as_ptr(&el.tree) as usize;
            super::super::on_event::set_on_event_handler(tree_ptr, el.node_id, interned, None, ctx);
        }
    }
    Ok(JsValue::undefined())
}

/// Native implementation of element.hasAttribute(name)
pub(super) fn has_attribute_fn(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
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

/// Native implementation of element.hasAttributes()
pub(super) fn has_attributes_fn(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "hasAttributes");

    let tree = el.tree.borrow();
    let has = tree.has_attributes(el.node_id);
    Ok(JsValue::from(has))
}

/// Native implementation of element.toggleAttribute(qualifiedName, force?)
pub(super) fn toggle_attribute_fn(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "toggleAttribute");
    let name = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    // Validate the attribute name per spec
    if !crate::dom::is_valid_attribute_name(&name) {
        let exc = super::super::create_dom_exception(ctx, "InvalidCharacterError", "String contains an invalid character", 5)?;
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
            super::super::mutation_observer::remove_attribute_with_observer(ctx, &el.tree, el.node_id, &name);
            Ok(JsValue::from(false))
        } else {
            Ok(JsValue::from(true))
        }
    } else if !has_force || force_arg.unwrap().to_boolean() {
        // force missing or true: add the attribute with empty value, return true
        super::super::mutation_observer::set_attribute_with_observer(ctx, &el.tree, el.node_id, &name, "");
        Ok(JsValue::from(true))
    } else {
        // force=false and attribute doesn't exist: return false
        Ok(JsValue::from(false))
    }
}

/// Native implementation of element.getAttributeNames()
/// Returns a JS Array of qualified attribute names in order.
pub(super) fn get_attribute_names_fn(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
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
