//! JS bindings for NodeList and HTMLCollection interfaces.
//!
//! NodeList: returned by childNodes (live) and querySelectorAll (static).
//! HTMLCollection: returned by children, getElementsByTagName, getElementsByClassName (live).
//!
//! Both use JS Proxy objects to support live bracket-index access ([0], [1], etc.)
//! as well as `length`, `item()`, and iterator methods.
//!
//! IMPORTANT: Proxy creation uses pre-built factory functions (stored in per-realm state)
//! instead of context.eval(). This avoids a Boa bug where eval() inside native functions
//! can corrupt the calling scope's variable environment (index-out-of-bounds in DefInitVar).

use std::cell::RefCell;
use std::rc::Rc;

use boa_engine::{
    js_string,
    native_function::NativeFunction,
    object::{FunctionObjectBuilder, ObjectInitializer},
    property::{Attribute, PropertyDescriptor},
    Context, JsError, JsObject, JsResult, JsValue, Source,
};

use crate::dom::{DomTree, NodeData, NodeId};
use crate::js::realm_state;

use super::element::get_or_create_js_element;

// ---------------------------------------------------------------------------
// Registration of NodeList and HTMLCollection globals
// ---------------------------------------------------------------------------

/// Register `NodeList` and `HTMLCollection` as global constructors with proper prototypes.
/// Must be called during runtime initialization.
pub(crate) fn register_collections(context: &mut Context) {
    // ---------------------------------------------------------------
    // NodeList.prototype
    // ---------------------------------------------------------------
    let nodelist_proto = ObjectInitializer::new(context).build();

    // Copy Array.prototype iteration methods onto NodeList.prototype
    // per spec: NodeList.prototype.forEach === Array.prototype.forEach, etc.
    context
        .register_global_property(
            js_string!("__braille_nl_proto"),
            nodelist_proto.clone(),
            Attribute::all(),
        )
        .expect("failed to register temp nl proto");

    context
        .eval(Source::from_bytes(
            r#"
            (function() {
                var proto = __braille_nl_proto;
                proto.forEach = Array.prototype.forEach;
                proto.keys = Array.prototype.keys;
                if (Array.prototype.values) {
                    proto.values = Array.prototype.values;
                }
                proto.entries = Array.prototype.entries;
                proto[Symbol.iterator] = Array.prototype[Symbol.iterator];
                delete self.__braille_nl_proto;
            })();
            "#,
        ))
        .expect("failed to set up NodeList.prototype iteration methods");

    // NodeList constructor (abstract, throws)
    let nodelist_ctor = make_illegal_constructor(context, "NodeList");
    nodelist_ctor
        .define_property_or_throw(
            js_string!("prototype"),
            PropertyDescriptor::builder()
                .value(nodelist_proto.clone())
                .writable(false)
                .configurable(false)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to define NodeList.prototype");

    nodelist_proto
        .define_property_or_throw(
            js_string!("constructor"),
            PropertyDescriptor::builder()
                .value(nodelist_ctor.clone())
                .writable(true)
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to set NodeList.prototype.constructor");

    context
        .register_global_property(
            js_string!("NodeList"),
            nodelist_ctor,
            Attribute::WRITABLE | Attribute::CONFIGURABLE,
        )
        .expect("failed to register NodeList global");

    // Store in realm state
    realm_state::set_nodelist_proto(context, nodelist_proto);

    // ---------------------------------------------------------------
    // HTMLCollection.prototype
    // ---------------------------------------------------------------
    let htmlcollection_proto = ObjectInitializer::new(context).build();

    // HTMLCollection constructor (abstract, throws)
    let htmlcollection_ctor = make_illegal_constructor(context, "HTMLCollection");
    htmlcollection_ctor
        .define_property_or_throw(
            js_string!("prototype"),
            PropertyDescriptor::builder()
                .value(htmlcollection_proto.clone())
                .writable(false)
                .configurable(false)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to define HTMLCollection.prototype");

    htmlcollection_proto
        .define_property_or_throw(
            js_string!("constructor"),
            PropertyDescriptor::builder()
                .value(htmlcollection_ctor.clone())
                .writable(true)
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to set HTMLCollection.prototype.constructor");

    // HTMLCollection also gets Symbol.iterator, item(), namedItem() on prototype
    context
        .register_global_property(
            js_string!("__braille_hc_proto"),
            htmlcollection_proto.clone(),
            Attribute::all(),
        )
        .expect("failed to register temp hc proto");

    context
        .eval(Source::from_bytes(
            r#"
            (function() {
                var proto = __braille_hc_proto;
                proto[Symbol.iterator] = Array.prototype[Symbol.iterator];
                // item() dispatches to backing object's __braille_item function
                proto.item = function(index) {
                    var fn = this.__braille_item;
                    if (fn) return fn(index);
                    return null;
                };
                // namedItem() dispatches to backing object's __braille_namedItem function
                proto.namedItem = function(name) {
                    var fn = this.__braille_namedItem;
                    if (fn) return fn(name);
                    return null;
                };
                delete self.__braille_hc_proto;
            })();
            "#,
        ))
        .expect("failed to set up HTMLCollection.prototype iteration methods");

    context
        .register_global_property(
            js_string!("HTMLCollection"),
            htmlcollection_ctor,
            Attribute::WRITABLE | Attribute::CONFIGURABLE,
        )
        .expect("failed to register HTMLCollection global");

    // Store in realm state
    realm_state::set_htmlcollection_proto(context, htmlcollection_proto);

    // Also put NodeList and HTMLCollection on window object
    let global = context.global_object();
    let window_val = global
        .get(js_string!("window"), context)
        .expect("window global should exist");
    if let Some(window_obj) = window_val.as_object() {
        for name in &["NodeList", "HTMLCollection"] {
            let val = global
                .get(js_string!(*name), context)
                .expect("global should have this property");
            let _ = window_obj.define_property_or_throw(
                js_string!(*name),
                PropertyDescriptor::builder()
                    .value(val)
                    .writable(true)
                    .configurable(true)
                    .enumerable(false)
                    .build(),
                context,
            );
        }
    }

    // ---------------------------------------------------------------
    // Pre-build proxy factory functions
    // ---------------------------------------------------------------
    // These are JS functions that take (backing, getLength, getChild) and return
    // a Proxy. By creating them at init time (in global scope), we avoid calling
    // context.eval() from native functions which triggers a Boa environment bug.

    let nl_factory = context
        .eval(Source::from_bytes(
            r#"
            (function __braille_nl_factory(backing, getLength, getChild) {
                var handler = {
                    get: function(target, prop, receiver) {
                        if (prop === 'length') {
                            return getLength();
                        }
                        if (typeof prop === 'string' && /^\d+$/.test(prop)) {
                            return getChild(parseInt(prop, 10));
                        }
                        var val = target[prop];
                        if (typeof val === 'function') {
                            return val;
                        }
                        return val;
                    },
                    has: function(target, prop) {
                        if (prop === 'length') return true;
                        if (typeof prop === 'string' && /^\d+$/.test(prop)) {
                            var idx = parseInt(prop, 10);
                            return idx >= 0 && idx < getLength();
                        }
                        return prop in target;
                    },
                    ownKeys: function(target) {
                        var keys = [];
                        var len = getLength();
                        for (var i = 0; i < len; i++) {
                            keys.push(String(i));
                        }
                        return keys;
                    },
                    getOwnPropertyDescriptor: function(target, prop) {
                        if (typeof prop === 'string' && /^\d+$/.test(prop)) {
                            var idx = parseInt(prop, 10);
                            if (idx >= 0 && idx < getLength()) {
                                return {
                                    value: getChild(idx),
                                    writable: false,
                                    enumerable: true,
                                    configurable: true
                                };
                            }
                        }
                        return Object.getOwnPropertyDescriptor(target, prop);
                    }
                };
                return new Proxy(backing, handler);
            })
            "#,
        ))
        .expect("failed to create NodeList proxy factory");

    realm_state::set_nl_proxy_factory(
        context,
        nl_factory
            .as_object()
            .expect("NL factory should be an object")
            .clone(),
    );

    let hc_factory = context
        .eval(Source::from_bytes(
            r#"
            (function __braille_hc_factory(backing, getLength, getChild, getNamed, getNamedKeys) {
                var expandos = Object.create(null);
                var handler = {
                    get: function(target, prop, receiver) {
                        // Check expandos first (set by user code like l.item = "pass")
                        if (prop in expandos) {
                            return expandos[prop];
                        }
                        if (prop === 'length') {
                            return getLength();
                        }
                        if (typeof prop === 'string' && /^\d+$/.test(prop)) {
                            return getChild(parseInt(prop, 10));
                        }
                        var val = target[prop];
                        if (val !== undefined) {
                            return val;
                        }
                        if (typeof prop === 'string' && prop !== 'length') {
                            var named = getNamed(prop);
                            if (named !== undefined) {
                                return named;
                            }
                        }
                        return undefined;
                    },
                    set: function(target, prop, value) {
                        // Numeric indices are read-only (return false to throw in strict mode)
                        if (typeof prop === 'string' && /^\d+$/.test(prop)) {
                            return false;
                        }
                        // Allow expandos for named properties
                        expandos[prop] = value;
                        return true;
                    },
                    has: function(target, prop) {
                        if (prop === 'length') return true;
                        if (typeof prop === 'string' && /^\d+$/.test(prop)) {
                            var idx = parseInt(prop, 10);
                            return idx >= 0 && idx < getLength();
                        }
                        if (prop in target) return true;
                        if (typeof prop === 'string') {
                            var named = getNamed(prop);
                            return named !== undefined;
                        }
                        return false;
                    },
                    ownKeys: function(target) {
                        var keys = [];
                        var len = getLength();
                        for (var i = 0; i < len; i++) {
                            keys.push(String(i));
                        }
                        var namedStr = getNamedKeys();
                        if (namedStr) {
                            var names = namedStr.split('\0');
                            for (var j = 0; j < names.length; j++) {
                                if (names[j]) keys.push(names[j]);
                            }
                        }
                        return keys;
                    },
                    getOwnPropertyDescriptor: function(target, prop) {
                        if (typeof prop === 'string' && /^\d+$/.test(prop)) {
                            var idx = parseInt(prop, 10);
                            if (idx >= 0 && idx < getLength()) {
                                return {
                                    value: getChild(idx),
                                    writable: false,
                                    enumerable: true,
                                    configurable: true
                                };
                            }
                        }
                        if (typeof prop === 'string' && prop !== 'length') {
                            var named = getNamed(prop);
                            if (named !== undefined) {
                                return {
                                    value: named,
                                    writable: false,
                                    enumerable: false,
                                    configurable: true
                                };
                            }
                        }
                        return Object.getOwnPropertyDescriptor(target, prop);
                    }
                };
                return new Proxy(backing, handler);
            })
            "#,
        ))
        .expect("failed to create HTMLCollection proxy factory");

    realm_state::set_hc_proxy_factory(
        context,
        hc_factory
            .as_object()
            .expect("HC factory should be an object")
            .clone(),
    );
}

fn make_illegal_constructor(context: &mut Context, name: &str) -> JsObject {
    let ctor = unsafe {
        NativeFunction::from_closure(|_this, _args, _ctx| {
            Err(JsError::from_opaque(JsValue::from(js_string!(
                "Illegal constructor"
            ))))
        })
    };
    FunctionObjectBuilder::new(context.realm(), ctor)
        .name(js_string!(name))
        .length(0)
        .constructor(true)
        .build()
        .into()
}

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
            let index = args
                .first()
                .map(|v| v.to_number(ctx))
                .transpose()?
                .unwrap_or(0.0) as i64;

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
            let index = args
                .first()
                .map(|v| v.to_number(ctx))
                .transpose()?
                .unwrap_or(0.0) as i64;

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
    let factory = realm_state::nl_proxy_factory(context)
        .expect("NodeList proxy factory not initialized");

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
            let index = args
                .first()
                .map(|v| v.to_number(ctx))
                .transpose()?
                .unwrap_or(0.0) as i64;

            if index < 0 {
                return Ok(JsValue::null());
            }

            match node_ids_for_item.get(index as usize) {
                Some(&node_id) => {
                    let js_obj =
                        get_or_create_js_element(node_id, tree_for_item.clone(), ctx)?;
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
            let index = args
                .first()
                .map(|v| v.to_number(ctx))
                .transpose()?
                .unwrap_or(0.0) as i64;

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
                if let NodeData::Element { ref attributes, ref namespace, .. } = node.data {
                    let is_html = namespace == "http://www.w3.org/1999/xhtml";

                    if let Some(id_val) = attributes.iter().find(|a| a.local_name == "id").map(|a| a.value.as_str()) {
                        if id_val == name {
                            let tree_clone = tree_for_named.clone();
                            drop(tree_ref);
                            let js_obj = get_or_create_js_element(child_id, tree_clone, ctx)?;
                            return Ok(js_obj.into());
                        }
                    }
                    if is_html {
                        if let Some(name_val) = attributes.iter().find(|a| a.local_name == "name").map(|a| a.value.as_str()) {
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
            let index = args
                .first()
                .map(|v| v.to_number(ctx))
                .transpose()?
                .unwrap_or(0.0) as i64;

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
                if let NodeData::Element { ref attributes, ref namespace, .. } = node.data {
                    let is_html = namespace == "http://www.w3.org/1999/xhtml";

                    if let Some(id_val) = attributes.iter().find(|a| a.local_name == "id").map(|a| a.value.as_str()) {
                        if id_val == name {
                            let tree_clone = tree_for_named2.clone();
                            drop(tree_ref);
                            let js_obj = get_or_create_js_element(child_id, tree_clone, ctx)?;
                            return Ok(js_obj.into());
                        }
                    }
                    if is_html {
                        if let Some(name_val) = attributes.iter().find(|a| a.local_name == "name").map(|a| a.value.as_str()) {
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
                if let NodeData::Element { ref attributes, ref namespace, .. } = node.data {
                    let is_html = namespace == "http://www.w3.org/1999/xhtml";

                    if let Some(id_val) = attributes.iter().find(|a| a.local_name == "id").map(|a| a.value.clone()) {
                        if !id_val.is_empty() && !names.contains(&id_val) {
                            names.push(id_val);
                        }
                    }
                    if is_html {
                        if let Some(name_val) = attributes.iter().find(|a| a.local_name == "name").map(|a| a.value.clone()) {
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
    let factory = realm_state::hc_proxy_factory(context)
        .expect("HTMLCollection proxy factory not initialized");

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
    is_root: bool,
) {
    let node = tree.get_node(node_id);
    if !is_root {
        if let NodeData::Element { ref attributes, .. } = node.data {
            if let Some(class_attr) = attributes
                .iter()
                .find(|a| a.local_name == "class")
                .map(|a| a.value.as_str())
            {
                let element_classes: Vec<&str> = class_attr.split_ascii_whitespace().collect();
                if class_names.iter().all(|cn| element_classes.contains(&cn.as_str())) {
                    results.push(node_id);
                }
            }
        }
    }
    let children: Vec<NodeId> = node.children.clone();
    for child_id in children {
        collect_descendants_by_class_recursive(tree, child_id, class_names, results, false);
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
    let class_names: Vec<String> = class_arg
        .split_ascii_whitespace()
        .map(|s| s.to_string())
        .collect();

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
            let index = args
                .first()
                .map(|v| v.to_number(ctx))
                .transpose()?
                .unwrap_or(0.0) as i64;
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
                if let NodeData::Element { ref attributes, ref namespace, .. } = node.data {
                    let is_html = namespace == "http://www.w3.org/1999/xhtml";
                    if let Some(id_val) = attributes.iter().find(|a| a.local_name == "id").map(|a| a.value.as_str()) {
                        if id_val == name {
                            let tc = tree_for_named.clone();
                            drop(tree_ref);
                            return Ok(get_or_create_js_element(nid, tc, ctx)?.into());
                        }
                    }
                    if is_html {
                        if let Some(name_val) = attributes.iter().find(|a| a.local_name == "name").map(|a| a.value.as_str()) {
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
            let index = args
                .first()
                .map(|v| v.to_number(ctx))
                .transpose()?
                .unwrap_or(0.0) as i64;
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
                if let NodeData::Element { ref attributes, ref namespace, .. } = node.data {
                    let is_html = namespace == "http://www.w3.org/1999/xhtml";
                    if let Some(id_val) = attributes.iter().find(|a| a.local_name == "id").map(|a| a.value.as_str()) {
                        if id_val == name {
                            let tc = tree_for_named2.clone();
                            drop(tree_ref);
                            return Ok(get_or_create_js_element(nid, tc, ctx)?.into());
                        }
                    }
                    if is_html {
                        if let Some(name_val) = attributes.iter().find(|a| a.local_name == "name").map(|a| a.value.as_str()) {
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
                if let NodeData::Element { ref attributes, ref namespace, .. } = node.data {
                    let is_html = namespace == "http://www.w3.org/1999/xhtml";
                    if let Some(id_val) = attributes.iter().find(|a| a.local_name == "id").map(|a| a.value.clone()) {
                        if !id_val.is_empty() && !names.contains(&id_val) {
                            names.push(id_val);
                        }
                    }
                    if is_html {
                        if let Some(name_val) = attributes.iter().find(|a| a.local_name == "name").map(|a| a.value.clone()) {
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
    let factory = realm_state::hc_proxy_factory(context)
        .expect("HTMLCollection proxy factory not initialized");

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
            if search_tag == "*" {
                results.push(node_id);
            } else if tree.is_html_document() {
                // Per spec for HTML documents:
                // - HTML-namespace elements: compare search_tag ASCII-lowercased against
                //   the element's qualified name AS-IS (not lowercased)
                // - Non-HTML-namespace elements: compare search_tag AS-IS (exact match)
                let is_html_ns = namespace == "http://www.w3.org/1999/xhtml";
                if is_html_ns {
                    if search_tag.to_ascii_lowercase() == *tag_name {
                        results.push(node_id);
                    }
                } else {
                    if search_tag == tag_name {
                        results.push(node_id);
                    }
                }
            } else {
                // XML document: exact match for all namespaces
                if search_tag == tag_name {
                    results.push(node_id);
                }
            }
        }
    }
    let children: Vec<NodeId> = node.children.clone();
    for child_id in children {
        collect_descendants_by_tag_recursive(tree, child_id, search_tag, results, false);
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
            let index = args
                .first()
                .map(|v| v.to_number(ctx))
                .transpose()?
                .unwrap_or(0.0) as i64;
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
                if let NodeData::Element { ref attributes, ref namespace, .. } = node.data {
                    let is_html = namespace == "http://www.w3.org/1999/xhtml";
                    if let Some(id_val) = attributes.iter().find(|a| a.local_name == "id").map(|a| a.value.as_str()) {
                        if id_val == name {
                            let tc = tree_for_named.clone();
                            drop(tree_ref);
                            return Ok(get_or_create_js_element(nid, tc, ctx)?.into());
                        }
                    }
                    if is_html {
                        if let Some(name_val) = attributes.iter().find(|a| a.local_name == "name").map(|a| a.value.as_str()) {
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
            let index = args
                .first()
                .map(|v| v.to_number(ctx))
                .transpose()?
                .unwrap_or(0.0) as i64;
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
                if let NodeData::Element { ref attributes, ref namespace, .. } = node.data {
                    let is_html = namespace == "http://www.w3.org/1999/xhtml";
                    if let Some(id_val) = attributes.iter().find(|a| a.local_name == "id").map(|a| a.value.as_str()) {
                        if id_val == name {
                            let tc = tree_for_named2.clone();
                            drop(tree_ref);
                            return Ok(get_or_create_js_element(nid, tc, ctx)?.into());
                        }
                    }
                    if is_html {
                        if let Some(name_val) = attributes.iter().find(|a| a.local_name == "name").map(|a| a.value.as_str()) {
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
                if let NodeData::Element { ref attributes, ref namespace, .. } = node.data {
                    let is_html = namespace == "http://www.w3.org/1999/xhtml";
                    if let Some(id_val) = attributes.iter().find(|a| a.local_name == "id").map(|a| a.value.clone()) {
                        if !id_val.is_empty() && !names.contains(&id_val) {
                            names.push(id_val);
                        }
                    }
                    if is_html {
                        if let Some(name_val) = attributes.iter().find(|a| a.local_name == "name").map(|a| a.value.clone()) {
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
    let factory = realm_state::hc_proxy_factory(context)
        .expect("HTMLCollection proxy factory not initialized");

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
fn collect_descendants_by_tag_ns(
    tree: &DomTree,
    root: NodeId,
    namespace: &str,
    local_name: &str,
) -> Vec<NodeId> {
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
            let index = args
                .first()
                .map(|v| v.to_number(ctx))
                .transpose()?
                .unwrap_or(0.0) as i64;
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
                if let NodeData::Element { ref attributes, ref namespace, .. } = node.data {
                    let is_html = namespace == "http://www.w3.org/1999/xhtml";
                    if let Some(id_val) = attributes.iter().find(|a| a.local_name == "id").map(|a| a.value.as_str()) {
                        if id_val == name {
                            let tc = tree_for_named.clone();
                            drop(tree_ref);
                            return Ok(get_or_create_js_element(nid, tc, ctx)?.into());
                        }
                    }
                    if is_html {
                        if let Some(name_val) = attributes.iter().find(|a| a.local_name == "name").map(|a| a.value.as_str()) {
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
            let index = args
                .first()
                .map(|v| v.to_number(ctx))
                .transpose()?
                .unwrap_or(0.0) as i64;
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
                if let NodeData::Element { ref attributes, ref namespace, .. } = node.data {
                    let is_html = namespace == "http://www.w3.org/1999/xhtml";
                    if let Some(id_val) = attributes.iter().find(|a| a.local_name == "id").map(|a| a.value.as_str()) {
                        if id_val == name {
                            let tc = tree_for_named2.clone();
                            drop(tree_ref);
                            return Ok(get_or_create_js_element(nid, tc, ctx)?.into());
                        }
                    }
                    if is_html {
                        if let Some(name_val) = attributes.iter().find(|a| a.local_name == "name").map(|a| a.value.as_str()) {
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
                if let NodeData::Element { ref attributes, ref namespace, .. } = node.data {
                    let is_html = namespace == "http://www.w3.org/1999/xhtml";
                    if let Some(id_val) = attributes.iter().find(|a| a.local_name == "id").map(|a| a.value.clone()) {
                        if !id_val.is_empty() && !names.contains(&id_val) {
                            names.push(id_val);
                        }
                    }
                    if is_html {
                        if let Some(name_val) = attributes.iter().find(|a| a.local_name == "name").map(|a| a.value.clone()) {
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
    let factory = realm_state::hc_proxy_factory(context)
        .expect("HTMLCollection proxy factory not initialized");

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

#[cfg(test)]
mod tests {
    use crate::Engine;

    /// Verify that NodeList proxy works correctly even inside complex JS scopes.
    /// This was previously failing due to a Boa bug where context.eval() called from
    /// native functions corrupted the calling scope's variable environment.
    #[test]
    fn wpt_iterator_in_complex_scope() {
        let mut engine = Engine::new();

        engine.load_html(r#"<!DOCTYPE html>
<meta charset=utf-8>
<title>Debug</title>
<div id="test"><span>1</span><span>2</span></div>
"#);

        // Run the full iterator test inside a try/catch (like the WPT harness does)
        let result = engine.eval_js(r#"
(function() {
    var result = { status: 0, message: "" };
    try {
        var node = document.createElement("div");
        var kid1 = document.createElement("p");
        var kid2 = document.createTextNode("hey");
        var kid3 = document.createElement("span");
        node.appendChild(kid1);
        node.appendChild(kid2);
        node.appendChild(kid3);

        var list = node.childNodes;

        // Spread
        var spread = [...list];
        if (spread.length !== 3) throw new Error("spread length: " + spread.length);
        if (spread[0] !== kid1) throw new Error("spread[0] wrong");

        // keys
        var keys = list.keys();
        if (keys instanceof Array) throw new Error("keys instanceof Array");
        keys = [...keys];
        if (keys.length !== 3 || keys[0] !== 0 || keys[1] !== 1 || keys[2] !== 2)
            throw new Error("keys wrong: " + JSON.stringify(keys));

        // values
        var values = list.values();
        values = [...values];
        if (values.length !== 3 || values[0] !== kid1)
            throw new Error("values wrong");

        // entries
        var entries = list.entries();
        entries = [...entries];
        if (entries.length !== 3) throw new Error("entries wrong");

        // forEach
        var cur = 0;
        var thisObj = {};
        list.forEach(function(value, key, listObj) {
            if (listObj !== list) throw new Error("listObj !== list");
            if (this !== thisObj) throw new Error("this !== thisObj");
            cur++;
        }, thisObj);
        if (cur !== 3) throw new Error("forEach count: " + cur);

        // Identity checks
        if (list[Symbol.iterator] !== Array.prototype[Symbol.iterator])
            throw new Error("Symbol.iterator identity");
        if (list.keys !== Array.prototype.keys)
            throw new Error("keys identity");
        if (list.forEach !== Array.prototype.forEach)
            throw new Error("forEach identity");

    } catch(e) {
        result.status = 1;
        result.message = e.message || String(e);
    }
    return JSON.stringify(result);
})()
        "#).unwrap();

        eprintln!("Result: {}", result);
        assert!(result.contains("\"status\":0"), "Test failed: {}", result);
    }

    #[test]
    fn htmlcollection_children_named_props() {
        let mut engine = Engine::new();
        engine.load_html(r#"<!DOCTYPE html>
<div id="test"><img><img id=foo><img id=foo><img name="bar"></div>"#);
        let result = engine.eval_js(r#"
(function() {
    var container = document.getElementById("test");
    var child = document.createElementNS("", "img");
    child.setAttribute("id", "baz");
    container.appendChild(child);
    child = document.createElementNS("", "img");
    child.setAttribute("name", "qux");
    container.appendChild(child);

    var list = container.children;
    var errors = [];

    // children.length should be 6
    if (list.length !== 6) errors.push("length=" + list.length);

    // namespaceURI: parsed element = xhtml, createElementNS("") = null
    if (list[0].namespaceURI !== "http://www.w3.org/1999/xhtml")
        errors.push("parsed ns=" + list[0].namespaceURI);
    if (list[4].namespaceURI !== null)
        errors.push("createElementNS ns=" + list[4].namespaceURI);

    // for..in + hasOwnProperty should only yield numeric indices
    var forIn = [];
    for (var p in list) {
        if (list.hasOwnProperty(p)) forIn.push(p);
    }
    if (forIn.length !== 6) errors.push("forIn=" + JSON.stringify(forIn));

    // Object.getOwnPropertyNames should include named props (but not qux)
    var own = Object.getOwnPropertyNames(list);
    if (own.indexOf("foo") === -1) errors.push("missing foo in ownPropertyNames");
    if (own.indexOf("bar") === -1) errors.push("missing bar in ownPropertyNames");
    if (own.indexOf("baz") === -1) errors.push("missing baz in ownPropertyNames");
    if (own.indexOf("qux") !== -1) errors.push("qux should not be in ownPropertyNames");

    return errors.length === 0 ? "ok" : errors.join("; ");
})()
"#).unwrap();
        assert_eq!(result, "ok", "HTMLCollection test failed: {}", result);
    }
}
