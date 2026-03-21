use boa_engine::{
    class::{Class, ClassBuilder},
    js_string,
    native_function::NativeFunction,
    Context, JsResult, JsValue,
};
use boa_gc::{Finalize, Trace};

use super::event_target::{add_event_listener_impl, remove_event_listener_impl, resolve_event_target_key};

/// Minimal XMLHttpRequest stub — only acts as an EventTarget.
/// No HTTP functionality; exists so `new XMLHttpRequest()` works in tests
/// that use it purely as an event target.
#[derive(Debug, Trace, Finalize, boa_engine::JsData)]
pub(crate) struct JsXMLHttpRequest {
    #[unsafe_ignore_trace]
    pub(crate) id: usize,
}

fn xhr_add_event_listener(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let (listener_key, tree_for_passive) = resolve_event_target_key(this, ctx)?;
    add_event_listener_impl(listener_key, tree_for_passive.as_deref(), args, ctx)
}

fn xhr_remove_event_listener(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let (listener_key, _tree) = resolve_event_target_key(this, ctx)?;
    remove_event_listener_impl(listener_key, args, ctx)
}

fn xhr_dispatch_event(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    // Delegate to JsEventTarget's universal dispatch which handles all target types
    super::event_target::JsEventTarget::dispatch_event_public(this, args, ctx)
}

impl Class for JsXMLHttpRequest {
    const NAME: &'static str = "XMLHttpRequest";
    const LENGTH: usize = 0;

    fn data_constructor(_new_target: &JsValue, _args: &[JsValue], _context: &mut Context) -> JsResult<Self> {
        Ok(JsXMLHttpRequest {
            id: super::event_target::next_event_target_id(),
        })
    }

    fn init(class: &mut ClassBuilder) -> JsResult<()> {
        class.method(
            js_string!("addEventListener"),
            2,
            NativeFunction::from_fn_ptr(xhr_add_event_listener),
        );

        class.method(
            js_string!("removeEventListener"),
            2,
            NativeFunction::from_fn_ptr(xhr_remove_event_listener),
        );

        class.method(
            js_string!("dispatchEvent"),
            1,
            NativeFunction::from_fn_ptr(xhr_dispatch_event),
        );

        Ok(())
    }
}
