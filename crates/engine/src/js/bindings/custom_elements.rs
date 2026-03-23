//! Custom Elements registry — `window.customElements.define/get/whenDefined`.

use std::cell::RefCell;
use std::rc::Rc;

use boa_engine::{
    js_string,
    native_function::NativeFunction,
    object::ObjectInitializer,
    Context, JsError, JsObject, JsResult, JsValue,
};

use crate::dom::{DomTree, NodeData, NodeId};
use crate::js::realm_state::{self, CustomElementDefinition};

/// Register `customElements` object on the global scope and the window object.
pub(crate) fn register_custom_elements(context: &mut Context) {
    let custom_elements_obj = ObjectInitializer::new(context)
        .function(NativeFunction::from_fn_ptr(define_fn), js_string!("define"), 2)
        .function(NativeFunction::from_fn_ptr(get_fn), js_string!("get"), 1)
        .function(NativeFunction::from_fn_ptr(when_defined_fn), js_string!("whenDefined"), 1)
        .build();

    context
        .global_object()
        .set(js_string!("customElements"), JsValue::from(custom_elements_obj.clone()), false, context)
        .expect("set customElements global");

    // Also set on window object if available
    if let Some(window) = realm_state::window_object(context) {
        let _ = window.set(js_string!("customElements"), JsValue::from(custom_elements_obj), false, context);
    }
}

/// `customElements.define(name, constructor, options?)`
fn define_fn(_this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    // 1. Get name
    let name = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    // 2. Validate name contains a hyphen and is non-empty
    if name.is_empty() || !name.contains('-') {
        let exc = super::create_dom_exception(
            ctx,
            "SyntaxError",
            "The element name must contain a hyphen",
            12,
        )?;
        return Err(JsError::from_opaque(exc.into()));
    }

    // 3. Get constructor (must be a callable object)
    let ctor = args
        .get(1)
        .and_then(|v| v.as_object())
        .ok_or_else(|| {
            JsError::from_opaque(js_string!("customElements.define: argument 2 is not a constructor").into())
        })?
        .clone();

    // 4. Check not already defined
    {
        let registry = realm_state::custom_elements(ctx);
        if registry.borrow().definitions.contains_key(&name) {
            let exc = super::create_dom_exception(
                ctx,
                "NotSupportedError",
                &format!("'{name}' has already been defined as a custom element"),
                9,
            )?;
            return Err(JsError::from_opaque(exc.into()));
        }
    }

    // 5. Read observedAttributes from constructor
    let observed_attributes = read_observed_attributes(&ctor, ctx);

    // 6. Read lifecycle callbacks from constructor.prototype
    let (connected_cb, disconnected_cb, attr_changed_cb) = read_lifecycle_callbacks(&ctor, ctx);

    // 7. Store definition in registry
    {
        let registry = realm_state::custom_elements(ctx);
        registry.borrow_mut().definitions.insert(
            name.clone(),
            CustomElementDefinition {
                constructor: ctor.clone(),
                observed_attributes,
                connected_callback: connected_cb,
                disconnected_callback: disconnected_cb,
                attribute_changed_callback: attr_changed_cb,
            },
        );
    }

    // 8. Upgrade existing elements: walk the tree for elements matching the tag name.
    //    For each matching element in NODE_CACHE, set its __proto__ to constructor.prototype.
    upgrade_existing_elements(&name, &ctor, ctx);

    Ok(JsValue::undefined())
}

/// `customElements.get(name)` — return constructor or undefined
fn get_fn(_this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let name = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    let registry = realm_state::custom_elements(ctx);
    let reg = registry.borrow();
    if let Some(def) = reg.definitions.get(&name) {
        Ok(JsValue::from(def.constructor.clone()))
    } else {
        Ok(JsValue::undefined())
    }
}

/// `customElements.whenDefined(name)` — return a resolved Promise if defined,
/// otherwise return a pending Promise (simplified: always resolve immediately
/// since define() is synchronous in our engine).
fn when_defined_fn(_this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let name = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    // Validate name
    if name.is_empty() || !name.contains('-') {
        let exc = super::create_dom_exception(
            ctx,
            "SyntaxError",
            "The element name must contain a hyphen",
            12,
        )?;
        return Err(JsError::from_opaque(exc.into()));
    }

    let resolve_value = {
        let registry = realm_state::custom_elements(ctx);
        let reg = registry.borrow();
        if let Some(def) = reg.definitions.get(&name) {
            JsValue::from(def.constructor.clone())
        } else {
            JsValue::undefined()
        }
    };

    // Use Promise.resolve() to return a resolved promise
    let promise_ctor = ctx.global_object().get(js_string!("Promise"), ctx)?;
    if let Some(promise_obj) = promise_ctor.as_object() {
        let resolve_fn = promise_obj.get(js_string!("resolve"), ctx)?;
        if let Some(resolve) = resolve_fn.as_object() {
            if let Ok(result) = resolve.call(&promise_ctor, &[resolve_value], ctx) {
                return Ok(result);
            }
        }
    }
    Ok(JsValue::undefined())
}

