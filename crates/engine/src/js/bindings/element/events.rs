use std::cell::RefCell;
use std::rc::Rc;

use boa_engine::{
    js_string,
    property::PropertyDescriptor,
    Context, JsError, JsObject, JsResult, JsValue,
};

use crate::dom::{DomTree, NodeData, NodeId};

use crate::js::realm_state;

use super::cache::get_or_create_js_element;
use super::super::event::JsEvent;
use super::super::window::WINDOW_LISTENER_ID;
use super::JsElement;

/// Shared event state passed through all dispatch phases to avoid too-many-arguments.
pub(super) struct DispatchEvent<'a> {
    pub(super) tree_scope: usize,
    pub(super) event_obj: &'a JsObject,
    pub(super) event_val: &'a JsValue,
    pub(super) event_type: &'a str,
}

impl JsElement {
    /// Native implementation of element.addEventListener(type, callback, options?)
    pub(super) fn add_event_listener(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
        extract_element!(el, this, "addEventListener");
        let listener_key = (Rc::as_ptr(&el.tree) as usize, el.node_id);
        let tree_ref = el.tree.clone();
        drop(el);
        super::super::event_target::add_event_listener_impl(listener_key, Some(&tree_ref), args, ctx)
    }

    /// Native implementation of element.removeEventListener(type, callback, options?)
    pub(super) fn remove_event_listener(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
        extract_element!(el, this, "removeEventListener");
        let listener_key = (Rc::as_ptr(&el.tree) as usize, el.node_id);
        drop(el);
        super::super::event_target::remove_event_listener_impl(listener_key, args, ctx)
    }

    /// Public entry point for element.dispatchEvent — called from EventTarget.prototype.dispatchEvent
    pub(crate) fn dispatch_event_public(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
        Self::dispatch_event(this, args, ctx)
    }

