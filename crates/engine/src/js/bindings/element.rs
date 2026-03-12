use std::cell::RefCell;
use std::rc::Rc;

use boa_engine::{
    class::{Class, ClassBuilder},
    js_string,
    native_function::NativeFunction,
    property::{Attribute, PropertyDescriptor},
    Context, JsData, JsError, JsObject, JsResult, JsValue,
};
use boa_gc::{Finalize, Trace};

use crate::dom::{DomTree, NodeId};

use super::class_list::JsClassList;
use super::event::JsEvent;
use super::event_target::{ListenerEntry, EVENT_LISTENERS};

// ---------------------------------------------------------------------------
// JsElement — the Class-based wrapper around a DomTree node
// ---------------------------------------------------------------------------

#[derive(Debug, Trace, Finalize, JsData)]
pub(crate) struct JsElement {
    #[unsafe_ignore_trace]
    pub(crate) node_id: NodeId,
    #[unsafe_ignore_trace]
    pub(crate) tree: Rc<RefCell<DomTree>>,
}

impl JsElement {
    pub fn new(node_id: NodeId, tree: Rc<RefCell<DomTree>>) -> Self {
        Self { node_id, tree }
    }

    /// Native implementation of element.appendChild(child)
    fn append_child(this: &JsValue, args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
        let this_obj = this
            .as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("appendChild: `this` is not an object").into()))?;
        let parent = this_obj
            .downcast_ref::<JsElement>()
            .ok_or_else(|| JsError::from_opaque(js_string!("appendChild: `this` is not an Element").into()))?;
        let parent_id = parent.node_id;
        let tree = parent.tree.clone();

        let child_arg = args
            .first()
            .ok_or_else(|| JsError::from_opaque(js_string!("appendChild: missing argument").into()))?;
        let child_obj = child_arg
            .as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("appendChild: argument is not an object").into()))?;
        let child = child_obj
            .downcast_ref::<JsElement>()
            .ok_or_else(|| JsError::from_opaque(js_string!("appendChild: argument is not an Element").into()))?;
        let child_id = child.node_id;

        tree.borrow_mut().append_child(parent_id, child_id);

        // appendChild returns the appended child
        Ok(child_arg.clone())
    }

    /// Native getter for element.textContent
    fn get_text_content(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
        let obj = this
            .as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("textContent getter: `this` is not an object").into()))?;
        let el = obj
            .downcast_ref::<JsElement>()
            .ok_or_else(|| JsError::from_opaque(js_string!("textContent getter: `this` is not an Element").into()))?;
        let text = el.tree.borrow().get_text_content(el.node_id);
        Ok(JsValue::from(js_string!(text)))
    }

    /// Native setter for element.textContent
    fn set_text_content(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
        let obj = this
            .as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("textContent setter: `this` is not an object").into()))?;
        let el = obj
            .downcast_ref::<JsElement>()
            .ok_or_else(|| JsError::from_opaque(js_string!("textContent setter: `this` is not an Element").into()))?;
        let text = args
            .first()
            .map(|v| v.to_string(ctx))
            .transpose()?
            .map(|s| s.to_std_string_escaped())
            .unwrap_or_default();
        el.tree.borrow_mut().set_text_content(el.node_id, &text);
        Ok(JsValue::undefined())
    }

    /// Native getter for element.classList
    fn get_class_list(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
        let obj = this
            .as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("classList getter: `this` is not an object").into()))?;
        let el = obj
            .downcast_ref::<JsElement>()
            .ok_or_else(|| JsError::from_opaque(js_string!("classList getter: `this` is not an Element").into()))?;

        let class_list = JsClassList::new(el.node_id, el.tree.clone());
        let js_obj = JsClassList::from_data(class_list, ctx)?;
        Ok(js_obj.into())
    }

    /// Parse the third argument to addEventListener/removeEventListener.
    /// Returns (capture, once). `once` only matters for addEventListener.
    fn parse_listener_options(args: &[JsValue], ctx: &mut Context) -> JsResult<(bool, bool)> {
        let mut capture = false;
        let mut once = false;

        if let Some(opt_val) = args.get(2) {
            if let Some(b) = opt_val.as_boolean() {
                // addEventListener(type, cb, useCapture)
                capture = b;
            } else if let Some(opt_obj) = opt_val.as_object() {
                let c = opt_obj.get(js_string!("capture"), ctx)?;
                if !c.is_undefined() {
                    capture = c.to_boolean();
                }
                let o = opt_obj.get(js_string!("once"), ctx)?;
                if !o.is_undefined() {
                    once = o.to_boolean();
                }
            }
        }

        Ok((capture, once))
    }

    /// Native implementation of element.addEventListener(type, callback, options?)
    fn add_event_listener(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
        let this_obj = this
            .as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("addEventListener: `this` is not an object").into()))?;
        let el = this_obj
            .downcast_ref::<JsElement>()
            .ok_or_else(|| JsError::from_opaque(js_string!("addEventListener: `this` is not an Element").into()))?;
        let node_id = el.node_id;

        // First arg: event type string
        let event_type = args
            .first()
            .ok_or_else(|| JsError::from_opaque(js_string!("addEventListener: missing type argument").into()))?
            .to_string(ctx)?
            .to_std_string_escaped();

        // Second arg: callback (must be callable)
        let callback_val = args
            .get(1)
            .ok_or_else(|| JsError::from_opaque(js_string!("addEventListener: missing callback argument").into()))?;

        // If callback is null or undefined, silently return (per spec)
        if callback_val.is_null() || callback_val.is_undefined() {
            return Ok(JsValue::undefined());
        }

        let callback = callback_val
            .as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("addEventListener: callback is not an object").into()))?
            .clone();

        // Third arg: options
        let (capture, once) = Self::parse_listener_options(args, ctx)?;

        EVENT_LISTENERS.with(|el| {
            let rc = el.borrow();
            let listeners_rc = rc.as_ref().expect("EVENT_LISTENERS not initialized");
            let mut map = listeners_rc.borrow_mut();
            let entries = map.entry(node_id).or_insert_with(Vec::new);

            // Check for duplicates: same event_type + same callback object (by pointer) + same capture
            let duplicate = entries.iter().any(|entry| {
                entry.event_type == event_type
                    && entry.capture == capture
                    && entry.callback == callback
            });

            if !duplicate {
                entries.push(ListenerEntry {
                    event_type,
                    callback,
                    capture,
                    once,
                });
            }
        });

        Ok(JsValue::undefined())
    }

    /// Native implementation of element.removeEventListener(type, callback, options?)
    fn remove_event_listener(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
        let this_obj = this
            .as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("removeEventListener: `this` is not an object").into()))?;
        let el = this_obj
            .downcast_ref::<JsElement>()
            .ok_or_else(|| JsError::from_opaque(js_string!("removeEventListener: `this` is not an Element").into()))?;
        let node_id = el.node_id;

        // First arg: event type string
        let event_type = args
            .first()
            .ok_or_else(|| JsError::from_opaque(js_string!("removeEventListener: missing type argument").into()))?
            .to_string(ctx)?
            .to_std_string_escaped();

        // Second arg: callback
        let callback_val = args
            .get(1)
            .ok_or_else(|| JsError::from_opaque(js_string!("removeEventListener: missing callback argument").into()))?;

        // If callback is null or undefined, silently return
        if callback_val.is_null() || callback_val.is_undefined() {
            return Ok(JsValue::undefined());
        }

        let callback = callback_val
            .as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("removeEventListener: callback is not an object").into()))?
            .clone();

        // Third arg: options (only capture matters for removal)
        let (capture, _once) = Self::parse_listener_options(args, ctx)?;

        EVENT_LISTENERS.with(|el| {
            let rc = el.borrow();
            let listeners_rc = rc.as_ref().expect("EVENT_LISTENERS not initialized");
            let mut map = listeners_rc.borrow_mut();
            if let Some(entries) = map.get_mut(&node_id) {
                entries.retain(|entry| {
                    !(entry.event_type == event_type
                        && entry.capture == capture
                        && entry.callback == callback)
                });
                // Clean up empty vec
                if entries.is_empty() {
                    map.remove(&node_id);
                }
            }
        });

        Ok(JsValue::undefined())
    }

    /// Native implementation of element.dispatchEvent(event)
    ///
    /// Implements the W3C event dispatch algorithm:
    /// 1. Build propagation path from target up to root
    /// 2. Capture phase (root -> parent of target)
    /// 3. At-target phase (target itself)
    /// 4. Bubble phase (parent of target -> root), only if event.bubbles
    fn dispatch_event(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
        let this_obj = this
            .as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("dispatchEvent: `this` is not an object").into()))?;
        let el = this_obj
            .downcast_ref::<JsElement>()
            .ok_or_else(|| JsError::from_opaque(js_string!("dispatchEvent: `this` is not an Element").into()))?;
        let target_node_id = el.node_id;
        let tree = el.tree.clone();

        let event_val = args
            .first()
            .ok_or_else(|| JsError::from_opaque(js_string!("dispatchEvent: missing event argument").into()))?
            .clone();
        let event_obj = event_val
            .as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("dispatchEvent: argument is not an object").into()))?
            .clone();

        // Read event_type and bubbles from the event's native data
        let (event_type, bubbles) = {
            let evt = event_obj
                .downcast_ref::<JsEvent>()
                .ok_or_else(|| JsError::from_opaque(js_string!("dispatchEvent: argument is not an Event").into()))?;
            (evt.event_type.clone(), evt.bubbles)
        };

        // 1. Build propagation path: [root, ..., grandparent, parent, target]
        let propagation_path = {
            let tree_ref = tree.borrow();
            let mut path = vec![target_node_id];
            let mut current = target_node_id;
            while let Some(parent_id) = tree_ref.get_node(current).parent {
                path.push(parent_id);
                current = parent_id;
            }
            path.reverse();
            path
        };

        // 2. Set event.target = this element's NodeId
        {
            let mut evt = event_obj
                .downcast_mut::<JsEvent>()
                .ok_or_else(|| JsError::from_opaque(js_string!("dispatchEvent: cannot mutate event").into()))?;
            evt.target = Some(target_node_id);
        }

        // Set the JS-level target property to the actual JS element object.
        // Use define_property_or_throw to create an own data property that
        // shadows the prototype accessor getter.
        Self::set_event_prop(&event_obj, "target", this.clone(), ctx)?;

        // Helper: create a JsElement JS object for a given NodeId
        let make_js_element = |node_id: NodeId, ctx: &mut Context| -> JsResult<JsObject> {
            let element = JsElement::new(node_id, tree.clone());
            JsElement::from_data(element, ctx)
        };

        // 3. Capture phase (phase = 1): Walk from root down to (but NOT including) the target
        let target_index = propagation_path.len() - 1;
        for i in 0..target_index {
            let node_id = propagation_path[i];

            // Set event.current_target and event.phase
            {
                let mut evt = event_obj.downcast_mut::<JsEvent>().unwrap();
                evt.current_target = Some(node_id);
                evt.phase = 1; // CAPTURING_PHASE
            }
            let current_target_js = make_js_element(node_id, ctx)?;
            Self::set_event_prop(&event_obj, "currentTarget", JsValue::from(current_target_js), ctx)?;

            let should_stop = Self::invoke_listeners_for_node(
                node_id, &event_type, &event_obj, &event_val, true, false, ctx,
            )?;
            if should_stop {
                return Self::finish_dispatch(&event_obj, ctx);
            }
        }

        // 4. At-target phase (phase = 2): Process the target element itself
        {
            let mut evt = event_obj.downcast_mut::<JsEvent>().unwrap();
            evt.current_target = Some(target_node_id);
            evt.phase = 2; // AT_TARGET
        }
        Self::set_event_prop(&event_obj, "currentTarget", this.clone(), ctx)?;

        let should_stop = Self::invoke_listeners_for_node(
            target_node_id, &event_type, &event_obj, &event_val, false, true, ctx,
        )?;
        if should_stop {
            return Self::finish_dispatch(&event_obj, ctx);
        }

        // 5. Bubble phase (phase = 3): Only if event.bubbles. Walk from parent up to root.
        if bubbles {
            for i in (0..target_index).rev() {
                let node_id = propagation_path[i];

                {
                    let mut evt = event_obj.downcast_mut::<JsEvent>().unwrap();
                    evt.current_target = Some(node_id);
                    evt.phase = 3; // BUBBLING_PHASE
                }
                let current_target_js = make_js_element(node_id, ctx)?;
                Self::set_event_prop(&event_obj, "currentTarget", JsValue::from(current_target_js), ctx)?;

                let should_stop = Self::invoke_listeners_for_node(
                    node_id, &event_type, &event_obj, &event_val, false, false, ctx,
                )?;
                if should_stop {
                    return Self::finish_dispatch(&event_obj, ctx);
                }
            }
        }

        Self::finish_dispatch(&event_obj, ctx)
    }

    /// Invoke matching listeners for a specific node during event dispatch.
    ///
    /// - `capture_only`: if true, only invoke listeners with capture=true (capture phase)
    /// - `at_target`: if true, invoke ALL matching listeners regardless of capture flag
    ///
    /// For the bubble phase, call with capture_only=false, at_target=false,
    /// which invokes only listeners with capture=false.
    ///
    /// Returns true if propagation was stopped and dispatch should halt.
    fn invoke_listeners_for_node(
        node_id: NodeId,
        event_type: &str,
        event_obj: &JsObject,
        event_val: &JsValue,
        capture_only: bool,
        at_target: bool,
        ctx: &mut Context,
    ) -> JsResult<bool> {
        // Collect matching listeners (snapshot to avoid borrow issues during callback invocation)
        let matching: Vec<(JsObject, bool)> = EVENT_LISTENERS.with(|el| {
            let rc = el.borrow();
            let listeners_rc = rc.as_ref().expect("EVENT_LISTENERS not initialized");
            let map = listeners_rc.borrow();
            match map.get(&node_id) {
                Some(entries) => entries
                    .iter()
                    .filter(|entry| {
                        if entry.event_type != event_type {
                            return false;
                        }
                        if at_target {
                            // At target: fire all matching listeners regardless of capture flag
                            true
                        } else if capture_only {
                            entry.capture
                        } else {
                            !entry.capture
                        }
                    })
                    .map(|entry| (entry.callback.clone(), entry.once))
                    .collect(),
                None => Vec::new(),
            }
        });

        for (callback, once) in &matching {
            // Remove `once` listeners before invocation
            if *once {
                EVENT_LISTENERS.with(|el| {
                    let rc = el.borrow();
                    let listeners_rc = rc.as_ref().expect("EVENT_LISTENERS not initialized");
                    let mut map = listeners_rc.borrow_mut();
                    if let Some(entries) = map.get_mut(&node_id) {
                        entries.retain(|entry| {
                            !(entry.event_type == event_type && entry.callback == *callback && entry.once)
                        });
                        if entries.is_empty() {
                            map.remove(&node_id);
                        }
                    }
                });
            }

            // Call the listener callback
            callback.call(&JsValue::undefined(), &[event_val.clone()], ctx)?;

            // Check if immediate propagation was stopped
            {
                let evt = event_obj.downcast_ref::<JsEvent>().unwrap();
                if evt.immediate_propagation_stopped {
                    return Ok(true);
                }
            }

            // Check if propagation was stopped (continue processing listeners on this node, but stop after)
            {
                let evt = event_obj.downcast_ref::<JsEvent>().unwrap();
                if evt.propagation_stopped {
                    // Don't return yet -- we still process remaining listeners on this node
                    // unless immediate_propagation_stopped is set
                }
            }
        }

        // After processing all listeners on this node, check if propagation was stopped
        let evt = event_obj.downcast_ref::<JsEvent>().unwrap();
        Ok(evt.propagation_stopped)
    }

    /// Set an own data property on the event object, overriding any prototype accessor.
    fn set_event_prop(event_obj: &JsObject, name: &str, value: JsValue, ctx: &mut Context) -> JsResult<()> {
        event_obj.define_property_or_throw(
            js_string!(name),
            PropertyDescriptor::builder()
                .value(value)
                .writable(true)
                .configurable(true)
                .enumerable(true)
                .build(),
            ctx,
        )?;
        Ok(())
    }

    /// Reset event phase and currentTarget after dispatch, return !defaultPrevented.
    fn finish_dispatch(event_obj: &JsObject, ctx: &mut Context) -> JsResult<JsValue> {
        let default_prevented = {
            let mut evt = event_obj.downcast_mut::<JsEvent>().unwrap();
            evt.phase = 0;
            evt.current_target = None;
            evt.default_prevented
        };
        Self::set_event_prop(event_obj, "currentTarget", JsValue::null(), ctx)?;
        Ok(JsValue::from(!default_prevented))
    }
}

