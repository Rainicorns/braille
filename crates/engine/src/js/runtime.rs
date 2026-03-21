use boa_engine::{
    class::Class,
    js_string,
    native_function::NativeFunction,
    object::{FunctionObjectBuilder, JsObject, ObjectInitializer},
    property::Attribute,
    Context, JsError, JsNativeError, JsResult, JsValue, Source,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::dom::DomTree;

use super::bindings;
use super::bindings::element::{get_or_create_js_element, DomPrototypes};
use super::prop_desc;
use super::realm_state;

// ---------------------------------------------------------------------------
// Shared constructor name arrays — used in both registration and window-copy
// ---------------------------------------------------------------------------

/// Event and UI subclass constructor names.
pub(crate) const EVENT_CONSTRUCTOR_NAMES: &[&str] = &[
    "MouseEvent",
    "KeyboardEvent",
    "WheelEvent",
    "FocusEvent",
    "AnimationEvent",
    "TransitionEvent",
    "UIEvent",
    "CompositionEvent",
    "Event",
    "CustomEvent",
];

/// DOM utility constructor names (MutationObserver, etc.).
pub(crate) const DOM_UTILITY_NAMES: &[&str] = &[
    "MutationObserver",
    "MutationRecord",
    "NodeFilter",
    "AbortController",
    "AbortSignal",
];

/// Core DOM type constructor names.
pub(crate) const CORE_DOM_TYPE_NAMES: &[&str] = &[
    "Node",
    "CharacterData",
    "Text",
    "Comment",
    "ProcessingInstruction",
    "Attr",
    "DocumentFragment",
    "ShadowRoot",
    "DocumentType",
    "Document",
    "Element",
];

pub struct JsRuntime {
    pub(crate) context: Context,
    tree: Rc<RefCell<DomTree>>,
    console_buffer: Rc<RefCell<Vec<String>>>,
}

impl JsRuntime {
    /// Creates a new JS runtime wired to the given DomTree.
    /// Registers the `document` global, the `Element` class,
    /// the `window` global, and the `console` object.
    pub fn new(tree: Rc<RefCell<DomTree>>) -> Self {
        let mut context = Context::default();
        let console_buffer = Rc::new(RefCell::new(Vec::new()));
        realm_state::register_realm_globals(
            &mut context,
            Rc::clone(&tree),
            Rc::clone(&console_buffer),
        );
        Self {
            context,
            tree,
            console_buffer,
        }
    }

    /// Evaluates a JS source string and returns the result.
    pub fn eval(&mut self, code: &str) -> JsResult<JsValue> {
        let result = self.context.eval(Source::from_bytes(code));
        let _ = self.context.run_jobs();
        result
    }

    /// Deliver pending MutationObserver records to their callbacks.
    pub fn notify_mutation_observers(&mut self) {
        bindings::mutation_observer::notify_mutation_observers(&mut self.context);
    }

    /// Run microtask queue (Promises). Returns true if any jobs were executed.
    pub fn run_jobs(&mut self) -> bool {
        // Boa's run_jobs returns JsResult<()>. We detect work by checking
        // if there were any jobs queued. Since Boa doesn't expose a count,
        // we rely on the settle loop's MO check for convergence detection.
        let _ = self.context.run_jobs();
        // Conservative: always report true to let caller check MO state.
        // The settle loop uses MO pending-records count for real quiescence.
        true
    }

    /// Returns true if there are pending MutationObserver records.
    pub fn has_pending_mutation_observers(&self) -> bool {
        bindings::mutation_observer::has_pending_records(&self.context)
    }

    /// Returns a reference to the shared DomTree.
    pub fn tree(&self) -> &Rc<RefCell<DomTree>> {
        &self.tree
    }

    /// Returns a clone of the console output buffer.
    pub fn console_output(&self) -> Vec<String> {
        self.console_buffer.borrow().clone()
    }

    /// Fire all timers whose deadline has passed. Returns true if any fired.
    /// setTimeout entries are removed after firing; setInterval entries are re-queued.
    pub fn fire_ready_timers(&mut self) -> bool {
        let ts = realm_state::timer_state(&self.context);
        let current_time = ts.borrow().current_time_ms;

        // Collect ready timer IDs
        let ready: Vec<(u32, bool)> = ts
            .borrow()
            .entries
            .values()
            .filter(|e| e.registered_at + e.delay_ms <= current_time)
            .map(|e| (e.id, e.is_interval))
            .collect();

        if ready.is_empty() {
            return false;
        }

        for (id, is_interval) in ready {
            // Extract callback (and maybe remove entry)
            let entry_data = {
                let mut state = ts.borrow_mut();
                if let Some(entry) = state.entries.get(&id) {
                    let cb = entry.callback.clone();
                    let delay = entry.delay_ms;
                    if is_interval {
                        // Re-queue with new registered_at
                        let e = state.entries.get_mut(&id).unwrap();
                        e.registered_at = current_time;
                    } else {
                        state.entries.remove(&id);
                    }
                    Some((cb, delay))
                } else {
                    None
                }
            };

            if let Some((callback, _delay)) = entry_data {
                if let Some(cb_obj) = callback.as_object() {
                    let _ = cb_obj.call(&JsValue::undefined(), &[], &mut self.context);
                    let _ = self.context.run_jobs();
                }
            }
        }

        true
    }

    /// Advance the virtual timer clock to the next pending deadline.
    /// Returns true if time was advanced, false if no timers are pending.
    pub fn advance_timers_to_next_deadline(&mut self) -> bool {
        let ts = realm_state::timer_state(&self.context);
        let state = ts.borrow();
        let current = state.current_time_ms;

        let next_deadline = state
            .entries
            .values()
            .map(|e| e.registered_at + e.delay_ms)
            .filter(|&deadline| deadline > current)
            .min();

        drop(state);

        if let Some(deadline) = next_deadline {
            // Cap at 10000ms virtual advance to prevent infinite loops
            let capped = deadline.min(current + 10000);
            ts.borrow_mut().current_time_ms = capped;
            true
        } else {
            false
        }
    }

    /// Returns true if there are any pending timer entries.
    pub fn has_pending_timers(&self) -> bool {
        let ts = realm_state::timer_state(&self.context);
        let empty = ts.borrow().entries.is_empty();
        !empty
    }
}

// ---------------------------------------------------------------------------
// Free functions — extracted from impl JsRuntime for reuse by register_realm_globals
// ---------------------------------------------------------------------------

/// Replace the Event and CustomEvent global constructors with wrappers that:
/// 1. Create the event via `from_data` (gets the right prototype from Class registration)
/// 2. Attach `isTrusted` as an own accessor property on the instance
///
/// This is needed because Boa's `Class::data_constructor` returns the Rust struct,
/// not the JsObject, so there's no hook to add own properties within the trait.
pub(crate) fn wrap_event_constructors(context: &mut Context) {
    use bindings::event::EventKind;

    let global = context.global_object();

    // Get the original Event.prototype (set up by register_global_class)
    let orig_event_ctor = global
        .get(js_string!("Event"), context)
        .expect("Event constructor should exist");
    let event_proto = orig_event_ctor
        .as_object()
        .expect("Event should be object")
        .get(js_string!("prototype"), context)
        .expect("Event.prototype should exist");
    let event_proto_obj = event_proto.as_object().expect("Event.prototype object").clone();

    // --- Event (reuse existing Event.prototype from Class registration) ---
    register_event_type_with_proto(
        context,
        "Event",
        event_proto_obj.clone(),
        |_opts, _ctx| Ok((EventKind::Standard, Vec::new())),
    );

    // --- CustomEvent ---
    register_event_type(
        context,
        "CustomEvent",
        &event_proto_obj,
        |proto, ctx| {
            let detail_getter = NativeFunction::from_fn_ptr(bindings::event::JsEvent::get_detail);
            let realm = ctx.realm().clone();
            proto
                .define_property_or_throw(
                    js_string!("detail"),
                    prop_desc::readonly_accessor(detail_getter.to_js_function(&realm)),
                    ctx,
                )
                .expect("failed to define CustomEvent.prototype.detail");

            let init_fn = NativeFunction::from_fn_ptr(bindings::event::JsEvent::init_custom_event);
            proto
                .define_property_or_throw(
                    js_string!("initCustomEvent"),
                    prop_desc::data_prop(
                        FunctionObjectBuilder::new(ctx.realm(), init_fn)
                            .name(js_string!("initCustomEvent"))
                            .length(4)
                            .build(),
                    ),
                    ctx,
                )
                .expect("failed to define CustomEvent.prototype.initCustomEvent");
        },
        |opts, ctx| {
            let mut detail = JsValue::null();
            if let Some(obj) = opts {
                let d = obj.get(js_string!("detail"), ctx)?;
                if !d.is_undefined() {
                    detail = d;
                }
            }
            Ok((EventKind::Custom { detail }, Vec::new()))
        },
    );

    // --- MouseEvent ---
    let mouse_event_proto = create_mouse_event_constructor(context, &event_proto_obj);

    // --- UIEvent ---
    let ui_event_proto = register_event_type(
        context,
        "UIEvent",
        &event_proto_obj,
        setup_ui_event_prototype,
        |opts, ctx| {
            let (view, detail_val) = parse_ui_event_options(opts, ctx, true)?;
            Ok((EventKind::Standard, vec![
                (js_string!("__view"), view),
                (js_string!("__detail"), detail_val),
            ]))
        },
    );

    // --- FocusEvent (separate from loop — needs relatedTarget) ---
    register_event_type(
        context,
        "FocusEvent",
        &ui_event_proto,
        |proto, ctx| {
            let related_target_getter =
                NativeFunction::from_fn_ptr(|this: &JsValue, _args: &[JsValue], ctx: &mut Context| {
                    if let Some(obj) = this.as_object() {
                        if let Ok(v) = obj.get(js_string!("__relatedTarget"), ctx) {
                            if !v.is_undefined() {
                                return Ok(v);
                            }
                        }
                    }
                    Ok(JsValue::null())
                });
            let realm = ctx.realm().clone();
            proto
                .define_property_or_throw(
                    js_string!("relatedTarget"),
                    prop_desc::readonly_accessor(related_target_getter.to_js_function(&realm)),
                    ctx,
                )
                .expect("failed to define FocusEvent.prototype.relatedTarget");
        },
        |opts, ctx| {
            let (view, detail_val) = parse_ui_event_options(opts, ctx, false)?;
            let mut related_target = JsValue::null();
            if let Some(obj) = opts {
                let rt = obj.get(js_string!("relatedTarget"), ctx)?;
                if !rt.is_undefined() {
                    related_target = rt;
                }
            }
            Ok((EventKind::Focus, vec![
                (js_string!("__view"), view),
                (js_string!("__detail"), detail_val),
                (js_string!("__relatedTarget"), related_target),
            ]))
        },
    );

    // --- WheelEvent (inherits MouseEvent.prototype) ---
    register_event_type(
        context,
        "WheelEvent",
        &mouse_event_proto,
        |proto, ctx| {
            use bindings::event::JsEvent;

            let realm = ctx.realm().clone();
            // WheelEvent-specific delta getters
            macro_rules! wheel_getter {
                ($field:ident, $js_name:expr, $default:expr) => {{
                    let getter = NativeFunction::from_fn_ptr(|this, _args, _ctx| {
                        let obj = match this.as_object() {
                            Some(o) => o,
                            None => return Ok(JsValue::from($default)),
                        };
                        let evt = match obj.downcast_ref::<JsEvent>() {
                            Some(e) => e,
                            None => return Ok(JsValue::from($default)),
                        };
                        match &evt.kind {
                            EventKind::Wheel { $field, .. } => Ok(JsValue::from(*$field)),
                            _ => Ok(JsValue::from($default)),
                        }
                    });
                    proto
                        .define_property_or_throw(
                            js_string!($js_name),
                            boa_engine::property::PropertyDescriptor::builder()
                                .get(getter.to_js_function(&realm))
                                .configurable(true)
                                .enumerable(true)
                                .build(),
                            ctx,
                        )
                        .expect(concat!("failed to define WheelEvent.", $js_name));
                }};
            }
            wheel_getter!(delta_x, "deltaX", 0.0);
            wheel_getter!(delta_y, "deltaY", 0.0);
            wheel_getter!(delta_z, "deltaZ", 0.0);
            wheel_getter!(delta_mode, "deltaMode", 0);

            // WheelEvent constants
            proto
                .set(js_string!("DOM_DELTA_PIXEL"), JsValue::from(0), false, ctx)
                .expect("set DOM_DELTA_PIXEL");
            proto
                .set(js_string!("DOM_DELTA_LINE"), JsValue::from(1), false, ctx)
                .expect("set DOM_DELTA_LINE");
            proto
                .set(js_string!("DOM_DELTA_PAGE"), JsValue::from(2), false, ctx)
                .expect("set DOM_DELTA_PAGE");
        },
        |opts, ctx| {
            let (view, detail_val) = parse_ui_event_options(opts, ctx, false)?;
            let mut button: i16 = 0;
            let mut buttons: u16 = 0;
            let mut client_x: f64 = 0.0;
            let mut client_y: f64 = 0.0;
            let mut screen_x: f64 = 0.0;
            let mut screen_y: f64 = 0.0;
            let mut alt_key = false;
            let mut ctrl_key = false;
            let mut meta_key = false;
            let mut shift_key = false;
            let mut delta_x: f64 = 0.0;
            let mut delta_y: f64 = 0.0;
            let mut delta_z: f64 = 0.0;
            let mut delta_mode: u32 = 0;

            if let Some(obj) = opts {
                macro_rules! parse_num {
                    ($name:expr, $var:ident, $ty:ty) => {
                        let v = obj.get(js_string!($name), ctx)?;
                        if !v.is_undefined() { $var = v.to_number(ctx)? as $ty; }
                    };
                }
                macro_rules! parse_bool {
                    ($name:expr, $var:ident) => {
                        let v = obj.get(js_string!($name), ctx)?;
                        if !v.is_undefined() { $var = v.to_boolean(); }
                    };
                }
                parse_num!("button", button, i16);
                parse_num!("buttons", buttons, u16);
                parse_num!("clientX", client_x, f64);
                parse_num!("clientY", client_y, f64);
                parse_num!("screenX", screen_x, f64);
                parse_num!("screenY", screen_y, f64);
                parse_bool!("altKey", alt_key);
                parse_bool!("ctrlKey", ctrl_key);
                parse_bool!("metaKey", meta_key);
                parse_bool!("shiftKey", shift_key);
                parse_num!("deltaX", delta_x, f64);
                parse_num!("deltaY", delta_y, f64);
                parse_num!("deltaZ", delta_z, f64);
                parse_num!("deltaMode", delta_mode, u32);
            }

            Ok((EventKind::Wheel {
                button, buttons, client_x, client_y, screen_x, screen_y,
                alt_key, ctrl_key, meta_key, shift_key,
                delta_x, delta_y, delta_z, delta_mode,
            }, vec![
                (js_string!("__view"), view),
                (js_string!("__detail"), detail_val),
            ]))
        },
    );

    // --- KeyboardEvent (inherits UIEvent.prototype) ---
    register_event_type(
        context,
        "KeyboardEvent",
        &ui_event_proto,
        |proto, ctx| {
            let realm = ctx.realm().clone();
            // KeyboardEvent property getters via hidden __* props
            macro_rules! kb_string_getter {
                ($prop:expr, $hidden:expr) => {{
                    let getter = NativeFunction::from_fn_ptr(|this, _args, ctx| {
                        if let Some(obj) = this.as_object() {
                            if let Ok(v) = obj.get(js_string!($hidden), ctx) {
                                if !v.is_undefined() {
                                    return Ok(v);
                                }
                            }
                        }
                        Ok(JsValue::from(js_string!("")))
                    });
                    proto
                        .define_property_or_throw(
                            js_string!($prop),
                            prop_desc::readonly_accessor(getter.to_js_function(&realm)),
                            ctx,
                        )
                        .expect(concat!("failed to define KeyboardEvent.", $prop));
                }};
            }
            macro_rules! kb_num_getter {
                ($prop:expr, $hidden:expr) => {{
                    let getter = NativeFunction::from_fn_ptr(|this, _args, ctx| {
                        if let Some(obj) = this.as_object() {
                            if let Ok(v) = obj.get(js_string!($hidden), ctx) {
                                if !v.is_undefined() {
                                    return Ok(v);
                                }
                            }
                        }
                        Ok(JsValue::from(0))
                    });
                    proto
                        .define_property_or_throw(
                            js_string!($prop),
                            prop_desc::readonly_accessor(getter.to_js_function(&realm)),
                            ctx,
                        )
                        .expect(concat!("failed to define KeyboardEvent.", $prop));
                }};
            }
            macro_rules! kb_bool_getter {
                ($prop:expr, $hidden:expr) => {{
                    let getter = NativeFunction::from_fn_ptr(|this, _args, ctx| {
                        if let Some(obj) = this.as_object() {
                            if let Ok(v) = obj.get(js_string!($hidden), ctx) {
                                if !v.is_undefined() {
                                    return Ok(v);
                                }
                            }
                        }
                        Ok(JsValue::from(false))
                    });
                    proto
                        .define_property_or_throw(
                            js_string!($prop),
                            prop_desc::readonly_accessor(getter.to_js_function(&realm)),
                            ctx,
                        )
                        .expect(concat!("failed to define KeyboardEvent.", $prop));
                }};
            }
            kb_string_getter!("key", "__key");
            kb_string_getter!("code", "__code");
            kb_num_getter!("location", "__location");
            kb_bool_getter!("repeat", "__repeat");
            kb_bool_getter!("isComposing", "__isComposing");
            kb_num_getter!("charCode", "__charCode");
            kb_num_getter!("keyCode", "__keyCode");
            kb_num_getter!("which", "__which");
            kb_bool_getter!("ctrlKey", "__ctrlKey");
            kb_bool_getter!("shiftKey", "__shiftKey");
            kb_bool_getter!("altKey", "__altKey");
            kb_bool_getter!("metaKey", "__metaKey");

            // KeyboardEvent constants
            proto
                .set(js_string!("DOM_KEY_LOCATION_STANDARD"), JsValue::from(0), false, ctx)
                .expect("set DOM_KEY_LOCATION_STANDARD");
            proto
                .set(js_string!("DOM_KEY_LOCATION_LEFT"), JsValue::from(1), false, ctx)
                .expect("set DOM_KEY_LOCATION_LEFT");
            proto
                .set(js_string!("DOM_KEY_LOCATION_RIGHT"), JsValue::from(2), false, ctx)
                .expect("set DOM_KEY_LOCATION_RIGHT");
            proto
                .set(js_string!("DOM_KEY_LOCATION_NUMPAD"), JsValue::from(3), false, ctx)
                .expect("set DOM_KEY_LOCATION_NUMPAD");
        },
        |opts, ctx| {
            let (view, detail_val) = parse_ui_event_options(opts, ctx, false)?;
            let mut hidden_props = vec![
                (js_string!("__view"), view),
                (js_string!("__detail"), detail_val),
            ];
            if let Some(obj) = opts {
                macro_rules! parse_kb_string {
                    ($js:expr, $hidden:expr) => {{
                        let v = obj.get(js_string!($js), ctx)?;
                        if !v.is_undefined() {
                            hidden_props.push((js_string!($hidden), v.to_string(ctx)?.into()));
                        }
                    }};
                }
                macro_rules! parse_kb_num {
                    ($js:expr, $hidden:expr) => {{
                        let v = obj.get(js_string!($js), ctx)?;
                        if !v.is_undefined() {
                            hidden_props.push((js_string!($hidden), v));
                        }
                    }};
                }
                macro_rules! parse_kb_bool {
                    ($js:expr, $hidden:expr) => {{
                        let v = obj.get(js_string!($js), ctx)?;
                        if !v.is_undefined() {
                            hidden_props.push((js_string!($hidden), JsValue::from(v.to_boolean())));
                        }
                    }};
                }
                parse_kb_string!("key", "__key");
                parse_kb_string!("code", "__code");
                parse_kb_num!("location", "__location");
                parse_kb_bool!("repeat", "__repeat");
                parse_kb_bool!("isComposing", "__isComposing");
                parse_kb_num!("charCode", "__charCode");
                parse_kb_num!("keyCode", "__keyCode");
                parse_kb_num!("which", "__which");
                parse_kb_bool!("ctrlKey", "__ctrlKey");
                parse_kb_bool!("shiftKey", "__shiftKey");
                parse_kb_bool!("altKey", "__altKey");
                parse_kb_bool!("metaKey", "__metaKey");
            }
            Ok((EventKind::Keyboard, hidden_props))
        },
    );

    // --- AnimationEvent, TransitionEvent (remaining UIEvent subclasses) ---
    for (name, kind) in &[
        ("AnimationEvent", EventKind::Animation),
        ("TransitionEvent", EventKind::Transition),
    ] {
        let kind = kind.clone();
        register_event_type(
            context,
            name,
            &ui_event_proto,
            |_proto, _ctx| {},
            move |opts, ctx| {
                let (view, detail_val) = parse_ui_event_options(opts, ctx, false)?;
                Ok((kind.clone(), vec![
                    (js_string!("__view"), view),
                    (js_string!("__detail"), detail_val),
                ]))
            },
        );
    }

    // --- CompositionEvent ---
    register_event_type(
        context,
        "CompositionEvent",
        &ui_event_proto,
        |proto, ctx| {
            let data_getter = NativeFunction::from_fn_ptr(|this: &JsValue, _args: &[JsValue], _ctx: &mut Context| {
                if let Some(obj) = this.as_object() {
                    if let Ok(v) = obj.get(js_string!("__composition_data"), _ctx) {
                        if !v.is_undefined() {
                            return Ok(v);
                        }
                    }
                }
                Ok(JsValue::from(js_string!("")))
            });
            let realm = ctx.realm().clone();
            proto
                .define_property_or_throw(
                    js_string!("data"),
                    prop_desc::readonly_accessor(data_getter.to_js_function(&realm)),
                    ctx,
                )
                .expect("failed to define CompositionEvent.prototype.data");
        },
        |opts, ctx| {
            let (view, detail_val) = parse_ui_event_options(opts, ctx, false)?;
            let mut data = JsValue::from(js_string!(""));
            if let Some(obj) = opts {
                let da = obj.get(js_string!("data"), ctx)?;
                if !da.is_undefined() {
                    data = da;
                }
            }
            Ok((EventKind::Composition, vec![
                (js_string!("__view"), view),
                (js_string!("__detail"), detail_val),
                (js_string!("__composition_data"), data),
            ]))
        },
    );
}

/// Parse UIEvent-specific options (view, detail) from the options object.
fn parse_ui_event_options(
    opts: Option<&JsObject>,
    ctx: &mut Context,
    validate_view_type: bool,
) -> JsResult<(JsValue, JsValue)> {
    let mut view = JsValue::null();
    let mut detail_val = JsValue::from(0);
    if let Some(obj) = opts {
        let v = obj.get(js_string!("view"), ctx)?;
        if !v.is_undefined() && !v.is_null() {
            if validate_view_type && !v.is_object() {
                return Err(JsError::from_native(
                    JsNativeError::typ()
                        .with_message("Failed to construct 'UIEvent': member view is not of type Window."),
                ));
            }
            view = v;
        }
        let d = obj.get(js_string!("detail"), ctx)?;
        if !d.is_undefined() {
            detail_val = d;
        }
    }
    Ok((view, detail_val))
}

/// Add view and detail getters to a UIEvent-like prototype.
fn setup_ui_event_prototype(proto: &JsObject, ctx: &mut Context) {
    let view_getter = NativeFunction::from_fn_ptr(|this: &JsValue, _args: &[JsValue], _ctx: &mut Context| {
        if let Some(obj) = this.as_object() {
            if let Ok(v) = obj.get(js_string!("__view"), _ctx) {
                if !v.is_undefined() {
                    return Ok(v);
                }
            }
        }
        Ok(JsValue::null())
    });
    let realm = ctx.realm().clone();
    proto
        .define_property_or_throw(
            js_string!("view"),
            prop_desc::readonly_accessor(view_getter.to_js_function(&realm)),
            ctx,
        )
        .expect("failed to define UIEvent.prototype.view");

    let detail_getter = NativeFunction::from_fn_ptr(|this: &JsValue, _args: &[JsValue], _ctx: &mut Context| {
        if let Some(obj) = this.as_object() {
            if let Ok(v) = obj.get(js_string!("__detail"), _ctx) {
                if !v.is_undefined() {
                    return Ok(v);
                }
            }
        }
        Ok(JsValue::from(0))
    });
    proto
        .define_property_or_throw(
            js_string!("detail"),
            prop_desc::readonly_accessor(detail_getter.to_js_function(&realm)),
            ctx,
        )
        .expect("failed to define UIEvent.prototype.detail");
}

/// Generic event type registration helper.
///
/// Creates a prototype inheriting from `parent_proto`, calls `setup_prototype` to add
/// type-specific getters/methods, builds a constructor that parses event_type + options,
/// delegates to `parse_options` for type-specific fields, and registers as a global.
///
/// Returns the prototype object (useful when subtypes need to inherit from it).
/// Register an event type with a new prototype inheriting from `parent_proto`.
fn register_event_type<F, P>(
    context: &mut Context,
    name: &'static str,
    parent_proto: &JsObject,
    setup_prototype: F,
    parse_options: P,
) -> JsObject
where
    F: FnOnce(&JsObject, &mut Context),
    P: Fn(Option<&JsObject>, &mut Context) -> JsResult<(bindings::event::EventKind, Vec<(boa_engine::JsString, JsValue)>)>
        + 'static,
{
    let proto = ObjectInitializer::new(context).build();
    proto.set_prototype(Some(parent_proto.clone()));
    setup_prototype(&proto, context);
    register_event_type_with_proto(context, name, proto.clone(), parse_options);
    proto
}

/// Register an event type using an existing prototype object (e.g., for Event which reuses
/// the prototype from Boa's Class registration).
fn register_event_type_with_proto<P>(
    context: &mut Context,
    name: &'static str,
    proto: JsObject,
    parse_options: P,
)
where
    P: Fn(Option<&JsObject>, &mut Context) -> JsResult<(bindings::event::EventKind, Vec<(boa_engine::JsString, JsValue)>)>
        + 'static,
{
    use bindings::event::{attach_is_trusted_own_property, JsEvent};

    let proto_for_closure = proto.clone();
    let ctor = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            if _this.is_undefined() {
                return Err(JsError::from_native(
                    JsNativeError::typ().with_message(format!(
                        "Failed to construct '{}': Please use the 'new' operator, this DOM object constructor cannot be called as a function.",
                        name
                    )),
                ));
            }
            let event_type = args
                .first()
                .ok_or_else(|| {
                    JsError::from_native(JsNativeError::typ().with_message(format!(
                        "Failed to construct '{}': 1 argument required, but only 0 present.",
                        name
                    )))
                })?
                .to_string(ctx)?
                .to_std_string_escaped();

            let mut bubbles = false;
            let mut cancelable = false;
            let mut composed = false;
            let opts_obj: Option<JsObject> = args.get(1).and_then(|v| v.as_object());
            if let Some(ref obj) = opts_obj {
                let b = obj.get(js_string!("bubbles"), ctx)?;
                if !b.is_undefined() {
                    bubbles = b.to_boolean();
                }
                let c = obj.get(js_string!("cancelable"), ctx)?;
                if !c.is_undefined() {
                    cancelable = c.to_boolean();
                }
                let comp = obj.get(js_string!("composed"), ctx)?;
                if !comp.is_undefined() {
                    composed = comp.to_boolean();
                }
            }

            let (kind, hidden_props) = parse_options(opts_obj.as_ref(), ctx)?;

            let event = JsEvent {
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
                time_stamp: bindings::event::dom_high_res_time_stamp(ctx),
                initialized: true,
                composed,
                kind,
            };
            let js_obj = JsEvent::from_data(event, ctx)?;
            js_obj.set_prototype(Some(proto_for_closure.clone()));
            attach_is_trusted_own_property(&js_obj, ctx)?;
            for (key, val) in hidden_props {
                js_obj.set(key, val, false, ctx)?;
            }
            Ok(JsValue::from(js_obj))
        })
    };

    let ctor_fn = FunctionObjectBuilder::new(context.realm(), ctor)
        .name(js_string!(name))
        .length(1)
        .constructor(true)
        .build();
    ctor_fn
        .define_property_or_throw(js_string!("prototype"), prop_desc::prototype_on_ctor(proto.clone()), context)
        .expect("failed to define prototype on event constructor");
    context
        .register_global_property(js_string!(name), ctor_fn, Attribute::WRITABLE | Attribute::CONFIGURABLE)
        .expect("failed to register event constructor");
}

