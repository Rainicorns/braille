use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Instant;

use boa_engine::{
    class::Class,
    js_string,
    native_function::NativeFunction,
    object::{FunctionObjectBuilder, JsObject, ObjectInitializer},
    property::Attribute,
    Context, JsError, JsNativeError, JsResult, JsValue, Source,
};

use crate::dom::DomTree;

use super::bindings;
use super::bindings::event_target::{ListenerMap, EVENT_LISTENERS};
use super::bindings::element::{
    get_or_create_js_element, DomPrototypes, NodeCache, DOM_PROTOTYPES, DOM_TREE, NODE_CACHE,
};
use super::bindings::event::RUNTIME_CREATION_TIME;

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
        let creation_time = Instant::now();
        let mut context = Context::default();
        let console_buffer = Rc::new(RefCell::new(Vec::new()));
        let listeners: Rc<RefCell<ListenerMap>> = Rc::new(RefCell::new(HashMap::new()));
        let node_cache: Rc<RefCell<NodeCache>> = Rc::new(RefCell::new(HashMap::new()));

        // Store the listeners Rc in the thread-local so NativeFunction callbacks
        // (addEventListener, removeEventListener) can access it.
        EVENT_LISTENERS.with(|el| {
            *el.borrow_mut() = Some(Rc::clone(&listeners));
        });

        // Store the node cache Rc in the thread-local so NativeFunction callbacks
        // can return the same JsObject for the same NodeId (object identity).
        NODE_CACHE.with(|cell| {
            *cell.borrow_mut() = Some(Rc::clone(&node_cache));
        });

        // Store the DomTree in the thread-local so Text/Comment constructors can access it
        DOM_TREE.with(|cell| {
            *cell.borrow_mut() = Some(Rc::clone(&tree));
        });

        // Initialize the iframe content docs map
        super::bindings::element::IFRAME_CONTENT_DOCS.with(|cell| {
            *cell.borrow_mut() = Some(Rc::new(RefCell::new(HashMap::new())));
        });

        // Initialize the iframe src content map (populated later by Engine::populate_iframe_src_content)
        super::bindings::element::IFRAME_SRC_CONTENT.with(|cell| {
            *cell.borrow_mut() = Some(Rc::new(RefCell::new(HashMap::new())));
        });

        // Initialize the iframe onload handlers map
        super::bindings::element::IFRAME_ONLOAD_HANDLERS.with(|cell| {
            *cell.borrow_mut() = Some(Rc::new(RefCell::new(HashMap::new())));
        });

        // Initialize the unified on* event handler map
        super::bindings::on_event::init_on_event_handlers();

        // Store the creation time in the thread-local so event.rs can compute DOMHighResTimeStamp
        RUNTIME_CREATION_TIME.with(|cell| {
            *cell.borrow_mut() = Some(creation_time);
        });

        // Initialize MutationObserver state
        bindings::mutation_observer::init_mutation_observer_state();

        // Register DOMImplementation global constructor (for instanceof) — must be before register_document
        bindings::document::register_domimplementation(&mut context);

        // Register DOMParser global constructor — must be before register_window which copies globals to window
        bindings::dom_parser::register_dom_parser(&mut context);

        // Register DOMException global constructor — must be before register_window
        bindings::register_dom_exception(&mut context);

        bindings::register_document(Rc::clone(&tree), &mut context);
        bindings::window::register_window(&mut context, Rc::clone(&console_buffer), Rc::clone(&tree));

        // Register the unified Event class (all event subtypes use JsEvent with EventKind)
        context.register_global_class::<bindings::event::JsEvent>().unwrap();

        // Wrap Event/CustomEvent/UIEvent subclass constructors to add isTrusted as own property on each instance.
        // Must happen BEFORE register_event_constants so constants go on the wrapper constructors.
        Self::wrap_event_constructors(&mut context);
        bindings::event::register_event_constants(&mut context);

        // Register performance.now() global
        Self::register_performance_global(&mut context);

        // Register CSSStyleDeclaration class for getComputedStyle
        context.register_global_class::<bindings::computed_style::JsComputedStyle>().unwrap();

        // Register global Node, CharacterData, Text, Comment with proper prototype chain
        Self::register_dom_type_hierarchy(&mut context);

        // Register NodeList and HTMLCollection globals
        bindings::collections::register_collections(&mut context);

        // Register `location` global stub (needed by WPT Node-properties test)
        Self::register_location_global(&mut context);

        // Register EventTarget class (standalone constructor: new EventTarget())
        context.register_global_class::<bindings::event_target::JsEventTarget>().unwrap();

        // Register MutationObserver and MutationRecord globals
        bindings::mutation_observer::register_mutation_observer_global(&mut context);
        bindings::mutation_observer::register_mutation_record_global(&mut context);

        // Add composedPath() to Event.prototype and CustomEvent.prototype
        Self::register_composed_path(&mut context);

        // Copy EventTarget to window, and copy event listener methods to global
        // so that globalThis.addEventListener/removeEventListener/dispatchEvent work.
        {
            let global = context.global_object();
            let window_val = global.get(js_string!("window"), &mut context)
                .expect("window global should exist");
            if let Some(window_obj) = window_val.as_object() {
                let et_val = global.get(js_string!("EventTarget"), &mut context)
                    .expect("EventTarget should be registered");
                let _ = window_obj.define_property_or_throw(
                    js_string!("EventTarget"),
                    boa_engine::property::PropertyDescriptor::builder()
                        .value(et_val)
                        .writable(true)
                        .configurable(true)
                        .enumerable(false)
                        .build(),
                    &mut context,
                );

                // Copy UIEvent subclass constructors to window so window["MouseEvent"] etc. work
                for ctor_name in &["MouseEvent", "KeyboardEvent", "WheelEvent", "FocusEvent", "Event", "CustomEvent", "MutationObserver", "MutationRecord"] {
                    let ctor_val = global.get(js_string!(*ctor_name), &mut context)
                        .expect("event constructor should be registered");
                    let _ = window_obj.define_property_or_throw(
                        js_string!(*ctor_name),
                        boa_engine::property::PropertyDescriptor::builder()
                            .value(ctor_val)
                            .writable(true)
                            .configurable(true)
                            .enumerable(false)
                            .build(),
                        &mut context,
                    );
                }

                // Copy addEventListener, removeEventListener, dispatchEvent from window to global
                for method_name in &["addEventListener", "removeEventListener", "dispatchEvent"] {
                    if let Ok(method_val) = window_obj.get(js_string!(*method_name), &mut context) {
                        if !method_val.is_undefined() {
                            let _ = global.define_property_or_throw(
                                js_string!(*method_name),
                                boa_engine::property::PropertyDescriptor::builder()
                                    .value(method_val)
                                    .writable(true)
                                    .configurable(true)
                                    .enumerable(false)
                                    .build(),
                                &mut context,
                            );
                        }
                    }
                }
            }
        }

        Self { context, tree, console_buffer }
    }

    /// Replace the Event and CustomEvent global constructors with wrappers that:
    /// 1. Create the event via `from_data` (gets the right prototype from Class registration)
    /// 2. Attach `isTrusted` as an own accessor property on the instance
    ///
    /// This is needed because Boa's `Class::data_constructor` returns the Rust struct,
    /// not the JsObject, so there's no hook to add own properties within the trait.
    fn wrap_event_constructors(context: &mut Context) {
        use bindings::event::{JsEvent, EventKind, attach_is_trusted_own_property};

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

        // Build replacement Event constructor
        let event_proto_obj = event_proto.as_object().expect("Event.prototype object").clone();
        let event_proto_for_closure = event_proto_obj.clone();
        let event_ctor = unsafe {
            NativeFunction::from_closure(move |_this, args, ctx| {
                if _this.is_undefined() {
                    return Err(JsError::from_native(
                        boa_engine::JsNativeError::typ()
                            .with_message("Failed to construct 'Event': Please use the 'new' operator, this DOM object constructor cannot be called as a function.")
                    ));
                }
                // Parse args the same way as JsEvent::data_constructor
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

                let mut bubbles = false;
                let mut cancelable = false;
                if let Some(opts_val) = args.get(1) {
                    if let Some(opts_obj) = opts_val.as_object() {
                        let b = opts_obj.get(js_string!("bubbles"), ctx)?;
                        if !b.is_undefined() { bubbles = b.to_boolean(); }
                        let c = opts_obj.get(js_string!("cancelable"), ctx)?;
                        if !c.is_undefined() { cancelable = c.to_boolean(); }
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
                    time_stamp: bindings::event::dom_high_res_time_stamp(),
                    initialized: true,
                    kind: EventKind::Standard,
                };
                let js_obj = JsEvent::from_data(event, ctx)?;
                js_obj.set_prototype(Some(event_proto_for_closure.clone()));
                attach_is_trusted_own_property(&js_obj, ctx)?;
                Ok(JsValue::from(js_obj))
            })
        };

        let event_ctor_fn = FunctionObjectBuilder::new(context.realm(), event_ctor)
            .name(js_string!("Event"))
            .length(1)
            .constructor(true)
            .build();

        // Set Event.prototype on the new constructor
        event_ctor_fn.define_property_or_throw(
            js_string!("prototype"),
            boa_engine::property::PropertyDescriptor::builder()
                .value(event_proto)
                .writable(false)
                .configurable(false)
                .enumerable(false)
                .build(),
            context,
        ).expect("failed to define Event.prototype on wrapper");

        // Replace the global Event
        context
            .register_global_property(js_string!("Event"), event_ctor_fn, Attribute::WRITABLE | Attribute::CONFIGURABLE)
            .expect("failed to register Event wrapper");

        // --- CustomEvent ---
        // Build CustomEvent.prototype inheriting from Event.prototype
        let event_proto_for_custom = event_proto_obj.clone();
        let custom_proto = ObjectInitializer::new(context).build();
        custom_proto.set_prototype(Some(event_proto_for_custom));

        // Add detail getter to CustomEvent.prototype
        let detail_getter = NativeFunction::from_fn_ptr(JsEvent::get_detail);
        let realm = context.realm().clone();
        custom_proto.define_property_or_throw(
            js_string!("detail"),
            boa_engine::property::PropertyDescriptor::builder()
                .get(detail_getter.to_js_function(&realm))
                .set(JsValue::undefined())
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        ).expect("failed to define CustomEvent.prototype.detail");

        // Add initCustomEvent method to CustomEvent.prototype
        let init_custom_event_fn = NativeFunction::from_fn_ptr(JsEvent::init_custom_event);
        custom_proto.define_property_or_throw(
            js_string!("initCustomEvent"),
            boa_engine::property::PropertyDescriptor::builder()
                .value(FunctionObjectBuilder::new(context.realm(), init_custom_event_fn)
                    .name(js_string!("initCustomEvent"))
                    .length(4)
                    .build())
                .writable(true)
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        ).expect("failed to define CustomEvent.prototype.initCustomEvent");

        let custom_proto_for_closure = custom_proto.clone();
        let custom_ctor = unsafe {
            NativeFunction::from_closure(move |_this, args, ctx| {
                if _this.is_undefined() {
                    return Err(JsError::from_native(
                        boa_engine::JsNativeError::typ()
                            .with_message("Failed to construct 'CustomEvent': Please use the 'new' operator, this DOM object constructor cannot be called as a function.")
                    ));
                }
                let event_type = args
                    .first()
                    .ok_or_else(|| {
                        JsError::from_native(
                            boa_engine::JsNativeError::typ()
                                .with_message("Failed to execute 'CustomEvent' constructor: 1 argument required, but only 0 present.")
                        )
                    })?
                    .to_string(ctx)?
                    .to_std_string_escaped();

                let mut bubbles = false;
                let mut cancelable = false;
                let mut detail = JsValue::null();
                if let Some(opts_val) = args.get(1) {
                    if let Some(opts_obj) = opts_val.as_object() {
                        let b = opts_obj.get(js_string!("bubbles"), ctx)?;
                        if !b.is_undefined() { bubbles = b.to_boolean(); }
                        let c = opts_obj.get(js_string!("cancelable"), ctx)?;
                        if !c.is_undefined() { cancelable = c.to_boolean(); }
                        let d = opts_obj.get(js_string!("detail"), ctx)?;
                        if !d.is_undefined() { detail = d; }
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
                    time_stamp: bindings::event::dom_high_res_time_stamp(),
                    initialized: true,
                    kind: EventKind::Custom { detail },
                };
                let js_obj = JsEvent::from_data(event, ctx)?;
                js_obj.set_prototype(Some(custom_proto_for_closure.clone()));
                attach_is_trusted_own_property(&js_obj, ctx)?;
                Ok(JsValue::from(js_obj))
            })
        };

        let custom_ctor_fn = FunctionObjectBuilder::new(context.realm(), custom_ctor)
            .name(js_string!("CustomEvent"))
            .length(1)
            .constructor(true)
            .build();

        custom_ctor_fn.define_property_or_throw(
            js_string!("prototype"),
            boa_engine::property::PropertyDescriptor::builder()
                .value(custom_proto)
                .writable(false)
                .configurable(false)
                .enumerable(false)
                .build(),
            context,
        ).expect("failed to define CustomEvent.prototype on wrapper");

        context
            .register_global_property(js_string!("CustomEvent"), custom_ctor_fn, Attribute::WRITABLE | Attribute::CONFIGURABLE)
            .expect("failed to register CustomEvent wrapper");

        // --- UIEvent subclasses: MouseEvent, KeyboardEvent, WheelEvent, FocusEvent ---
        // All use unified JsEvent with EventKind variants. Prototypes inherit Event.prototype.
        macro_rules! wrap_ui_event_subclass {
            ($kind:expr, $name:expr) => {{
                // Build SubclassEvent.prototype inheriting from Event.prototype
                let proto = ObjectInitializer::new(context).build();
                proto.set_prototype(Some(event_proto_obj.clone()));

                let proto_for_closure = proto.clone();
                let ctor_name: &'static str = $name;
                let ctor = unsafe {
                    NativeFunction::from_closure(move |_this, args, ctx| {
                        if _this.is_undefined() {
                            return Err(JsError::from_native(
                                boa_engine::JsNativeError::typ()
                                    .with_message(format!("Failed to construct '{}': Please use the 'new' operator, this DOM object constructor cannot be called as a function.", ctor_name))
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
                        if let Some(opts_val) = args.get(1) {
                            if let Some(opts_obj) = opts_val.as_object() {
                                let b = opts_obj.get(js_string!("bubbles"), ctx)?;
                                if !b.is_undefined() { bubbles = b.to_boolean(); }
                                let c = opts_obj.get(js_string!("cancelable"), ctx)?;
                                if !c.is_undefined() { cancelable = c.to_boolean(); }
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
                            time_stamp: bindings::event::dom_high_res_time_stamp(),
                            initialized: true,
                            kind: $kind,
                        };
                        let js_obj = JsEvent::from_data(event, ctx)?;
                        js_obj.set_prototype(Some(proto_for_closure.clone()));
                        attach_is_trusted_own_property(&js_obj, ctx)?;
                        Ok(JsValue::from(js_obj))
                    })
                };

                let ctor_fn = FunctionObjectBuilder::new(context.realm(), ctor)
                    .name(js_string!($name))
                    .length(1)
                    .constructor(true)
                    .build();

                ctor_fn.define_property_or_throw(
                    js_string!("prototype"),
                    boa_engine::property::PropertyDescriptor::builder()
                        .value(proto)
                        .writable(false)
                        .configurable(false)
                        .enumerable(false)
                        .build(),
                    context,
                ).expect(concat!("failed to define ", $name, ".prototype on wrapper"));

                context
                    .register_global_property(js_string!($name), ctor_fn, Attribute::WRITABLE | Attribute::CONFIGURABLE)
                    .expect(concat!("failed to register ", $name, " wrapper"));
            }};
        }

        // MouseEvent — custom constructor that parses mouse-specific options
        {
            let proto = ObjectInitializer::new(context).build();
            proto.set_prototype(Some(event_proto_obj.clone()));

            // Add mouse-specific property getters on prototype
            let realm = context.realm().clone();

            macro_rules! mouse_getter_i16 {
                ($field:ident, $js_name:expr) => {{
                    let getter = NativeFunction::from_fn_ptr(|this, _args, _ctx| {
                        let obj = match this.as_object() {
                            Some(o) => o,
                            None => return Ok(JsValue::from(0)),
                        };
                        let evt = match obj.downcast_ref::<JsEvent>() {
                            Some(e) => e,
                            None => return Ok(JsValue::from(0)),
                        };
                        match &evt.kind {
                            EventKind::Mouse { $field, .. } => Ok(JsValue::from(*$field as i32)),
                            _ => Ok(JsValue::from(0)),
                        }
                    });
                    proto.define_property_or_throw(
                        js_string!($js_name),
                        boa_engine::property::PropertyDescriptor::builder()
                            .get(getter.to_js_function(&realm))
                            .configurable(true)
                            .enumerable(true)
                            .build(),
                        context,
                    ).expect(concat!("failed to define MouseEvent.", $js_name));
                }};
            }

            macro_rules! mouse_getter_f64 {
                ($field:ident, $js_name:expr) => {{
                    let getter = NativeFunction::from_fn_ptr(|this, _args, _ctx| {
                        let obj = match this.as_object() {
                            Some(o) => o,
                            None => return Ok(JsValue::from(0.0)),
                        };
                        let evt = match obj.downcast_ref::<JsEvent>() {
                            Some(e) => e,
                            None => return Ok(JsValue::from(0.0)),
                        };
                        match &evt.kind {
                            EventKind::Mouse { $field, .. } => Ok(JsValue::from(*$field)),
                            _ => Ok(JsValue::from(0.0)),
                        }
                    });
                    proto.define_property_or_throw(
                        js_string!($js_name),
                        boa_engine::property::PropertyDescriptor::builder()
                            .get(getter.to_js_function(&realm))
                            .configurable(true)
                            .enumerable(true)
                            .build(),
                        context,
                    ).expect(concat!("failed to define MouseEvent.", $js_name));
                }};
            }

            macro_rules! mouse_getter_bool {
                ($field:ident, $js_name:expr) => {{
                    let getter = NativeFunction::from_fn_ptr(|this, _args, _ctx| {
                        let obj = match this.as_object() {
                            Some(o) => o,
                            None => return Ok(JsValue::from(false)),
                        };
                        let evt = match obj.downcast_ref::<JsEvent>() {
                            Some(e) => e,
                            None => return Ok(JsValue::from(false)),
                        };
                        match &evt.kind {
                            EventKind::Mouse { $field, .. } => Ok(JsValue::from(*$field)),
                            _ => Ok(JsValue::from(false)),
                        }
                    });
                    proto.define_property_or_throw(
                        js_string!($js_name),
                        boa_engine::property::PropertyDescriptor::builder()
                            .get(getter.to_js_function(&realm))
                            .configurable(true)
                            .enumerable(true)
                            .build(),
                        context,
                    ).expect(concat!("failed to define MouseEvent.", $js_name));
                }};
            }

            mouse_getter_i16!(button, "button");
            mouse_getter_i16!(buttons, "buttons");
            mouse_getter_f64!(client_x, "clientX");
            mouse_getter_f64!(client_y, "clientY");
            mouse_getter_f64!(screen_x, "screenX");
            mouse_getter_f64!(screen_y, "screenY");
            mouse_getter_bool!(alt_key, "altKey");
            mouse_getter_bool!(ctrl_key, "ctrlKey");
            mouse_getter_bool!(meta_key, "metaKey");
            mouse_getter_bool!(shift_key, "shiftKey");

            // relatedTarget (always null for now)
            let related_target_getter = NativeFunction::from_fn_ptr(|_this, _args, _ctx| {
                Ok(JsValue::null())
            });
            proto.define_property_or_throw(
                js_string!("relatedTarget"),
                boa_engine::property::PropertyDescriptor::builder()
                    .get(related_target_getter.to_js_function(&realm))
                    .configurable(true)
                    .enumerable(true)
                    .build(),
                context,
            ).expect("failed to define MouseEvent.relatedTarget");

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
                            if !b.is_undefined() { bubbles = b.to_boolean(); }
                            let c = opts_obj.get(js_string!("cancelable"), ctx)?;
                            if !c.is_undefined() { cancelable = c.to_boolean(); }

                            let v = opts_obj.get(js_string!("button"), ctx)?;
                            if !v.is_undefined() { button = v.to_number(ctx)? as i16; }
                            let v = opts_obj.get(js_string!("buttons"), ctx)?;
                            if !v.is_undefined() { buttons = v.to_number(ctx)? as u16; }
                            let v = opts_obj.get(js_string!("clientX"), ctx)?;
                            if !v.is_undefined() { client_x = v.to_number(ctx)?; }
                            let v = opts_obj.get(js_string!("clientY"), ctx)?;
                            if !v.is_undefined() { client_y = v.to_number(ctx)?; }
                            let v = opts_obj.get(js_string!("screenX"), ctx)?;
                            if !v.is_undefined() { screen_x = v.to_number(ctx)?; }
                            let v = opts_obj.get(js_string!("screenY"), ctx)?;
                            if !v.is_undefined() { screen_y = v.to_number(ctx)?; }
                            let v = opts_obj.get(js_string!("altKey"), ctx)?;
                            if !v.is_undefined() { alt_key = v.to_boolean(); }
                            let v = opts_obj.get(js_string!("ctrlKey"), ctx)?;
                            if !v.is_undefined() { ctrl_key = v.to_boolean(); }
                            let v = opts_obj.get(js_string!("metaKey"), ctx)?;
                            if !v.is_undefined() { meta_key = v.to_boolean(); }
                            let v = opts_obj.get(js_string!("shiftKey"), ctx)?;
                            if !v.is_undefined() { shift_key = v.to_boolean(); }
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
                        time_stamp: bindings::event::dom_high_res_time_stamp(),
                        initialized: true,
                        kind: EventKind::Mouse {
                            button, buttons, client_x, client_y,
                            screen_x, screen_y, alt_key, ctrl_key, meta_key, shift_key,
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

            ctor_fn.define_property_or_throw(
                js_string!("prototype"),
                boa_engine::property::PropertyDescriptor::builder()
                    .value(proto)
                    .writable(false)
                    .configurable(false)
                    .enumerable(false)
                    .build(),
                context,
            ).expect("failed to define MouseEvent.prototype on wrapper");

            context
                .register_global_property(js_string!("MouseEvent"), ctor_fn, Attribute::WRITABLE | Attribute::CONFIGURABLE)
                .expect("failed to register MouseEvent wrapper");
        }

        wrap_ui_event_subclass!(EventKind::Keyboard, "KeyboardEvent");
        wrap_ui_event_subclass!(EventKind::Wheel, "WheelEvent");
        wrap_ui_event_subclass!(EventKind::Focus, "FocusEvent");
    }

    /// Register the full DOM type hierarchy:
    ///   Node (interface object + prototype with constants)
    ///   CharacterData (prototype inherits from Node.prototype)
    ///   Text (constructor + prototype inherits from CharacterData.prototype)
    ///   Comment (constructor + prototype inherits from CharacterData.prototype)
    ///
    /// Also stores Text.prototype and Comment.prototype in the DOM_PROTOTYPES thread-local
    /// so that get_or_create_js_element can set the right prototype on created objects.
    fn register_dom_type_hierarchy(context: &mut Context) {
        // Get the Element class prototype — this is what all JsElement instances inherit from.
        // We'll make Node.prototype the parent of this prototype,
        // so Element instances get the Node constants via prototype chain.
        let element_constructor = context.global_object()
            .get(js_string!("Element"), context)
            .expect("Element should be registered");
        let element_proto = element_constructor
            .as_object()
            .expect("Element should be an object")
            .get(js_string!("prototype"), context)
            .expect("Element.prototype should exist");
        let element_proto_obj = element_proto.as_object().expect("Element.prototype should be an object").clone();

        // ---------------------------------------------------------------
        // Node.prototype — the base prototype with node type constants
        // ---------------------------------------------------------------
        let node_proto = ObjectInitializer::new(context).build();

        // Add all Node constants to Node.prototype
        let node_constants: &[(&str, i32)] = &[
            ("ELEMENT_NODE", 1), ("ATTRIBUTE_NODE", 2), ("TEXT_NODE", 3),
            ("CDATA_SECTION_NODE", 4), ("ENTITY_REFERENCE_NODE", 5), ("ENTITY_NODE", 6),
            ("PROCESSING_INSTRUCTION_NODE", 7), ("COMMENT_NODE", 8), ("DOCUMENT_NODE", 9),
            ("DOCUMENT_TYPE_NODE", 10), ("DOCUMENT_FRAGMENT_NODE", 11), ("NOTATION_NODE", 12),
        ];
        let doc_position_constants: &[(&str, i32)] = &[
            ("DOCUMENT_POSITION_DISCONNECTED", 0x01), ("DOCUMENT_POSITION_PRECEDING", 0x02),
            ("DOCUMENT_POSITION_FOLLOWING", 0x04), ("DOCUMENT_POSITION_CONTAINS", 0x08),
            ("DOCUMENT_POSITION_CONTAINED_BY", 0x10), ("DOCUMENT_POSITION_IMPLEMENTATION_SPECIFIC", 0x20),
        ];

        for (name, value) in node_constants.iter().chain(doc_position_constants.iter()) {
            node_proto.define_property_or_throw(
                js_string!(*name),
                boa_engine::property::PropertyDescriptor::builder()
                    .value(JsValue::from(*value))
                    .writable(false)
                    .configurable(false)
                    .enumerable(false)
                    .build(),
                context,
            ).expect("failed to define Node.prototype constant");
        }

        // Make Element.prototype inherit from Node.prototype
        element_proto_obj.set_prototype(Some(node_proto.clone()));

        // Copy Node-level methods from Element.prototype to Node.prototype
        // so that `Node.prototype.insertBefore` etc. work (used by WPT tests).
        // Per DOM spec, these are Node interface methods.
        let node_methods = &[
            "appendChild", "insertBefore", "removeChild", "replaceChild",
            "cloneNode", "normalize", "hasChildNodes", "contains",
            "isEqualNode", "isSameNode", "compareDocumentPosition",
            "getRootNode", "append", "prepend", "replaceChildren",
            "before", "after", "replaceWith", "remove",
            "insertAdjacentElement", "insertAdjacentText",
        ];
        for name in node_methods {
            if let Ok(val) = element_proto_obj.get(js_string!(*name), context) {
                if !val.is_undefined() {
                    node_proto.set(js_string!(*name), val, false, context)
                        .expect("failed to copy method to Node.prototype");
                }
            }
        }


        // ---------------------------------------------------------------
        // Node interface object — must be a callable function so that
        // `obj instanceof Node` works (requires [[Call]] on the RHS).
        // Node is abstract; calling `new Node()` throws "Illegal constructor".
        // ---------------------------------------------------------------
        let node_ctor = unsafe {
            NativeFunction::from_closure(|_this, _args, _ctx| {
                Err(JsError::from_opaque(JsValue::from(js_string!("Illegal constructor"))))
            })
        };
        let node_ctor_fn = FunctionObjectBuilder::new(context.realm(), node_ctor)
            .name(js_string!("Node"))
            .length(0)
            .constructor(true)
            .build();
        // Set Node.prototype
        node_ctor_fn.define_property_or_throw(
            js_string!("prototype"),
            boa_engine::property::PropertyDescriptor::builder()
                .value(node_proto.clone())
                .writable(false)
                .configurable(false)
                .enumerable(false)
                .build(),
            context,
        ).expect("failed to define Node.prototype");
        // Add constants to Node constructor itself (e.g. Node.ELEMENT_NODE)
        for (name, value) in node_constants.iter().chain(doc_position_constants.iter()) {
            node_ctor_fn.define_property_or_throw(
                js_string!(*name),
                boa_engine::property::PropertyDescriptor::builder()
                    .value(JsValue::from(*value))
                    .writable(false)
                    .configurable(false)
                    .enumerable(false)
                    .build(),
                context,
            ).expect("failed to define Node constant");
        }

        context
            .register_global_property(js_string!("Node"), node_ctor_fn, Attribute::WRITABLE | Attribute::CONFIGURABLE)
            .expect("failed to register Node global");

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
        context.register_global_property(
            js_string!("__braille_elem_proto"),
            element_proto_obj.clone(),
            Attribute::all(),
        ).expect("failed to register temp elem proto");
        context.register_global_property(
            js_string!("__braille_cd_proto"),
            char_data_proto.clone(),
            Attribute::all(),
        ).expect("failed to register temp cd proto");

        // Use JS to copy all property descriptors from Element.prototype to CharacterData.prototype
        context.eval(Source::from_bytes(
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
        )).expect("failed to copy Element.prototype properties to CharacterData.prototype");

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
        char_data_ctor_fn.define_property_or_throw(
            js_string!("prototype"),
            boa_engine::property::PropertyDescriptor::builder()
                .value(char_data_proto.clone())
                .writable(false)
                .configurable(false)
                .enumerable(false)
                .build(),
            context,
        ).expect("failed to define CharacterData.prototype");

        context
            .register_global_property(js_string!("CharacterData"), char_data_ctor_fn, Attribute::WRITABLE | Attribute::CONFIGURABLE)
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

                let tree = DOM_TREE.with(|cell| {
                    let rc = cell.borrow();
                    rc.as_ref().expect("DOM_TREE not initialized").clone()
                });

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
        text_ctor_fn.define_property_or_throw(
            js_string!("prototype"),
            boa_engine::property::PropertyDescriptor::builder()
                .value(text_proto.clone())
                .writable(false)
                .configurable(false)
                .enumerable(false)
                .build(),
            context,
        ).expect("failed to define Text.prototype");

        // Set Text.prototype.constructor = Text
        text_proto.define_property_or_throw(
            js_string!("constructor"),
            boa_engine::property::PropertyDescriptor::builder()
                .value(text_ctor_fn.clone())
                .writable(true)
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        ).expect("failed to define Text.prototype.constructor");

        context
            .register_global_property(js_string!("Text"), text_ctor_fn, Attribute::WRITABLE | Attribute::CONFIGURABLE)
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

                let tree = DOM_TREE.with(|cell| {
                    let rc = cell.borrow();
                    rc.as_ref().expect("DOM_TREE not initialized").clone()
                });

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
        comment_ctor_fn.define_property_or_throw(
            js_string!("prototype"),
            boa_engine::property::PropertyDescriptor::builder()
                .value(comment_proto.clone())
                .writable(false)
                .configurable(false)
                .enumerable(false)
                .build(),
            context,
        ).expect("failed to define Comment.prototype");

        comment_proto.define_property_or_throw(
            js_string!("constructor"),
            boa_engine::property::PropertyDescriptor::builder()
                .value(comment_ctor_fn.clone())
                .writable(true)
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        ).expect("failed to define Comment.prototype.constructor");

        context
            .register_global_property(js_string!("Comment"), comment_ctor_fn, Attribute::WRITABLE | Attribute::CONFIGURABLE)
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
        pi_ctor_fn.define_property_or_throw(
            js_string!("prototype"),
            boa_engine::property::PropertyDescriptor::builder()
                .value(pi_proto.clone())
                .writable(false)
                .configurable(false)
                .enumerable(false)
                .build(),
            context,
        ).expect("failed to define ProcessingInstruction.prototype");

        pi_proto.define_property_or_throw(
            js_string!("constructor"),
            boa_engine::property::PropertyDescriptor::builder()
                .value(pi_ctor_fn.clone())
                .writable(true)
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        ).expect("failed to define ProcessingInstruction.prototype.constructor");

        context
            .register_global_property(js_string!("ProcessingInstruction"), pi_ctor_fn, Attribute::WRITABLE | Attribute::CONFIGURABLE)
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
        attr_ctor_fn.define_property_or_throw(
            js_string!("prototype"),
            boa_engine::property::PropertyDescriptor::builder()
                .value(attr_proto.clone())
                .writable(false)
                .configurable(false)
                .enumerable(false)
                .build(),
            context,
        ).expect("failed to define Attr.prototype");

        attr_proto.define_property_or_throw(
            js_string!("constructor"),
            boa_engine::property::PropertyDescriptor::builder()
                .value(attr_ctor_fn.clone())
                .writable(true)
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        ).expect("failed to define Attr.prototype.constructor");

        context
            .register_global_property(js_string!("Attr"), attr_ctor_fn, Attribute::WRITABLE | Attribute::CONFIGURABLE)
            .expect("failed to register Attr global");

        // ---------------------------------------------------------------
        // Store prototypes in thread-local for get_or_create_js_element
        // ---------------------------------------------------------------
        DOM_PROTOTYPES.with(|cell| {
            *cell.borrow_mut() = Some(DomPrototypes {
                text_proto,
                comment_proto,
                pi_proto: Some(pi_proto),
                attr_proto: Some(attr_proto),
                html_tag_protos: HashMap::new(),
                html_element_proto: None,
                html_unknown_proto: None,
                document_fragment_proto: None,
                document_type_proto: None,
                document_proto: None,
                xml_document_proto: None,
            });
        });

        // ---------------------------------------------------------------
        // Register HTML element type constructors as globals for instanceof
        // These are all abstract constructors that throw "Illegal constructor"
        // but have proper prototypes inheriting from Element.prototype
        // so that `el instanceof HTMLDivElement` etc. works.
        // ---------------------------------------------------------------
        Self::register_html_element_types(context, &element_proto_obj);

        // ---------------------------------------------------------------
        // Register on* event handler accessors on Element.prototype
        // ---------------------------------------------------------------
        super::bindings::on_event::register_on_event_accessors(
            &element_proto_obj,
            &["click", "change", "input", "submit", "reset", "toggle", "load", "error",
              "mousedown", "mouseup", "mouseover", "mouseout", "mousemove",
              "keydown", "keyup", "keypress", "focus", "blur"],
            context,
        );

        // ---------------------------------------------------------------
        // Register DocumentFragment and DocumentType constructors
        // ---------------------------------------------------------------
        Self::register_document_fragment_type(context, &element_proto_obj);
        Self::register_document_type_type(context, &element_proto_obj);
        Self::register_document_constructor(context, &element_proto_obj);
        Self::register_xml_document_global(context);
        Self::populate_dom_prototypes(context);

        // ---------------------------------------------------------------
        // Copy Node/CharacterData/Text/Comment globals onto window object
        // so that `window.Text`, `window.Node`, etc. work (used by WPT tests)
        // ---------------------------------------------------------------
        let global = context.global_object();
        let window_val = global.get(js_string!("window"), context)
            .expect("window global should exist");
        if let Some(window_obj) = window_val.as_object() {
            // Core DOM types
            let core_types = &[
                "Node", "CharacterData", "Text", "Comment", "ProcessingInstruction",
                "Attr", "DocumentFragment", "DocumentType", "Document", "Element",
            ];
            for name in core_types {
                let val = global.get(js_string!(*name), context)
                    .expect("global should have this property");
                window_obj.define_property_or_throw(
                    js_string!(*name),
                    boa_engine::property::PropertyDescriptor::builder()
                        .value(val)
                        .writable(true)
                        .configurable(true)
                        .enumerable(false)
                        .build(),
                    context,
                ).expect("failed to set window property");
            }

            // HTML element types
            let html_types = Self::html_element_type_names();
            for name in &html_types {
                if let Ok(val) = global.get(js_string!(*name), context) {
                    if !val.is_undefined() {
                        let _ = window_obj.define_property_or_throw(
                            js_string!(*name),
                            boa_engine::property::PropertyDescriptor::builder()
                                .value(val)
                                .writable(true)
                                .configurable(true)
                                .enumerable(false)
                                .build(),
                            context,
                        );
                    }
                }
            }
        }
    }

    /// Returns the list of HTML element type constructor names.
    fn html_element_type_names() -> Vec<&'static str> {
        vec![
            "HTMLElement", "HTMLAnchorElement", "HTMLAreaElement",
            "HTMLAudioElement", "HTMLBaseElement", "HTMLBodyElement",
            "HTMLBRElement", "HTMLButtonElement", "HTMLCanvasElement",
            "HTMLTableCaptionElement", "HTMLTableColElement",
            "HTMLDataElement", "HTMLDataListElement", "HTMLDialogElement",
            "HTMLModElement", "HTMLDirectoryElement", "HTMLDivElement",
            "HTMLDListElement", "HTMLEmbedElement", "HTMLFieldSetElement",
            "HTMLFontElement", "HTMLFormElement", "HTMLFrameElement",
            "HTMLFrameSetElement", "HTMLHeadingElement", "HTMLHeadElement",
            "HTMLHRElement", "HTMLHtmlElement", "HTMLIFrameElement",
            "HTMLImageElement", "HTMLInputElement", "HTMLLabelElement",
            "HTMLLegendElement", "HTMLLIElement", "HTMLLinkElement",
            "HTMLMapElement", "HTMLMetaElement", "HTMLMeterElement",
            "HTMLObjectElement", "HTMLOListElement", "HTMLOptGroupElement",
            "HTMLOptionElement", "HTMLOutputElement", "HTMLParagraphElement",
            "HTMLParamElement", "HTMLPreElement", "HTMLProgressElement",
            "HTMLQuoteElement", "HTMLScriptElement", "HTMLSelectElement",
            "HTMLSourceElement", "HTMLSpanElement", "HTMLStyleElement",
            "HTMLTableElement", "HTMLTableSectionElement",
            "HTMLTableCellElement", "HTMLTemplateElement",
            "HTMLTextAreaElement", "HTMLTimeElement", "HTMLTitleElement",
            "HTMLTableRowElement", "HTMLTrackElement", "HTMLUListElement",
            "HTMLVideoElement", "HTMLUnknownElement",
        ]
    }

    /// Register all HTML element type constructors as globals.
    /// Each one has a prototype that inherits from Element.prototype,
    /// and HTMLElement.prototype is the base for most of them.
    fn register_html_element_types(context: &mut Context, element_proto: &JsObject) {
        // Create HTMLElement.prototype inheriting from Element.prototype
        let html_element_proto = ObjectInitializer::new(context).build();
        html_element_proto.set_prototype(Some(element_proto.clone()));

        let html_element_ctor = Self::make_illegal_constructor(context, "HTMLElement");
        html_element_ctor.define_property_or_throw(
            js_string!("prototype"),
            boa_engine::property::PropertyDescriptor::builder()
                .value(html_element_proto.clone())
                .writable(false)
                .configurable(false)
                .enumerable(false)
                .build(),
            context,
        ).expect("failed to define HTMLElement.prototype");

        html_element_proto.define_property_or_throw(
            js_string!("constructor"),
            boa_engine::property::PropertyDescriptor::builder()
                .value(html_element_ctor.clone())
                .writable(true)
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        ).expect("failed to set HTMLElement.prototype.constructor");

        context
            .register_global_property(js_string!("HTMLElement"), html_element_ctor, Attribute::WRITABLE | Attribute::CONFIGURABLE)
            .expect("failed to register HTMLElement global");

        // Register all specific HTML element types, each inheriting from HTMLElement.prototype
        let specific_types = Self::html_element_type_names();
        for name in &specific_types {
            if *name == "HTMLElement" {
                continue; // Already registered
            }

            let proto = ObjectInitializer::new(context).build();
            proto.set_prototype(Some(html_element_proto.clone()));

            let ctor = Self::make_illegal_constructor(context, name);
            ctor.define_property_or_throw(
                js_string!("prototype"),
                boa_engine::property::PropertyDescriptor::builder()
                    .value(proto.clone())
                    .writable(false)
                    .configurable(false)
                    .enumerable(false)
                    .build(),
                context,
            ).expect("failed to define prototype");

            proto.define_property_or_throw(
                js_string!("constructor"),
                boa_engine::property::PropertyDescriptor::builder()
                    .value(ctor.clone())
                    .writable(true)
                    .configurable(true)
                    .enumerable(false)
                    .build(),
                context,
            ).expect("failed to set prototype.constructor");

            context
                .register_global_property(js_string!(*name), ctor, Attribute::WRITABLE | Attribute::CONFIGURABLE)
                .expect("failed to register HTML element type global");
        }
    }

    /// Register DocumentFragment constructor.
    /// `new DocumentFragment()` creates a new empty DocumentFragment node per spec.
    fn register_document_fragment_type(context: &mut Context, element_proto: &JsObject) {
        let proto = ObjectInitializer::new(context).build();
        proto.set_prototype(Some(element_proto.clone()));

        let ctor_native = unsafe {
            NativeFunction::from_closure(move |_this, _args, ctx| {
                // Get the global document's tree, or create a standalone tree
                let tree = DOM_TREE.with(|cell| cell.borrow().clone());
                let tree = tree.unwrap_or_else(|| {
                    std::rc::Rc::new(std::cell::RefCell::new(crate::dom::DomTree::new()))
                });
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
            boa_engine::property::PropertyDescriptor::builder()
                .value(proto.clone())
                .writable(false)
                .configurable(false)
                .enumerable(false)
                .build(),
            context,
        ).expect("failed to define DocumentFragment.prototype");

        proto.define_property_or_throw(
            js_string!("constructor"),
            boa_engine::property::PropertyDescriptor::builder()
                .value(ctor.clone())
                .writable(true)
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        ).expect("failed to set DocumentFragment.prototype.constructor");

        // Add getElementById to DocumentFragment.prototype (NonElementParentNode mixin)
        let get_by_id_fn = NativeFunction::from_fn_ptr(
            bindings::query::fragment_get_element_by_id,
        );
        proto.set(
            js_string!("getElementById"),
            get_by_id_fn.to_js_function(context.realm()),
            false,
            context,
        ).expect("failed to set DocumentFragment.prototype.getElementById");

        context
            .register_global_property(js_string!("DocumentFragment"), ctor, Attribute::WRITABLE | Attribute::CONFIGURABLE)
            .expect("failed to register DocumentFragment global");
    }

    /// Register DocumentType constructor
    fn register_document_type_type(context: &mut Context, element_proto: &JsObject) {
        let proto = ObjectInitializer::new(context).build();
        proto.set_prototype(Some(element_proto.clone()));

        let ctor = Self::make_illegal_constructor(context, "DocumentType");
        ctor.define_property_or_throw(
            js_string!("prototype"),
            boa_engine::property::PropertyDescriptor::builder()
                .value(proto.clone())
                .writable(false)
                .configurable(false)
                .enumerable(false)
                .build(),
            context,
        ).expect("failed to define DocumentType.prototype");

        proto.define_property_or_throw(
            js_string!("constructor"),
            boa_engine::property::PropertyDescriptor::builder()
                .value(ctor.clone())
                .writable(true)
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        ).expect("failed to set DocumentType.prototype.constructor");

        context
            .register_global_property(js_string!("DocumentType"), ctor, Attribute::WRITABLE | Attribute::CONFIGURABLE)
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
            proto.define_property_or_throw(
                js_string!(*name),
                boa_engine::property::PropertyDescriptor::builder()
                    .value(JsValue::from(js_string!(*value)))
                    .writable(true)
                    .configurable(true)
                    .enumerable(false)
                    .build(),
                context,
            ).expect("failed to set Document.prototype default property");
        }

        let ctor = unsafe {
            NativeFunction::from_closure(move |_this, _args, ctx| {
                bindings::document::create_blank_xml_document(ctx)
            })
        };

        let ctor_fn = FunctionObjectBuilder::new(context.realm(), ctor)
            .name(js_string!("Document"))
            .length(0)
            .constructor(true)
            .build();

        ctor_fn.define_property_or_throw(
            js_string!("prototype"),
            boa_engine::property::PropertyDescriptor::builder()
                .value(proto.clone())
                .writable(false)
                .configurable(false)
                .enumerable(false)
                .build(),
            context,
        ).expect("failed to define Document.prototype");

        proto.define_property_or_throw(
            js_string!("constructor"),
            boa_engine::property::PropertyDescriptor::builder()
                .value(ctor_fn.clone())
                .writable(true)
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        ).expect("failed to set Document.prototype.constructor");

        context
            .register_global_property(js_string!("Document"), ctor_fn, Attribute::WRITABLE | Attribute::CONFIGURABLE)
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
            Err(JsNativeError::typ()
                .with_message("Illegal constructor")
                .into())
        });

        let ctor_fn = FunctionObjectBuilder::new(context.realm(), ctor)
            .name(js_string!("XMLDocument"))
            .length(0)
            .constructor(true)
            .build();

        ctor_fn.define_property_or_throw(
            js_string!("prototype"),
            boa_engine::property::PropertyDescriptor::builder()
                .value(xml_proto.clone())
                .writable(false)
                .configurable(false)
                .enumerable(false)
                .build(),
            context,
        ).expect("failed to define XMLDocument.prototype");

        xml_proto.define_property_or_throw(
            js_string!("constructor"),
            boa_engine::property::PropertyDescriptor::builder()
                .value(ctor_fn.clone())
                .writable(true)
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        ).expect("failed to set XMLDocument.prototype.constructor");

        context
            .register_global_property(js_string!("XMLDocument"), ctor_fn, Attribute::WRITABLE | Attribute::CONFIGURABLE)
            .expect("failed to register XMLDocument global");
    }

    /// Populate the DomPrototypes thread-local with prototypes from the globally
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
        let document_type_proto = get_proto("DocumentType");
        let document_proto = get_proto("Document");
        let xml_document_proto = get_proto("XMLDocument");

        DOM_PROTOTYPES.with(|cell| {
            let mut protos = cell.borrow_mut();
            if let Some(ref mut p) = *protos {
                p.html_tag_protos = html_tag_protos;
                p.html_element_proto = html_element_proto;
                p.html_unknown_proto = html_unknown_proto;
                p.document_fragment_proto = document_fragment_proto;
                p.document_type_proto = document_type_proto;
                p.document_proto = document_proto;
                p.xml_document_proto = xml_document_proto;
            }
        });
    }

    /// Register the `performance` global object with a `now()` method.
    /// `performance.now()` returns a DOMHighResTimeStamp: milliseconds elapsed since runtime creation.
    fn register_performance_global(context: &mut Context) {
        use bindings::event::dom_high_res_time_stamp;

        let now_fn = NativeFunction::from_fn_ptr(|_this, _args, _ctx| {
            Ok(JsValue::from(dom_high_res_time_stamp()))
        });

        let performance = ObjectInitializer::new(context)
            .function(now_fn, js_string!("now"), 0)
            .build();

        context
            .register_global_property(js_string!("performance"), performance, Attribute::WRITABLE | Attribute::CONFIGURABLE)
            .expect("failed to register performance global");
    }

    /// Register a minimal `location` global object stub.
    /// The Node-properties WPT test uses `String(location)` to get the document URL.
    fn register_location_global(context: &mut Context) {
        // Create a location object with toString() returning "about:blank"
        let location = ObjectInitializer::new(context).build();
        // Make String(location) return "about:blank"
        let to_string_fn = unsafe {
            NativeFunction::from_closure(|_this, _args, _ctx| {
                Ok(JsValue::from(js_string!("about:blank")))
            })
        };
        let realm = context.realm().clone();
        location.define_property_or_throw(
            js_string!("toString"),
            boa_engine::property::PropertyDescriptor::builder()
                .value(to_string_fn.to_js_function(&realm))
                .writable(true)
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        ).expect("failed to define location.toString");
        location.define_property_or_throw(
            js_string!("href"),
            boa_engine::property::PropertyDescriptor::builder()
                .value(JsValue::from(js_string!("about:blank")))
                .writable(true)
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        ).expect("failed to define location.href");

        context
            .register_global_property(js_string!("location"), location, Attribute::WRITABLE | Attribute::CONFIGURABLE)
            .expect("failed to register location global");
    }

    /// Add composedPath() to Event.prototype and CustomEvent.prototype.
    /// This is needed for EventTarget-constructible WPT test which checks e.composedPath().
    fn register_composed_path(context: &mut Context) {
        let global = context.global_object();
        let composed_path_fn = NativeFunction::from_fn_ptr(bindings::event_target::composed_path)
            .to_js_function(context.realm());

        // Add to Event.prototype
        let event_ctor = global.get(js_string!("Event"), context)
            .expect("Event constructor should exist");
        if let Some(event_obj) = event_ctor.as_object() {
            let proto = event_obj.get(js_string!("prototype"), context)
                .expect("Event.prototype should exist");
            if let Some(proto_obj) = proto.as_object() {
                proto_obj.set(
                    js_string!("composedPath"),
                    composed_path_fn.clone(),
                    false,
                    context,
                ).expect("failed to set Event.prototype.composedPath");
            }
        }

        // Add to CustomEvent.prototype
        let custom_ctor = global.get(js_string!("CustomEvent"), context)
            .expect("CustomEvent constructor should exist");
        if let Some(custom_obj) = custom_ctor.as_object() {
            let proto = custom_obj.get(js_string!("prototype"), context)
                .expect("CustomEvent.prototype should exist");
            if let Some(proto_obj) = proto.as_object() {
                proto_obj.set(
                    js_string!("composedPath"),
                    composed_path_fn,
                    false,
                    context,
                ).expect("failed to set CustomEvent.prototype.composedPath");
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

    /// Evaluates a JS source string and returns the result.
    pub fn eval(&mut self, code: &str) -> JsResult<JsValue> {
        self.context.eval(Source::from_bytes(code))
    }

    /// Deliver pending MutationObserver records to their callbacks.
    pub fn notify_mutation_observers(&mut self) {
        bindings::mutation_observer::notify_mutation_observers(&mut self.context);
    }

    /// Returns a reference to the shared DomTree.
    pub fn tree(&self) -> &Rc<RefCell<DomTree>> {
        &self.tree
    }

    /// Returns a clone of the console output buffer.
    pub fn console_output(&self) -> Vec<String> {
        self.console_buffer.borrow().clone()
    }
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

        let text = result
            .to_string(&mut rt.context)
            .unwrap()
            .to_std_string_escaped();
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
        let title = result
            .to_string(&mut rt.context)
            .unwrap()
            .to_std_string_escaped();
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
        let title = result
            .to_string(&mut rt.context)
            .unwrap()
            .to_std_string_escaped();
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
        let title = result
            .to_string(&mut rt.context)
            .unwrap()
            .to_std_string_escaped();
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
        let title = result
            .to_string(&mut rt.context)
            .unwrap()
            .to_std_string_escaped();
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
        let result1 = rt.eval(
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
        let result2 = rt.eval(
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

        let result1 = rt.eval(
            r#"
            var el = document.getElementById("app");
            el.classList.contains("foo");
        "#,
        )
        .unwrap();
        assert_eq!(result1.as_boolean(), Some(true));

        let result2 = rt.eval(
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

        let result0 = rt.eval(
            r#"
            var el = document.getElementById("app");
            el.classList.item(0);
        "#,
        )
        .unwrap();
        let text0 = result0
            .to_string(&mut rt.context)
            .unwrap()
            .to_std_string_escaped();
        assert_eq!(text0, "foo");

        let result1 = rt.eval(
            r#"
            var el = document.getElementById("app");
            el.classList.item(1);
        "#,
        )
        .unwrap();
        let text1 = result1
            .to_string(&mut rt.context)
            .unwrap()
            .to_std_string_escaped();
        assert_eq!(text1, "bar");

        let result_out_of_bounds = rt.eval(
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

        let result_empty = rt.eval(
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

        let result_three = rt.eval(
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
        let result = rt.eval(r#"
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
        "#).unwrap();
        let s = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        eprintln!("{}", s);
        assert!(s.contains("doctype2.parentNode === doc: true"), "doctype2.parentNode should be doc: {}", s);
    }

    #[test]
    fn node_prototype_insert_before_is_callable() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt.eval(r#"
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
        "#).unwrap();
        let s = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        eprintln!("{}", s);
        assert!(s.contains("typeof Node.prototype.insertBefore = function"), "insertBefore should be a function on Node.prototype: {}", s);
    }

}