    /// Native implementation of element.dispatchEvent(event)
    ///
    /// Implements the W3C event dispatch algorithm:
    /// 1. Build propagation path from target up to root
    /// 2. Capture phase (root -> parent of target)
    /// 3. At-target phase (target itself)
    /// 4. Bubble phase (parent of target -> root), only if event.bubbles
    pub(super) fn dispatch_event(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
        let this_obj = this
            .as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("dispatchEvent: `this` is not an object").into()))?;
        let (target_node_id, tree) = {
            let el = this_obj
                .downcast_ref::<JsElement>()
                .ok_or_else(|| JsError::from_opaque(js_string!("dispatchEvent: `this` is not an Element").into()))?;
            (el.node_id, el.tree.clone())
        };
        let tree_scope = Rc::as_ptr(&tree) as usize;

        let event_val = args
            .first()
            .ok_or_else(|| {
                JsError::from_native(boa_engine::JsNativeError::typ().with_message(
                    "Failed to execute 'dispatchEvent' on 'EventTarget': 1 argument required, but only 0 present.",
                ))
            })?
            .clone();

        // null/undefined arg -> TypeError
        if event_val.is_null() || event_val.is_undefined() {
            return Err(JsError::from_native(boa_engine::JsNativeError::typ().with_message(
                "Failed to execute 'dispatchEvent' on 'EventTarget': parameter 1 is not of type 'Event'.",
            )));
        }

        let event_obj = event_val
            .as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("dispatchEvent: argument is not an object").into()))?
            .clone();

        // Read event_type, bubbles, and whether this is a Mouse click from the event's native data
        let (event_type, bubbles, is_click_mouse, composed) = {
            let evt = event_obj
                .downcast_ref::<JsEvent>()
                .ok_or_else(|| JsError::from_opaque(js_string!("dispatchEvent: argument is not an Event").into()))?;
            if !evt.initialized {
                return Err(JsError::from_opaque(
                    js_string!("InvalidStateError: The event is not initialized.").into(),
                ));
            }
            if evt.dispatching {
                return Err(JsError::from_opaque(
                    js_string!("InvalidStateError: The event is already being dispatched.").into(),
                ));
            }
            let is_click = evt.event_type == "click" && evt.kind.is_mouse();
            (evt.event_type.clone(), evt.bubbles, is_click, evt.composed)
        };

        // Check cancelBubble (propagation_stopped) — if already set, dispatch is a no-op
        let already_stopped = event_obj.downcast_ref::<JsEvent>().unwrap().propagation_stopped;
        if already_stopped {
            return Self::finish_dispatch_generic(&event_obj, ctx);
        }

        // 1. Build propagation path: [root, ..., grandparent, parent, target]
        let propagation_path = build_propagation_path(&tree.borrow(), target_node_id, composed);

        // --- relatedTarget retargeting (DOM spec section 2.7) ---
        // Compute retargeting info while holding the tree borrow
        enum RetargetAction {
            EarlyReturn,
            Retarget { retargeted_id: NodeId },
            None,
        }
        let (clear_targets, retarget_action) = {
            let tree_ref = tree.borrow();
            let mut clear = false;

            // Check if target's root is a ShadowRoot
            let target_root = tree_ref.root_of(target_node_id);
            if matches!(tree_ref.get_node(target_root).data, NodeData::ShadowRoot { .. }) {
                clear = true;
            }

            let related_target_val =
                event_obj.get(js_string!("__relatedTarget"), ctx).unwrap_or(JsValue::undefined());
            let action = if !related_target_val.is_undefined() && !related_target_val.is_null() {
                if let Some(rt_obj) = related_target_val.as_object() {
                    if let Some(rt_el) = rt_obj.downcast_ref::<JsElement>() {
                        let rt_node_id = rt_el.node_id;
                        let retargeted = tree_ref.retarget(rt_node_id, Some(target_node_id));

                        // clearTargets: check retargeted relatedTarget's root only
                        let rt_root = tree_ref.root_of(retargeted);
                        if matches!(tree_ref.get_node(rt_root).data, NodeData::ShadowRoot { .. }) {
                            clear = true;
                        }

                        // Spec step 5.4: skip dispatch when retargetedRT = target AND
                        // original relatedTarget != target
                        if retargeted == target_node_id && rt_node_id != target_node_id {
                            RetargetAction::EarlyReturn
                        } else if retargeted != rt_node_id {
                            RetargetAction::Retarget { retargeted_id: retargeted }
                        } else {
                            RetargetAction::None
                        }
                    } else {
                        RetargetAction::None
                    }
                } else {
                    RetargetAction::None
                }
            } else {
                RetargetAction::None
            };

            (clear, action)
        };

        // Apply retarget action (tree borrow released)
        match retarget_action {
            RetargetAction::EarlyReturn => {
                // Early return: no dispatch. Set target/relatedTarget to retargeted values,
                // then null them out (clearTargets is always true for early return since the
                // original relatedTarget was in a shadow tree).
                Self::set_event_prop(&event_obj, "target", JsValue::null(), ctx)?;
                Self::set_event_prop(&event_obj, "srcElement", JsValue::null(), ctx)?;
                event_obj.set(js_string!("__relatedTarget"), JsValue::null(), false, ctx)?;
                event_obj.downcast_mut::<JsEvent>().unwrap().target = None;
                let dp = event_obj.downcast_ref::<JsEvent>().unwrap().default_prevented;
                return Ok(JsValue::from(!dp));
            }
            RetargetAction::Retarget { retargeted_id } => {
                let retargeted_js = get_or_create_js_element(retargeted_id, tree.clone(), ctx)?;
                event_obj.define_property_or_throw(
                    js_string!("__relatedTarget"),
                    PropertyDescriptor::builder()
                        .value(JsValue::from(retargeted_js))
                        .writable(true)
                        .configurable(true)
                        .enumerable(false)
                        .build(),
                    ctx,
                )?;
            }
            RetargetAction::None => {}
        }

        // Activation behavior: find activation target and run pre-activation
        let (activation_target, saved_activation) = if is_click_mouse {
            let tree_ref = tree.borrow();
            let at = super::super::activation::find_activation_target(&tree_ref, &propagation_path, bubbles);
            drop(tree_ref);
            if let Some(at_id) = at {
                let saved = super::super::activation::run_legacy_pre_activation(&mut tree.borrow_mut(), at_id);
                (Some(at_id), Some(saved))
            } else {
                (None, None)
            }
        } else {
            (None, None)
        };

        // Check if window should be in the propagation path.
        // Window is added when the root of the path is the Document node.
        let include_window = {
            let tree_ref = tree.borrow();
            if let Some(&root_id) = propagation_path.first() {
                if matches!(tree_ref.get_node(root_id).data, NodeData::Document) {
                    // Only include window for the global document, not created documents
                    {
                        let global_tree = realm_state::dom_tree(ctx);
                        Rc::ptr_eq(&tree, &global_tree)
                    }
                } else {
                    false
                }
            } else {
                false
            }
        };

        // 2. Set event.target and dispatching flag
        {
            let mut evt = event_obj.downcast_mut::<JsEvent>().unwrap();
            evt.target = Some(target_node_id);
            evt.dispatching = true;
        }

        // Set the JS-level target and srcElement properties
        Self::set_event_prop(&event_obj, "target", this.clone(), ctx)?;
        Self::set_event_prop(&event_obj, "srcElement", this.clone(), ctx)?;

        let target_index = propagation_path.len() - 1;

        let evt = DispatchEvent {
            tree_scope,
            event_obj: &event_obj,
            event_val: &event_val,
            event_type: &event_type,
        };

        // 3. Capture phase
        let mut dispatch_stopped =
            run_capture_phase(&propagation_path, target_index, include_window, &tree, &evt, ctx)?;

        // 4. At-target phase
        if !dispatch_stopped {
            dispatch_stopped = run_at_target_phase(target_node_id, this, &evt, ctx)?;
        }

        // 5. Bubble phase
        if bubbles && !dispatch_stopped {
            run_bubble_phase(&propagation_path, target_index, include_window, &tree, &evt, ctx)?;
        }

        // 6. Finish dispatch — reset event state
        let result = Self::finish_dispatch_generic(&event_obj, ctx)?;

        // 6b. clearTargets — null out target/relatedTarget if either was in a shadow tree
        if clear_targets {
            Self::set_event_prop(&event_obj, "target", JsValue::null(), ctx)?;
            Self::set_event_prop(&event_obj, "srcElement", JsValue::null(), ctx)?;
            event_obj.set(js_string!("__relatedTarget"), JsValue::null(), false, ctx)?;
            // Also clear native target so composedPath() returns []
            event_obj.downcast_mut::<JsEvent>().unwrap().target = None;
        }

        // 7. Activation behavior (post-dispatch)
        if let (Some(at_id), Some(saved)) = (activation_target, saved_activation) {
            let default_prevented = event_obj.downcast_ref::<JsEvent>().unwrap().default_prevented;
            if default_prevented {
                super::super::activation::restore_activation(&mut tree.borrow_mut(), at_id, saved);
            } else {
                super::super::activation::run_post_activation(&tree, at_id, ctx);
            }
        }

        Ok(result)
    }

    /// Delegates to the standalone `invoke_listeners_for_node` function.
    #[allow(dead_code)]
    pub(super) fn invoke_listeners_for_node(
        listener_key: (usize, NodeId),
        event_type: &str,
        event_obj: &JsObject,
        event_val: &JsValue,
        capture_only: bool,
        at_target: bool,
        ctx: &mut Context,
    ) -> JsResult<bool> {
        invoke_listeners_for_node(
            listener_key,
            event_type,
            event_obj,
            event_val,
            capture_only,
            at_target,
            ctx,
        )
    }

    /// Set an own data property on the event object, overriding any prototype accessor.
    pub(crate) fn set_event_prop(event_obj: &JsObject, name: &str, value: JsValue, ctx: &mut Context) -> JsResult<()> {
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

    /// Reset event phase, currentTarget, propagation flags, dispatching after dispatch.
    pub(super) fn finish_dispatch_generic(event_obj: &JsObject, ctx: &mut Context) -> JsResult<JsValue> {
        let default_prevented = {
            let mut evt = event_obj.downcast_mut::<JsEvent>().unwrap();
            evt.phase = 0;
            evt.current_target = None;
            evt.propagation_stopped = false;
            evt.immediate_propagation_stopped = false;
            evt.dispatching = false;
            evt.default_prevented
        };
        Self::set_event_prop(event_obj, "currentTarget", JsValue::null(), ctx)?;
        Ok(JsValue::from(!default_prevented))
    }
}