/// Register the `NodeFilter` global with its constants (SHOW_ALL, SHOW_ELEMENT, FILTER_ACCEPT, etc.).
pub(crate) fn register_node_filter(context: &mut Context) {
    let nf = ObjectInitializer::new(context)
        .property(js_string!("FILTER_ACCEPT"), 1u32, Attribute::all())
        .property(js_string!("FILTER_REJECT"), 2u32, Attribute::all())
        .property(js_string!("FILTER_SKIP"), 3u32, Attribute::all())
        .property(js_string!("SHOW_ALL"), 0xFFFFFFFFu32, Attribute::all())
        .property(js_string!("SHOW_ELEMENT"), 0x1u32, Attribute::all())
        .property(js_string!("SHOW_ATTRIBUTE"), 0x2u32, Attribute::all())
        .property(js_string!("SHOW_TEXT"), 0x4u32, Attribute::all())
        .property(js_string!("SHOW_CDATA_SECTION"), 0x8u32, Attribute::all())
        .property(js_string!("SHOW_ENTITY_REFERENCE"), 0x10u32, Attribute::all())
        .property(js_string!("SHOW_ENTITY"), 0x20u32, Attribute::all())
        .property(js_string!("SHOW_PROCESSING_INSTRUCTION"), 0x40u32, Attribute::all())
        .property(js_string!("SHOW_COMMENT"), 0x80u32, Attribute::all())
        .property(js_string!("SHOW_NOTATION"), 0x800u32, Attribute::all())
        .property(js_string!("SHOW_DOCUMENT"), 0x100u32, Attribute::all())
        .property(js_string!("SHOW_DOCUMENT_TYPE"), 0x200u32, Attribute::all())
        .property(js_string!("SHOW_DOCUMENT_FRAGMENT"), 0x400u32, Attribute::all())
        .build();

    context
        .register_global_property(js_string!("NodeFilter"), nf, Attribute::WRITABLE | Attribute::CONFIGURABLE)
        .expect("failed to register NodeFilter global");
}