impl Class for JsElement {
    const NAME: &'static str = "Element";
    const LENGTH: usize = 0;

    fn data_constructor(
        _new_target: &JsValue,
        _args: &[JsValue],
        _context: &mut Context,
    ) -> JsResult<Self> {
        Err(JsError::from_opaque(
            js_string!("Element cannot be constructed directly from JS").into(),
        ))
    }

    fn init(class: &mut ClassBuilder) -> JsResult<()> {
        // appendChild method
        class.method(
            js_string!("appendChild"),
            1,
            NativeFunction::from_fn_ptr(Self::append_child),
        );

        // textContent getter/setter
        let realm = class.context().realm().clone();

        let getter = NativeFunction::from_fn_ptr(Self::get_text_content);
        let setter = NativeFunction::from_fn_ptr(Self::set_text_content);

        class.accessor(
            js_string!("textContent"),
            Some(getter.to_js_function(&realm)),
            Some(setter.to_js_function(&realm)),
            Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
        );

        // classList getter
        let class_list_getter = NativeFunction::from_fn_ptr(Self::get_class_list);
        class.accessor(
            js_string!("classList"),
            Some(class_list_getter.to_js_function(&realm)),
            None,
            Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
        );

        // Register traversal properties (parentNode, firstChild, etc.)
        super::traversal::register_traversal(class)?;

        // Register attribute methods (getAttribute, setAttribute, etc.)
        super::attributes::register_attributes(class)?;

        // Register node info properties (nodeType, nodeName, tagName, etc.)
        super::node_info::register_node_info(class)?;

        // Register innerHTML, outerHTML, insertAdjacentHTML
        super::inner_html::register_inner_html(class)?;

        // Register mutation methods (insertBefore, replaceChild, removeChild, cloneNode)
        super::mutation::register_mutation(class)?;

        // Register style accessor
        super::style::register_style(class)?;

        // Register query methods (querySelector, querySelectorAll, etc.)
        super::query::register_query(class)?;

        // Register input properties (value, checked, type, disabled, name, placeholder)
        super::input_props::register_input_props(class)?;

        // Register select/option properties (select.value, selectedIndex, options, option.selected/text)
        super::select_props::register_select_props(class)?;

        // Register anchor/form/dataset properties (href, action, method, elements, hidden, dataset)
        super::anchor_form::register_anchor_form(class)?;

        // Register common HTMLElement properties (tabIndex, title, lang, dir, getBoundingClientRect, focus, blur, click)
        super::html_element::register_html_element(class)?;

        // addEventListener / removeEventListener / dispatchEvent
        class.method(
            js_string!("addEventListener"),
            2,
            NativeFunction::from_fn_ptr(Self::add_event_listener),
        );
        class.method(
            js_string!("removeEventListener"),
            2,
            NativeFunction::from_fn_ptr(Self::remove_event_listener),
        );
        class.method(
            js_string!("dispatchEvent"),
            1,
            NativeFunction::from_fn_ptr(Self::dispatch_event),
        );

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
    fn add_event_listener_basic() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><button id='btn'>Click</button></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        // Should not throw
        runtime
            .eval("document.getElementById('btn').addEventListener('click', function() {})")
            .unwrap();
    }