/// Read `constructor.observedAttributes` — if it's an array, collect as Vec<String>.
fn read_observed_attributes(ctor: &JsObject, ctx: &mut Context) -> Vec<String> {
    let val = match ctor.get(js_string!("observedAttributes"), ctx) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let obj = match val.as_object() {
        Some(o) => o,
        None => return Vec::new(),
    };
    let length = match obj.get(js_string!("length"), ctx) {
        Ok(v) => match v.to_u32(ctx) {
            Ok(n) => n,
            Err(_) => return Vec::new(),
        },
        Err(_) => return Vec::new(),
    };
    let mut attrs = Vec::new();
    for i in 0..length {
        if let Ok(item) = obj.get(i, ctx) {
            if let Ok(s) = item.to_string(ctx) {
                attrs.push(s.to_std_string_escaped());
            }
        }
    }
    attrs
}

/// Read lifecycle callbacks from constructor.prototype.
fn read_lifecycle_callbacks(
    ctor: &JsObject,
    ctx: &mut Context,
) -> (Option<JsObject>, Option<JsObject>, Option<JsObject>) {
    let proto_val = match ctor.get(js_string!("prototype"), ctx) {
        Ok(v) => v,
        Err(_) => return (None, None, None),
    };
    let proto = match proto_val.as_object() {
        Some(o) => o,
        None => return (None, None, None),
    };

    let connected = get_callable(&proto, "connectedCallback", ctx);
    let disconnected = get_callable(&proto, "disconnectedCallback", ctx);
    let attr_changed = get_callable(&proto, "attributeChangedCallback", ctx);

    (connected, disconnected, attr_changed)
}

/// Get a callable property from an object, or None if not present/not callable.
fn get_callable(obj: &JsObject, name: &str, ctx: &mut Context) -> Option<JsObject> {
    let val = obj.get(js_string!(name), ctx).ok()?;
    let func = val.as_object()?.clone();
    if func.is_callable() {
        Some(func)
    } else {
        None
    }
}

/// Upgrade existing elements in the tree that match the custom element name.
/// For each matching element with a cached JsObject, set its prototype to constructor.prototype.
fn upgrade_existing_elements(name: &str, ctor: &JsObject, ctx: &mut Context) {
    let proto_val = match ctor.get(js_string!("prototype"), ctx) {
        Ok(v) => v,
        Err(_) => return,
    };
    let proto = match proto_val.as_object() {
        Some(o) => o.clone(),
        None => return,
    };

    let tree = realm_state::dom_tree(ctx);
    let tree_ptr = Rc::as_ptr(&tree) as usize;

    // Walk the tree iteratively to find all elements with matching tag name
    let matching_ids: Vec<NodeId> = {
        let t = tree.borrow();
        let mut stack = vec![t.document()];
        let mut found = Vec::new();
        while let Some(nid) = stack.pop() {
            let node = t.get_node(nid);
            if let NodeData::Element { tag_name, .. } = &node.data {
                if tag_name == name {
                    found.push(nid);
                }
            }
            // Push children in reverse order so we process them in document order
            for &child in node.children.iter().rev() {
                stack.push(child);
            }
        }
        found
    };

    // For each matching element in the node cache, set its prototype
    let cache = realm_state::node_cache(ctx);
    for nid in matching_ids {
        let cache_key = (tree_ptr, nid);
        if let Some(js_obj) = cache.borrow().get(&cache_key).cloned() {
            js_obj.set_prototype(Some(proto.clone()));
        }
    }
}

/// Look up a custom element definition for a tag name that contains a hyphen.
/// Called during `create_js_element` to set the correct prototype.
pub(crate) fn lookup_custom_element_proto(
    tag_name: &str,
    ctx: &mut Context,
) -> Option<JsObject> {
    let registry = realm_state::custom_elements(ctx);
    let reg = registry.borrow();
    let def = reg.definitions.get(tag_name)?;
    let ctor = def.constructor.clone();
    drop(reg);
    let proto_val = ctor.get(js_string!("prototype"), ctx).ok()?;
    let obj = proto_val.as_object()?;
    Some(obj.clone())
}