/// Build the MouseEvent constructor with all mouse-specific property getters and register it.
/// Returns the MouseEvent prototype object (for WheelEvent to inherit from).
fn create_mouse_event_constructor(context: &mut Context, event_proto_obj: &JsObject) -> JsObject {
    use bindings::event::{attach_is_trusted_own_property, EventKind, JsEvent};

    let proto = ObjectInitializer::new(context).build();
    proto.set_prototype(Some(event_proto_obj.clone()));

    // Add mouse-specific property getters on prototype
    let realm = context.realm().clone();

    // Unified macro for mouse event property getters.
    // - $field: the field name in EventKind::Mouse { $field, .. }
    // - $js_name: the JavaScript property name string
    // - $default: the default JsValue when not a Mouse event
    // - optional `as $cast_ty`: cast the field value before wrapping in JsValue
    macro_rules! mouse_getter {
        ($field:ident, $js_name:expr, $default:expr, as $cast_ty:ty) => {{
            let getter = NativeFunction::from_fn_ptr(|this, _args, _ctx| {
                let obj = match this.as_object() {
                    Some(o) => o,
                    None => return Ok(JsValue::from($default)),
                };
                let evt = match obj.downcast_ref::<JsEvent>() {
                    Some(e) => e,
                    None => return Ok(JsValue::from($default)),
                };
                match &evt.kind {
                    EventKind::Mouse { $field, .. } | EventKind::Wheel { $field, .. } => Ok(JsValue::from(*$field as $cast_ty)),
                    _ => Ok(JsValue::from($default)),
                }
            });
            proto
                .define_property_or_throw(
                    js_string!($js_name),
                    boa_engine::property::PropertyDescriptor::builder()
                        .get(getter.to_js_function(&realm))
                        .configurable(true)
                        .enumerable(true)
                        .build(),
                    context,
                )
                .expect(concat!("failed to define MouseEvent.", $js_name));
        }};
        ($field:ident, $js_name:expr, $default:expr) => {{
            let getter = NativeFunction::from_fn_ptr(|this, _args, _ctx| {
                let obj = match this.as_object() {
                    Some(o) => o,
                    None => return Ok(JsValue::from($default)),
                };
                let evt = match obj.downcast_ref::<JsEvent>() {
                    Some(e) => e,
                    None => return Ok(JsValue::from($default)),
                };
                match &evt.kind {
                    EventKind::Mouse { $field, .. } | EventKind::Wheel { $field, .. } => Ok(JsValue::from(*$field)),
                    _ => Ok(JsValue::from($default)),
                }
            });
            proto
                .define_property_or_throw(
                    js_string!($js_name),
                    boa_engine::property::PropertyDescriptor::builder()
                        .get(getter.to_js_function(&realm))
                        .configurable(true)
                        .enumerable(true)
                        .build(),
                    context,
                )
                .expect(concat!("failed to define MouseEvent.", $js_name));
        }};
    }

    mouse_getter!(button, "button", 0, as i32);
    mouse_getter!(buttons, "buttons", 0, as i32);
    mouse_getter!(client_x, "clientX", 0.0);
    mouse_getter!(client_y, "clientY", 0.0);
    mouse_getter!(screen_x, "screenX", 0.0);
    mouse_getter!(screen_y, "screenY", 0.0);
    mouse_getter!(alt_key, "altKey", false);
    mouse_getter!(ctrl_key, "ctrlKey", false);
    mouse_getter!(meta_key, "metaKey", false);
    mouse_getter!(shift_key, "shiftKey", false);

    // offsetX/offsetY — no layout engine, so return clientX/clientY (element pos is 0,0)
    mouse_getter!(client_x, "offsetX", 0.0);
    mouse_getter!(client_y, "offsetY", 0.0);

    // relatedTarget (always null for now)
    let related_target_getter = NativeFunction::from_fn_ptr(|_this, _args, _ctx| Ok(JsValue::null()));
    proto
        .define_property_or_throw(
            js_string!("relatedTarget"),
            boa_engine::property::PropertyDescriptor::builder()
                .get(related_target_getter.to_js_function(&realm))
                .configurable(true)
                .enumerable(true)
                .build(),
            context,
        )
        .expect("failed to define MouseEvent.relatedTarget");

    let proto_for_closure = proto.clone();
    let ctor = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            if _this.is_undefined() {
                return Err(JsError::from_native(
                        boa_engine::JsNativeError::typ()
                            .with_message("Failed to construct 'MouseEvent': Please use the 'new' operator, this DOM object constructor cannot be called as a function.")
                    ));
            }
            let event_type = args
                .first()
                .map(|v| v.to_string(ctx))
                .transpose()?
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_default();

            let mut bubbles = false;
            let mut cancelable = false;
            let mut button: i16 = 0;
            let mut buttons: u16 = 0;
            let mut client_x: f64 = 0.0;
            let mut client_y: f64 = 0.0;
            let mut screen_x: f64 = 0.0;
            let mut screen_y: f64 = 0.0;
            let mut alt_key = false;
            let mut ctrl_key = false;
            let mut meta_key = false;
            let mut shift_key = false;

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

                    let v = opts_obj.get(js_string!("button"), ctx)?;
                    if !v.is_undefined() {
                        button = v.to_number(ctx)? as i16;
                    }
                    let v = opts_obj.get(js_string!("buttons"), ctx)?;
                    if !v.is_undefined() {
                        buttons = v.to_number(ctx)? as u16;
                    }
                    let v = opts_obj.get(js_string!("clientX"), ctx)?;
                    if !v.is_undefined() {
                        client_x = v.to_number(ctx)?;
                    }
                    let v = opts_obj.get(js_string!("clientY"), ctx)?;
                    if !v.is_undefined() {
                        client_y = v.to_number(ctx)?;
                    }
                    let v = opts_obj.get(js_string!("screenX"), ctx)?;
                    if !v.is_undefined() {
                        screen_x = v.to_number(ctx)?;
                    }
                    let v = opts_obj.get(js_string!("screenY"), ctx)?;
                    if !v.is_undefined() {
                        screen_y = v.to_number(ctx)?;
                    }
                    let v = opts_obj.get(js_string!("altKey"), ctx)?;
                    if !v.is_undefined() {
                        alt_key = v.to_boolean();
                    }
                    let v = opts_obj.get(js_string!("ctrlKey"), ctx)?;
                    if !v.is_undefined() {
                        ctrl_key = v.to_boolean();
                    }
                    let v = opts_obj.get(js_string!("metaKey"), ctx)?;
                    if !v.is_undefined() {
                        meta_key = v.to_boolean();
                    }
                    let v = opts_obj.get(js_string!("shiftKey"), ctx)?;
                    if !v.is_undefined() {
                        shift_key = v.to_boolean();
                    }
                }
            }

            let event = JsEvent {
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
                time_stamp: bindings::event::dom_high_res_time_stamp(ctx),
                initialized: true,
                composed: false,
                kind: EventKind::Mouse {
                    button,
                    buttons,
                    client_x,
                    client_y,
                    screen_x,
                    screen_y,
                    alt_key,
                    ctrl_key,
                    meta_key,
                    shift_key,
                },
            };
            let js_obj = JsEvent::from_data(event, ctx)?;
            js_obj.set_prototype(Some(proto_for_closure.clone()));
            attach_is_trusted_own_property(&js_obj, ctx)?;
            Ok(JsValue::from(js_obj))
        })
    };

    let ctor_fn = FunctionObjectBuilder::new(context.realm(), ctor)
        .name(js_string!("MouseEvent"))
        .length(1)
        .constructor(true)
        .build();

    ctor_fn
        .define_property_or_throw(js_string!("prototype"), prop_desc::prototype_on_ctor(proto.clone()), context)
        .expect("failed to define MouseEvent.prototype on wrapper");

    context
        .register_global_property(
            js_string!("MouseEvent"),
            ctor_fn,
            Attribute::WRITABLE | Attribute::CONFIGURABLE,
        )
        .expect("failed to register MouseEvent wrapper");

    proto
}