    #[test]
    fn remove_event_listener_basic() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><button id='btn'>Click</button></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        runtime
            .eval(
                r#"
                var handler = function() {};
                var btn = document.getElementById('btn');
                btn.addEventListener('click', handler);
                btn.removeEventListener('click', handler);
            "#,
            )
            .unwrap();

        // Listener map should be empty after removal
        let count: usize = runtime
            .listeners
            .borrow()
            .values()
            .map(|v| v.len())
            .sum();
        assert_eq!(count, 0);
    }

    #[test]
    fn add_event_listener_with_capture_bool() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><div id='d'></div></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        runtime
            .eval("document.getElementById('d').addEventListener('click', function() {}, true)")
            .unwrap();

        let count: usize = runtime
            .listeners
            .borrow()
            .values()
            .map(|v| v.len())
            .sum();
        assert_eq!(count, 1);

        // Verify the capture flag is true
        let map = runtime.listeners.borrow();
        let entries = map.values().next().unwrap();
        assert!(entries[0].capture);
        assert!(!entries[0].once);
    }

    #[test]
    fn add_event_listener_with_options_object() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><div id='d'></div></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        runtime
            .eval("document.getElementById('d').addEventListener('click', function() {}, { capture: true, once: true })")
            .unwrap();

        let map = runtime.listeners.borrow();
        let entries = map.values().next().unwrap();
        assert!(entries[0].capture);
        assert!(entries[0].once);
    }

    #[test]
    fn listener_count_increases() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><div id='d'></div></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        runtime
            .eval(
                r#"
                var d = document.getElementById('d');
                d.addEventListener('click', function() { console.log('click') });
                d.addEventListener('mouseover', function() { console.log('hover') });
            "#,
            )
            .unwrap();

        let count: usize = runtime
            .listeners
            .borrow()
            .values()
            .map(|v| v.len())
            .sum();
        assert_eq!(count, 2);
    }

    #[test]
    fn no_duplicate_listeners() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><div id='d'></div></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        runtime
            .eval(
                r#"
                var d = document.getElementById('d');
                var handler = function() {};
                d.addEventListener('click', handler);
                d.addEventListener('click', handler);
                d.addEventListener('click', handler);
            "#,
            )
            .unwrap();

        // Same callback + same type + same capture should only be stored once
        let count: usize = runtime
            .listeners
            .borrow()
            .values()
            .map(|v| v.len())
            .sum();
        assert_eq!(count, 1);
    }

    #[test]
    fn same_callback_different_capture_not_duplicate() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><div id='d'></div></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        runtime
            .eval(
                r#"
                var d = document.getElementById('d');
                var handler = function() {};
                d.addEventListener('click', handler, false);
                d.addEventListener('click', handler, true);
            "#,
            )
            .unwrap();

        // Different capture flag means they are distinct listeners
        let count: usize = runtime
            .listeners
            .borrow()
            .values()
            .map(|v| v.len())
            .sum();
        assert_eq!(count, 2);
    }

    #[test]
    fn remove_only_matching_listener() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><div id='d'></div></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        runtime
            .eval(
                r#"
                var d = document.getElementById('d');
                var h1 = function() {};
                var h2 = function() {};
                d.addEventListener('click', h1);
                d.addEventListener('click', h2);
                d.removeEventListener('click', h1);
            "#,
            )
            .unwrap();

        // Only h2 should remain
        let count: usize = runtime
            .listeners
            .borrow()
            .values()
            .map(|v| v.len())
            .sum();
        assert_eq!(count, 1);
    }

    #[test]
    fn remove_nonexistent_listener_is_noop() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><div id='d'></div></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        runtime
            .eval(
                r#"
                var d = document.getElementById('d');
                var h1 = function() {};
                var h2 = function() {};
                d.addEventListener('click', h1);
                d.removeEventListener('click', h2);
            "#,
            )
            .unwrap();

        // h1 should still be there, h2 was never added
        let count: usize = runtime
            .listeners
            .borrow()
            .values()
            .map(|v| v.len())
            .sum();
        assert_eq!(count, 1);
    }

    #[test]
    fn remove_with_capture_must_match() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><div id='d'></div></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        runtime
            .eval(
                r#"
                var d = document.getElementById('d');
                var handler = function() {};
                d.addEventListener('click', handler, true);
                d.removeEventListener('click', handler, false);
            "#,
            )
            .unwrap();

        // Capture flag doesn't match, so the listener should NOT be removed
        let count: usize = runtime
            .listeners
            .borrow()
            .values()
            .map(|v| v.len())
            .sum();
        assert_eq!(count, 1);

        // Now remove with matching capture
        runtime
            .eval(
                r#"
                var d = document.getElementById('d');
                d.removeEventListener('click', handler, true);
            "#,
            )
            .unwrap();

        let count: usize = runtime
            .listeners
            .borrow()
            .values()
            .map(|v| v.len())
            .sum();
        assert_eq!(count, 0);
    }

    #[test]
    fn listeners_on_multiple_elements() {
        let mut engine = Engine::new();
        engine.load_html(
            "<html><body><div id='a'></div><div id='b'></div></body></html>",
        );
        let runtime = engine.runtime.as_mut().unwrap();
        runtime
            .eval(
                r#"
                document.getElementById('a').addEventListener('click', function() {});
                document.getElementById('b').addEventListener('click', function() {});
            "#,
            )
            .unwrap();

        // Two different elements, each with one listener
        let map = runtime.listeners.borrow();
        assert_eq!(map.len(), 2);
        let total: usize = map.values().map(|v| v.len()).sum();
        assert_eq!(total, 2);
    }

    #[test]
    fn add_event_listener_null_callback_is_noop() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><div id='d'></div></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        // null callback should not throw
        runtime
            .eval("document.getElementById('d').addEventListener('click', null)")
            .unwrap();

        let count: usize = runtime
            .listeners
            .borrow()
            .values()
            .map(|v| v.len())
            .sum();
        assert_eq!(count, 0);
    }

    #[test]
    fn remove_event_listener_null_callback_is_noop() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><div id='d'></div></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        runtime
            .eval(
                r#"
                var d = document.getElementById('d');
                d.addEventListener('click', function() {});
                d.removeEventListener('click', null);
            "#,
            )
            .unwrap();

        // The listener should still be there
        let count: usize = runtime
            .listeners
            .borrow()
            .values()
            .map(|v| v.len())
            .sum();
        assert_eq!(count, 1);
    }

    #[test]
    fn add_event_listener_default_options() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><div id='d'></div></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        runtime
            .eval("document.getElementById('d').addEventListener('click', function() {})")
            .unwrap();

        let map = runtime.listeners.borrow();
        let entries = map.values().next().unwrap();
        assert!(!entries[0].capture);
        assert!(!entries[0].once);
    }

    #[test]
    fn event_type_stored_correctly() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><div id='d'></div></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        runtime
            .eval(
                r#"
                var d = document.getElementById('d');
                d.addEventListener('mousedown', function() {});
                d.addEventListener('mouseup', function() {});
                d.addEventListener('keypress', function() {});
            "#,
            )
            .unwrap();

        let map = runtime.listeners.borrow();
        let entries = map.values().next().unwrap();
        let types: Vec<&str> = entries.iter().map(|e| e.event_type.as_str()).collect();
        assert!(types.contains(&"mousedown"));
        assert!(types.contains(&"mouseup"));
        assert!(types.contains(&"keypress"));
    }

    // ---- dispatchEvent tests ----

    #[test]
    fn dispatch_event_fires_listener() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><button id='btn'>Click</button></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        runtime.eval(r#"
            var result = '';
            var btn = document.getElementById('btn');
            btn.addEventListener('click', function(e) { result = 'fired:' + e.type; });
            btn.dispatchEvent(new Event('click'));
        "#).unwrap();
        let result = runtime.eval("result").unwrap();
        let s = result.to_string(&mut runtime.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "fired:click");
    }

    #[test]
    fn dispatch_event_bubbles_to_parent() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><div id='parent'><button id='btn'>Click</button></div></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        runtime.eval(r#"
            var log = [];
            document.getElementById('parent').addEventListener('click', function() { log.push('parent'); });
            document.getElementById('btn').addEventListener('click', function() { log.push('btn'); });
            document.getElementById('btn').dispatchEvent(new Event('click', { bubbles: true }));
        "#).unwrap();
        let result = runtime.eval("log.join(',')").unwrap();
        let s = result.to_string(&mut runtime.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "btn,parent");
    }

    #[test]
    fn dispatch_event_no_bubbles_stays_at_target() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><div id='parent'><button id='btn'>Click</button></div></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        runtime.eval(r#"
            var log = [];
            document.getElementById('parent').addEventListener('click', function() { log.push('parent'); });
            document.getElementById('btn').addEventListener('click', function() { log.push('btn'); });
            document.getElementById('btn').dispatchEvent(new Event('click', { bubbles: false }));
        "#).unwrap();
        let result = runtime.eval("log.join(',')").unwrap();
        let s = result.to_string(&mut runtime.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "btn");
    }

    #[test]
    fn dispatch_event_capture_phase() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><div id='outer'><div id='inner'><button id='btn'>Click</button></div></div></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        runtime.eval(r#"
            var log = [];
            document.getElementById('outer').addEventListener('click', function() { log.push('outer-capture'); }, true);
            document.getElementById('inner').addEventListener('click', function() { log.push('inner-capture'); }, true);
            document.getElementById('btn').addEventListener('click', function() { log.push('btn-target'); });
            document.getElementById('outer').addEventListener('click', function() { log.push('outer-bubble'); });
            document.getElementById('btn').dispatchEvent(new Event('click', { bubbles: true }));
        "#).unwrap();
        let result = runtime.eval("log.join(',')").unwrap();
        let s = result.to_string(&mut runtime.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "outer-capture,inner-capture,btn-target,outer-bubble");
    }

    #[test]
    fn dispatch_event_stop_propagation() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><div id='parent'><button id='btn'>Click</button></div></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        runtime.eval(r#"
            var log = [];
            document.getElementById('btn').addEventListener('click', function(e) { log.push('btn'); e.stopPropagation(); });
            document.getElementById('parent').addEventListener('click', function() { log.push('parent'); });
            document.getElementById('btn').dispatchEvent(new Event('click', { bubbles: true }));
        "#).unwrap();
        let result = runtime.eval("log.join(',')").unwrap();
        let s = result.to_string(&mut runtime.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "btn");
    }

    #[test]
    fn dispatch_event_stop_immediate_propagation() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><button id='btn'>Click</button></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        runtime.eval(r#"
            var log = [];
            var btn = document.getElementById('btn');
            btn.addEventListener('click', function(e) { log.push('first'); e.stopImmediatePropagation(); });
            btn.addEventListener('click', function() { log.push('second'); });
            btn.dispatchEvent(new Event('click'));
        "#).unwrap();
        let result = runtime.eval("log.join(',')").unwrap();
        let s = result.to_string(&mut runtime.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "first");
    }

    #[test]
    fn dispatch_event_once_removes_listener() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><button id='btn'>Click</button></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        runtime.eval(r#"
            var count = 0;
            var btn = document.getElementById('btn');
            btn.addEventListener('click', function() { count++; }, { once: true });
            btn.dispatchEvent(new Event('click'));
            btn.dispatchEvent(new Event('click'));
        "#).unwrap();
        let result = runtime.eval("count").unwrap();
        let n = result.to_number(&mut runtime.context).unwrap();
        assert_eq!(n, 1.0);
    }

    #[test]
    fn dispatch_event_returns_true_if_not_prevented() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><button id='btn'>Click</button></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        runtime.eval(r#"
            var btn = document.getElementById('btn');
            btn.addEventListener('click', function() {});
            var result = btn.dispatchEvent(new Event('click'));
        "#).unwrap();
        let result = runtime.eval("result").unwrap();
        assert_eq!(result.to_boolean(), true);
    }

    #[test]
    fn dispatch_event_returns_false_if_prevented() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><button id='btn'>Click</button></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        runtime.eval(r#"
            var btn = document.getElementById('btn');
            btn.addEventListener('click', function(e) { e.preventDefault(); });
            var result = btn.dispatchEvent(new Event('click', { cancelable: true }));
        "#).unwrap();
        let result = runtime.eval("result").unwrap();
        assert_eq!(result.to_boolean(), false);
    }

    #[test]
    fn dispatch_event_target_has_correct_tag() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><div id='parent'><button id='btn'>Click</button></div></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        runtime.eval(r#"
            var info = [];
            document.getElementById('parent').addEventListener('click', function(e) {
                info.push('target-tag:' + e.target.tagName);
                info.push('currentTarget-tag:' + e.currentTarget.tagName);
            });
            document.getElementById('btn').dispatchEvent(new Event('click', { bubbles: true }));
        "#).unwrap();
        let result = runtime.eval("info.join(',')").unwrap();
        let s = result.to_string(&mut runtime.context).unwrap().to_std_string_escaped();
        // tagName returns uppercase for HTML elements (per spec), but our impl may
        // return lowercase depending on the parser. Check case-insensitively.
        let s_lower = s.to_ascii_lowercase();
        assert!(s_lower.contains("target-tag:button"), "target should be button: {}", s);
        assert!(s_lower.contains("currenttarget-tag:div"), "currentTarget should be div: {}", s);
    }

    #[test]
    fn dispatch_event_stop_propagation_in_capture_phase() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><div id='outer'><div id='inner'><button id='btn'>Click</button></div></div></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        runtime.eval(r#"
            var log = [];
            document.getElementById('outer').addEventListener('click', function(e) {
                log.push('outer-capture');
                e.stopPropagation();
            }, true);
            document.getElementById('inner').addEventListener('click', function() { log.push('inner-capture'); }, true);
            document.getElementById('btn').addEventListener('click', function() { log.push('btn-target'); });
            document.getElementById('btn').dispatchEvent(new Event('click', { bubbles: true }));
        "#).unwrap();
        let result = runtime.eval("log.join(',')").unwrap();
        let s = result.to_string(&mut runtime.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "outer-capture");
    }

    #[test]
    fn dispatch_event_no_listeners_returns_true() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><button id='btn'>Click</button></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        runtime.eval(r#"
            var btn = document.getElementById('btn');
            var result = btn.dispatchEvent(new Event('click'));
        "#).unwrap();
        let result = runtime.eval("result").unwrap();
        assert_eq!(result.to_boolean(), true);
    }

    #[test]
    fn dispatch_event_at_target_fires_both_capture_and_bubble_listeners() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><button id='btn'>Click</button></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        runtime.eval(r#"
            var log = [];
            var btn = document.getElementById('btn');
            btn.addEventListener('click', function() { log.push('capture'); }, true);
            btn.addEventListener('click', function() { log.push('bubble'); }, false);
            btn.dispatchEvent(new Event('click'));
        "#).unwrap();
        let result = runtime.eval("log.join(',')").unwrap();
        let s = result.to_string(&mut runtime.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "capture,bubble");
    }

    #[test]
    fn dispatch_event_stop_propagation_still_fires_remaining_listeners_on_same_node() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><button id='btn'>Click</button></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        runtime.eval(r#"
            var log = [];
            var btn = document.getElementById('btn');
            btn.addEventListener('click', function(e) { log.push('first'); e.stopPropagation(); });
            btn.addEventListener('click', function() { log.push('second'); });
            btn.dispatchEvent(new Event('click'));
        "#).unwrap();
        let result = runtime.eval("log.join(',')").unwrap();
        let s = result.to_string(&mut runtime.context).unwrap().to_std_string_escaped();
        // stopPropagation stops at the next node, but remaining listeners on this node still fire
        assert_eq!(s, "first,second");
    }
}
