use std::cell::RefCell;

use boa_engine::{
    class::{Class, ClassBuilder},
    js_string,
    native_function::NativeFunction,
    object::JsObject,
    property::{Attribute, PropertyDescriptor},
    Context, JsData, JsError, JsResult, JsValue,
};
use boa_gc::{Finalize, Trace};

use crate::dom::NodeId;

// Cached getter for isTrusted — same JsObject across all Event instances.
thread_local! {
    static IS_TRUSTED_GETTER: RefCell<Option<JsObject>> = const { RefCell::new(None) };
}

/// Attach `isTrusted` as an own accessor property on the given event object.
/// Uses a cached getter function so that all instances share the same getter
/// (required by the spec: `desc1.get === desc2.get`).
pub(crate) fn attach_is_trusted_own_property(event_obj: &JsObject, ctx: &mut Context) -> JsResult<()> {
    let getter = IS_TRUSTED_GETTER.with(|cell| {
        let mut opt = cell.borrow_mut();
        if let Some(ref g) = *opt {
            g.clone()
        } else {
            let realm = ctx.realm().clone();
            let fn_obj = NativeFunction::from_fn_ptr(JsEvent::get_is_trusted)
                .to_js_function(&realm);
            // to_js_function returns a JsFunction which we store as JsObject
            let g: JsObject = fn_obj.into();
            *opt = Some(g.clone());
            g
        }
    });

    event_obj.define_property_or_throw(
        js_string!("isTrusted"),
        PropertyDescriptor::builder()
            .get(getter)
            .set(JsValue::undefined())
            .configurable(false)
            .enumerable(true)
            .build(),
        ctx,
    )?;

    Ok(())
}

// ---------------------------------------------------------------------------
// JsEvent — the Class-based wrapper for DOM Event
// ---------------------------------------------------------------------------

#[derive(Debug, Trace, Finalize, JsData)]
pub(crate) struct JsEvent {
    #[unsafe_ignore_trace]
    pub(crate) event_type: String,
    pub(crate) bubbles: bool,
    pub(crate) cancelable: bool,
    pub(crate) default_prevented: bool,
    pub(crate) propagation_stopped: bool,
    pub(crate) immediate_propagation_stopped: bool,
    #[unsafe_ignore_trace]
    pub(crate) target: Option<NodeId>,
    #[unsafe_ignore_trace]
    pub(crate) current_target: Option<NodeId>,
    pub(crate) phase: u8,
    pub(crate) dispatching: bool,
}

impl JsEvent {
    // -- New getters for WPT --

    fn get_is_trusted(_this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
        Ok(JsValue::from(false))
    }

    fn get_time_stamp(_this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
        // Return a positive number (spec says DOMHighResTimeStamp)
        Ok(JsValue::from(1))
    }

    fn get_composed(_this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
        Ok(JsValue::from(false))
    }

    fn get_src_element(_this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
        Ok(JsValue::null())
    }