/// Register the full DOM type hierarchy and copy constructors to window.
pub(crate) fn register_dom_type_hierarchy(context: &mut Context) {
    let element_constructor = context
        .global_object()
        .get(js_string!("Element"), context)
        .expect("Element should be registered");
    let element_proto = element_constructor
        .as_object()
        .expect("Element should be an object")
        .get(js_string!("prototype"), context)
        .expect("Element.prototype should exist");
    let element_proto_obj = element_proto
        .as_object()
        .expect("Element.prototype should be an object")
        .clone();

    let node_proto = register_node_prototype(context, &element_proto_obj);
    register_character_data_hierarchy(context, &node_proto, &element_proto_obj);
    register_html_element_types(context, &element_proto_obj);
    bindings::on_event::register_on_event_accessors(
        &element_proto_obj,
        &[
            "click", "change", "input", "submit", "reset", "toggle", "load", "error", "mousedown",
            "mouseup", "mouseover", "mouseout", "mousemove", "keydown", "keyup", "keypress", "focus",
            "blur", "animationstart", "animationend", "animationiteration", "transitionend",
            "transitionstart", "transitionrun", "webkitanimationstart", "webkitanimationend",
            "webkitanimationiteration", "webkittransitionend",
        ],
        context,
    );
    register_document_fragment_type(context, &node_proto);
    register_shadow_root_type(context);
    register_document_type_type(context, &element_proto_obj);
    register_document_constructor(context, &element_proto_obj);
    register_xml_document_global(context);
    populate_dom_prototypes(context);
    copy_dom_types_to_window(context);
}

/// Build Node.prototype with constants + Node constructor, wire Element.prototype inheritance.
fn register_node_prototype(context: &mut Context, element_proto_obj: &JsObject) -> JsObject {
    let node_proto = ObjectInitializer::new(context).build();

    let node_constants: &[(&str, i32)] = &[
        ("ELEMENT_NODE", 1),
        ("ATTRIBUTE_NODE", 2),
        ("TEXT_NODE", 3),
        ("CDATA_SECTION_NODE", 4),
        ("ENTITY_REFERENCE_NODE", 5),
        ("ENTITY_NODE", 6),
        ("PROCESSING_INSTRUCTION_NODE", 7),
        ("COMMENT_NODE", 8),
        ("DOCUMENT_NODE", 9),
        ("DOCUMENT_TYPE_NODE", 10),
        ("DOCUMENT_FRAGMENT_NODE", 11),
        ("NOTATION_NODE", 12),
    ];
    let doc_position_constants: &[(&str, i32)] = &[
        ("DOCUMENT_POSITION_DISCONNECTED", 0x01),
        ("DOCUMENT_POSITION_PRECEDING", 0x02),
        ("DOCUMENT_POSITION_FOLLOWING", 0x04),
        ("DOCUMENT_POSITION_CONTAINS", 0x08),
        ("DOCUMENT_POSITION_CONTAINED_BY", 0x10),
        ("DOCUMENT_POSITION_IMPLEMENTATION_SPECIFIC", 0x20),
    ];

    let all_constants = node_constants.iter().chain(doc_position_constants.iter());
    for (name, value) in all_constants.clone() {
        node_proto
            .define_property_or_throw(
                js_string!(*name),
                prop_desc::readonly_constant(JsValue::from(*value)),
                context,
            )
            .expect("failed to define Node.prototype constant");
    }

    element_proto_obj.set_prototype(Some(node_proto.clone()));

    // Copy Node-level methods from Element.prototype to Node.prototype
    let node_methods = &[
        "appendChild",
        "insertBefore",
        "removeChild",
        "replaceChild",
        "cloneNode",
        "normalize",
        "hasChildNodes",
        "contains",
        "isEqualNode",
        "isSameNode",
        "compareDocumentPosition",
        "getRootNode",
        "append",
        "prepend",
        "replaceChildren",
        "before",
        "after",
        "replaceWith",
        "remove",
        "insertAdjacentElement",
        "insertAdjacentText",
    ];
    for name in node_methods {
        if let Ok(val) = element_proto_obj.get(js_string!(*name), context) {
            if !val.is_undefined() {
                node_proto
                    .set(js_string!(*name), val, false, context)
                    .expect("failed to copy method to Node.prototype");
            }
        }
    }

    // Node constructor (illegal — abstract interface)
    let node_ctor = make_illegal_constructor(context, "Node");
    node_ctor
        .define_property_or_throw(js_string!("prototype"), prop_desc::prototype_on_ctor(node_proto.clone()), context)
        .expect("failed to define Node.prototype");
    for (name, value) in all_constants {
        node_ctor
            .define_property_or_throw(
                js_string!(*name),
                prop_desc::readonly_constant(JsValue::from(*value)),
                context,
            )
            .expect("failed to define Node constant");
    }
    context
        .register_global_property(js_string!("Node"), node_ctor, Attribute::WRITABLE | Attribute::CONFIGURABLE)
        .expect("failed to register Node global");

    node_proto
}

/// Register CharacterData, Text, Comment, ProcessingInstruction, and Attr types.
fn register_character_data_hierarchy(
    context: &mut Context,
    node_proto: &JsObject,
    element_proto_obj: &JsObject,
) {
    // ---------------------------------------------------------------
    // CharacterData.prototype — inherits from Node.prototype
    // We copy all properties from Element.prototype onto it so that
    // CharacterData instances (Text, Comment) get access to .data,
    // .nodeType, .textContent, etc. without Element.prototype being
    // in the chain (which would break the WPT prototype chain checks).
    // ---------------------------------------------------------------
    let char_data_proto = ObjectInitializer::new(context).build();
    char_data_proto.set_prototype(Some(node_proto.clone()));

    // Store Element.prototype and CharacterData.prototype as JS globals temporarily,
    // then use JS to copy all property descriptors.
    context
        .register_global_property(
            js_string!("__braille_elem_proto"),
            element_proto_obj.clone(),
            Attribute::all(),
        )
        .expect("failed to register temp elem proto");
    context
        .register_global_property(
            js_string!("__braille_cd_proto"),
            char_data_proto.clone(),
            Attribute::all(),
        )
        .expect("failed to register temp cd proto");

    // Use JS to copy all property descriptors from Element.prototype to CharacterData.prototype
    context
        .eval(Source::from_bytes(
            r#"
            (function() {
                var src = __braille_elem_proto;
                var dst = __braille_cd_proto;
                var names = Object.getOwnPropertyNames(src);
                for (var i = 0; i < names.length; i++) {
                    var name = names[i];
                    if (name === 'constructor') continue;
                    var desc = Object.getOwnPropertyDescriptor(src, name);
                    if (desc) {
                        Object.defineProperty(dst, name, desc);
                    }
                }
                delete self.__braille_elem_proto;
                delete self.__braille_cd_proto;
            })();
            "#,
        ))
        .expect("failed to copy Element.prototype properties to CharacterData.prototype");

    // CharacterData is abstract; calling `new CharacterData()` throws.
    // Must be a callable function for `obj instanceof CharacterData` to work.
    let char_data_ctor = unsafe {
        NativeFunction::from_closure(|_this, _args, _ctx| {
            Err(JsError::from_opaque(JsValue::from(js_string!("Illegal constructor"))))
        })
    };
    let char_data_ctor_fn = FunctionObjectBuilder::new(context.realm(), char_data_ctor)
        .name(js_string!("CharacterData"))
        .length(0)
        .constructor(true)
        .build();
    char_data_ctor_fn
        .define_property_or_throw(
            js_string!("prototype"),
            prop_desc::prototype_on_ctor(char_data_proto.clone()),
            context,
        )
        .expect("failed to define CharacterData.prototype");

    context
        .register_global_property(
            js_string!("CharacterData"),
            char_data_ctor_fn,
            Attribute::WRITABLE | Attribute::CONFIGURABLE,
        )
        .expect("failed to register CharacterData global");

    // ---------------------------------------------------------------
    // Text.prototype — inherits from CharacterData.prototype
    // ---------------------------------------------------------------
    let text_proto = ObjectInitializer::new(context).build();
    text_proto.set_prototype(Some(char_data_proto.clone()));

    // Text constructor: new Text(data?) creates a Text node
    let text_proto_for_closure = text_proto.clone();
    let text_ctor = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let data = if args.is_empty() || args[0].is_undefined() {
                String::new()
            } else {
                args[0].to_string(ctx)?.to_std_string_escaped()
            };

            let tree = realm_state::dom_tree(ctx);

            let node_id = tree.borrow_mut().create_text(&data);
            let js_obj = get_or_create_js_element(node_id, tree, ctx)?;
            // Ensure prototype is Text.prototype (get_or_create_js_element may already do this)
            js_obj.set_prototype(Some(text_proto_for_closure.clone()));
            Ok(JsValue::from(js_obj))
        })
    };

    // Build the Text constructor function object (constructor: true enables `new Text()`)
    let text_ctor_fn = FunctionObjectBuilder::new(context.realm(), text_ctor)
        .name(js_string!("Text"))
        .length(0)
        .constructor(true)
        .build();
    text_ctor_fn
        .define_property_or_throw(
            js_string!("prototype"),
            prop_desc::prototype_on_ctor(text_proto.clone()),
            context,
        )
        .expect("failed to define Text.prototype");

    // Set Text.prototype.constructor = Text
    text_proto
        .define_property_or_throw(
            js_string!("constructor"),
            prop_desc::constructor_on_proto(text_ctor_fn.clone()),
            context,
        )
        .expect("failed to define Text.prototype.constructor");

    context
        .register_global_property(
            js_string!("Text"),
            text_ctor_fn,
            Attribute::WRITABLE | Attribute::CONFIGURABLE,
        )
        .expect("failed to register Text global");

    // ---------------------------------------------------------------
    // Comment.prototype — inherits from CharacterData.prototype
    // ---------------------------------------------------------------
    let comment_proto = ObjectInitializer::new(context).build();
    comment_proto.set_prototype(Some(char_data_proto.clone()));

    // Comment constructor: new Comment(data?) creates a Comment node
    let comment_proto_for_closure = comment_proto.clone();
    let comment_ctor = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let data = if args.is_empty() || args[0].is_undefined() {
                String::new()
            } else {
                args[0].to_string(ctx)?.to_std_string_escaped()
            };

            let tree = realm_state::dom_tree(ctx);

            let node_id = tree.borrow_mut().create_comment(&data);
            let js_obj = get_or_create_js_element(node_id, tree, ctx)?;
            js_obj.set_prototype(Some(comment_proto_for_closure.clone()));
            Ok(JsValue::from(js_obj))
        })
    };

    let comment_ctor_fn = FunctionObjectBuilder::new(context.realm(), comment_ctor)
        .name(js_string!("Comment"))
        .length(0)
        .constructor(true)
        .build();
    comment_ctor_fn
        .define_property_or_throw(
            js_string!("prototype"),
            prop_desc::prototype_on_ctor(comment_proto.clone()),
            context,
        )
        .expect("failed to define Comment.prototype");

    comment_proto
        .define_property_or_throw(
            js_string!("constructor"),
            prop_desc::constructor_on_proto(comment_ctor_fn.clone()),
            context,
        )
        .expect("failed to define Comment.prototype.constructor");

    context
        .register_global_property(
            js_string!("Comment"),
            comment_ctor_fn,
            Attribute::WRITABLE | Attribute::CONFIGURABLE,
        )
        .expect("failed to register Comment global");

    // ---------------------------------------------------------------
    // ProcessingInstruction.prototype — inherits from CharacterData.prototype
    // ---------------------------------------------------------------
    let pi_proto = ObjectInitializer::new(context).build();
    pi_proto.set_prototype(Some(char_data_proto.clone()));

    // ProcessingInstruction is abstract; calling `new ProcessingInstruction()` throws.
    let pi_ctor = unsafe {
        NativeFunction::from_closure(|_this, _args, _ctx| {
            Err(JsError::from_opaque(JsValue::from(js_string!("Illegal constructor"))))
        })
    };
    let pi_ctor_fn = FunctionObjectBuilder::new(context.realm(), pi_ctor)
        .name(js_string!("ProcessingInstruction"))
        .length(0)
        .constructor(true)
        .build();
    pi_ctor_fn
        .define_property_or_throw(
            js_string!("prototype"),
            prop_desc::prototype_on_ctor(pi_proto.clone()),
            context,
        )
        .expect("failed to define ProcessingInstruction.prototype");

    pi_proto
        .define_property_or_throw(
            js_string!("constructor"),
            prop_desc::constructor_on_proto(pi_ctor_fn.clone()),
            context,
        )
        .expect("failed to define ProcessingInstruction.prototype.constructor");

    context
        .register_global_property(
            js_string!("ProcessingInstruction"),
            pi_ctor_fn,
            Attribute::WRITABLE | Attribute::CONFIGURABLE,
        )
        .expect("failed to register ProcessingInstruction global");

    // ---------------------------------------------------------------
    // Attr.prototype — inherits from Node.prototype (Element.prototype)
    // ---------------------------------------------------------------
    let attr_proto = ObjectInitializer::new(context).build();
    attr_proto.set_prototype(Some(element_proto_obj.clone()));

    let attr_ctor = unsafe {
        NativeFunction::from_closure(|_this, _args, _ctx| {
            Err(JsError::from_opaque(JsValue::from(js_string!("Illegal constructor"))))
        })
    };
    let attr_ctor_fn = FunctionObjectBuilder::new(context.realm(), attr_ctor)
        .name(js_string!("Attr"))
        .length(0)
        .constructor(true)
        .build();
    attr_ctor_fn
        .define_property_or_throw(
            js_string!("prototype"),
            prop_desc::prototype_on_ctor(attr_proto.clone()),
            context,
        )
        .expect("failed to define Attr.prototype");

    attr_proto
        .define_property_or_throw(
            js_string!("constructor"),
            prop_desc::constructor_on_proto(attr_ctor_fn.clone()),
            context,
        )
        .expect("failed to define Attr.prototype.constructor");

    context
        .register_global_property(
            js_string!("Attr"),
            attr_ctor_fn,
            Attribute::WRITABLE | Attribute::CONFIGURABLE,
        )
        .expect("failed to register Attr global");

    realm_state::set_dom_prototypes(
        context,
        DomPrototypes {
            text_proto,
            comment_proto,
            pi_proto: Some(pi_proto),
            attr_proto: Some(attr_proto),
            html_tag_protos: HashMap::new(),
            html_element_proto: None,
            html_unknown_proto: None,
            document_fragment_proto: None,
            shadow_root_proto: None,
            document_type_proto: None,
            document_proto: None,
            xml_document_proto: None,
        },
    );
}

