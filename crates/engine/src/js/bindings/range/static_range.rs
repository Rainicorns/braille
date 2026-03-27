//! StaticRange — read-only snapshot range (no live tracking, no methods).

use boa_engine::{
    js_string, native_function::NativeFunction,
    Context, JsError, JsObject, JsValue,
};

use crate::dom::NodeData;
use crate::js::prop_desc;
use crate::js::bindings::element::JsElement;

/// Native data for a StaticRange JS object.
#[derive(Debug, boa_engine::JsData, boa_gc::Trace, boa_gc::Finalize)]
struct JsStaticRange {
    #[unsafe_ignore_trace]
    start_container: JsValue,
    #[unsafe_ignore_trace]
    start_offset: usize,
    #[unsafe_ignore_trace]
    end_container: JsValue,
    #[unsafe_ignore_trace]
    end_offset: usize,
}

fn create_static_range_prototype(ctx: &mut Context) -> JsObject {
    let realm = ctx.realm().clone();
    let proto = JsObject::with_null_proto();

    use prop_desc::readonly_accessor;

    // startContainer
    let getter = NativeFunction::from_fn_ptr(|this, _args, _ctx| {
        let obj = this.as_object().ok_or_else(|| JsError::from_opaque(js_string!("not a StaticRange").into()))?;
        let sr = obj.downcast_ref::<JsStaticRange>().ok_or_else(|| JsError::from_opaque(js_string!("not a StaticRange").into()))?;
        Ok(sr.start_container.clone())
    });
    proto.define_property_or_throw(js_string!("startContainer"), readonly_accessor(getter.to_js_function(&realm)), ctx).expect("sr.startContainer");

    // startOffset
    let getter = NativeFunction::from_fn_ptr(|this, _args, _ctx| {
        let obj = this.as_object().ok_or_else(|| JsError::from_opaque(js_string!("not a StaticRange").into()))?;
        let sr = obj.downcast_ref::<JsStaticRange>().ok_or_else(|| JsError::from_opaque(js_string!("not a StaticRange").into()))?;
        Ok(JsValue::from(sr.start_offset as f64))
    });
    proto.define_property_or_throw(js_string!("startOffset"), readonly_accessor(getter.to_js_function(&realm)), ctx).expect("sr.startOffset");

    // endContainer
    let getter = NativeFunction::from_fn_ptr(|this, _args, _ctx| {
        let obj = this.as_object().ok_or_else(|| JsError::from_opaque(js_string!("not a StaticRange").into()))?;
        let sr = obj.downcast_ref::<JsStaticRange>().ok_or_else(|| JsError::from_opaque(js_string!("not a StaticRange").into()))?;
        Ok(sr.end_container.clone())
    });
    proto.define_property_or_throw(js_string!("endContainer"), readonly_accessor(getter.to_js_function(&realm)), ctx).expect("sr.endContainer");

    // endOffset
    let getter = NativeFunction::from_fn_ptr(|this, _args, _ctx| {
        let obj = this.as_object().ok_or_else(|| JsError::from_opaque(js_string!("not a StaticRange").into()))?;
        let sr = obj.downcast_ref::<JsStaticRange>().ok_or_else(|| JsError::from_opaque(js_string!("not a StaticRange").into()))?;
        Ok(JsValue::from(sr.end_offset as f64))
    });
    proto.define_property_or_throw(js_string!("endOffset"), readonly_accessor(getter.to_js_function(&realm)), ctx).expect("sr.endOffset");

    // collapsed
    let getter = NativeFunction::from_fn_ptr(|this, _args, _ctx| {
        let obj = this.as_object().ok_or_else(|| JsError::from_opaque(js_string!("not a StaticRange").into()))?;
        let sr = obj.downcast_ref::<JsStaticRange>().ok_or_else(|| JsError::from_opaque(js_string!("not a StaticRange").into()))?;
        let collapsed = sr.start_offset == sr.end_offset && sr.start_container == sr.end_container;
        Ok(JsValue::from(collapsed))
    });
    proto.define_property_or_throw(js_string!("collapsed"), readonly_accessor(getter.to_js_function(&realm)), ctx).expect("sr.collapsed");

    // Symbol.toStringTag
    proto
        .define_property_or_throw(
            boa_engine::JsSymbol::to_string_tag(),
            boa_engine::property::PropertyDescriptor::builder()
                .value(js_string!("StaticRange"))
                .configurable(true)
                .build(),
            ctx,
        )
        .expect("StaticRange toStringTag");

    proto
}

