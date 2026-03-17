use std::cell::RefCell;
use std::rc::Rc;

use boa_engine::class::Class;
use boa_engine::{js_string, Context, JsValue};

use crate::dom::{DomTree, NodeData, NodeId};

use super::element::get_or_create_js_element;
use super::event::{EventKind, JsEvent};

/// Saved state for legacy pre-activation (checkbox/radio toggle undo on cancel).
pub(crate) enum SavedActivationState {
    Checkbox {
        was_checked: bool,
    },
    Radio {
        node_id: NodeId,
        was_checked: bool,
        siblings: Vec<(NodeId, bool)>,
    },
    None,
}

/// Check if an element is disabled (has "disabled" attribute and is a form element).
pub(crate) fn is_disabled(tree: &DomTree, node_id: NodeId) -> bool {
    let node = tree.get_node(node_id);
    if let NodeData::Element {
        tag_name, attributes, ..
    } = &node.data
    {
        let tag = tag_name.to_lowercase();
        if matches!(tag.as_str(), "input" | "button" | "select" | "textarea") {
            return attributes.iter().any(|a| a.local_name == "disabled");
        }
    }
    false
}

/// Check if an element has activation behavior.
pub(crate) fn has_activation_behavior(tree: &DomTree, node_id: NodeId) -> bool {
    let node = tree.get_node(node_id);
    match &node.data {
        NodeData::Element {
            tag_name, attributes, ..
        } => {
            let tag = tag_name.to_lowercase();
            match tag.as_str() {
                "input" => {
                    let input_type = attributes
                        .iter()
                        .find(|a| a.local_name == "type")
                        .map(|a| a.value.to_lowercase())
                        .unwrap_or_else(|| "text".to_string());
                    matches!(
                        input_type.as_str(),
                        "checkbox" | "radio" | "submit" | "reset" | "button" | "image"
                    )
                }
                "button" => true,
                "a" => attributes.iter().any(|a| a.local_name == "href"),
                "area" => attributes.iter().any(|a| a.local_name == "href"),
                "details" => true,
                "label" => true,
                _ => false,
            }
        }
        _ => false,
    }
}

/// Find the activation target per the DOM spec:
/// "Set activationTarget to the nearest inclusive ancestor of target that has activation behavior."
/// The propagation_path is ordered root-to-target, so we walk backwards from the target.
/// When `bubbles` is false, only the target itself is checked (parents are not reached).
pub(crate) fn find_activation_target(tree: &DomTree, propagation_path: &[NodeId], bubbles: bool) -> Option<NodeId> {
    if bubbles {
        // Walk from target (last) towards root (first) to find nearest inclusive ancestor
        for &node_id in propagation_path.iter().rev() {
            if has_activation_behavior(tree, node_id) {
                return Some(node_id);
            }
        }
        None
    } else {
        // Non-bubbling: only check the target itself
        let target = *propagation_path.last()?;
        if has_activation_behavior(tree, target) {
            Some(target)
        } else {
            None
        }
    }
}

/// Run legacy pre-activation behavior.
/// For checkbox: save checked state, toggle it.
/// For radio: save group state, check this radio, uncheck siblings.
pub(crate) fn run_legacy_pre_activation(tree: &mut DomTree, node_id: NodeId) -> SavedActivationState {
    let node = tree.get_node(node_id);
    let (tag, input_type) = match &node.data {
        NodeData::Element {
            tag_name, attributes, ..
        } => {
            let tag = tag_name.to_lowercase();
            let t = attributes
                .iter()
                .find(|a| a.local_name == "type")
                .map(|a| a.value.to_lowercase())
                .unwrap_or_default();
            (tag, t)
        }
        _ => return SavedActivationState::None,
    };

    if tag != "input" {
        return SavedActivationState::None;
    }

    match input_type.as_str() {
        "checkbox" => {
            let was_checked = tree.get_attribute(node_id, "checked").is_some();
            // Toggle
            if was_checked {
                tree.remove_attribute(node_id, "checked");
            } else {
                tree.set_attribute(node_id, "checked", "");
            }
            SavedActivationState::Checkbox { was_checked }
        }
        "radio" => {
            let was_checked = tree.get_attribute(node_id, "checked").is_some();

            // Find radio group name
            let radio_name = tree.get_attribute(node_id, "name").unwrap_or_default();

            // Find siblings in same radio group
            let siblings = find_radio_group_siblings(tree, node_id, &radio_name);

            // Save sibling states
            let saved_siblings: Vec<(NodeId, bool)> = siblings
                .iter()
                .map(|&sid| {
                    let checked = tree.get_attribute(sid, "checked").is_some();
                    (sid, checked)
                })
                .collect();

            // Check this radio
            tree.set_attribute(node_id, "checked", "");

            // Uncheck siblings
            for &sid in &siblings {
                tree.remove_attribute(sid, "checked");
            }

            SavedActivationState::Radio {
                node_id,
                was_checked,
                siblings: saved_siblings,
            }
        }
        _ => SavedActivationState::None,
    }
}