/// Copy DOM type constructors and HTML element constructors onto window object.
fn copy_dom_types_to_window(context: &mut Context) {
    let global = context.global_object();
    let window_val = global
        .get(js_string!("window"), context)
        .expect("window global should exist");
    if let Some(window_obj) = window_val.as_object() {
        for name in CORE_DOM_TYPE_NAMES {
            let val = global
                .get(js_string!(*name), context)
                .expect("global should have this property");
            window_obj
                .define_property_or_throw(js_string!(*name), prop_desc::data_prop(val), context)
                .expect("failed to set window property");
        }

        for name in HTML_ELEMENT_TYPE_NAMES {
            if let Ok(val) = global.get(js_string!(*name), context) {
                if !val.is_undefined() {
                    let _ =
                        window_obj.define_property_or_throw(js_string!(*name), prop_desc::data_prop(val), context);
                }
            }
        }
    }
}

/// HTML element type constructor names.
const HTML_ELEMENT_TYPE_NAMES: &[&str] = &[
    "HTMLElement",
    "HTMLAnchorElement",
    "HTMLAreaElement",
    "HTMLAudioElement",
    "HTMLBaseElement",
    "HTMLBodyElement",
    "HTMLBRElement",
    "HTMLButtonElement",
    "HTMLCanvasElement",
    "HTMLTableCaptionElement",
    "HTMLTableColElement",
    "HTMLDataElement",
    "HTMLDataListElement",
    "HTMLDialogElement",
    "HTMLModElement",
    "HTMLDirectoryElement",
    "HTMLDivElement",
    "HTMLDListElement",
    "HTMLEmbedElement",
    "HTMLFieldSetElement",
    "HTMLFontElement",
    "HTMLFormElement",
    "HTMLFrameElement",
    "HTMLFrameSetElement",
    "HTMLHeadingElement",
    "HTMLHeadElement",
    "HTMLHRElement",
    "HTMLHtmlElement",
    "HTMLIFrameElement",
    "HTMLImageElement",
    "HTMLInputElement",
    "HTMLLabelElement",
    "HTMLLegendElement",
    "HTMLLIElement",
    "HTMLLinkElement",
    "HTMLMapElement",
    "HTMLMetaElement",
    "HTMLMeterElement",
    "HTMLObjectElement",
    "HTMLOListElement",
    "HTMLOptGroupElement",
    "HTMLOptionElement",
    "HTMLOutputElement",
    "HTMLParagraphElement",
    "HTMLParamElement",
    "HTMLPreElement",
    "HTMLProgressElement",
    "HTMLQuoteElement",
    "HTMLScriptElement",
    "HTMLSelectElement",
    "HTMLSourceElement",
    "HTMLSpanElement",
    "HTMLStyleElement",
    "HTMLTableElement",
    "HTMLTableSectionElement",
    "HTMLTableCellElement",
    "HTMLTemplateElement",
    "HTMLTextAreaElement",
    "HTMLTimeElement",
    "HTMLTitleElement",
    "HTMLTableRowElement",
    "HTMLTrackElement",
    "HTMLUListElement",
    "HTMLVideoElement",
    "HTMLUnknownElement",
];

/// Register all HTML element type constructors as globals.
/// Each one has a prototype that inherits from Element.prototype,
/// and HTMLElement.prototype is the base for most of them.
fn register_html_element_types(context: &mut Context, element_proto: &JsObject) {
    // Create HTMLElement.prototype inheriting from Element.prototype
    let html_element_proto = ObjectInitializer::new(context).build();
    html_element_proto.set_prototype(Some(element_proto.clone()));

    let html_element_ctor = make_illegal_constructor(context, "HTMLElement");
    html_element_ctor
        .define_property_or_throw(
            js_string!("prototype"),
            prop_desc::prototype_on_ctor(html_element_proto.clone()),
            context,
        )
        .expect("failed to define HTMLElement.prototype");

    html_element_proto
        .define_property_or_throw(
            js_string!("constructor"),
            prop_desc::constructor_on_proto(html_element_ctor.clone()),
            context,
        )
        .expect("failed to set HTMLElement.prototype.constructor");

    context
        .register_global_property(
            js_string!("HTMLElement"),
            html_element_ctor,
            Attribute::WRITABLE | Attribute::CONFIGURABLE,
        )
        .expect("failed to register HTMLElement global");

    // Register all specific HTML element types, each inheriting from HTMLElement.prototype
    for name in HTML_ELEMENT_TYPE_NAMES {
        if *name == "HTMLElement" {
            continue; // Already registered
        }

        let proto = ObjectInitializer::new(context).build();
        proto.set_prototype(Some(html_element_proto.clone()));

        let ctor = make_illegal_constructor(context, name);
        ctor.define_property_or_throw(
            js_string!("prototype"),
            prop_desc::prototype_on_ctor(proto.clone()),
            context,
        )
        .expect("failed to define prototype");

        proto
            .define_property_or_throw(
                js_string!("constructor"),
                boa_engine::property::PropertyDescriptor::builder()
                    .value(ctor.clone())
                    .writable(true)
                    .configurable(true)
                    .enumerable(false)
                    .build(),
                context,
            )
            .expect("failed to set prototype.constructor");

        context
            .register_global_property(js_string!(*name), ctor, Attribute::WRITABLE | Attribute::CONFIGURABLE)
            .expect("failed to register HTML element type global");
    }
}

/// Register DocumentFragment constructor.
/// `new DocumentFragment()` creates a new empty DocumentFragment node per spec.
fn register_document_fragment_type(context: &mut Context, node_proto: &JsObject) {
    // DocumentFragment.prototype inherits from Node.prototype (NOT Element.prototype).
    // Per spec, DocumentFragment does NOT have matches/closest/webkitMatchesSelector.
    // ParentNode mixin methods (querySelector, etc.) are copied from Element.prototype below.
    let proto = ObjectInitializer::new(context).build();
    proto.set_prototype(Some(node_proto.clone()));

    // Copy ParentNode mixin methods and accessors from Element.prototype to
    // DocumentFragment.prototype using JS, similar to the CharacterData copy pattern.
    // This copies querySelector, querySelectorAll, children, childElementCount, etc.
    // but excludes matches/closest/webkitMatchesSelector (Element-only methods).
    context
        .register_global_property(js_string!("__braille_df_proto"), proto.clone(), Attribute::all())
        .expect("failed to register temp df proto");
    context
        .eval(Source::from_bytes(
            r#"
            (function() {
                var src = Element.prototype;
                var dst = __braille_df_proto;
                // Copy specific ParentNode/ChildNode mixin properties
                var names = Object.getOwnPropertyNames(src);
                // Exclude Element-only methods that DocumentFragment must NOT have
                var exclude = {
                    'constructor': true,
                    'matches': true,
                    'closest': true,
                    'webkitMatchesSelector': true
                };
                for (var i = 0; i < names.length; i++) {
                    var name = names[i];
                    if (exclude[name]) continue;
                    // Skip if already defined on Node.prototype (inherited)
                    var desc = Object.getOwnPropertyDescriptor(src, name);
                    if (desc) {
                        Object.defineProperty(dst, name, desc);
                    }
                }
                delete self.__braille_df_proto;
            })();
            "#,
        ))
        .expect("failed to copy ParentNode properties to DocumentFragment.prototype");

    let ctor_native = unsafe {
        NativeFunction::from_closure(move |_this, _args, ctx| {
            // Get the global document's tree from RealmState
            let tree = realm_state::dom_tree(ctx);
            let node_id = tree.borrow_mut().create_document_fragment();
            let js_obj = bindings::element::get_or_create_js_element(node_id, tree, ctx)?;
            Ok(js_obj.into())
        })
    };

    let ctor = FunctionObjectBuilder::new(context.realm(), ctor_native)
        .name(js_string!("DocumentFragment"))
        .length(0)
        .constructor(true)
        .build();

    ctor.define_property_or_throw(
        js_string!("prototype"),
        prop_desc::prototype_on_ctor(proto.clone()),
        context,
    )
    .expect("failed to define DocumentFragment.prototype");

    proto
        .define_property_or_throw(
            js_string!("constructor"),
            prop_desc::constructor_on_proto(ctor.clone()),
            context,
        )
        .expect("failed to set DocumentFragment.prototype.constructor");

    // Add getElementById to DocumentFragment.prototype (NonElementParentNode mixin)
    let get_by_id_fn = NativeFunction::from_fn_ptr(bindings::query::fragment_get_element_by_id);
    proto
        .set(
            js_string!("getElementById"),
            get_by_id_fn.to_js_function(context.realm()),
            false,
            context,
        )
        .expect("failed to set DocumentFragment.prototype.getElementById");

    context
        .register_global_property(
            js_string!("DocumentFragment"),
            ctor,
            Attribute::WRITABLE | Attribute::CONFIGURABLE,
        )
        .expect("failed to register DocumentFragment global");
}

/// Register ShadowRoot constructor (illegal) and prototype inheriting from DocumentFragment.
fn register_shadow_root_type(context: &mut Context) {
    // ShadowRoot.prototype inherits from DocumentFragment.prototype.
    // ShadowRoot.prototype inherits from DocumentFragment.prototype.
    let proto = ObjectInitializer::new(context).build();
    if let Ok(df_ctor_val) = context.global_object().get(js_string!("DocumentFragment"), context) {
        if let Some(df_ctor) = df_ctor_val.as_object() {
            if let Ok(df_proto_val) = df_ctor.get(js_string!("prototype"), context) {
                if let Some(df_proto) = df_proto_val.as_object() {
                    proto.set_prototype(Some(df_proto.clone()));
                }
            }
        }
    }

    // mode getter
    let mode_getter = NativeFunction::from_fn_ptr(|this, _args, _ctx| {
        extract_element!(el, this, "ShadowRoot.mode getter");
        let tree = el.tree.borrow();
        let node = tree.get_node(el.node_id);
        match &node.data {
            crate::dom::NodeData::ShadowRoot { mode, .. } => {
                let s = match mode {
                    crate::dom::node::ShadowRootMode::Open => "open",
                    crate::dom::node::ShadowRootMode::Closed => "closed",
                };
                Ok(JsValue::from(js_string!(s)))
            }
            _ => Ok(JsValue::undefined()),
        }
    });

    // host getter
    let host_getter = NativeFunction::from_fn_ptr(|this, _args, ctx| {
        extract_element!(el, this, "ShadowRoot.host getter");
        let tree_rc = el.tree.clone();
        let tree = tree_rc.borrow();
        let node = tree.get_node(el.node_id);
        match node.data {
            crate::dom::NodeData::ShadowRoot { host, .. } => {
                drop(tree);
                let js_obj = bindings::element::get_or_create_js_element(host, tree_rc, ctx)?;
                Ok(js_obj.into())
            }
            _ => Ok(JsValue::null()),
        }
    });

    let realm = context.realm().clone();
    proto
        .define_property_or_throw(
            js_string!("mode"),
            boa_engine::property::PropertyDescriptor::builder()
                .get(mode_getter.to_js_function(&realm))
                .configurable(true)
                .enumerable(true)
                .build(),
            context,
        )
        .expect("failed to define ShadowRoot.prototype.mode");

    proto
        .define_property_or_throw(
            js_string!("host"),
            boa_engine::property::PropertyDescriptor::builder()
                .get(host_getter.to_js_function(&realm))
                .configurable(true)
                .enumerable(true)
                .build(),
            context,
        )
        .expect("failed to define ShadowRoot.prototype.host");

    // innerHTML getter/setter on ShadowRoot.prototype — delegates to the existing Element innerHTML implementation
    // (ShadowRoot already inherits querySelector/appendChild from DocumentFragment.prototype)
    let inner_html_getter = NativeFunction::from_fn_ptr(bindings::inner_html::get_inner_html);
    let inner_html_setter = NativeFunction::from_fn_ptr(bindings::inner_html::set_inner_html);
    proto
        .define_property_or_throw(
            js_string!("innerHTML"),
            boa_engine::property::PropertyDescriptor::builder()
                .get(inner_html_getter.to_js_function(&realm))
                .set(inner_html_setter.to_js_function(&realm))
                .configurable(true)
                .enumerable(true)
                .build(),
            context,
        )
        .expect("failed to define ShadowRoot.prototype.innerHTML");

    let ctor = make_illegal_constructor(context, "ShadowRoot");
    ctor.define_property_or_throw(
        js_string!("prototype"),
        prop_desc::prototype_on_ctor(proto.clone()),
        context,
    )
    .expect("failed to define ShadowRoot.prototype");

    proto
        .define_property_or_throw(
            js_string!("constructor"),
            prop_desc::constructor_on_proto(ctor.clone()),
            context,
        )
        .expect("failed to set ShadowRoot.prototype.constructor");

    context
        .register_global_property(
            js_string!("ShadowRoot"),
            ctor,
            Attribute::WRITABLE | Attribute::CONFIGURABLE,
        )
        .expect("failed to register ShadowRoot global");
}