pub(crate) fn register_static_range_global(ctx: &mut Context) {
    use boa_engine::object::FunctionObjectBuilder;
    use boa_engine::property::Attribute;

    let proto = create_static_range_prototype(ctx);

    // Store prototype in RealmState
    crate::js::realm_state::set_static_range_proto(ctx, proto.clone());

    let proto_for_ctor = proto.clone();
    let ctor = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx2| {
            // StaticRange(init) — init is required dictionary
            let init = args.first().cloned().unwrap_or(JsValue::undefined());
            let init_obj = init.as_object().ok_or_else(|| {
                JsError::from_native(boa_engine::JsNativeError::typ().with_message(
                    "Failed to construct 'StaticRange': parameter 1 is not of type 'StaticRangeInit'.",
                ))
            })?;

            let start_container = init_obj.get(js_string!("startContainer"), ctx2)?;
            if start_container.is_undefined() || start_container.is_null() {
                return Err(JsError::from_native(
                    boa_engine::JsNativeError::typ().with_message("Failed to construct 'StaticRange': member startContainer is required and must be an instance of Node."),
                ));
            }

            let end_container = init_obj.get(js_string!("endContainer"), ctx2)?;
            if end_container.is_undefined() || end_container.is_null() {
                return Err(JsError::from_native(
                    boa_engine::JsNativeError::typ().with_message("Failed to construct 'StaticRange': member endContainer is required and must be an instance of Node."),
                ));
            }

            let start_offset_val = init_obj.get(js_string!("startOffset"), ctx2)?;
            if start_offset_val.is_undefined() {
                return Err(JsError::from_native(
                    boa_engine::JsNativeError::typ().with_message("Failed to construct 'StaticRange': member startOffset is required."),
                ));
            }
            let start_offset = start_offset_val.to_number(ctx2)? as usize;

            let end_offset_val = init_obj.get(js_string!("endOffset"), ctx2)?;
            if end_offset_val.is_undefined() {
                return Err(JsError::from_native(
                    boa_engine::JsNativeError::typ().with_message("Failed to construct 'StaticRange': member endOffset is required."),
                ));
            }
            let end_offset = end_offset_val.to_number(ctx2)? as usize;

            // Validate containers: must not be DocumentType or Attr
            fn is_invalid_container(container: &JsValue) -> bool {
                if let Some(obj) = container.as_object() {
                    if let Some(el) = obj.downcast_ref::<JsElement>() {
                        let tree_ref = el.tree.borrow();
                        let node = tree_ref.get_node(el.node_id);
                        return matches!(&node.data, NodeData::Doctype { .. } | NodeData::Attr { .. });
                    }
                }
                false
            }
            if is_invalid_container(&start_container) {
                let exc = crate::js::bindings::create_dom_exception(
                    ctx2,
                    "InvalidNodeTypeError",
                    "Failed to construct 'StaticRange': startContainer cannot be a DocumentType or Attr node.",
                    24,
                )?;
                return Err(JsError::from_opaque(exc.into()));
            }
            if is_invalid_container(&end_container) {
                let exc = crate::js::bindings::create_dom_exception(
                    ctx2,
                    "InvalidNodeTypeError",
                    "Failed to construct 'StaticRange': endContainer cannot be a DocumentType or Attr node.",
                    24,
                )?;
                return Err(JsError::from_opaque(exc.into()));
            }

            let data = JsStaticRange {
                start_container: start_container.clone(),
                start_offset,
                end_container: end_container.clone(),
                end_offset,
            };

            let obj = boa_engine::object::ObjectInitializer::with_native_data(data, ctx2).build();
            obj.set_prototype(Some(proto_for_ctor.clone()));
            Ok(obj.into())
        })
    };

    let ctor_fn = FunctionObjectBuilder::new(ctx.realm(), ctor)
        .name(js_string!("StaticRange"))
        .length(1)
        .constructor(true)
        .build();

    // Set StaticRange.prototype
    ctor_fn
        .define_property_or_throw(js_string!("prototype"), prop_desc::prototype_on_ctor(proto.clone()), ctx)
        .expect("StaticRange.prototype");

    // Set constructor on prototype
    proto
        .define_property_or_throw(js_string!("constructor"), prop_desc::constructor_on_proto(ctor_fn.clone()), ctx)
        .expect("proto.constructor");

    // Register as global
    ctx.register_global_property(js_string!("StaticRange"), ctor_fn, Attribute::WRITABLE | Attribute::CONFIGURABLE)
        .expect("register StaticRange global");
}
