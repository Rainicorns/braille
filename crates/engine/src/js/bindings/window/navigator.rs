use boa_engine::{
    js_string,
    native_function::NativeFunction,
    object::{builtins::JsArray, ObjectInitializer},
    property::{Attribute, PropertyDescriptor},
    Context, JsValue,
};

pub(super) fn build_navigator(context: &mut Context) -> boa_engine::JsObject {
    let realm = context.realm().clone();

    let navigator = ObjectInitializer::new(context).build();

    // Helper to define a getter property on navigator
    macro_rules! nav_getter {
        ($name:expr, $val:expr) => {
            let getter = unsafe { NativeFunction::from_closure($val) };
            navigator
                .define_property_or_throw(
                    js_string!($name),
                    PropertyDescriptor::builder()
                        .get(getter.to_js_function(&realm))
                        .configurable(true)
                        .enumerable(true)
                        .build(),
                    context,
                )
                .expect(concat!("failed to define navigator.", $name));
        };
    }

    nav_getter!("userAgent", |_this, _args, _ctx| Ok(JsValue::from(
        js_string!("Braille/0.1")
    )));
    nav_getter!("language", |_this, _args, _ctx| Ok(JsValue::from(
        js_string!("en-US")
    )));
    nav_getter!("platform", |_this, _args, _ctx| Ok(JsValue::from(
        js_string!("Linux")
    )));
    nav_getter!("onLine", |_this, _args, _ctx| Ok(JsValue::from(true)));
    nav_getter!("cookieEnabled", |_this, _args, _ctx| Ok(JsValue::from(
        false
    )));
    nav_getter!("maxTouchPoints", |_this, _args, _ctx| Ok(JsValue::from(
        0
    )));
    nav_getter!("hardwareConcurrency", |_this, _args, _ctx| Ok(
        JsValue::from(1)
    ));

    // languages — frozen array ["en-US", "en"]
    let languages_getter = unsafe {
        NativeFunction::from_closure(|_this, _args, ctx| {
            let arr = JsArray::new(ctx);
            arr.push(JsValue::from(js_string!("en-US")), ctx)?;
            arr.push(JsValue::from(js_string!("en")), ctx)?;
            let arr_obj: JsValue = arr.into();
            let frozen = ctx.global_object()
                .get(js_string!("Object"), ctx)?
                .as_object()
                .unwrap()
                .get(js_string!("freeze"), ctx)?
                .as_callable()
                .unwrap()
                .call(&JsValue::undefined(), std::slice::from_ref(&arr_obj), ctx)?;
            Ok(frozen)
        })
    };
    navigator
        .define_property_or_throw(
            js_string!("languages"),
            PropertyDescriptor::builder()
                .get(languages_getter.to_js_function(&realm))
                .configurable(true)
                .enumerable(true)
                .build(),
            context,
        )
        .expect("failed to define navigator.languages");

    // mediaDevices — empty object
    let media_devices = ObjectInitializer::new(context).build();
    navigator
        .define_property_or_throw(
            js_string!("mediaDevices"),
            PropertyDescriptor::builder()
                .value(media_devices)
                .writable(true)
                .configurable(true)
                .enumerable(true)
                .build(),
            context,
        )
        .expect("failed to define navigator.mediaDevices");

    // clipboard — empty object
    let clipboard = ObjectInitializer::new(context).build();
    navigator
        .define_property_or_throw(
            js_string!("clipboard"),
            PropertyDescriptor::builder()
                .value(clipboard)
                .writable(true)
                .configurable(true)
                .enumerable(true)
                .build(),
            context,
        )
        .expect("failed to define navigator.clipboard");

    // serviceWorker — object with register() returning rejected Promise
    let sw_register = NativeFunction::from_fn_ptr(|_this, _args, ctx| {
        use boa_engine::object::builtins::JsPromise;
        let err = boa_engine::JsNativeError::typ().with_message("Service workers are not supported");
        let promise = JsPromise::reject(err, ctx);
        Ok(JsValue::from(promise))
    });
    let service_worker = ObjectInitializer::new(context)
        .function(sw_register, js_string!("register"), 1)
        .build();
    navigator
        .define_property_or_throw(
            js_string!("serviceWorker"),
            PropertyDescriptor::builder()
                .value(service_worker)
                .writable(true)
                .configurable(true)
                .enumerable(true)
                .build(),
            context,
        )
        .expect("failed to define navigator.serviceWorker");

    // permissions — object with query() returning Promise resolving {state: "denied"}
    let perm_query = NativeFunction::from_fn_ptr(|_this, _args, ctx| {
        use boa_engine::object::builtins::JsPromise;
        let result = ObjectInitializer::new(ctx)
            .property(js_string!("state"), js_string!("denied"), Attribute::all())
            .build();
        let promise = JsPromise::resolve(JsValue::from(result), ctx);
        Ok(JsValue::from(promise))
    });
    let permissions = ObjectInitializer::new(context)
        .function(perm_query, js_string!("query"), 1)
        .build();
    navigator
        .define_property_or_throw(
            js_string!("permissions"),
            PropertyDescriptor::builder()
                .value(permissions)
                .writable(true)
                .configurable(true)
                .enumerable(true)
                .build(),
            context,
        )
        .expect("failed to define navigator.permissions");

    navigator
}