/// Register DocumentType constructor
fn register_document_type_type(context: &mut Context, element_proto: &JsObject) {
    let proto = ObjectInitializer::new(context).build();
    proto.set_prototype(Some(element_proto.clone()));

    let ctor = make_illegal_constructor(context, "DocumentType");
    ctor.define_property_or_throw(
        js_string!("prototype"),
        boa_engine::property::PropertyDescriptor::builder()
            .value(proto.clone())
            .writable(false)
            .configurable(false)
            .enumerable(false)
            .build(),
        context,
    )
    .expect("failed to define DocumentType.prototype");

    proto
        .define_property_or_throw(
            js_string!("constructor"),
            prop_desc::constructor_on_proto(ctor.clone()),
            context,
        )
        .expect("failed to set DocumentType.prototype.constructor");

    context
        .register_global_property(
            js_string!("DocumentType"),
            ctor,
            Attribute::WRITABLE | Attribute::CONFIGURABLE,
        )
        .expect("failed to register DocumentType global");
}

/// Register a working Document constructor global.
/// `new Document()` creates a new blank XML document (contentType: application/xml).
/// Document.prototype inherits from Element.prototype so it gets cloneNode etc.
fn register_document_constructor(context: &mut Context, element_proto: &JsObject) {
    let proto = ObjectInitializer::new(context).build();
    proto.set_prototype(Some(element_proto.clone()));

    // Add default Document properties to the prototype so all Document instances
    // (including clones) inherit them
    let doc_defaults: &[(&str, &str)] = &[
        ("charset", "UTF-8"),
        ("characterSet", "UTF-8"),
        ("inputEncoding", "UTF-8"),
        ("URL", "about:blank"),
        ("documentURI", "about:blank"),
        ("compatMode", "CSS1Compat"),
        ("contentType", "application/xml"),
    ];
    for (name, value) in doc_defaults {
        proto
            .define_property_or_throw(
                js_string!(*name),
                boa_engine::property::PropertyDescriptor::builder()
                    .value(JsValue::from(js_string!(*value)))
                    .writable(true)
                    .configurable(true)
                    .enumerable(false)
                    .build(),
                context,
            )
            .expect("failed to set Document.prototype default property");
    }

    let ctor = unsafe {
        NativeFunction::from_closure(move |_this, _args, ctx| bindings::document::create_blank_xml_document(ctx))
    };

    let ctor_fn = FunctionObjectBuilder::new(context.realm(), ctor)
        .name(js_string!("Document"))
        .length(0)
        .constructor(true)
        .build();

    ctor_fn
        .define_property_or_throw(
            js_string!("prototype"),
            prop_desc::prototype_on_ctor(proto.clone()),
            context,
        )
        .expect("failed to define Document.prototype");

    proto
        .define_property_or_throw(
            js_string!("constructor"),
            boa_engine::property::PropertyDescriptor::builder()
                .value(ctor_fn.clone())
                .writable(true)
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to set Document.prototype.constructor");

    context
        .register_global_property(
            js_string!("Document"),
            ctor_fn,
            Attribute::WRITABLE | Attribute::CONFIGURABLE,
        )
        .expect("failed to register Document global");
}

/// Register XMLDocument as a global constructor.
/// XMLDocument extends Document in the spec. We create a simple constructor
/// whose prototype inherits from Document.prototype so `instanceof XMLDocument` works.
fn register_xml_document_global(context: &mut Context) {
    // Get Document.prototype from the global Document constructor
    let global = context.global_object();
    let doc_ctor = global
        .get(js_string!("Document"), context)
        .expect("Document not registered");
    let doc_ctor_obj = doc_ctor.as_object().expect("Document should be an object");
    let doc_proto = doc_ctor_obj
        .get(js_string!("prototype"), context)
        .expect("Document.prototype missing");
    let doc_proto_obj = doc_proto.as_object().expect("Document.prototype should be an object");

    // Create XMLDocument.prototype that inherits from Document.prototype
    let xml_proto = ObjectInitializer::new(context).build();
    xml_proto.set_prototype(Some(doc_proto_obj.clone()));

    // XMLDocument constructor (not callable from JS in practice, but needed for instanceof)
    let ctor = NativeFunction::from_fn_ptr(|_this, _args, _ctx| {
        Err(JsNativeError::typ().with_message("Illegal constructor").into())
    });

    let ctor_fn = FunctionObjectBuilder::new(context.realm(), ctor)
        .name(js_string!("XMLDocument"))
        .length(0)
        .constructor(true)
        .build();

    ctor_fn
        .define_property_or_throw(
            js_string!("prototype"),
            boa_engine::property::PropertyDescriptor::builder()
                .value(xml_proto.clone())
                .writable(false)
                .configurable(false)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to define XMLDocument.prototype");

    xml_proto
        .define_property_or_throw(
            js_string!("constructor"),
            boa_engine::property::PropertyDescriptor::builder()
                .value(ctor_fn.clone())
                .writable(true)
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to set XMLDocument.prototype.constructor");

    context
        .register_global_property(
            js_string!("XMLDocument"),
            ctor_fn,
            Attribute::WRITABLE | Attribute::CONFIGURABLE,
        )
        .expect("failed to register XMLDocument global");
}

/// Populate the RealmState dom_prototypes with prototypes from the globally
/// registered HTML element type constructors, DocumentFragment, and DocumentType.
fn populate_dom_prototypes(context: &mut Context) {
    let global = context.global_object();

    // Build tag-name -> prototype mapping
    let tag_to_type: &[(&str, &str)] = &[
        ("a", "HTMLAnchorElement"),
        ("area", "HTMLAreaElement"),
        ("audio", "HTMLAudioElement"),
        ("base", "HTMLBaseElement"),
        ("body", "HTMLBodyElement"),
        ("br", "HTMLBRElement"),
        ("button", "HTMLButtonElement"),
        ("canvas", "HTMLCanvasElement"),
        ("caption", "HTMLTableCaptionElement"),
        ("col", "HTMLTableColElement"),
        ("colgroup", "HTMLTableColElement"),
        ("data", "HTMLDataElement"),
        ("datalist", "HTMLDataListElement"),
        ("dialog", "HTMLDialogElement"),
        ("del", "HTMLModElement"),
        ("ins", "HTMLModElement"),
        ("dir", "HTMLDirectoryElement"),
        ("div", "HTMLDivElement"),
        ("dl", "HTMLDListElement"),
        ("embed", "HTMLEmbedElement"),
        ("fieldset", "HTMLFieldSetElement"),
        ("font", "HTMLFontElement"),
        ("form", "HTMLFormElement"),
        ("frame", "HTMLFrameElement"),
        ("frameset", "HTMLFrameSetElement"),
        ("h1", "HTMLHeadingElement"),
        ("h2", "HTMLHeadingElement"),
        ("h3", "HTMLHeadingElement"),
        ("h4", "HTMLHeadingElement"),
        ("h5", "HTMLHeadingElement"),
        ("h6", "HTMLHeadingElement"),
        ("head", "HTMLHeadElement"),
        ("hr", "HTMLHRElement"),
        ("html", "HTMLHtmlElement"),
        ("iframe", "HTMLIFrameElement"),
        ("img", "HTMLImageElement"),
        ("input", "HTMLInputElement"),
        ("label", "HTMLLabelElement"),
        ("legend", "HTMLLegendElement"),
        ("li", "HTMLLIElement"),
        ("link", "HTMLLinkElement"),
        ("map", "HTMLMapElement"),
        ("meta", "HTMLMetaElement"),
        ("meter", "HTMLMeterElement"),
        ("object", "HTMLObjectElement"),
        ("ol", "HTMLOListElement"),
        ("optgroup", "HTMLOptGroupElement"),
        ("option", "HTMLOptionElement"),
        ("output", "HTMLOutputElement"),
        ("p", "HTMLParagraphElement"),
        ("param", "HTMLParamElement"),
        ("pre", "HTMLPreElement"),
        ("progress", "HTMLProgressElement"),
        ("q", "HTMLQuoteElement"),
        ("script", "HTMLScriptElement"),
        ("select", "HTMLSelectElement"),
        ("source", "HTMLSourceElement"),
        ("span", "HTMLSpanElement"),
        ("style", "HTMLStyleElement"),
        ("table", "HTMLTableElement"),
        ("tbody", "HTMLTableSectionElement"),
        ("thead", "HTMLTableSectionElement"),
        ("tfoot", "HTMLTableSectionElement"),
        ("td", "HTMLTableCellElement"),
        ("th", "HTMLTableCellElement"),
        ("template", "HTMLTemplateElement"),
        ("textarea", "HTMLTextAreaElement"),
        ("time", "HTMLTimeElement"),
        ("title", "HTMLTitleElement"),
        ("tr", "HTMLTableRowElement"),
        ("track", "HTMLTrackElement"),
        ("ul", "HTMLUListElement"),
        ("video", "HTMLVideoElement"),
    ];

    // Helper: extract Constructor.prototype from a global constructor name
    let mut get_proto = |name: &str| -> Option<JsObject> {
        let ctor_val = global.get(js_string!(name), context).ok()?;
        let ctor_obj = ctor_val.as_object()?;
        let proto_val = ctor_obj.get(js_string!("prototype"), context).ok()?;
        Some(proto_val.as_object()?.clone())
    };

    let mut html_tag_protos = HashMap::new();
    for (tag, type_name) in tag_to_type {
        if let Some(proto) = get_proto(type_name) {
            html_tag_protos.insert(tag.to_string(), proto);
        }
    }

    let html_element_proto = get_proto("HTMLElement");
    let html_unknown_proto = get_proto("HTMLUnknownElement");
    let document_fragment_proto = get_proto("DocumentFragment");
    let shadow_root_proto = get_proto("ShadowRoot");
    let document_type_proto = get_proto("DocumentType");
    let document_proto = get_proto("Document");
    let xml_document_proto = get_proto("XMLDocument");

    if let Some(mut p) = realm_state::dom_prototypes(context) {
        p.html_tag_protos = html_tag_protos;
        p.html_element_proto = html_element_proto;
        p.html_unknown_proto = html_unknown_proto;
        p.document_fragment_proto = document_fragment_proto;
        p.shadow_root_proto = shadow_root_proto;
        p.document_type_proto = document_type_proto;
        p.document_proto = document_proto;
        p.xml_document_proto = xml_document_proto;
        realm_state::set_dom_prototypes(context, p);
    }
}

/// Register the `performance` global object with a `now()` method.
/// `performance.now()` returns a DOMHighResTimeStamp: milliseconds elapsed since runtime creation.
pub(crate) fn register_performance_global(context: &mut Context) {
    use bindings::event::dom_high_res_time_stamp;

    let now_fn = NativeFunction::from_fn_ptr(|_this, _args, ctx| Ok(JsValue::from(dom_high_res_time_stamp(ctx))));

    let performance = ObjectInitializer::new(context)
        .function(now_fn, js_string!("now"), 0)
        .build();

    context
        .register_global_property(
            js_string!("performance"),
            performance,
            Attribute::WRITABLE | Attribute::CONFIGURABLE,
        )
        .expect("failed to register performance global");
}

/// Register a minimal `location` global object stub.
/// The Node-properties WPT test uses `String(location)` to get the document URL.
pub(crate) fn register_location_global(context: &mut Context) {
    let location = ObjectInitializer::new(context).build();
    let realm = context.realm().clone();

    // Shared hash state for getter/setter
    let hash_state: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));

    // toString returns full URL
    let hash_for_tostring = hash_state.clone();
    let to_string_fn = unsafe {
        NativeFunction::from_closure(move |_this, _args, _ctx| {
            let h = hash_for_tostring.borrow();
            let url = if h.is_empty() {
                "about:blank".to_string()
            } else {
                format!("about:blank#{}", h)
            };
            Ok(JsValue::from(js_string!(url)))
        })
    };
    location
        .define_property_or_throw(
            js_string!("toString"),
            boa_engine::property::PropertyDescriptor::builder()
                .value(to_string_fn.to_js_function(&realm))
                .writable(true)
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to define location.toString");

    // href getter/setter
    let hash_for_href_get = hash_state.clone();
    let href_getter = unsafe {
        NativeFunction::from_closure(move |_this, _args, _ctx| {
            let h = hash_for_href_get.borrow();
            let url = if h.is_empty() {
                "about:blank".to_string()
            } else {
                format!("about:blank#{}", h)
            };
            Ok(JsValue::from(js_string!(url)))
        })
    };
    let hash_for_href_set = hash_state.clone();
    let href_setter = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx2| {
            let val = args
                .first()
                .map(|v| v.to_string(ctx2))
                .transpose()?
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_default();
            // Extract fragment from href
            if let Some(idx) = val.find('#') {
                *hash_for_href_set.borrow_mut() = val[idx + 1..].to_string();
            }
            Ok(JsValue::undefined())
        })
    };
    location
        .define_property_or_throw(
            js_string!("href"),
            boa_engine::property::PropertyDescriptor::builder()
                .get(href_getter.to_js_function(&realm))
                .set(href_setter.to_js_function(&realm))
                .configurable(true)
                .enumerable(true)
                .build(),
            context,
        )
        .expect("failed to define location.href");

    // hash getter/setter — setter fires hashchange on window
    let hash_for_get = hash_state.clone();
    let hash_getter = unsafe {
        NativeFunction::from_closure(move |_this, _args, _ctx| {
            let h = hash_for_get.borrow();
            if h.is_empty() {
                Ok(JsValue::from(js_string!("")))
            } else {
                Ok(JsValue::from(js_string!(format!("#{}", h))))
            }
        })
    };
    let hash_for_set = hash_state.clone();
    let hash_setter = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx2| {
            let new_hash = args
                .first()
                .map(|v| v.to_string(ctx2))
                .transpose()?
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_default();

            // Strip leading # if present
            let new_hash = new_hash.strip_prefix('#').unwrap_or(&new_hash).to_string();

            let old_hash = hash_for_set.borrow().clone();
            if old_hash == new_hash {
                return Ok(JsValue::undefined());
            }

            let old_url = if old_hash.is_empty() {
                "about:blank".to_string()
            } else {
                format!("about:blank#{}", old_hash)
            };
            let new_url = if new_hash.is_empty() {
                "about:blank".to_string()
            } else {
                format!("about:blank#{}", new_hash)
            };

            *hash_for_set.borrow_mut() = new_hash;

            // Fire hashchange event on window
            fire_hashchange_event(ctx2, &old_url, &new_url);

            Ok(JsValue::undefined())
        })
    };
    location
        .define_property_or_throw(
            js_string!("hash"),
            boa_engine::property::PropertyDescriptor::builder()
                .get(hash_getter.to_js_function(&realm))
                .set(hash_setter.to_js_function(&realm))
                .configurable(true)
                .enumerable(true)
                .build(),
            context,
        )
        .expect("failed to define location.hash");

    context
        .register_global_property(
            js_string!("location"),
            location,
            Attribute::WRITABLE | Attribute::CONFIGURABLE,
        )
        .expect("failed to register location global");
}