/// Restore activation state (undo pre-activation on cancel).
pub(crate) fn restore_activation(tree: &mut DomTree, node_id: NodeId, saved: SavedActivationState) {
    match saved {
        SavedActivationState::Checkbox { was_checked } => {
            if was_checked {
                tree.set_attribute(node_id, "checked", "");
            } else {
                tree.remove_attribute(node_id, "checked");
            }
        }
        SavedActivationState::Radio {
            node_id: rid,
            was_checked,
            siblings,
        } => {
            if was_checked {
                tree.set_attribute(rid, "checked", "");
            } else {
                tree.remove_attribute(rid, "checked");
            }
            for (sid, was_checked) in siblings {
                if was_checked {
                    tree.set_attribute(sid, "checked", "");
                } else {
                    tree.remove_attribute(sid, "checked");
                }
            }
        }
        SavedActivationState::None => {}
    }
}

/// Run post-activation behavior after successful dispatch (not canceled).
pub(crate) fn run_post_activation(tree: &Rc<RefCell<DomTree>>, node_id: NodeId, ctx: &mut Context) {
    let (tag, input_type) = {
        let t = tree.borrow();
        let node = t.get_node(node_id);
        match &node.data {
            NodeData::Element {
                tag_name, attributes, ..
            } => {
                let tag = tag_name.to_lowercase();
                let it = attributes
                    .iter()
                    .find(|a| a.local_name == "type")
                    .map(|a| a.value.to_lowercase())
                    .unwrap_or_default();
                (tag, it)
            }
            _ => return,
        }
    };

    match tag.as_str() {
        "input" => {
            match input_type.as_str() {
                "checkbox" | "radio" => {
                    // Fire 'input' and 'change' events
                    fire_simple_event(tree, node_id, "input", true, false, ctx);
                    fire_simple_event(tree, node_id, "change", true, false, ctx);
                }
                "submit" | "image" => {
                    // Drop tree borrow before fire_simple_event (which re-enters dispatch)
                    let form_id = { find_ancestor_form(&tree.borrow(), node_id) };
                    if let Some(form_id) = form_id {
                        // Don't bubble submit from activation — prevents double activation
                        // when forms are nested (child form + parent form).
                        fire_simple_event(tree, form_id, "submit", false, true, ctx);
                    }
                }
                "reset" => {
                    let form_id = { find_ancestor_form(&tree.borrow(), node_id) };
                    if let Some(form_id) = form_id {
                        fire_simple_event(tree, form_id, "reset", false, true, ctx);
                    }
                }
                _ => {}
            }
        }
        "button" => {
            let button_type = if input_type.is_empty() { "submit" } else { &input_type };
            match button_type {
                "submit" => {
                    let form_id = { find_ancestor_form(&tree.borrow(), node_id) };
                    if let Some(form_id) = form_id {
                        fire_simple_event(tree, form_id, "submit", false, true, ctx);
                    }
                }
                "reset" => {
                    let form_id = { find_ancestor_form(&tree.borrow(), node_id) };
                    if let Some(form_id) = form_id {
                        fire_simple_event(tree, form_id, "reset", false, true, ctx);
                    }
                }
                _ => {}
            }
        }
        "label" => {
            // Drop tree borrow before click dispatch (which re-enters dispatch)
            let control_id = { find_label_control(&tree.borrow(), node_id) };
            if let Some(control_id) = control_id {
                let control_js = get_or_create_js_element(control_id, tree.clone(), ctx);
                if let Ok(js_obj) = control_js {
                    let click_fn = js_obj.get(js_string!("click"), ctx);
                    if let Ok(click_val) = click_fn {
                        if let Some(click_obj) = click_val.as_object().filter(|o| o.is_callable()) {
                            let _ = click_obj.call(&JsValue::from(js_obj), &[], ctx);
                        }
                    }
                }
            }
        }
        "a" | "area" => {
            // Navigate to href — for fragment-only hrefs, set location.hash
            let href = {
                let t = tree.borrow();
                t.get_attribute(node_id, "href").unwrap_or_default().to_string()
            };
            if let Some(fragment) = href.strip_prefix('#') {
                // Set location.hash which fires hashchange
                let global = ctx.global_object();
                if let Ok(loc) = global.get(js_string!("location"), ctx) {
                    if let Some(loc_obj) = loc.as_object() {
                        let _ = loc_obj.set(
                            js_string!("hash"),
                            JsValue::from(js_string!(fragment)),
                            false,
                            ctx,
                        );
                    }
                }
            }
        }
        "details" => {
            // Toggle the open attribute
            let has_open = tree.borrow().get_attribute(node_id, "open").is_some();
            if has_open {
                tree.borrow_mut().remove_attribute(node_id, "open");
            } else {
                tree.borrow_mut().set_attribute(node_id, "open", "");
            }
            // Fire toggle event
            fire_simple_event(tree, node_id, "toggle", false, false, ctx);
        }
        _ => {}
    }
}