/// Build the event propagation path from target up to the root, returned as [root, ..., parent, target].
/// When `composed` is true, the path crosses shadow DOM boundaries: when we reach a ShadowRoot
/// (parent is None), we jump to the host element and continue walking up.
fn build_propagation_path(tree: &DomTree, target_node_id: NodeId, composed: bool) -> Vec<NodeId> {
    use crate::dom::NodeData;

    let mut path = vec![target_node_id];
    let mut current = target_node_id;
    loop {
        if let Some(parent_id) = tree.get_node(current).parent {
            path.push(parent_id);
            current = parent_id;
        } else if composed {
            // If current is a ShadowRoot, jump to its host element
            if let NodeData::ShadowRoot { host, .. } = tree.get_node(current).data {
                path.push(host);
                current = host;
            } else {
                break;
            }
        } else {
            break;
        }
    }
    path.reverse();
    path
}

/// Set the native event phase and current_target fields on a JsEvent.
fn set_event_phase(event_obj: &JsObject, node_id: Option<NodeId>, phase: u8) {
    let mut evt = event_obj.downcast_mut::<JsEvent>().unwrap();
    evt.current_target = node_id;
    evt.phase = phase;
}

/// Run the capture phase (phase 1): window capture listeners (if included), then walk from root
/// down to (but NOT including) the target. Returns whether dispatch was stopped.
fn run_capture_phase(
    propagation_path: &[NodeId],
    target_index: usize,
    include_window: bool,
    tree: &Rc<RefCell<DomTree>>,
    evt: &DispatchEvent<'_>,
    ctx: &mut Context,
) -> JsResult<bool> {
    // Window capture listeners first (if applicable)
    if include_window {
        set_event_phase(evt.event_obj, Some(WINDOW_LISTENER_ID), 1);
        let window_val: JsValue = realm_state::window_object(ctx)
            .map(JsValue::from)
            .unwrap_or(JsValue::undefined());
        JsElement::set_event_prop(evt.event_obj, "currentTarget", window_val.clone(), ctx)?;

        let should_stop = invoke_listeners_for_node(
            (usize::MAX, WINDOW_LISTENER_ID),
            evt.event_type,
            evt.event_obj,
            evt.event_val,
            true,
            false,
            ctx,
        )?;

        if should_stop {
            return Ok(true);
        }
    }

    // Walk from root down to (but not including) the target
    for &node_id in &propagation_path[..target_index] {
        set_event_phase(evt.event_obj, Some(node_id), 1);
        let current_target_js = get_or_create_js_element(node_id, tree.clone(), ctx)?;
        JsElement::set_event_prop(evt.event_obj, "currentTarget", JsValue::from(current_target_js), ctx)?;

        let should_stop = invoke_listeners_for_node(
            (evt.tree_scope, node_id),
            evt.event_type,
            evt.event_obj,
            evt.event_val,
            true,
            false,
            ctx,
        )?;
        if should_stop {
            return Ok(true);
        }
    }

    Ok(false)
}

