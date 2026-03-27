use boa_engine::{
    js_string,
    native_function::NativeFunction,
    object::ObjectInitializer,
    property::{Attribute, PropertyDescriptor},
    Context, JsValue,
};

/// Build a matchMedia stub: returns a MediaQueryList-like object with matches=false.
pub(super) fn build_match_media(_context: &mut Context) -> NativeFunction {
    unsafe {
        NativeFunction::from_closure(|_this, args, ctx| {
            let query = args
                .first()
                .and_then(|v| v.as_string())
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_default();

            let noop = NativeFunction::from_fn_ptr(|_, _, _| Ok(JsValue::undefined()));
            let noop_fn = noop.to_js_function(ctx.realm());

            let mql = ObjectInitializer::new(ctx)
                .property(js_string!("matches"), false, Attribute::all())
                .property(js_string!("media"), js_string!(query), Attribute::all())
                .property(js_string!("onchange"), JsValue::null(), Attribute::all())
                .build();

            // addEventListener / removeEventListener / addListener / removeListener — all no-ops
            for name in &["addEventListener", "removeEventListener", "addListener", "removeListener"] {
                mql.define_property_or_throw(
                    js_string!(*name),
                    PropertyDescriptor::builder()
                        .value(noop_fn.clone())
                        .writable(true)
                        .configurable(true)
                        .enumerable(false)
                        .build(),
                    ctx,
                )?;
            }

            Ok(JsValue::from(mql))
        })
    }
}
