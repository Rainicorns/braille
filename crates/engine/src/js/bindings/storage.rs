use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use boa_engine::{
    js_string,
    native_function::NativeFunction,
    object::{FunctionObjectBuilder, ObjectInitializer},
    property::PropertyDescriptor,
    Context, JsObject, JsValue,
};

use crate::js::prop_desc;

// ---------------------------------------------------------------------------
// JsStorage — native data wrapping an in-memory key/value store
// ---------------------------------------------------------------------------

#[derive(Debug, boa_engine::JsData, boa_gc::Trace, boa_gc::Finalize)]
pub(crate) struct JsStorage {
    #[unsafe_ignore_trace]
    store: Rc<RefCell<HashMap<String, String>>>,
}

fn get_storage(this: &JsValue, method: &str) -> Result<Rc<RefCell<HashMap<String, String>>>, boa_engine::JsError> {
    let obj = this
        .as_object()
        .ok_or_else(|| boa_engine::JsNativeError::typ().with_message(format!("Storage.{method} called on non-object")))?;
    let data = obj
        .downcast_ref::<JsStorage>()
        .ok_or_else(|| boa_engine::JsNativeError::typ().with_message(format!("Storage.{method} called on non-Storage")))?;
    Ok(Rc::clone(&data.store))
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

pub(crate) fn register_storage_globals(ctx: &mut Context) {
    let proto = ObjectInitializer::new(ctx).build();
    let realm = ctx.realm().clone();

    // getItem(key) → value or null
    let get_item_fn = NativeFunction::from_fn_ptr(|this, args, ctx| {
        let store = get_storage(this, "getItem")?;
        let key = args.first().map(|v| v.to_string(ctx)).transpose()?.map(|s| s.to_std_string_escaped()).unwrap_or_default();
        let map = store.borrow();
        match map.get(&key) {
            Some(v) => Ok(JsValue::from(js_string!(v.clone()))),
            None => Ok(JsValue::null()),
        }
    });

    // setItem(key, value)
    let set_item_fn = NativeFunction::from_fn_ptr(|this, args, ctx| {
        let store = get_storage(this, "setItem")?;
        let key = args.first().map(|v| v.to_string(ctx)).transpose()?.map(|s| s.to_std_string_escaped()).unwrap_or_default();
        let value = args.get(1).map(|v| v.to_string(ctx)).transpose()?.map(|s| s.to_std_string_escaped()).unwrap_or_default();
        store.borrow_mut().insert(key, value);
        Ok(JsValue::undefined())
    });

    // removeItem(key)
    let remove_item_fn = NativeFunction::from_fn_ptr(|this, args, ctx| {
        let store = get_storage(this, "removeItem")?;
        let key = args.first().map(|v| v.to_string(ctx)).transpose()?.map(|s| s.to_std_string_escaped()).unwrap_or_default();
        store.borrow_mut().remove(&key);
        Ok(JsValue::undefined())
    });

    // clear()
    let clear_fn = NativeFunction::from_fn_ptr(|this, _args, _ctx| {
        let store = get_storage(this, "clear")?;
        store.borrow_mut().clear();
        Ok(JsValue::undefined())
    });

    // key(index) → nth key or null
    let key_fn = NativeFunction::from_fn_ptr(|this, args, ctx| {
        let store = get_storage(this, "key")?;
        let index = args.first().map(|v| v.to_u32(ctx)).transpose()?.unwrap_or(0) as usize;
        let map = store.borrow();
        // HashMap iteration order isn't spec-guaranteed to be insertion order,
        // but for MVP this is acceptable
        match map.keys().nth(index) {
            Some(k) => Ok(JsValue::from(js_string!(k.clone()))),
            None => Ok(JsValue::null()),
        }
    });

    // length getter
    let length_get = NativeFunction::from_fn_ptr(|this, _args, _ctx| {
        let store = get_storage(this, "length")?;
        let len = store.borrow().len() as u32;
        Ok(JsValue::from(len))
    });

    // Define methods on prototype
    for (name, func) in [
        ("getItem", &get_item_fn),
        ("setItem", &set_item_fn),
        ("removeItem", &remove_item_fn),
        ("clear", &clear_fn),
        ("key", &key_fn),
    ] {
        proto
            .define_property_or_throw(js_string!(name), prop_desc::data_prop(func.clone().to_js_function(&realm)), ctx)
            .unwrap_or_else(|_| panic!("define Storage.prototype.{name}"));
    }

    // length as getter
    proto
        .define_property_or_throw(
            js_string!("length"),
            prop_desc::readonly_accessor(length_get.to_js_function(&realm)),
            ctx,
        )
        .expect("define Storage.prototype.length");

    // Symbol.toStringTag
    proto
        .define_property_or_throw(
            boa_engine::JsSymbol::to_string_tag(),
            PropertyDescriptor::builder()
                .value(js_string!("Storage"))
                .writable(false)
                .configurable(true)
                .enumerable(false)
                .build(),
            ctx,
        )
        .expect("define Storage.prototype[Symbol.toStringTag]");

    // Constructor (not typically called directly, but needed for prototype chain)
    let proto_for_ctor = proto.clone();
    let storage_ctor_fn = NativeFunction::from_fn_ptr(move |_this, _args, _ctx| {
        Err(boa_engine::JsNativeError::typ()
            .with_message("Illegal constructor")
            .into())
    });

    let ctor: JsObject = FunctionObjectBuilder::new(ctx.realm(), storage_ctor_fn)
        .name(js_string!("Storage"))
        .length(0)
        .constructor(true)
        .build()
        .into();

    ctor.define_property_or_throw(js_string!("prototype"), prop_desc::prototype_on_ctor(proto.clone()), ctx)
        .expect("set Storage.prototype");
    proto
        .define_property_or_throw(js_string!("constructor"), prop_desc::constructor_on_proto(ctor.clone()), ctx)
        .expect("set Storage.prototype.constructor");

    // Create localStorage and sessionStorage instances
    let make_storage_instance = |ctx: &mut Context| -> JsObject {
        let data = JsStorage {
            store: Rc::new(RefCell::new(HashMap::new())),
        };
        let obj = ObjectInitializer::with_native_data(data, ctx).build();
        obj.set_prototype(Some(proto_for_ctor.clone()));
        obj
    };

    let local_storage = make_storage_instance(ctx);
    let session_storage = make_storage_instance(ctx);

    // Register as globals
    ctx.global_object()
        .set(js_string!("localStorage"), JsValue::from(local_storage.clone()), false, ctx)
        .expect("set localStorage global");
    ctx.global_object()
        .set(js_string!("sessionStorage"), JsValue::from(session_storage.clone()), false, ctx)
        .expect("set sessionStorage global");

    // Also register on window if it exists
    let window: Option<JsObject> = ctx.global_object().get(js_string!("window"), ctx).ok().and_then(|v| v.as_object());
    if let Some(win) = window {
        win.set(js_string!("localStorage"), JsValue::from(local_storage), false, ctx)
            .expect("set window.localStorage");
        win.set(js_string!("sessionStorage"), JsValue::from(session_storage), false, ctx)
            .expect("set window.sessionStorage");
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use crate::dom::DomTree;
    use crate::js::JsRuntime;

    fn make_runtime() -> JsRuntime {
        let tree = Rc::new(RefCell::new(DomTree::new()));
        {
            let mut t = tree.borrow_mut();
            let html = t.create_element("html");
            let body = t.create_element("body");
            let doc = t.document();
            t.append_child(doc, html);
            t.append_child(html, body);
        }
        JsRuntime::new(tree)
    }

    #[test]
    fn local_storage_set_get() {
        let mut rt = make_runtime();
        let r = rt.eval(r#"
            localStorage.setItem('key', 'value');
            localStorage.getItem('key') === 'value'
        "#).unwrap();
        assert_eq!(r.as_boolean(), Some(true));
    }

    #[test]
    fn local_storage_get_missing_returns_null() {
        let mut rt = make_runtime();
        let r = rt.eval("localStorage.getItem('nope') === null").unwrap();
        assert_eq!(r.as_boolean(), Some(true));
    }

    #[test]
    fn local_storage_remove_item() {
        let mut rt = make_runtime();
        let r = rt.eval(r#"
            localStorage.setItem('a', '1');
            localStorage.removeItem('a');
            localStorage.getItem('a') === null
        "#).unwrap();
        assert_eq!(r.as_boolean(), Some(true));
    }

    #[test]
    fn local_storage_clear() {
        let mut rt = make_runtime();
        let r = rt.eval(r#"
            localStorage.setItem('a', '1');
            localStorage.setItem('b', '2');
            localStorage.clear();
            localStorage.length === 0
        "#).unwrap();
        assert_eq!(r.as_boolean(), Some(true));
    }

    #[test]
    fn local_storage_length() {
        let mut rt = make_runtime();
        let r = rt.eval(r#"
            localStorage.setItem('x', '1');
            localStorage.setItem('y', '2');
            localStorage.length === 2
        "#).unwrap();
        assert_eq!(r.as_boolean(), Some(true));
    }

    #[test]
    fn session_storage_independent() {
        let mut rt = make_runtime();
        let r = rt.eval(r#"
            localStorage.setItem('shared', 'local');
            sessionStorage.setItem('shared', 'session');
            localStorage.getItem('shared') === 'local' && sessionStorage.getItem('shared') === 'session'
        "#).unwrap();
        assert_eq!(r.as_boolean(), Some(true));
    }

    #[test]
    fn storage_on_window() {
        let mut rt = make_runtime();
        let r = rt.eval("window.localStorage === localStorage && window.sessionStorage === sessionStorage").unwrap();
        assert_eq!(r.as_boolean(), Some(true));
    }
}