/// Run the at-target phase (phase 2): capture listeners first, then non-capture listeners,
/// then on* handler invocation. Returns whether dispatch was stopped.
fn run_at_target_phase(
    target_node_id: NodeId,
    this: &JsValue,
    evt: &DispatchEvent<'_>,
    ctx: &mut Context,
) -> JsResult<bool> {
    // First: capture listeners at target
    set_event_phase(evt.event_obj, Some(target_node_id), 2);
    JsElement::set_event_prop(evt.event_obj, "currentTarget", this.clone(), ctx)?;

    let should_stop_capture = invoke_listeners_for_node(
        (evt.tree_scope, target_node_id),
        evt.event_type,
        evt.event_obj,
        evt.event_val,
        true,
        false,
        ctx,
    )?;
    if should_stop_capture {
        return Ok(true);
    }

    // Second: non-capture listeners at target
    set_event_phase(evt.event_obj, Some(target_node_id), 2);
    let should_stop_bubble = invoke_listeners_for_node(
        (evt.tree_scope, target_node_id),
        evt.event_type,
        evt.event_obj,
        evt.event_val,
        false,
        false,
        ctx,
    )?;

    // Invoke on* handler at target
    super::super::on_event::invoke_on_event_handler(
        evt.tree_scope,
        target_node_id,
        evt.event_type,
        this,
        evt.event_val,
        evt.event_obj,
        ctx,
    );

    Ok(should_stop_bubble)
}