/// Fire a simple Event on a node via its dispatchEvent.
fn fire_simple_event(
    tree: &Rc<RefCell<DomTree>>,
    node_id: NodeId,
    event_type: &str,
    bubbles: bool,
    cancelable: bool,
    ctx: &mut Context,
) {
    let js_obj = get_or_create_js_element(node_id, tree.clone(), ctx);
    if let Ok(target_obj) = js_obj {
        let event = JsEvent {
            event_type: event_type.to_string(),
            bubbles,
            cancelable,
            default_prevented: false,
            propagation_stopped: false,
            immediate_propagation_stopped: false,
            target: None,
            current_target: None,
            phase: 0,
            dispatching: false,
            time_stamp: super::event::dom_high_res_time_stamp(ctx),
            initialized: true,
            kind: EventKind::Standard,
        };

        let event_obj: boa_engine::JsObject = match JsEvent::from_data(event, ctx) {
            Ok(e) => e,
            Err(_) => return,
        };
        let event_val = JsValue::from(event_obj);

        let dispatch_fn = target_obj.get(js_string!("dispatchEvent"), ctx);
        if let Ok(dispatch_val) = dispatch_fn {
            if let Some(dispatch_obj) = dispatch_val.as_object().filter(|o| o.is_callable()) {
                let _ = dispatch_obj.call(&JsValue::from(target_obj), &[event_val], ctx);
            }
        }
    }
}

/// Check if a node is connected to the document (has a Document ancestor).
fn is_connected(tree: &DomTree, node_id: NodeId) -> bool {
    let mut current = Some(node_id);
    while let Some(nid) = current {
        let node = tree.get_node(nid);
        if matches!(node.data, NodeData::Document) {
            return true;
        }
        current = node.parent;
    }
    false
}

/// Find an ancestor `<form>` element that is connected to the document.
fn find_ancestor_form(tree: &DomTree, node_id: NodeId) -> Option<NodeId> {
    let mut current = tree.get_node(node_id).parent;
    while let Some(pid) = current {
        let node = tree.get_node(pid);
        if let NodeData::Element { tag_name, .. } = &node.data {
            if tag_name.eq_ignore_ascii_case("form") {
                // Only return form if it's connected to the document
                if is_connected(tree, pid) {
                    return Some(pid);
                }
                return None;
            }
        }
        current = node.parent;
    }
    None
}

/// Find the control associated with a `<label>` element.
/// 1. Check `for` attribute → getElementById
/// 2. Otherwise find first labelable descendant
fn find_label_control(tree: &DomTree, label_id: NodeId) -> Option<NodeId> {
    // Check `for` attribute
    if let Some(for_id) = tree.get_attribute(label_id, "for") {
        if !for_id.is_empty() {
            return tree.get_element_by_id(&for_id);
        }
    }

    // Find first labelable descendant
    find_first_labelable_descendant(tree, label_id)
}

/// Find the first labelable descendant (input, select, textarea, button) in tree order.
fn find_first_labelable_descendant(tree: &DomTree, node_id: NodeId) -> Option<NodeId> {
    let node = tree.get_node(node_id);
    for &child_id in &node.children {
        let child = tree.get_node(child_id);
        if let NodeData::Element { tag_name, .. } = &child.data {
            let tag = tag_name.to_lowercase();
            if matches!(tag.as_str(), "input" | "select" | "textarea" | "button") {
                return Some(child_id);
            }
        }
        if let Some(found) = find_first_labelable_descendant(tree, child_id) {
            return Some(found);
        }
    }
    None
}

/// Find all radio inputs in the same radio group (same name attribute) under the
/// same form ancestor (or document root), excluding the given node.
fn find_radio_group_siblings(tree: &DomTree, node_id: NodeId, radio_name: &str) -> Vec<NodeId> {
    // Find the form ancestor, or use document root
    let group_root = find_ancestor_form(tree, node_id).unwrap_or(tree.document());

    let mut result = Vec::new();
    collect_radio_siblings(tree, group_root, node_id, radio_name, &mut result);
    result
}

fn collect_radio_siblings(
    tree: &DomTree,
    current: NodeId,
    exclude: NodeId,
    radio_name: &str,
    result: &mut Vec<NodeId>,
) {
    let node = tree.get_node(current);
    for &child_id in &node.children {
        if child_id == exclude {
            continue;
        }
        let child = tree.get_node(child_id);
        if let NodeData::Element {
            tag_name, attributes, ..
        } = &child.data
        {
            if tag_name.eq_ignore_ascii_case("input") {
                let is_radio = attributes
                    .iter()
                    .any(|a| a.local_name == "type" && a.value.eq_ignore_ascii_case("radio"));
                let name_matches = attributes
                    .iter()
                    .find(|a| a.local_name == "name")
                    .map(|a| a.value == radio_name)
                    .unwrap_or(radio_name.is_empty());
                if is_radio && name_matches {
                    result.push(child_id);
                }
            }
        }
        collect_radio_siblings(tree, child_id, exclude, radio_name, result);
    }
}