    fn get_cancel_bubble(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
        let obj = this
            .as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("Event.cancelBubble getter: not an object").into()))?;
        let evt = obj
            .downcast_ref::<JsEvent>()
            .ok_or_else(|| JsError::from_opaque(js_string!("Event.cancelBubble getter: not an Event").into()))?;
        Ok(JsValue::from(evt.propagation_stopped))
    }

    fn set_cancel_bubble(this: &JsValue, args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
        let value = args.first().map(|v| v.to_boolean()).unwrap_or(false);
        if value {
            let obj = this
                .as_object()
                .ok_or_else(|| JsError::from_opaque(js_string!("Event.cancelBubble setter: not an object").into()))?;
            let mut evt = obj
                .downcast_mut::<JsEvent>()
                .ok_or_else(|| JsError::from_opaque(js_string!("Event.cancelBubble setter: not an Event").into()))?;
            evt.propagation_stopped = true;
        }
        Ok(JsValue::undefined())
    }

    fn get_return_value(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
        let obj = this
            .as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("Event.returnValue getter: not an object").into()))?;
        let evt = obj
            .downcast_ref::<JsEvent>()
            .ok_or_else(|| JsError::from_opaque(js_string!("Event.returnValue getter: not an Event").into()))?;
        Ok(JsValue::from(!evt.default_prevented))
    }

    fn set_return_value(this: &JsValue, args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
        let value = args.first().map(|v| v.to_boolean()).unwrap_or(true);
        if !value {
            let obj = this
                .as_object()
                .ok_or_else(|| JsError::from_opaque(js_string!("Event.returnValue setter: not an object").into()))?;
            let mut evt = obj
                .downcast_mut::<JsEvent>()
                .ok_or_else(|| JsError::from_opaque(js_string!("Event.returnValue setter: not an Event").into()))?;
            if evt.cancelable {
                evt.default_prevented = true;
            }
        }
        Ok(JsValue::undefined())
    }

    fn init_event(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
        let obj = this
            .as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("Event.initEvent: not an object").into()))?;
        let mut evt = obj
            .downcast_mut::<JsEvent>()
            .ok_or_else(|| JsError::from_opaque(js_string!("Event.initEvent: not an Event").into()))?;

        // Per spec: if dispatching flag is set, return (no-op)
        if evt.dispatching {
            return Ok(JsValue::undefined());
        }

        let event_type = args
            .first()
            .ok_or_else(|| {
                JsError::from_native(
                    boa_engine::JsNativeError::typ()
                        .with_message("Failed to execute 'initEvent' on 'Event': 1 argument required, but only 0 present.")
                )
            })?
            .to_string(ctx)?
            .to_std_string_escaped();
        let bubbles = args.get(1).map(|v| v.to_boolean()).unwrap_or(false);
        let cancelable = args.get(2).map(|v| v.to_boolean()).unwrap_or(false);
        evt.event_type = event_type;
        evt.bubbles = bubbles;
        evt.cancelable = cancelable;
        evt.default_prevented = false;
        evt.propagation_stopped = false;
        evt.immediate_propagation_stopped = false;
        Ok(JsValue::undefined())
    }

    // -- Getters --

    fn get_type(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
        let obj = this
            .as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("Event.type getter: `this` is not an object").into()))?;
        let evt = obj
            .downcast_ref::<JsEvent>()
            .ok_or_else(|| JsError::from_opaque(js_string!("Event.type getter: `this` is not an Event").into()))?;
        Ok(JsValue::from(js_string!(evt.event_type.clone())))
    }

    fn get_bubbles(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
        let obj = this
            .as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("Event.bubbles getter: `this` is not an object").into()))?;
        let evt = obj
            .downcast_ref::<JsEvent>()
            .ok_or_else(|| JsError::from_opaque(js_string!("Event.bubbles getter: `this` is not an Event").into()))?;
        Ok(JsValue::from(evt.bubbles))
    }

    fn get_cancelable(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
        let obj = this
            .as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("Event.cancelable getter: `this` is not an object").into()))?;
        let evt = obj
            .downcast_ref::<JsEvent>()
            .ok_or_else(|| JsError::from_opaque(js_string!("Event.cancelable getter: `this` is not an Event").into()))?;
        Ok(JsValue::from(evt.cancelable))
    }

    fn get_default_prevented(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
        let obj = this
            .as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("Event.defaultPrevented getter: `this` is not an object").into()))?;
        let evt = obj
            .downcast_ref::<JsEvent>()
            .ok_or_else(|| JsError::from_opaque(js_string!("Event.defaultPrevented getter: `this` is not an Event").into()))?;
        Ok(JsValue::from(evt.default_prevented))
    }

    fn get_target(_this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
        // Target is set during dispatch; for now always return null
        Ok(JsValue::null())
    }

    fn get_current_target(_this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
        // currentTarget is set during dispatch; for now always return null
        Ok(JsValue::null())
    }

    fn get_event_phase(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
        let obj = this
            .as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("Event.eventPhase getter: `this` is not an object").into()))?;
        let evt = obj
            .downcast_ref::<JsEvent>()
            .ok_or_else(|| JsError::from_opaque(js_string!("Event.eventPhase getter: `this` is not an Event").into()))?;
        Ok(JsValue::from(evt.phase as i32))
    }

    // -- Methods --

    fn prevent_default(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
        let obj = this
            .as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("Event.preventDefault: `this` is not an object").into()))?;
        let mut evt = obj
            .downcast_mut::<JsEvent>()
            .ok_or_else(|| JsError::from_opaque(js_string!("Event.preventDefault: `this` is not an Event").into()))?;
        if evt.cancelable {
            evt.default_prevented = true;
        }
        Ok(JsValue::undefined())
    }

    fn stop_propagation(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
        let obj = this
            .as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("Event.stopPropagation: `this` is not an object").into()))?;
        let mut evt = obj
            .downcast_mut::<JsEvent>()
            .ok_or_else(|| JsError::from_opaque(js_string!("Event.stopPropagation: `this` is not an Event").into()))?;
        evt.propagation_stopped = true;
        Ok(JsValue::undefined())
    }

    fn stop_immediate_propagation(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
        let obj = this
            .as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("Event.stopImmediatePropagation: `this` is not an object").into()))?;
        let mut evt = obj
            .downcast_mut::<JsEvent>()
            .ok_or_else(|| JsError::from_opaque(js_string!("Event.stopImmediatePropagation: `this` is not an Event").into()))?;
        evt.propagation_stopped = true;
        evt.immediate_propagation_stopped = true;
        Ok(JsValue::undefined())
    }
}

/// Helper: extract `bubbles` and `cancelable` booleans from the options object argument.
fn parse_event_options(args: &[JsValue], ctx: &mut Context) -> JsResult<(bool, bool)> {
    let mut bubbles = false;
    let mut cancelable = false;

    if let Some(opts_val) = args.get(1) {
        if let Some(opts_obj) = opts_val.as_object() {
            let b = opts_obj.get(js_string!("bubbles"), ctx)?;
            if !b.is_undefined() {
                bubbles = b.to_boolean();
            }
            let c = opts_obj.get(js_string!("cancelable"), ctx)?;
            if !c.is_undefined() {
                cancelable = c.to_boolean();
            }
        }
    }

    Ok((bubbles, cancelable))
}