/// Fire a HashChangeEvent on window with oldURL and newURL.
fn fire_hashchange_event(ctx: &mut Context, old_url: &str, new_url: &str) {
    if let Some(window) = realm_state::window_object(ctx) {
        // Create a plain object as the event with oldURL/newURL properties
        let event_obj = boa_engine::object::ObjectInitializer::new(ctx).build();
        let _ = event_obj.set(js_string!("type"), JsValue::from(js_string!("hashchange")), false, ctx);
        let _ = event_obj.set(js_string!("oldURL"), JsValue::from(js_string!(old_url)), false, ctx);
        let _ = event_obj.set(js_string!("newURL"), JsValue::from(js_string!(new_url)), false, ctx);
        let _ = event_obj.set(js_string!("bubbles"), JsValue::from(false), false, ctx);
        let _ = event_obj.set(js_string!("cancelable"), JsValue::from(false), false, ctx);

        // Call window.onhashchange if set
        if let Ok(handler) = window.get(js_string!("onhashchange"), ctx) {
            if let Some(handler_fn) = handler.as_object().filter(|o| o.is_callable()) {
                let _ = handler_fn.call(&JsValue::from(window.clone()), &[JsValue::from(event_obj.clone())], ctx);
            }
        }

        // Also fire on window's addEventListener("hashchange") listeners
        let listeners = realm_state::event_listeners(ctx);
        let window_key = (usize::MAX, bindings::window::WINDOW_LISTENER_ID);
        let hashchange_listeners: Vec<JsObject> = {
            let map = listeners.borrow();
            map.get(&window_key)
                .map(|entries| {
                    entries
                        .iter()
                        .filter(|e| e.event_type == "hashchange")
                        .map(|e| e.callback.clone())
                        .collect()
                })
                .unwrap_or_default()
        };
        for callback in &hashchange_listeners {
            if callback.is_callable() {
                let _ = callback.call(&JsValue::from(window.clone()), &[JsValue::from(event_obj.clone())], ctx);
            }
        }
    }
}

/// Add composedPath() to Event.prototype and CustomEvent.prototype.
/// This is needed for EventTarget-constructible WPT test which checks e.composedPath().
pub(crate) fn register_composed_path(context: &mut Context) {
    let global = context.global_object();
    let composed_path_fn =
        NativeFunction::from_fn_ptr(bindings::event_target::composed_path).to_js_function(context.realm());

    // Add to Event.prototype
    let event_ctor = global
        .get(js_string!("Event"), context)
        .expect("Event constructor should exist");
    if let Some(event_obj) = event_ctor.as_object() {
        let proto = event_obj
            .get(js_string!("prototype"), context)
            .expect("Event.prototype should exist");
        if let Some(proto_obj) = proto.as_object() {
            proto_obj
                .set(js_string!("composedPath"), composed_path_fn.clone(), false, context)
                .expect("failed to set Event.prototype.composedPath");
        }
    }

    // Add to CustomEvent.prototype
    let custom_ctor = global
        .get(js_string!("CustomEvent"), context)
        .expect("CustomEvent constructor should exist");
    if let Some(custom_obj) = custom_ctor.as_object() {
        let proto = custom_obj
            .get(js_string!("prototype"), context)
            .expect("CustomEvent.prototype should exist");
        if let Some(proto_obj) = proto.as_object() {
            proto_obj
                .set(js_string!("composedPath"), composed_path_fn, false, context)
                .expect("failed to set CustomEvent.prototype.composedPath");
        }
    }
}

