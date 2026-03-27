use boa_engine::{
    js_string,
    native_function::NativeFunction,
    object::{FunctionObjectBuilder, ObjectInitializer},
    property::{Attribute, PropertyDescriptor},
    Context, JsError, JsObject, JsValue, Source,
};

use crate::js::realm_state;

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