/// Helper: extract the event type string from the first argument.
fn parse_event_type(args: &[JsValue], ctx: &mut Context) -> JsResult<String> {
    let event_type = args
        .first()
        .ok_or_else(|| {
            JsError::from_native(
                boa_engine::JsNativeError::typ()
                    .with_message("Failed to execute 'Event' constructor: 1 argument required, but only 0 present.")
            )
        })?
        .to_string(ctx)?
        .to_std_string_escaped();
    Ok(event_type)
}

/// Register the phase constants (NONE, CAPTURING_PHASE, AT_TARGET, BUBBLING_PHASE)
/// on the Event constructor object and Event.prototype.
pub(crate) fn register_event_constants(ctx: &mut Context) {
    let global = ctx.global_object();
    let event_constructor = global
        .get(js_string!("Event"), ctx)
        .expect("Event constructor should exist after registration");

    let constants: &[(&str, i32)] = &[
        ("NONE", 0),
        ("CAPTURING_PHASE", 1),
        ("AT_TARGET", 2),
        ("BUBBLING_PHASE", 3),
    ];

    if let Some(event_obj) = event_constructor.as_object() {
        // Put constants on both the constructor and its prototype
        let prototype = event_obj
            .get(js_string!("prototype"), ctx)
            .expect("Event.prototype should exist");
        let proto_obj = prototype.as_object().expect("Event.prototype should be an object");

        // Also get CustomEvent constructor + prototype
        let custom_event_constructor = global
            .get(js_string!("CustomEvent"), ctx)
            .expect("CustomEvent constructor should exist");
        let custom_proto_val = custom_event_constructor
            .as_object()
            .and_then(|obj| obj.get(js_string!("prototype"), ctx).ok());

        for (name, value) in constants {
            let desc = boa_engine::property::PropertyDescriptor::builder()
                .value(JsValue::from(*value))
                .writable(false)
                .configurable(false)
                .enumerable(true)
                .build();

            event_obj
                .define_property_or_throw(js_string!(*name), desc.clone(), ctx)
                .unwrap_or_else(|_| panic!("failed to define Event.{name}"));

            proto_obj
                .define_property_or_throw(js_string!(*name), desc.clone(), ctx)
                .unwrap_or_else(|_| panic!("failed to define Event.prototype.{name}"));

            if let Some(cp) = custom_proto_val.as_ref().and_then(|v| v.as_object()) {
                cp.define_property_or_throw(js_string!(*name), desc, ctx)
                    .unwrap_or_else(|_| panic!("failed to define CustomEvent.prototype.{name}"));
            }
        }
    }
}

impl Class for JsEvent {
    const NAME: &'static str = "Event";
    const LENGTH: usize = 1;

    fn data_constructor(
        _new_target: &JsValue,
        args: &[JsValue],
        ctx: &mut Context,
    ) -> JsResult<Self> {
        let event_type = parse_event_type(args, ctx)?;
        let (bubbles, cancelable) = parse_event_options(args, ctx)?;

        Ok(JsEvent {
            event_type,
            bubbles,
            cancelable,
            default_prevented: false,
            propagation_stopped: false,
            immediate_propagation_stopped: false,
            target: None,
            current_target: None,
            phase: 0,
            dispatching: false,
        })
    }

