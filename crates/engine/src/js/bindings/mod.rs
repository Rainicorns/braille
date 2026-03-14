pub(crate) mod element;
pub(crate) mod document;
pub(crate) mod class_list;
pub(crate) mod traversal;
pub(crate) mod attributes;
pub(crate) mod node_info;
pub(crate) mod inner_html;
pub(crate) mod mutation;
pub(crate) mod style;
pub(crate) mod window;
pub(crate) mod query;
pub(crate) mod event;
pub(crate) mod event_target;
pub(crate) mod input_props;
pub(crate) mod select_props;
pub(crate) mod anchor_form;
pub(crate) mod html_element;
pub(crate) mod computed_style;
pub(crate) mod character_data;
pub(crate) mod collections;
pub(crate) mod dom_parser;

pub(crate) use document::register_document;

use boa_engine::{
    js_string, native_function::NativeFunction, object::ObjectInitializer, property::Attribute, Context, JsObject,
    JsResult, JsValue,
};

/// Register the global `DOMException` constructor.
/// `new DOMException(message, name)` creates an object with `.message`, `.name`, `.code`.
pub(crate) fn register_dom_exception(ctx: &mut Context) {
    let dom_exception_constructor = NativeFunction::from_fn_ptr(|_this, args, ctx| {
        let message = args
            .first()
            .map(|v| v.to_string(ctx))
            .transpose()?
            .map(|s| s.to_std_string_escaped())
            .unwrap_or_default();
        let name = args
            .get(1)
            .map(|v| v.to_string(ctx))
            .transpose()?
            .map(|s| s.to_std_string_escaped())
            .unwrap_or_else(|| "Error".to_string());

        let code = dom_exception_code(&name);

        // Get DOMException.prototype from the constructor so instances pass instanceof checks
        let global = ctx.global_object();
        let ctor_val = global.get(js_string!("DOMException"), ctx)?;
        let ctor_obj = ctor_val.as_object().expect("DOMException constructor must exist");
        let proto_val = ctor_obj.get(js_string!("prototype"), ctx)?;
        let proto = proto_val.as_object().expect("DOMException.prototype must exist");

        let obj = ObjectInitializer::with_native_data(DomExceptionData, ctx)
            .property(js_string!("message"), js_string!(message), Attribute::CONFIGURABLE | Attribute::WRITABLE)
            .property(js_string!("name"), js_string!(name), Attribute::CONFIGURABLE | Attribute::WRITABLE)
            .property(js_string!("code"), code, Attribute::CONFIGURABLE | Attribute::WRITABLE)
            .build();

        obj.set_prototype(Some(proto.clone()));

        Ok(JsValue::from(obj))
    });

    let proto = ObjectInitializer::new(ctx).build();

    let ctor = ObjectInitializer::new(ctx)
        .function(dom_exception_constructor, js_string!("DOMException"), 2)
        .property(js_string!("prototype"), proto.clone(), Attribute::empty())
        .build();

    // Set constructor on prototype
    proto
        .set(js_string!("constructor"), JsValue::from(ctor.clone()), false, ctx)
        .expect("set constructor on DOMException.prototype");

    ctx.global_object()
        .set(js_string!("DOMException"), JsValue::from(ctor), false, ctx)
        .expect("set DOMException global");
}

/// Map DOMException name → legacy error code.
fn dom_exception_code(name: &str) -> u16 {
    match name {
        "IndexSizeError" => 1,
        "HierarchyRequestError" => 3,
        "WrongDocumentError" => 4,
        "InvalidCharacterError" => 5,
        "NoModificationAllowedError" => 7,
        "NotFoundError" => 8,
        "NotSupportedError" => 9,
        "InUseAttributeError" => 10,
        "InvalidStateError" => 11,
        "SyntaxError" => 12,
        "InvalidModificationError" => 13,
        "NamespaceError" => 14,
        "InvalidAccessError" => 15,
        "SecurityError" => 18,
        "NetworkError" => 19,
        "AbortError" => 20,
        "URLMismatchError" => 21,
        "QuotaExceededError" => 22,
        "TimeoutError" => 23,
        "InvalidNodeTypeError" => 24,
        "DataCloneError" => 25,
        _ => 0,
    }
}

/// NativeData marker for DOMException instances (for potential future use with downcast_ref).
#[derive(Debug, boa_engine::JsData, boa_gc::Trace, boa_gc::Finalize)]
pub(crate) struct DomExceptionData;

/// Create a DOMException JsObject with the given name, message, and code.
/// The returned object has its prototype set to DOMException.prototype so `instanceof` works.
pub(crate) fn create_dom_exception(ctx: &mut Context, name: &str, message: &str, code: u16) -> JsResult<JsObject> {
    let global = ctx.global_object();
    let ctor_val = global.get(js_string!("DOMException"), ctx)?;
    let ctor_obj = ctor_val.as_object().expect("DOMException constructor must exist");
    let proto_val = ctor_obj.get(js_string!("prototype"), ctx)?;
    let proto = proto_val.as_object().expect("DOMException.prototype must exist");

    let obj = ObjectInitializer::with_native_data(DomExceptionData, ctx)
        .property(js_string!("message"), js_string!(message), Attribute::CONFIGURABLE | Attribute::WRITABLE)
        .property(js_string!("name"), js_string!(name), Attribute::CONFIGURABLE | Attribute::WRITABLE)
        .property(js_string!("code"), code, Attribute::CONFIGURABLE | Attribute::WRITABLE)
        .build();

    obj.set_prototype(Some(proto.clone()));

    Ok(obj)
}