/// Invoke `connectedCallback` on a custom element if it has one defined.
/// Best-effort: errors are silently ignored.
pub(crate) fn invoke_connected_callback(
    tree: &Rc<RefCell<DomTree>>,
    node_id: NodeId,
    ctx: &mut Context,
) {
    let tag = {
        let t = tree.borrow();
        match &t.get_node(node_id).data {
            NodeData::Element { tag_name, .. } => {
                if tag_name.contains('-') {
                    Some(tag_name.clone())
                } else {
                    None
                }
            }
            _ => None,
        }
    };

    let tag = match tag {
        Some(t) => t,
        None => return,
    };

    let callback = {
        let registry = realm_state::custom_elements(ctx);
        let reg = registry.borrow();
        reg.definitions.get(&tag).and_then(|d| d.connected_callback.clone())
    };

    if let Some(cb) = callback {
        let cache = realm_state::node_cache(ctx);
        let tree_ptr = Rc::as_ptr(tree) as usize;
        let js_obj = cache.borrow().get(&(tree_ptr, node_id)).cloned();
        if let Some(obj) = js_obj {
            let _ = cb.call(&JsValue::from(obj), &[], ctx);
        }
    }
}

/// Invoke `disconnectedCallback` on a custom element if it has one defined.
/// Best-effort: errors are silently ignored.
pub(crate) fn invoke_disconnected_callback(
    tree: &Rc<RefCell<DomTree>>,
    node_id: NodeId,
    ctx: &mut Context,
) {
    let tag = {
        let t = tree.borrow();
        match &t.get_node(node_id).data {
            NodeData::Element { tag_name, .. } => {
                if tag_name.contains('-') {
                    Some(tag_name.clone())
                } else {
                    None
                }
            }
            _ => None,
        }
    };

    let tag = match tag {
        Some(t) => t,
        None => return,
    };

    let callback = {
        let registry = realm_state::custom_elements(ctx);
        let reg = registry.borrow();
        reg.definitions.get(&tag).and_then(|d| d.disconnected_callback.clone())
    };

    if let Some(cb) = callback {
        let cache = realm_state::node_cache(ctx);
        let tree_ptr = Rc::as_ptr(tree) as usize;
        let js_obj = cache.borrow().get(&(tree_ptr, node_id)).cloned();
        if let Some(obj) = js_obj {
            let _ = cb.call(&JsValue::from(obj), &[], ctx);
        }
    }
}

/// Invoke `attributeChangedCallback(name, oldValue, newValue)` on a custom element
/// if the attribute is in `observedAttributes`.
/// Best-effort: errors are silently ignored.
pub(crate) fn invoke_attribute_changed_callback(
    tree: &Rc<RefCell<DomTree>>,
    node_id: NodeId,
    attr_name: &str,
    old_value: Option<&str>,
    new_value: Option<&str>,
    ctx: &mut Context,
) {
    let tag = {
        let t = tree.borrow();
        match &t.get_node(node_id).data {
            NodeData::Element { tag_name, .. } => {
                if tag_name.contains('-') {
                    Some(tag_name.clone())
                } else {
                    None
                }
            }
            _ => None,
        }
    };

    let tag = match tag {
        Some(t) => t,
        None => return,
    };

    let callback = {
        let registry = realm_state::custom_elements(ctx);
        let reg = registry.borrow();
        match reg.definitions.get(&tag) {
            Some(def) => {
                if def.observed_attributes.iter().any(|a| a == attr_name) {
                    def.attribute_changed_callback.clone()
                } else {
                    None
                }
            }
            None => None,
        }
    };

    if let Some(cb) = callback {
        let cache = realm_state::node_cache(ctx);
        let tree_ptr = Rc::as_ptr(tree) as usize;
        let js_obj = cache.borrow().get(&(tree_ptr, node_id)).cloned();
        if let Some(obj) = js_obj {
            let old_val = match old_value {
                Some(s) => JsValue::from(js_string!(s)),
                None => JsValue::null(),
            };
            let new_val = match new_value {
                Some(s) => JsValue::from(js_string!(s)),
                None => JsValue::null(),
            };
            let _ = cb.call(
                &JsValue::from(obj),
                &[JsValue::from(js_string!(attr_name)), old_val, new_val],
                ctx,
            );
        }
    }
}
