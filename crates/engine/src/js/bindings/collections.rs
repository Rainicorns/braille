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

use super::element::{get_or_create_js_element, JsElement};

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
                    if (name === '') return null;
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

    // ---------------------------------------------------------------
    // NamedNodeMap.prototype
    // ---------------------------------------------------------------
    let nnm_proto = ObjectInitializer::new(context).build();

    // NamedNodeMap prototype methods (dispatch to backing object)
    context
        .register_global_property(
            js_string!("__braille_nnm_proto"),
            nnm_proto.clone(),
            Attribute::all(),
        )
        .expect("failed to register temp nnm proto");

    context
        .eval(Source::from_bytes(
            r#"
            (function() {
                var proto = __braille_nnm_proto;
                proto.item = function(index) {
                    var fn = this.__braille_nnm_item;
                    if (fn) return fn(index);
                    return null;
                };
                proto.getNamedItem = function(name) {
                    var fn = this.__braille_nnm_getNamedItem;
                    if (fn) return fn(name);
                    return null;
                };
                proto.getNamedItemNS = function(ns, localName) {
                    var fn = this.__braille_nnm_getNamedItemNS;
                    if (fn) return fn(ns, localName);
                    return null;
                };
                proto.setNamedItem = function(attr) {
                    var fn = this.__braille_nnm_setNamedItem;
                    if (fn) return fn(attr);
                    return null;
                };
                proto.setNamedItemNS = function(attr) {
                    var fn = this.__braille_nnm_setNamedItemNS;
                    if (fn) return fn(attr);
                    return null;
                };
                proto.removeNamedItem = function(name) {
                    var fn = this.__braille_nnm_removeNamedItem;
                    if (fn) return fn(name);
                    return null;
                };
                proto.removeNamedItemNS = function(ns, localName) {
                    var fn = this.__braille_nnm_removeNamedItemNS;
                    if (fn) return fn(ns, localName);
                    return null;
                };
                proto[Symbol.iterator] = Array.prototype[Symbol.iterator];
                delete self.__braille_nnm_proto;
            })();
            "#,
        ))
        .expect("failed to set up NamedNodeMap.prototype methods");

    // NamedNodeMap constructor (abstract, throws)
    let nnm_ctor = make_illegal_constructor(context, "NamedNodeMap");
    nnm_ctor
        .define_property_or_throw(
            js_string!("prototype"),
            PropertyDescriptor::builder()
                .value(nnm_proto.clone())
                .writable(false)
                .configurable(false)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to define NamedNodeMap.prototype");

    nnm_proto
        .define_property_or_throw(
            js_string!("constructor"),
            PropertyDescriptor::builder()
                .value(nnm_ctor.clone())
                .writable(true)
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to set NamedNodeMap.prototype.constructor");

    context
        .register_global_property(
            js_string!("NamedNodeMap"),
            nnm_ctor,
            Attribute::WRITABLE | Attribute::CONFIGURABLE,
        )
        .expect("failed to register NamedNodeMap global");

    // Store in realm state
    realm_state::set_nnm_proto(context, nnm_proto);

    // Also put NodeList and HTMLCollection on window object
    let global = context.global_object();
    let window_val = global
        .get(js_string!("window"), context)
        .expect("window global should exist");
    if let Some(window_obj) = window_val.as_object() {
        for name in &["NodeList", "HTMLCollection", "NamedNodeMap"] {
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
        nl_factory.as_object().expect("NL factory should be an object").clone(),
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
                        if (typeof prop === 'string' && prop !== 'length' && prop !== '') {
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
                        // Named collection properties are also read-only
                        if (typeof prop === 'string' && prop !== '' && prop !== 'length') {
                            var named = getNamed(prop);
                            if (named !== undefined) {
                                return false;
                            }
                        }
                        // Allow expandos for other properties
                        expandos[prop] = value;
                        return true;
                    },
                    deleteProperty: function(target, prop) {
                        // Indexed properties cannot be deleted
                        if (typeof prop === 'string' && /^\d+$/.test(prop)) {
                            var idx = parseInt(prop, 10);
                            if (idx >= 0 && idx < getLength()) {
                                return false;
                            }
                        }
                        // Named collection properties cannot be deleted
                        if (typeof prop === 'string' && prop !== '' && prop !== 'length') {
                            var named = getNamed(prop);
                            if (named !== undefined) {
                                return false;
                            }
                        }
                        // Allow deleting expandos and non-existent properties
                        if (prop in expandos) {
                            delete expandos[prop];
                        }
                        return true;
                    },
                    has: function(target, prop) {
                        if (prop === 'length') return true;
                        if (typeof prop === 'string' && /^\d+$/.test(prop)) {
                            var idx = parseInt(prop, 10);
                            return idx >= 0 && idx < getLength();
                        }
                        if (prop in target) return true;
                        if (typeof prop === 'string' && prop !== '') {
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
        hc_factory.as_object().expect("HC factory should be an object").clone(),
    );

    // ---------------------------------------------------------------
    // NamedNodeMap proxy factory
    // ---------------------------------------------------------------
    let nnm_factory = context
        .eval(Source::from_bytes(
            r#"
            (function __braille_nnm_factory(backing, getLength, getChild, getNamed, getNamedKeys) {
                var handler = {
                    get: function(target, prop, receiver) {
                        if (prop === 'length') {
                            return getLength();
                        }
                        if (typeof prop === 'string' && /^\d+$/.test(prop)) {
                            return getChild(parseInt(prop, 10));
                        }
                        // Check backing (prototype methods like item, getNamedItem)
                        var val = target[prop];
                        if (val !== undefined) {
                            return val;
                        }
                        // Named attribute access (but not for symbols or 'length')
                        if (typeof prop === 'string') {
                            var named = getNamed(prop);
                            if (named !== undefined) {
                                return named;
                            }
                        }
                        return undefined;
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
        .expect("failed to create NamedNodeMap proxy factory");

    realm_state::set_nnm_proxy_factory(
        context,
        nnm_factory
            .as_object()
            .expect("NNM factory should be an object")
            .clone(),
    );

    // ---------------------------------------------------------------
    // DOMStringMap.prototype
    // ---------------------------------------------------------------
    let dsm_proto = ObjectInitializer::new(context).build();
    dsm_proto
        .define_property_or_throw(
            boa_engine::JsSymbol::to_string_tag(),
            PropertyDescriptor::builder()
                .value(js_string!("DOMStringMap"))
                .configurable(true)
                .build(),
            context,
        )
        .expect("failed to set DOMStringMap toStringTag");

    let dsm_ctor = make_illegal_constructor(context, "DOMStringMap");
    dsm_ctor
        .define_property_or_throw(
            js_string!("prototype"),
            PropertyDescriptor::builder()
                .value(dsm_proto.clone())
                .writable(false)
                .configurable(false)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to set DOMStringMap.prototype");
    context
        .register_global_property(
            js_string!("DOMStringMap"),
            dsm_ctor,
            Attribute::WRITABLE | Attribute::CONFIGURABLE,
        )
        .expect("failed to register DOMStringMap global");

    realm_state::set_dsm_proto(context, dsm_proto);

    // DOMStringMap proxy factory
    let dsm_factory = context
        .eval(Source::from_bytes(
            r#"
            (function __braille_dsm_factory(backing, getAttr, setAttr, deleteAttr, getKeys) {
                var handler = {
                    get: function(target, prop, receiver) {
                        var val = target[prop];
                        if (val !== undefined) return val;
                        if (typeof prop === 'string') {
                            var v = getAttr(prop);
                            if (v !== null) return v;
                        }
                        return undefined;
                    },
                    set: function(target, prop, value) {
                        if (typeof prop === 'string') {
                            setAttr(prop, String(value));
                            return true;
                        }
                        target[prop] = value;
                        return true;
                    },
                    deleteProperty: function(target, prop) {
                        if (typeof prop === 'string') {
                            return deleteAttr(prop);
                        }
                        return delete target[prop];
                    },
                    has: function(target, prop) {
                        if (prop in target) return true;
                        if (typeof prop === 'string') {
                            return getAttr(prop) !== null;
                        }
                        return false;
                    },
                    ownKeys: function(target) {
                        var keysStr = getKeys();
                        if (keysStr === null) return [];
                        return keysStr.split('\0');
                    },
                    getOwnPropertyDescriptor: function(target, prop) {
                        if (typeof prop === 'string') {
                            var v = getAttr(prop);
                            if (v !== null) {
                                return {
                                    value: v,
                                    writable: true,
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
        .expect("failed to create DOMStringMap proxy factory");

    realm_state::set_dsm_proxy_factory(
        context,
        dsm_factory
            .as_object()
            .expect("DSM factory should be an object")
            .clone(),
    );
}

fn make_illegal_constructor(context: &mut Context, name: &str) -> JsObject {
    let ctor = unsafe {
        NativeFunction::from_closure(|_this, _args, _ctx| {
            Err(JsError::from_opaque(JsValue::from(js_string!("Illegal constructor"))))
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
// Live DOMStringMap creation (for element.dataset)
// ---------------------------------------------------------------------------

/// Create a live DOMStringMap backed by the given element's data-* attributes.
/// The returned Proxy intercepts get/set/delete/ownKeys to read/write the DOM.
pub(crate) fn create_live_domstringmap(
    element_id: NodeId,
    tree: Rc<RefCell<DomTree>>,
    context: &mut Context,
) -> JsResult<JsObject> {
    use super::anchor_form::{camel_to_kebab, kebab_to_camel};

    let backing = ObjectInitializer::new(context).build();

    if let Some(p) = realm_state::dsm_proto(context) {
        backing.set_prototype(Some(p));
    }

    let realm = context.realm().clone();

    // getAttr(camelName) — returns attribute value or null
    let tree_get = tree.clone();
    let get_attr_fn = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let camel = args
                .first()
                .map(|v| v.to_string(ctx))
                .transpose()?
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_default();
            let kebab = camel_to_kebab(&camel);
            let attr_name = format!("data-{}", kebab);
            let t = tree_get.borrow();
            let node = t.get_node(element_id);
            match &node.data {
                NodeData::Element { attributes, .. } => {
                    for attr in attributes {
                        if attr.local_name == attr_name {
                            return Ok(JsValue::from(js_string!(attr.value.clone())));
                        }
                    }
                    Ok(JsValue::null())
                }
                _ => Ok(JsValue::null()),
            }
        })
    };

    // setAttr(camelName, value) — sets data-* attribute on element
    let tree_set = tree.clone();
    let set_attr_fn = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let camel = args
                .first()
                .map(|v| v.to_string(ctx))
                .transpose()?
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_default();
            let value = args
                .get(1)
                .map(|v| v.to_string(ctx))
                .transpose()?
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_default();
            let kebab = camel_to_kebab(&camel);
            let attr_name = format!("data-{}", kebab);
            let mut t = tree_set.borrow_mut();
            t.set_attribute(element_id, &attr_name, &value);
            Ok(JsValue::undefined())
        })
    };

    // deleteAttr(camelName) — removes data-* attribute, returns true if existed
    let tree_del = tree.clone();
    let delete_attr_fn = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let camel = args
                .first()
                .map(|v| v.to_string(ctx))
                .transpose()?
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_default();
            let kebab = camel_to_kebab(&camel);
            let attr_name = format!("data-{}", kebab);
            let mut t = tree_del.borrow_mut();
            let existed = t.get_attribute(element_id, &attr_name).is_some();
            if existed {
                t.remove_attribute(element_id, &attr_name);
            }
            Ok(JsValue::from(true))
        })
    };

    // getKeys() — returns NUL-separated camelCase keys
    let tree_keys = tree;
    let get_keys_fn = unsafe {
        NativeFunction::from_closure(move |_this, _args, _ctx| {
            let t = tree_keys.borrow();
            let node = t.get_node(element_id);
            match &node.data {
                NodeData::Element { attributes, .. } => {
                    let keys: Vec<String> = attributes
                        .iter()
                        .filter_map(|attr| {
                            attr.local_name.strip_prefix("data-").map(kebab_to_camel)
                        })
                        .collect();
                    if keys.is_empty() {
                        Ok(JsValue::null())
                    } else {
                        Ok(JsValue::from(js_string!(keys.join("\0"))))
                    }
                }
                _ => Ok(JsValue::null()),
            }
        })
    };

    let factory = realm_state::dsm_proxy_factory(context).expect("DOMStringMap proxy factory not initialized");
    let get_attr_js = FunctionObjectBuilder::new(&realm, get_attr_fn).build();
    let set_attr_js = FunctionObjectBuilder::new(&realm, set_attr_fn).build();
    let delete_attr_js = FunctionObjectBuilder::new(&realm, delete_attr_fn).build();
    let get_keys_js = FunctionObjectBuilder::new(&realm, get_keys_fn).build();

    let proxy = factory.call(
        &JsValue::undefined(),
        &[
            backing.into(),
            get_attr_js.into(),
            set_attr_js.into(),
            delete_attr_js.into(),
            get_keys_js.into(),
        ],
        context,
    )?;

    Ok(proxy.as_object().expect("DSM proxy should be an object").clone())
}

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
                        let exc = super::create_dom_exception(
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
            super::mutation_observer::set_attribute_with_observer(
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
                        let exc = super::create_dom_exception(
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
                super::mutation_observer::set_attribute_with_observer(
                    ctx,
                    &tree_for_snins,
                    element_id,
                    &name,
                    &value,
                );
            } else {
                super::mutation_observer::set_attribute_ns_with_observer(
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
                            let exc = super::create_dom_exception(
                                ctx,
                                "NotFoundError",
                                "The attribute was not found",
                                8,
                            )?;
                            return Err(JsError::from_opaque(exc.into()));
                        }
                    };

                    // Remove the attribute from the element
                    super::mutation_observer::remove_attribute_with_observer(
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
                    let exc = super::create_dom_exception(ctx, "NotFoundError", "The attribute was not found", 8)?;
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
                            let exc = super::create_dom_exception(
                                ctx,
                                "NotFoundError",
                                "The attribute was not found",
                                8,
                            )?;
                            return Err(JsError::from_opaque(exc.into()));
                        }
                    };

                    super::mutation_observer::remove_attribute_ns_with_observer(
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
                    let exc = super::create_dom_exception(ctx, "NotFoundError", "The attribute was not found", 8)?;
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

#[cfg(test)]
mod tests {
    use crate::Engine;

    /// Verify that NodeList proxy works correctly even inside complex JS scopes.
    /// This was previously failing due to a Boa bug where context.eval() called from
    /// native functions corrupted the calling scope's variable environment.
    #[test]
    fn wpt_iterator_in_complex_scope() {
        let mut engine = Engine::new();

        engine.load_html(
            r#"<!DOCTYPE html>
<meta charset=utf-8>
<title>Debug</title>
<div id="test"><span>1</span><span>2</span></div>
"#,
        );

        // Run the full iterator test inside a try/catch (like the WPT harness does)
        let result = engine
            .eval_js(
                r#"
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
        "#,
            )
            .unwrap();

        eprintln!("Result: {}", result);
        assert!(result.contains("\"status\":0"), "Test failed: {}", result);
    }

    #[test]
    fn htmlcollection_children_named_props() {
        let mut engine = Engine::new();
        engine.load_html(
            r#"<!DOCTYPE html>
<div id="test"><img><img id=foo><img id=foo><img name="bar"></div>"#,
        );
        let result = engine
            .eval_js(
                r#"
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
"#,
            )
            .unwrap();
        assert_eq!(result, "ok", "HTMLCollection test failed: {}", result);
    }
}