    fn init(class: &mut ClassBuilder) -> JsResult<()> {
        let realm = class.context().realm().clone();

        // Read-only getters
        let type_getter = NativeFunction::from_fn_ptr(Self::get_type);
        class.accessor(
            js_string!("type"),
            Some(type_getter.to_js_function(&realm)),
            None,
            Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
        );

        let bubbles_getter = NativeFunction::from_fn_ptr(Self::get_bubbles);
        class.accessor(
            js_string!("bubbles"),
            Some(bubbles_getter.to_js_function(&realm)),
            None,
            Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
        );

        let cancelable_getter = NativeFunction::from_fn_ptr(Self::get_cancelable);
        class.accessor(
            js_string!("cancelable"),
            Some(cancelable_getter.to_js_function(&realm)),
            None,
            Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
        );

        let default_prevented_getter = NativeFunction::from_fn_ptr(Self::get_default_prevented);
        class.accessor(
            js_string!("defaultPrevented"),
            Some(default_prevented_getter.to_js_function(&realm)),
            None,
            Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
        );

        let target_getter = NativeFunction::from_fn_ptr(Self::get_target);
        class.accessor(
            js_string!("target"),
            Some(target_getter.to_js_function(&realm)),
            None,
            Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
        );

        let current_target_getter = NativeFunction::from_fn_ptr(Self::get_current_target);
        class.accessor(
            js_string!("currentTarget"),
            Some(current_target_getter.to_js_function(&realm)),
            None,
            Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
        );

        let event_phase_getter = NativeFunction::from_fn_ptr(Self::get_event_phase);
        class.accessor(
            js_string!("eventPhase"),
            Some(event_phase_getter.to_js_function(&realm)),
            None,
            Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
        );

        // Methods
        class.method(
            js_string!("preventDefault"),
            0,
            NativeFunction::from_fn_ptr(Self::prevent_default),
        );

        class.method(
            js_string!("stopPropagation"),
            0,
            NativeFunction::from_fn_ptr(Self::stop_propagation),
        );

        class.method(
            js_string!("stopImmediatePropagation"),
            0,
            NativeFunction::from_fn_ptr(Self::stop_immediate_propagation),
        );

        class.method(
            js_string!("initEvent"),
            3,
            NativeFunction::from_fn_ptr(Self::init_event),
        );

        // isTrusted is NOT on the prototype — it's added as an own property on each instance
        // (see attach_is_trusted_own_property and the constructor wrappers in runtime.rs)

        // timeStamp (read-only)
        let time_stamp_getter = NativeFunction::from_fn_ptr(Self::get_time_stamp);
        class.accessor(
            js_string!("timeStamp"),
            Some(time_stamp_getter.to_js_function(&realm)),
            None,
            Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
        );

        // composed (read-only)
        let composed_getter = NativeFunction::from_fn_ptr(Self::get_composed);
        class.accessor(
            js_string!("composed"),
            Some(composed_getter.to_js_function(&realm)),
            None,
            Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
        );

        // srcElement (legacy alias for target)
        let src_element_getter = NativeFunction::from_fn_ptr(Self::get_src_element);
        class.accessor(
            js_string!("srcElement"),
            Some(src_element_getter.to_js_function(&realm)),
            None,
            Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
        );

        // cancelBubble (getter/setter)
        let cancel_bubble_getter = NativeFunction::from_fn_ptr(Self::get_cancel_bubble);
        let cancel_bubble_setter = NativeFunction::from_fn_ptr(Self::set_cancel_bubble);
        class.accessor(
            js_string!("cancelBubble"),
            Some(cancel_bubble_getter.to_js_function(&realm)),
            Some(cancel_bubble_setter.to_js_function(&realm)),
            Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
        );

        // returnValue (getter/setter)
        let return_value_getter = NativeFunction::from_fn_ptr(Self::get_return_value);
        let return_value_setter = NativeFunction::from_fn_ptr(Self::set_return_value);
        class.accessor(
            js_string!("returnValue"),
            Some(return_value_getter.to_js_function(&realm)),
            Some(return_value_setter.to_js_function(&realm)),
            Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
        );

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// JsCustomEvent — extends Event with a `detail` property
// ---------------------------------------------------------------------------

#[derive(Debug, Trace, Finalize, JsData)]
pub(crate) struct JsCustomEvent {
    #[unsafe_ignore_trace]
    pub(crate) event_type: String,
    pub(crate) bubbles: bool,
    pub(crate) cancelable: bool,
    pub(crate) default_prevented: bool,
    pub(crate) propagation_stopped: bool,
    pub(crate) immediate_propagation_stopped: bool,
    pub(crate) detail: JsValue,
    #[unsafe_ignore_trace]
    pub(crate) target: Option<NodeId>,
    #[unsafe_ignore_trace]
    pub(crate) current_target: Option<NodeId>,
    pub(crate) phase: u8,
    pub(crate) dispatching: bool,
}

impl JsCustomEvent {
    // -- Getters --

    fn get_type(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
        let obj = this
            .as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("CustomEvent.type getter: `this` is not an object").into()))?;
        let evt = obj
            .downcast_ref::<JsCustomEvent>()
            .ok_or_else(|| JsError::from_opaque(js_string!("CustomEvent.type getter: `this` is not a CustomEvent").into()))?;
        Ok(JsValue::from(js_string!(evt.event_type.clone())))
    }