/// Creates a constructor function that throws "Illegal constructor" when called.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dom::{NodeData, NodeId};

    /// Helper: build a DomTree with document > html > body > div#app
    fn make_test_tree() -> Rc<RefCell<DomTree>> {
        let tree = Rc::new(RefCell::new(DomTree::new()));
        {
            let mut t = tree.borrow_mut();
            let html = t.create_element("html");
            let body = t.create_element("body");
            let div = t.create_element("div");

            // Set id="app" on the div
            if let NodeData::Element { ref mut attributes, .. } = t.get_node_mut(div).data {
                attributes.push(crate::dom::node::DomAttribute::new("id", "app"));
            }

            let doc = t.document();
            t.append_child(doc, html);
            t.append_child(html, body);
            t.append_child(body, div);
        }
        tree
    }

    #[test]
    fn create_element_adds_node_to_tree() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        rt.eval(r#"document.createElement("p")"#).unwrap();

        // The tree should now have an extra "p" node (unattached)
        let t = tree.borrow();
        // Nodes: [0]=Document, [1]=html, [2]=body, [3]=div#app, [4]=p
        let p_node = t.get_node(4);
        match &p_node.data {
            NodeData::Element { tag_name, .. } => assert_eq!(tag_name, "p"),
            other => panic!("expected Element, got {:?}", other),
        }
        // Unattached — no parent
        assert!(p_node.parent.is_none());
    }

    #[test]
    fn get_element_by_id_returns_element() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        let result = rt.eval(r#"document.getElementById("app")"#).unwrap();

        // Should not be null or undefined
        assert!(!result.is_null());
        assert!(!result.is_undefined());
        // Should be an object
        assert!(result.is_object());
    }

    #[test]
    fn get_element_by_id_returns_null_for_missing() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        let result = rt.eval(r#"document.getElementById("nonexistent")"#).unwrap();
        assert!(result.is_null());
    }

    #[test]
    fn text_content_getter_and_setter() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        // Set textContent on the div#app
        rt.eval(
            r#"
            var el = document.getElementById("app");
            el.textContent = "hello";
        "#,
        )
        .unwrap();

        // Verify via DomTree
        let t = tree.borrow();
        let div_id: NodeId = 3; // div#app
        assert_eq!(t.get_text_content(div_id), "hello");

        drop(t); // release borrow before eval

        // Read back through JS
        let result = rt
            .eval(
                r#"
            var el2 = document.getElementById("app");
            el2.textContent
        "#,
            )
            .unwrap();

        let text = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(text, "hello");
    }

    #[test]
    fn append_child_wires_parent_and_child() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        rt.eval(
            r#"
            var p = document.createElement("p");
            p.textContent = "new paragraph";
            var app = document.getElementById("app");
            app.appendChild(p);
        "#,
        )
        .unwrap();

        let t = tree.borrow();
        let div_id: NodeId = 3; // div#app

        // div#app's children should include the new <p>
        let div_children = &t.get_node(div_id).children;
        // The <p> was created as node 4, then set_text_content created a text node as 5
        // and appended it as child of 4. Then we appended 4 to div_id(3).
        assert!(div_children.contains(&4));

        // Verify the text content through the tree
        assert_eq!(t.get_text_content(4), "new paragraph");
        // The <p> node's parent should be div#app
        assert_eq!(t.get_node(4).parent, Some(div_id));
    }

    #[test]
    fn full_spike_integration() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        // This mirrors the spike's JS test script:
        // 1. Create a <p> element
        // 2. Set its textContent
        // 3. Find div#app by id
        // 4. Append the <p> to div#app
        rt.eval(
            r#"
            var p = document.createElement("p");
            p.textContent = "Hello from JS!";
            var app = document.getElementById("app");
            app.appendChild(p);
        "#,
        )
        .unwrap();

        let t = tree.borrow();

        // div#app (node 3) should have the <p> as a child
        let div_children = &t.get_node(3).children;
        let p_id: NodeId = 4;
        assert!(div_children.contains(&p_id), "div#app should contain the <p>");

        // The <p> should contain the text "Hello from JS!"
        assert_eq!(t.get_text_content(p_id), "Hello from JS!");

        // Verify the tag name of the new element
        match &t.get_node(p_id).data {
            NodeData::Element { tag_name, .. } => assert_eq!(tag_name, "p"),
            other => panic!("expected Element('p'), got {:?}", other),
        }

        // Verify the full text content of div#app includes the paragraph
        assert_eq!(t.get_text_content(3), "Hello from JS!");
    }

    #[test]
    fn document_body_returns_body_element() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        // Access document.body
        rt.eval(
            r#"
            var body = document.body;
            body.textContent = "body content";
        "#,
        )
        .unwrap();

        let t = tree.borrow();
        let body_id: NodeId = 2; // body is node 2 in make_test_tree
        assert_eq!(t.get_text_content(body_id), "body content");
    }

    #[test]
    fn document_head_returns_head_element() {
        let tree = Rc::new(RefCell::new(DomTree::new()));
        {
            let mut t = tree.borrow_mut();
            let html = t.create_element("html");
            let head = t.create_element("head");
            let body = t.create_element("body");

            let doc = t.document();
            t.append_child(doc, html);
            t.append_child(html, head);
            t.append_child(html, body);
        }

        let mut rt = JsRuntime::new(Rc::clone(&tree));

        // Access document.head
        let result = rt.eval(r#"document.head"#).unwrap();
        assert!(!result.is_null());

        // Verify we can manipulate it
        rt.eval(
            r#"
            var head = document.head;
            head.textContent = "head content";
        "#,
        )
        .unwrap();

        let t = tree.borrow();
        let head_id: NodeId = 1; // head is node 1
        assert_eq!(t.get_text_content(head_id), "head content");
    }

    #[test]
    fn document_head_returns_null_when_absent() {
        let tree = Rc::new(RefCell::new(DomTree::new()));
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        let result = rt.eval(r#"document.head"#).unwrap();
        assert!(result.is_null());
    }

    #[test]
    fn document_create_text_node_creates_text() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        rt.eval(
            r#"
            var textNode = document.createTextNode("hello world");
            var app = document.getElementById("app");
            app.appendChild(textNode);
        "#,
        )
        .unwrap();

        let t = tree.borrow();
        let div_id: NodeId = 3; // div#app
        let text = t.get_text_content(div_id);
        assert_eq!(text, "hello world");
    }

    #[test]
    fn document_title_getter_returns_empty_when_no_title() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        let result = rt.eval(r#"document.title"#).unwrap();
        let title = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(title, "");
    }

    #[test]
    fn document_title_getter_reads_title_element() {
        let tree = Rc::new(RefCell::new(DomTree::new()));
        {
            let mut t = tree.borrow_mut();
            let html = t.create_element("html");
            let head = t.create_element("head");
            let title = t.create_element("title");

            t.set_text_content(title, "My Page Title");

            let doc = t.document();
            t.append_child(doc, html);
            t.append_child(html, head);
            t.append_child(head, title);
        }

        let mut rt = JsRuntime::new(Rc::clone(&tree));

        let result = rt.eval(r#"document.title"#).unwrap();
        let title = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(title, "My Page Title");
    }

    #[test]
    fn document_title_setter_creates_or_updates_title() {
        let tree = Rc::new(RefCell::new(DomTree::new()));
        {
            let mut t = tree.borrow_mut();
            let html = t.create_element("html");
            let head = t.create_element("head");

            let doc = t.document();
            t.append_child(doc, html);
            t.append_child(html, head);
        }

        let mut rt = JsRuntime::new(Rc::clone(&tree));

        // Set title (should create <title> element)
        rt.eval(r#"document.title = "New Title""#).unwrap();

        // Read it back
        let result = rt.eval(r#"document.title"#).unwrap();
        let title = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(title, "New Title");

        // Verify through DomTree
        let t = tree.borrow();
        let titles = t.get_elements_by_tag_name("title");
        assert_eq!(titles.len(), 1);
        assert_eq!(t.get_text_content(titles[0]), "New Title");
    }

    #[test]
    fn document_title_setter_updates_existing_title() {
        let tree = Rc::new(RefCell::new(DomTree::new()));
        {
            let mut t = tree.borrow_mut();
            let html = t.create_element("html");
            let head = t.create_element("head");
            let title = t.create_element("title");

            t.set_text_content(title, "Old Title");

            let doc = t.document();
            t.append_child(doc, html);
            t.append_child(html, head);
            t.append_child(head, title);
        }

        let mut rt = JsRuntime::new(Rc::clone(&tree));

        // Update title
        rt.eval(r#"document.title = "Updated Title""#).unwrap();

        // Read it back
        let result = rt.eval(r#"document.title"#).unwrap();
        let title = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(title, "Updated Title");

        // Verify only one title element exists
        let t = tree.borrow();
        let titles = t.get_elements_by_tag_name("title");
        assert_eq!(titles.len(), 1);
    }

    #[test]
    fn class_list_add() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        rt.eval(
            r#"
            var el = document.getElementById("app");
            el.classList.add("foo");
            el.classList.add("bar");
        "#,
        )
        .unwrap();

        let t = tree.borrow();
        let div_id: NodeId = 3; // div#app
        let class_attr = t.get_attribute(div_id, "class");
        assert_eq!(class_attr, Some("foo bar".to_string()));
    }

    #[test]
    fn class_list_add_multiple() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        rt.eval(
            r#"
            var el = document.getElementById("app");
            el.classList.add("foo", "bar", "baz");
        "#,
        )
        .unwrap();

        let t = tree.borrow();
        let div_id: NodeId = 3; // div#app
        let class_attr = t.get_attribute(div_id, "class");
        assert_eq!(class_attr, Some("foo bar baz".to_string()));
    }

    #[test]
    fn class_list_remove() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        rt.eval(
            r#"
            var el = document.getElementById("app");
            el.classList.add("foo", "bar", "baz");
            el.classList.remove("bar");
        "#,
        )
        .unwrap();

        let t = tree.borrow();
        let div_id: NodeId = 3; // div#app
        let class_attr = t.get_attribute(div_id, "class");
        assert_eq!(class_attr, Some("foo baz".to_string()));
    }

    #[test]
    fn class_list_remove_all() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        rt.eval(
            r#"
            var el = document.getElementById("app");
            el.classList.add("foo", "bar");
            el.classList.remove("foo", "bar");
        "#,
        )
        .unwrap();

        let t = tree.borrow();
        let div_id: NodeId = 3; // div#app
        let class_attr = t.get_attribute(div_id, "class");
        // Per spec, class attribute stays as empty string when all classes are removed
        assert_eq!(class_attr, Some("".to_string()));
    }

    #[test]
    fn class_list_toggle() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        // Toggle adds the class when not present, returns true
        let result1 = rt
            .eval(
                r#"
            var el = document.getElementById("app");
            el.classList.toggle("foo");
        "#,
            )
            .unwrap();
        assert_eq!(result1.as_boolean(), Some(true));

        let t = tree.borrow();
        let div_id: NodeId = 3;
        assert_eq!(t.get_attribute(div_id, "class"), Some("foo".to_string()));
        drop(t);

        // Toggle removes the class when present, returns false
        let result2 = rt
            .eval(
                r#"
            var el = document.getElementById("app");
            el.classList.toggle("foo");
        "#,
            )
            .unwrap();
        assert_eq!(result2.as_boolean(), Some(false));

        let t = tree.borrow();
        assert_eq!(t.get_attribute(div_id, "class"), Some("".to_string()));
    }

    #[test]
    fn class_list_contains() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        rt.eval(
            r#"
            var el = document.getElementById("app");
            el.classList.add("foo", "bar");
        "#,
        )
        .unwrap();

        let result1 = rt
            .eval(
                r#"
            var el = document.getElementById("app");
            el.classList.contains("foo");
        "#,
            )
            .unwrap();
        assert_eq!(result1.as_boolean(), Some(true));

        let result2 = rt
            .eval(
                r#"
            var el = document.getElementById("app");
            el.classList.contains("baz");
        "#,
            )
            .unwrap();
        assert_eq!(result2.as_boolean(), Some(false));
    }

    #[test]
    fn class_list_item() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        rt.eval(
            r#"
            var el = document.getElementById("app");
            el.classList.add("foo", "bar", "baz");
        "#,
        )
        .unwrap();

        let result0 = rt
            .eval(
                r#"
            var el = document.getElementById("app");
            el.classList.item(0);
        "#,
            )
            .unwrap();
        let text0 = result0.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(text0, "foo");

        let result1 = rt
            .eval(
                r#"
            var el = document.getElementById("app");
            el.classList.item(1);
        "#,
            )
            .unwrap();
        let text1 = result1.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(text1, "bar");

        let result_out_of_bounds = rt
            .eval(
                r#"
            var el = document.getElementById("app");
            el.classList.item(99);
        "#,
            )
            .unwrap();
        assert!(result_out_of_bounds.is_null());
    }

    #[test]
    fn class_list_length() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        let result_empty = rt
            .eval(
                r#"
            var el = document.getElementById("app");
            el.classList.length;
        "#,
            )
            .unwrap();
        assert_eq!(result_empty.as_number(), Some(0.0));

        rt.eval(
            r#"
            var el = document.getElementById("app");
            el.classList.add("foo", "bar", "baz");
        "#,
        )
        .unwrap();

        let result_three = rt
            .eval(
                r#"
            var el = document.getElementById("app");
            el.classList.length;
        "#,
            )
            .unwrap();
        assert_eq!(result_three.as_number(), Some(3.0));
    }

    #[test]
    fn class_list_no_duplicate_add() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        rt.eval(
            r#"
            var el = document.getElementById("app");
            el.classList.add("foo");
            el.classList.add("foo");
            el.classList.add("foo");
        "#,
        )
        .unwrap();

        let t = tree.borrow();
        let div_id: NodeId = 3;
        let class_attr = t.get_attribute(div_id, "class");
        // Should only have "foo" once
        assert_eq!(class_attr, Some("foo".to_string()));
    }

    #[test]
    fn class_list_workflow_integration() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        rt.eval(
            r#"
            var el = document.getElementById("app");

            // Start empty
            if (el.classList.length !== 0) throw new Error("Expected length 0");

            // Add some classes
            el.classList.add("foo", "bar");
            if (el.classList.length !== 2) throw new Error("Expected length 2");
            if (!el.classList.contains("foo")) throw new Error("Expected foo");
            if (!el.classList.contains("bar")) throw new Error("Expected bar");

            // Toggle off foo
            var removed = el.classList.toggle("foo");
            if (removed !== false) throw new Error("Expected toggle to return false");
            if (el.classList.contains("foo")) throw new Error("foo should be removed");
            if (el.classList.length !== 1) throw new Error("Expected length 1");

            // Toggle on baz
            var added = el.classList.toggle("baz");
            if (added !== true) throw new Error("Expected toggle to return true");
            if (!el.classList.contains("baz")) throw new Error("Expected baz");
            if (el.classList.length !== 2) throw new Error("Expected length 2");

            // Check items
            if (el.classList.item(0) !== "bar") throw new Error("Expected bar at index 0");
            if (el.classList.item(1) !== "baz") throw new Error("Expected baz at index 1");

            // Remove all
            el.classList.remove("bar", "baz");
            if (el.classList.length !== 0) throw new Error("Expected length 0");
        "#,
        )
        .unwrap();

        // All assertions passed in JS; verify final state in Rust
        let t = tree.borrow();
        let div_id: NodeId = 3;
        assert_eq!(t.get_attribute(div_id, "class"), Some("".to_string()));
    }

    #[test]
    fn text_constructor_debug() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        let result = rt.eval("typeof Text").unwrap();
        let s = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "function", "Text should be a function");

        let result2 = rt.eval("var t = new Text('hello'); t.data").unwrap();
        let s2 = result2.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(s2, "hello", "Text data should be 'hello'");

        // Check window.Text === Text
        let result3 = rt.eval("typeof window.Text").unwrap();
        let s3 = result3.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(s3, "function", "window.Text should be a function");

        // Check window[ctor] pattern used by WPT
        let result4 = rt.eval("var ctor = 'Text'; new window[ctor]('test').data").unwrap();
        let s4 = result4.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(s4, "test", "window['Text'] constructor should work");
    }

    #[test]
    fn cross_tree_replace_child_identity() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt
            .eval(
                r#"
            var results = [];
            var doc = document.implementation.createHTMLDocument("title");
            var doc2 = document.implementation.createHTMLDocument("title2");
            var doctype = doc.doctype;
            var doctype2 = doc2.doctype;

            results.push("before: doc.childNodes.length=" + doc.childNodes.length);
            results.push("before: doc2.childNodes.length=" + doc2.childNodes.length);
            results.push("doctype.nodeType=" + doctype.nodeType);
            results.push("doctype2.nodeType=" + doctype2.nodeType);

            doc.replaceChild(doc2.doctype, doc.doctype);

            results.push("after: doc.childNodes.length=" + doc.childNodes.length);
            results.push("after: doc2.childNodes.length=" + doc2.childNodes.length);

            results.push("doctype.parentNode === null: " + (doctype.parentNode === null));
            results.push("doctype2.parentNode === doc: " + (doctype2.parentNode === doc));
            results.push("doctype2.parentNode: " + doctype2.parentNode);
            results.push("doc: " + doc);

            // Check childNodes identity
            results.push("doc.childNodes[0] === doctype2: " + (doc.childNodes[0] === doctype2));

            results.join("\n");
        "#,
            )
            .unwrap();
        let s = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        eprintln!("{}", s);
        assert!(
            s.contains("doctype2.parentNode === doc: true"),
            "doctype2.parentNode should be doc: {}",
            s
        );
    }

    #[test]
    fn node_prototype_insert_before_is_callable() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt
            .eval(
                r#"
            var results = [];
            results.push("typeof Node.prototype.insertBefore = " + typeof Node.prototype.insertBefore);
            results.push("typeof Node.prototype.replaceChild = " + typeof Node.prototype.replaceChild);
            results.push("typeof Node.prototype.removeChild = " + typeof Node.prototype.removeChild);
            results.push("typeof Node.prototype.appendChild = " + typeof Node.prototype.appendChild);
            // Try calling via .call()
            try {
                var parent = document.createElement("div");
                var child = document.createElement("span");
                Node.prototype.insertBefore.call(parent, child, null);
                results.push("call succeeded, parent.childNodes.length=" + parent.childNodes.length);
            } catch(e) {
                results.push("call error: " + e.message);
            }
            // Test the exact WPT pattern: assign to var, then .call() on non-parent nodes
            var insertFunc = Node.prototype.insertBefore;
            results.push("insertFunc type = " + typeof insertFunc);
            try {
                var doctype = document.implementation.createDocumentType("html", "", "");
                var node = document.createElement("div");
                var child = document.createElement("div");
                insertFunc.call(doctype, node, child);
                results.push("doctype call: no error");
            } catch(e) {
                results.push("doctype call error: " + e.message);
            }
            results.join("\n");
        "#,
            )
            .unwrap();
        let s = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        eprintln!("{}", s);
        assert!(
            s.contains("typeof Node.prototype.insertBefore = function"),
            "insertBefore should be a function on Node.prototype: {}",
            s
        );
    }
}