/// Run the bubble phase (phase 3): walk from parent of target up to root, then window bubble
/// listeners (if included), invoking on* handlers at each step. Returns whether dispatch was stopped.
fn run_bubble_phase(
    propagation_path: &[NodeId],
    target_index: usize,
    include_window: bool,
    tree: &Rc<RefCell<DomTree>>,
    evt: &DispatchEvent<'_>,
    ctx: &mut Context,
) -> JsResult<bool> {
    for i in (0..target_index).rev() {
        let node_id = propagation_path[i];

        set_event_phase(evt.event_obj, Some(node_id), 3);
        let current_target_js = get_or_create_js_element(node_id, tree.clone(), ctx)?;
        JsElement::set_event_prop(
            evt.event_obj,
            "currentTarget",
            JsValue::from(current_target_js.clone()),
            ctx,
        )?;

        let should_stop = invoke_listeners_for_node(
            (evt.tree_scope, node_id),
            evt.event_type,
            evt.event_obj,
            evt.event_val,
            false,
            false,
            ctx,
        )?;

        // Invoke on* handler during bubble
        super::super::on_event::invoke_on_event_handler(
            evt.tree_scope,
            node_id,
            evt.event_type,
            &JsValue::from(current_target_js),
            evt.event_val,
            evt.event_obj,
            ctx,
        );

        if should_stop {
            return Ok(true);
        }
    }

    // Window bubble listeners last (if applicable)
    if include_window {
        set_event_phase(evt.event_obj, Some(WINDOW_LISTENER_ID), 3);
        let window_val: JsValue = realm_state::window_object(ctx)
            .map(JsValue::from)
            .unwrap_or(JsValue::undefined());
        JsElement::set_event_prop(evt.event_obj, "currentTarget", window_val.clone(), ctx)?;

        let _should_stop = invoke_listeners_for_node(
            (usize::MAX, WINDOW_LISTENER_ID),
            evt.event_type,
            evt.event_obj,
            evt.event_val,
            false,
            false,
            ctx,
        )?;

        // Invoke on* handler for window during bubble
        super::super::on_event::invoke_on_event_handler(
            super::super::on_event::WINDOW_TREE_PTR,
            WINDOW_LISTENER_ID,
            evt.event_type,
            &window_val,
            evt.event_val,
            evt.event_obj,
            ctx,
        );
    }

    Ok(false)
}

/// Returns true if the event type defaults to passive per spec section 2.10.
pub(crate) fn is_passive_default_event(event_type: &str) -> bool {
    matches!(event_type, "touchstart" | "touchmove" | "wheel" | "mousewheel")
}