    fn get_bubbles(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
        let obj = this
            .as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("CustomEvent.bubbles getter: `this` is not an object").into()))?;
        let evt = obj
            .downcast_ref::<JsCustomEvent>()
            .ok_or_else(|| JsError::from_opaque(js_string!("CustomEvent.bubbles getter: `this` is not a CustomEvent").into()))?;
        Ok(JsValue::from(evt.bubbles))
    }

    fn get_cancelable(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
        let obj = this
            .as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("CustomEvent.cancelable getter: `this` is not an object").into()))?;
        let evt = obj
            .downcast_ref::<JsCustomEvent>()
            .ok_or_else(|| JsError::from_opaque(js_string!("CustomEvent.cancelable getter: `this` is not a CustomEvent").into()))?;
        Ok(JsValue::from(evt.cancelable))
    }

    fn get_default_prevented(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
        let obj = this
            .as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("CustomEvent.defaultPrevented getter: `this` is not an object").into()))?;
        let evt = obj
            .downcast_ref::<JsCustomEvent>()
            .ok_or_else(|| JsError::from_opaque(js_string!("CustomEvent.defaultPrevented getter: `this` is not a CustomEvent").into()))?;
        Ok(JsValue::from(evt.default_prevented))
    }

    fn get_target(_this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
        Ok(JsValue::null())
    }

    fn get_current_target(_this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
        Ok(JsValue::null())
    }

    fn get_event_phase(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
        let obj = this
            .as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("CustomEvent.eventPhase getter: `this` is not an object").into()))?;
        let evt = obj
            .downcast_ref::<JsCustomEvent>()
            .ok_or_else(|| JsError::from_opaque(js_string!("CustomEvent.eventPhase getter: `this` is not a CustomEvent").into()))?;
        Ok(JsValue::from(evt.phase as i32))
    }

    fn get_detail(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
        let obj = this
            .as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("CustomEvent.detail getter: `this` is not an object").into()))?;
        let evt = obj
            .downcast_ref::<JsCustomEvent>()
            .ok_or_else(|| JsError::from_opaque(js_string!("CustomEvent.detail getter: `this` is not a CustomEvent").into()))?;
        Ok(evt.detail.clone())
    }

    // -- WPT getters --

    fn get_is_trusted(_this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
        Ok(JsValue::from(false))
    }

    fn get_time_stamp(_this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
        Ok(JsValue::from(1))
    }

    fn get_composed(_this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
        Ok(JsValue::from(false))
    }

    fn get_src_element(_this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
        Ok(JsValue::null())
    }

    fn get_cancel_bubble(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
        let obj = this.as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("CustomEvent.cancelBubble getter: not an object").into()))?;
        let evt = obj.downcast_ref::<JsCustomEvent>()
            .ok_or_else(|| JsError::from_opaque(js_string!("CustomEvent.cancelBubble getter: not a CustomEvent").into()))?;
        Ok(JsValue::from(evt.propagation_stopped))
    }

    fn set_cancel_bubble(this: &JsValue, args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
        let value = args.first().map(|v| v.to_boolean()).unwrap_or(false);
        if value {
            let obj = this.as_object()
                .ok_or_else(|| JsError::from_opaque(js_string!("CustomEvent.cancelBubble setter: not an object").into()))?;
            let mut evt = obj.downcast_mut::<JsCustomEvent>()
                .ok_or_else(|| JsError::from_opaque(js_string!("CustomEvent.cancelBubble setter: not a CustomEvent").into()))?;
            evt.propagation_stopped = true;
        }
        Ok(JsValue::undefined())
    }

    fn get_return_value(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
        let obj = this.as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("CustomEvent.returnValue getter: not an object").into()))?;
        let evt = obj.downcast_ref::<JsCustomEvent>()
            .ok_or_else(|| JsError::from_opaque(js_string!("CustomEvent.returnValue getter: not a CustomEvent").into()))?;
        Ok(JsValue::from(!evt.default_prevented))
    }

    fn set_return_value(this: &JsValue, args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
        let value = args.first().map(|v| v.to_boolean()).unwrap_or(true);
        if !value {
            let obj = this.as_object()
                .ok_or_else(|| JsError::from_opaque(js_string!("CustomEvent.returnValue setter: not an object").into()))?;
            let mut evt = obj.downcast_mut::<JsCustomEvent>()
                .ok_or_else(|| JsError::from_opaque(js_string!("CustomEvent.returnValue setter: not a CustomEvent").into()))?;
            if evt.cancelable {
                evt.default_prevented = true;
            }
        }
        Ok(JsValue::undefined())
    }

    fn init_event(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
        let obj = this.as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("CustomEvent.initEvent: not an object").into()))?;
        let mut evt = obj.downcast_mut::<JsCustomEvent>()
            .ok_or_else(|| JsError::from_opaque(js_string!("CustomEvent.initEvent: not a CustomEvent").into()))?;
        if evt.dispatching {
            return Ok(JsValue::undefined());
        }
        let event_type = args.first()
            .map(|v| v.to_string(ctx)).transpose()?
            .map(|s| s.to_std_string_escaped()).unwrap_or_default();
        let bubbles = args.get(1).map(|v| v.to_boolean()).unwrap_or(false);
        let cancelable = args.get(2).map(|v| v.to_boolean()).unwrap_or(false);
        evt.event_type = event_type;
        evt.bubbles = bubbles;
        evt.cancelable = cancelable;
        evt.default_prevented = false;
        evt.propagation_stopped = false;
        evt.immediate_propagation_stopped = false;
        Ok(JsValue::undefined())
    }

    fn init_custom_event(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
        let obj = this.as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("CustomEvent.initCustomEvent: not an object").into()))?;
        let mut evt = obj.downcast_mut::<JsCustomEvent>()
            .ok_or_else(|| JsError::from_opaque(js_string!("CustomEvent.initCustomEvent: not a CustomEvent").into()))?;
        if evt.dispatching {
            return Ok(JsValue::undefined());
        }
        let event_type = args.first()
            .ok_or_else(|| {
                JsError::from_native(
                    boa_engine::JsNativeError::typ()
                        .with_message("Failed to execute 'initCustomEvent' on 'CustomEvent': 1 argument required, but only 0 present.")
                )
            })?
            .to_string(ctx)?.to_std_string_escaped();
        let bubbles = args.get(1).map(|v| v.to_boolean()).unwrap_or(false);
        let cancelable = args.get(2).map(|v| v.to_boolean()).unwrap_or(false);
        let detail = args.get(3).cloned().unwrap_or(JsValue::null());
        evt.event_type = event_type;
        evt.bubbles = bubbles;
        evt.cancelable = cancelable;
        evt.detail = detail;
        evt.default_prevented = false;
        evt.propagation_stopped = false;
        evt.immediate_propagation_stopped = false;
        Ok(JsValue::undefined())
    }

    // -- Methods --

    fn prevent_default(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
        let obj = this
            .as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("CustomEvent.preventDefault: `this` is not an object").into()))?;
        let mut evt = obj
            .downcast_mut::<JsCustomEvent>()
            .ok_or_else(|| JsError::from_opaque(js_string!("CustomEvent.preventDefault: `this` is not a CustomEvent").into()))?;
        if evt.cancelable {
            evt.default_prevented = true;
        }
        Ok(JsValue::undefined())
    }

    fn stop_propagation(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
        let obj = this
            .as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("CustomEvent.stopPropagation: `this` is not an object").into()))?;
        let mut evt = obj
            .downcast_mut::<JsCustomEvent>()
            .ok_or_else(|| JsError::from_opaque(js_string!("CustomEvent.stopPropagation: `this` is not a CustomEvent").into()))?;
        evt.propagation_stopped = true;
        Ok(JsValue::undefined())
    }

    fn stop_immediate_propagation(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
        let obj = this
            .as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("CustomEvent.stopImmediatePropagation: `this` is not an object").into()))?;
        let mut evt = obj
            .downcast_mut::<JsCustomEvent>()
            .ok_or_else(|| JsError::from_opaque(js_string!("CustomEvent.stopImmediatePropagation: `this` is not a CustomEvent").into()))?;
        evt.propagation_stopped = true;
        evt.immediate_propagation_stopped = true;
        Ok(JsValue::undefined())
    }
}

impl Class for JsCustomEvent {
    const NAME: &'static str = "CustomEvent";
    const LENGTH: usize = 1;

    fn data_constructor(
        _new_target: &JsValue,
        args: &[JsValue],
        ctx: &mut Context,
    ) -> JsResult<Self> {
        let event_type = parse_event_type(args, ctx)?;
        let (bubbles, cancelable) = parse_event_options(args, ctx)?;

        // Also extract `detail` from options
        let detail = if let Some(opts_val) = args.get(1) {
            if let Some(opts_obj) = opts_val.as_object() {
                let d = opts_obj.get(js_string!("detail"), ctx)?;
                if d.is_undefined() {
                    JsValue::null()
                } else {
                    d
                }
            } else {
                JsValue::null()
            }
        } else {
            JsValue::null()
        };

        Ok(JsCustomEvent {
            event_type,
            bubbles,
            cancelable,
            default_prevented: false,
            propagation_stopped: false,
            immediate_propagation_stopped: false,
            detail,
            target: None,
            current_target: None,
            phase: 0,
            dispatching: false,
        })
    }

    fn init(class: &mut ClassBuilder) -> JsResult<()> {
        let realm = class.context().realm().clone();

        // Read-only getters
        let type_getter = NativeFunction::from_fn_ptr(Self::get_type);
        class.accessor(
            js_string!("type"),
            Some(type_getter.to_js_function(&realm)),
            None,
            Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
        );

        let bubbles_getter = NativeFunction::from_fn_ptr(Self::get_bubbles);
        class.accessor(
            js_string!("bubbles"),
            Some(bubbles_getter.to_js_function(&realm)),
            None,
            Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
        );

        let cancelable_getter = NativeFunction::from_fn_ptr(Self::get_cancelable);
        class.accessor(
            js_string!("cancelable"),
            Some(cancelable_getter.to_js_function(&realm)),
            None,
            Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
        );

        let default_prevented_getter = NativeFunction::from_fn_ptr(Self::get_default_prevented);
        class.accessor(
            js_string!("defaultPrevented"),
            Some(default_prevented_getter.to_js_function(&realm)),
            None,
            Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
        );

        let target_getter = NativeFunction::from_fn_ptr(Self::get_target);
        class.accessor(
            js_string!("target"),
            Some(target_getter.to_js_function(&realm)),
            None,
            Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
        );

        let current_target_getter = NativeFunction::from_fn_ptr(Self::get_current_target);
        class.accessor(
            js_string!("currentTarget"),
            Some(current_target_getter.to_js_function(&realm)),
            None,
            Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
        );

        let event_phase_getter = NativeFunction::from_fn_ptr(Self::get_event_phase);
        class.accessor(
            js_string!("eventPhase"),
            Some(event_phase_getter.to_js_function(&realm)),
            None,
            Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
        );

        let detail_getter = NativeFunction::from_fn_ptr(Self::get_detail);
        class.accessor(
            js_string!("detail"),
            Some(detail_getter.to_js_function(&realm)),
            None,
            Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
        );

        // Methods
        class.method(
            js_string!("preventDefault"),
            0,
            NativeFunction::from_fn_ptr(Self::prevent_default),
        );

        class.method(
            js_string!("stopPropagation"),
            0,
            NativeFunction::from_fn_ptr(Self::stop_propagation),
        );

        class.method(
            js_string!("stopImmediatePropagation"),
            0,
            NativeFunction::from_fn_ptr(Self::stop_immediate_propagation),
        );

        class.method(
            js_string!("initEvent"),
            3,
            NativeFunction::from_fn_ptr(Self::init_event),
        );

        class.method(
            js_string!("initCustomEvent"),
            4,
            NativeFunction::from_fn_ptr(Self::init_custom_event),
        );

        // isTrusted is NOT on the prototype — it's added as an own property on each instance
        // (see attach_is_trusted_own_property and the constructor wrappers in runtime.rs)

        // timeStamp
        let time_stamp_getter = NativeFunction::from_fn_ptr(Self::get_time_stamp);
        class.accessor(js_string!("timeStamp"), Some(time_stamp_getter.to_js_function(&realm)), None, Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE);

        // composed
        let composed_getter = NativeFunction::from_fn_ptr(Self::get_composed);
        class.accessor(js_string!("composed"), Some(composed_getter.to_js_function(&realm)), None, Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE);

        // srcElement
        let src_element_getter = NativeFunction::from_fn_ptr(Self::get_src_element);
        class.accessor(js_string!("srcElement"), Some(src_element_getter.to_js_function(&realm)), None, Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE);

        // cancelBubble
        let cancel_bubble_getter = NativeFunction::from_fn_ptr(Self::get_cancel_bubble);
        let cancel_bubble_setter = NativeFunction::from_fn_ptr(Self::set_cancel_bubble);
        class.accessor(js_string!("cancelBubble"), Some(cancel_bubble_getter.to_js_function(&realm)), Some(cancel_bubble_setter.to_js_function(&realm)), Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE);

        // returnValue
        let return_value_getter = NativeFunction::from_fn_ptr(Self::get_return_value);
        let return_value_setter = NativeFunction::from_fn_ptr(Self::set_return_value);
        class.accessor(js_string!("returnValue"), Some(return_value_getter.to_js_function(&realm)), Some(return_value_setter.to_js_function(&realm)), Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE);

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use crate::Engine;

    #[test]
    fn event_basic_type() {
        let mut engine = Engine::new();
        engine.load_html("<html><body></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        let result = runtime.eval("var e = new Event('click'); e.type").unwrap();
        let s = result
            .to_string(&mut runtime.context)
            .unwrap()
            .to_std_string_escaped();
        assert_eq!(s, "click");
    }

    #[test]
    fn event_defaults_bubbles_cancelable() {
        let mut engine = Engine::new();
        engine.load_html("<html><body></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();

        let bubbles = runtime
            .eval("var e = new Event('click'); e.bubbles")
            .unwrap();
        assert_eq!(bubbles.to_boolean(), false);

        let cancelable = runtime.eval("e.cancelable").unwrap();
        assert_eq!(cancelable.to_boolean(), false);
    }

    #[test]
    fn event_with_options() {
        let mut engine = Engine::new();
        engine.load_html("<html><body></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();

        runtime
            .eval("var e = new Event('click', { bubbles: true, cancelable: true })")
            .unwrap();

        let bubbles = runtime.eval("e.bubbles").unwrap();
        assert_eq!(bubbles.to_boolean(), true);

        let cancelable = runtime.eval("e.cancelable").unwrap();
        assert_eq!(cancelable.to_boolean(), true);
    }

    #[test]
    fn event_prevent_default_cancelable() {
        let mut engine = Engine::new();
        engine.load_html("<html><body></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();

        runtime
            .eval("var e = new Event('click', { cancelable: true }); e.preventDefault()")
            .unwrap();

        let dp = runtime.eval("e.defaultPrevented").unwrap();
        assert_eq!(dp.to_boolean(), true);
    }

    #[test]
    fn event_prevent_default_non_cancelable() {
        let mut engine = Engine::new();
        engine.load_html("<html><body></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();

        runtime
            .eval("var e = new Event('click', { cancelable: false }); e.preventDefault()")
            .unwrap();

        let dp = runtime.eval("e.defaultPrevented").unwrap();
        assert_eq!(dp.to_boolean(), false);
    }

    #[test]
    fn event_stop_propagation() {
        let mut engine = Engine::new();
        engine.load_html("<html><body></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();

        // stopPropagation should not throw and should succeed silently
        runtime
            .eval("var e = new Event('click'); e.stopPropagation()")
            .unwrap();

        // We can verify it worked by checking that the method exists and is callable.
        // The internal state is not directly accessible from JS, so we just verify no error.
        let type_val = runtime.eval("e.type").unwrap();
        let s = type_val
            .to_string(&mut runtime.context)
            .unwrap()
            .to_std_string_escaped();
        assert_eq!(s, "click");
    }

    #[test]
    fn custom_event_with_detail() {
        let mut engine = Engine::new();
        engine.load_html("<html><body></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();

        runtime
            .eval("var e = new CustomEvent('myevent', { detail: 42 })")
            .unwrap();

        let type_val = runtime.eval("e.type").unwrap();
        let s = type_val
            .to_string(&mut runtime.context)
            .unwrap()
            .to_std_string_escaped();
        assert_eq!(s, "myevent");

        let detail = runtime.eval("e.detail").unwrap();
        let n = detail.to_number(&mut runtime.context).unwrap();
        assert_eq!(n, 42.0);
    }

    #[test]
    fn custom_event_default_detail_is_null() {
        let mut engine = Engine::new();
        engine.load_html("<html><body></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();

        let result = runtime
            .eval("var e = new CustomEvent('myevent'); e.detail")
            .unwrap();
        assert!(result.is_null());
    }

    #[test]
    fn event_phase_constants() {
        let mut engine = Engine::new();
        engine.load_html("<html><body></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();

        let none = runtime.eval("Event.NONE").unwrap();
        assert_eq!(none.to_number(&mut runtime.context).unwrap(), 0.0);

        let capturing = runtime.eval("Event.CAPTURING_PHASE").unwrap();
        assert_eq!(capturing.to_number(&mut runtime.context).unwrap(), 1.0);

        let at_target = runtime.eval("Event.AT_TARGET").unwrap();
        assert_eq!(at_target.to_number(&mut runtime.context).unwrap(), 2.0);

        let bubbling = runtime.eval("Event.BUBBLING_PHASE").unwrap();
        assert_eq!(bubbling.to_number(&mut runtime.context).unwrap(), 3.0);
    }

    #[test]
    fn event_phase_defaults_to_zero() {
        let mut engine = Engine::new();
        engine.load_html("<html><body></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();

        let phase = runtime
            .eval("var e = new Event('click'); e.eventPhase")
            .unwrap();
        assert_eq!(phase.to_number(&mut runtime.context).unwrap(), 0.0);
    }

    #[test]
    fn event_target_and_current_target_null() {
        let mut engine = Engine::new();
        engine.load_html("<html><body></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();

        let target = runtime
            .eval("var e = new Event('click'); e.target")
            .unwrap();
        assert!(target.is_null());

        let current = runtime.eval("e.currentTarget").unwrap();
        assert!(current.is_null());
    }

    #[test]
    fn custom_event_detail_object() {
        let mut engine = Engine::new();
        engine.load_html("<html><body></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();

        runtime
            .eval("var e = new CustomEvent('test', { detail: { foo: 'bar' } })")
            .unwrap();

        let foo = runtime.eval("e.detail.foo").unwrap();
        let s = foo
            .to_string(&mut runtime.context)
            .unwrap()
            .to_std_string_escaped();
        assert_eq!(s, "bar");
    }

    #[test]
    fn custom_event_bubbles_and_cancelable() {
        let mut engine = Engine::new();
        engine.load_html("<html><body></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();

        runtime
            .eval("var e = new CustomEvent('test', { bubbles: true, cancelable: true, detail: 'hi' })")
            .unwrap();

        let bubbles = runtime.eval("e.bubbles").unwrap();
        assert_eq!(bubbles.to_boolean(), true);

        let cancelable = runtime.eval("e.cancelable").unwrap();
        assert_eq!(cancelable.to_boolean(), true);

        let detail = runtime.eval("e.detail").unwrap();
        let s = detail
            .to_string(&mut runtime.context)
            .unwrap()
            .to_std_string_escaped();
        assert_eq!(s, "hi");
    }

    #[test]
    fn event_stop_immediate_propagation() {
        let mut engine = Engine::new();
        engine.load_html("<html><body></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();

        // Should not throw
        runtime
            .eval("var e = new Event('click'); e.stopImmediatePropagation()")
            .unwrap();

        // Event should still function after calling stopImmediatePropagation
        let type_val = runtime.eval("e.type").unwrap();
        let s = type_val
            .to_string(&mut runtime.context)
            .unwrap()
            .to_std_string_escaped();
        assert_eq!(s, "click");
    }

    #[test]
    fn custom_event_prevent_default() {
        let mut engine = Engine::new();
        engine.load_html("<html><body></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();

        runtime
            .eval("var e = new CustomEvent('test', { cancelable: true }); e.preventDefault()")
            .unwrap();

        let dp = runtime.eval("e.defaultPrevented").unwrap();
        assert_eq!(dp.to_boolean(), true);
    }
}