/// Returns true if the target node is one of: document, documentElement, or body.
/// These are the targets where certain event types default to passive.
pub(crate) fn is_passive_default_target(node_id: NodeId, tree: &crate::dom::DomTree) -> bool {
    use crate::dom::NodeData;
    // Document node
    if matches!(tree.get_node(node_id).data, NodeData::Document) {
        return true;
    }
    // documentElement: first element child of document (node 0)
    let doc_id = 0;
    for child_id in tree.children(doc_id) {
        if matches!(tree.get_node(child_id).data, NodeData::Element { .. }) {
            if child_id == node_id {
                return true;
            }
            // body: first <body> child of documentElement
            for grandchild_id in tree.children(child_id) {
                if let NodeData::Element { ref tag_name, .. } = tree.get_node(grandchild_id).data {
                    if tag_name.eq_ignore_ascii_case("body") && grandchild_id == node_id {
                        return true;
                    }
                }
            }
            break;
        }
    }
    false
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
pub(crate) fn invoke_listeners_for_node(
    listener_key: (usize, NodeId),
    event_type: &str,
    event_obj: &JsObject,
    event_val: &JsValue,
    capture_only: bool,
    at_target: bool,
    ctx: &mut Context,
) -> JsResult<bool> {
    // Collect matching listeners (snapshot to avoid borrow issues during callback invocation)
    // Include the `removed` flag so we can detect mid-dispatch removal.
    type ListenerSnapshot = (JsObject, bool, std::rc::Rc<std::cell::Cell<bool>>, Option<bool>);
    let matching: Vec<ListenerSnapshot> = {
        let listeners = realm_state::event_listeners(ctx);
        let map = listeners.borrow();
        match map.get(&listener_key) {
            Some(entries) => entries
                .iter()
                .filter(|entry| {
                    if entry.event_type != event_type {
                        return false;
                    }
                    if at_target {
                        true
                    } else if capture_only {
                        entry.capture
                    } else {
                        !entry.capture
                    }
                })
                .map(|entry| (entry.callback.clone(), entry.once, entry.removed.clone(), entry.passive))
                .collect(),
            None => Vec::new(),
        }
    };

    // Save previous CURRENT_EVENT and set to current event (for window.event)
    let prev_event = realm_state::current_event(ctx);
    realm_state::set_current_event(ctx, Some(event_obj.clone()));

    for (callback, once, removed_flag, passive) in &matching {
        // Skip listeners that were removed during dispatch
        if removed_flag.get() {
            continue;
        }

        if *once {
            removed_flag.set(true);
            let listeners = realm_state::event_listeners(ctx);
            let mut map = listeners.borrow_mut();
            if let Some(entries) = map.get_mut(&listener_key) {
                entries.retain(|entry| {
                    if entry.event_type == event_type && entry.callback == *callback && entry.once {
                        entry.removed.set(true);
                        false
                    } else {
                        true
                    }
                });
                if entries.is_empty() {
                    map.remove(&listener_key);
                }
            }
        }

        // Per spec: if listener is passive, temporarily clear cancelable so preventDefault is a no-op
        let is_passive = passive.unwrap_or(false);
        let saved_cancelable = if is_passive {
            let saved = event_obj.downcast_ref::<JsEvent>().unwrap().cancelable;
            event_obj.downcast_mut::<JsEvent>().unwrap().cancelable = false;
            Some(saved)
        } else {
            None
        };

        // Per spec: if callback is callable, call with this=currentTarget.
        // If callback is an object with handleEvent method, look it up fresh and call with this=object.
        // Per spec: if a listener throws, report the error and continue to the next listener.
        let call_result = if callback.is_callable() {
            // Get currentTarget from the event to use as `this`
            let current_target = event_obj
                .get(js_string!("currentTarget"), ctx)
                .unwrap_or(JsValue::undefined());
            callback.call(&current_target, std::slice::from_ref(event_val), ctx)
        } else {
            // handleEvent protocol: look up handleEvent on the object each time
            let handle = callback.get(js_string!("handleEvent"), ctx);
            match handle {
                Ok(handle_val) => {
                    if let Some(handle_fn) = handle_val.as_object().filter(|o| o.is_callable()) {
                        handle_fn.call(&JsValue::from(callback.clone()), std::slice::from_ref(event_val), ctx)
                    } else {
                        Ok(JsValue::undefined())
                    }
                }
                Err(e) => Err(e),
            }
        };

        // Restore cancelable if we cleared it for passive listener
        if let Some(saved) = saved_cancelable {
            event_obj.downcast_mut::<JsEvent>().unwrap().cancelable = saved;
        }

        // If the listener threw, report via window.onerror and continue
        if let Err(err) = call_result {
            report_listener_error(err, ctx);
        }

        let (imm_stopped, prop_stopped) = {
            let evt = event_obj.downcast_ref::<JsEvent>().unwrap();
            (evt.immediate_propagation_stopped, evt.propagation_stopped)
        };

        if imm_stopped {
            // Restore previous CURRENT_EVENT before returning
            realm_state::set_current_event(ctx, prev_event.clone());
            return Ok(true);
        }
        // prop_stopped: don't return yet -- continue processing listeners on this node
        let _ = prop_stopped;
    }

    // Restore previous CURRENT_EVENT
    realm_state::set_current_event(ctx, prev_event);

    let propagation_stopped = event_obj.downcast_ref::<JsEvent>().unwrap().propagation_stopped;
    Ok(propagation_stopped)
}

/// Report a listener error via window.onerror. Per spec, when a listener throws,
/// the error is reported and dispatch continues to the next listener.
pub(crate) fn report_listener_error(err: JsError, ctx: &mut Context) {
    let error_value = err
        .as_opaque()
        .cloned()
        .unwrap_or_else(|| {
            err.as_native()
                .map(|_| {
                    // Convert JsNativeError to a proper JS Error object
                    err.to_opaque(ctx)
                })
                .unwrap_or(JsValue::undefined())
        });

    let message = error_value
        .to_string(ctx)
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_else(|_| "unknown error".to_string());

    // Per spec, report the exception by firing an ErrorEvent on window.
    // This allows addEventListener("error", ...) to catch it (used by EventWatcher in WPT).
    if let Some(window) = realm_state::window_object(ctx) {
        // First try calling window.onerror(message, filename, lineno, colno, error) for legacy support
        let onerror: Option<JsObject> = {
            let val = match window.get(js_string!("onerror"), ctx) {
                Ok(v) if !v.is_undefined() && !v.is_null() => v,
                _ => JsValue::undefined(),
            };
            #[allow(clippy::map_clone)]
            val.as_object().filter(|o| o.is_callable()).map(|o| o.clone())
        };

        if let Some(onerror_fn) = onerror {
            let _ = onerror_fn.call(
                &JsValue::undefined(),
                &[
                    JsValue::from(js_string!(message.clone())),
                    JsValue::from(js_string!("")), // filename
                    JsValue::from(0),              // lineno
                    JsValue::from(0),              // colno
                    error_value.clone(),           // error object
                ],
                ctx,
            );
        }

        // Also dispatch an ErrorEvent on window so addEventListener("error") catches it
        // Create a plain Event-like object with error/message properties
        let event_obj = boa_engine::object::ObjectInitializer::new(ctx).build();
        let _ = event_obj.set(js_string!("type"), JsValue::from(js_string!("error")), false, ctx);
        let _ = event_obj.set(js_string!("message"), JsValue::from(js_string!(message)), false, ctx);
        let _ = event_obj.set(js_string!("error"), error_value, false, ctx);
        let _ = event_obj.set(js_string!("filename"), JsValue::from(js_string!("")), false, ctx);
        let _ = event_obj.set(js_string!("lineno"), JsValue::from(0), false, ctx);
        let _ = event_obj.set(js_string!("colno"), JsValue::from(0), false, ctx);
        let _ = event_obj.set(js_string!("bubbles"), JsValue::from(false), false, ctx);
        let _ = event_obj.set(js_string!("cancelable"), JsValue::from(true), false, ctx);

        // Fire error event on window's listeners directly
        let listeners = realm_state::event_listeners(ctx);
        let window_listeners: Vec<(JsObject, bool)> = {
            let map = listeners.borrow();
            let window_key = (usize::MAX, WINDOW_LISTENER_ID);
            map.get(&window_key)
                .map(|entries| {
                    entries
                        .iter()
                        .filter(|e| e.event_type == "error")
                        .map(|e| (e.callback.clone(), e.once))
                        .collect()
                })
                .unwrap_or_default()
        };

        for (callback, _once) in &window_listeners {
            if callback.is_callable() {
                let _ = callback.call(&JsValue::from(window.clone()), &[JsValue::from(event_obj.clone())], ctx);
            } else if let Ok(handle) = callback.get(js_string!("handleEvent"), ctx) {
                if let Some(handle_fn) = handle.as_object().filter(|o| o.is_callable()) {
                    let _ = handle_fn.call(
                        &JsValue::from(callback.clone()),
                        &[JsValue::from(event_obj.clone())],
                        ctx,
                    );
                }
            }
        }

        // Remove once listeners
        if window_listeners.iter().any(|(_, once)| *once) {
            let mut map = listeners.borrow_mut();
            let window_key = (usize::MAX, WINDOW_LISTENER_ID);
            if let Some(entries) = map.get_mut(&window_key) {
                entries.retain(|e| !(e.event_type == "error" && e.once));
            }
        }
    }
}
