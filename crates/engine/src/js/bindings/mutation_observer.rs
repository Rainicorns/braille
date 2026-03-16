//! MutationObserver and MutationRecord JS bindings.
//!
//! Provides the `MutationObserver` constructor (observe/disconnect/takeRecords)
//! and the infrastructure for queuing mutation records when DOM mutations occur.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use boa_engine::{
    js_string,
    native_function::NativeFunction,
    object::{builtins::JsArray, FunctionObjectBuilder, ObjectInitializer},
    property::{Attribute, PropertyDescriptor},
    Context, JsData, JsError, JsNativeError, JsObject, JsResult, JsValue,
};
use boa_gc::{Finalize, Trace};

use crate::dom::{DomTree, NodeData, NodeId};
use crate::js::realm_state;

use super::collections::create_static_nodelist;
use super::document::JsDocument;
use super::element::{get_or_create_js_element, JsElement};

// ---------------------------------------------------------------------------
// State structs
// ---------------------------------------------------------------------------

/// Options parsed from the `observe()` call.
#[derive(Debug, Clone)]
struct MutationObserverInit {
    child_list: bool,
    attributes: bool,
    character_data: bool,
    subtree: bool,
    attribute_old_value: bool,
    character_data_old_value: bool,
    attribute_filter: Option<Vec<String>>,
}

/// A pure-Rust mutation record captured at mutation time.
/// Converted to a JS MutationRecord object when delivered.
#[derive(Debug, Clone)]
pub(crate) struct RawMutationRecord {
    mutation_type: String,
    target_tree: Rc<RefCell<DomTree>>,
    target_node_id: NodeId,
    attribute_name: Option<String>,
    attribute_namespace: Option<String>,
    old_value: Option<String>,
    added_node_ids: Vec<NodeId>,
    removed_node_ids: Vec<NodeId>,
    previous_sibling_id: Option<NodeId>,
    next_sibling_id: Option<NodeId>,
}

/// One registered MutationObserver entry.
struct ObserverEntry {
    callback: JsObject,
    js_object: JsObject,
    pending_records: Vec<RawMutationRecord>,
}

/// Links a node to a specific observer with its options.
struct NodeRegistration {
    observer_index: usize,
    options: MutationObserverInit,
}

/// Top-level state for the MutationObserver subsystem.
pub(crate) struct MutationObserverState {
    observers: Vec<ObserverEntry>,
    /// Key is (tree_ptr as usize, node_id).
    registrations: HashMap<(usize, NodeId), Vec<NodeRegistration>>,
}

impl MutationObserverState {
    /// Create an empty `MutationObserverState`.
    pub(crate) fn new() -> Self {
        Self {
            observers: Vec::new(),
            registrations: HashMap::new(),
        }
    }
}

// (Thread-locals removed — state is now stored in RealmState via realm_state accessors)

// ---------------------------------------------------------------------------
// JsMutationObserver native data
// ---------------------------------------------------------------------------

#[derive(Debug, Trace, Finalize, JsData)]
pub(crate) struct JsMutationObserver {
    #[unsafe_ignore_trace]
    observer_index: usize,
}

// ---------------------------------------------------------------------------
// Registration: MutationObserver global constructor
// ---------------------------------------------------------------------------

pub(crate) fn register_mutation_observer_global(ctx: &mut Context) {
    // Build MutationObserver.prototype with observe, disconnect, takeRecords
    let proto = ObjectInitializer::new(ctx)
        .function(NativeFunction::from_fn_ptr(observe_fn), js_string!("observe"), 2)
        .function(NativeFunction::from_fn_ptr(disconnect_fn), js_string!("disconnect"), 0)
        .function(
            NativeFunction::from_fn_ptr(take_records_fn),
            js_string!("takeRecords"),
            0,
        )
        .build();

    // Build the constructor function
    let proto_clone = proto.clone();
    let ctor = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            // Validate callback (first arg must be callable)
            let callback =
                args.first()
                    .and_then(|v| v.as_object())
                    .filter(|o| o.is_callable())
                    .ok_or_else(|| {
                        JsError::from_native(JsNativeError::typ().with_message(
                            "Failed to construct 'MutationObserver': parameter 1 is not of type 'Function'",
                        ))
                    })?
                    .clone();

            // Create the observer entry (with a placeholder js_object)
            let placeholder = ObjectInitializer::new(ctx).build();
            let observer_index = {
                let state_rc = realm_state::mutation_observer_state(ctx);
                let mut state = state_rc.borrow_mut();
                let idx = state.observers.len();
                state.observers.push(ObserverEntry {
                    callback,
                    js_object: placeholder,
                    pending_records: Vec::new(),
                });
                idx
            };

            // Create the JS object with native data
            let obj = ObjectInitializer::with_native_data(JsMutationObserver { observer_index }, ctx).build();
            obj.set_prototype(Some(proto_clone.clone()));

            // Store the JsObject back in the entry
            {
                let state_rc = realm_state::mutation_observer_state(ctx);
                let mut state = state_rc.borrow_mut();
                state.observers[observer_index].js_object = obj.clone();
            }

            Ok(JsValue::from(obj))
        })
    };

    let ctor_fn = FunctionObjectBuilder::new(ctx.realm(), ctor)
        .name(js_string!("MutationObserver"))
        .length(1)
        .constructor(true)
        .build();

    ctor_fn
        .define_property_or_throw(
            js_string!("prototype"),
            PropertyDescriptor::builder()
                .value(proto.clone())
                .writable(false)
                .configurable(false)
                .enumerable(false)
                .build(),
            ctx,
        )
        .expect("define MutationObserver.prototype");

    proto
        .set(js_string!("constructor"), JsValue::from(ctor_fn.clone()), false, ctx)
        .expect("set constructor on MutationObserver.prototype");

    ctx.register_global_property(
        js_string!("MutationObserver"),
        ctor_fn,
        Attribute::WRITABLE | Attribute::CONFIGURABLE,
    )
    .expect("register MutationObserver global");
}

// ---------------------------------------------------------------------------
// Registration: MutationRecord global (non-constructable)
// ---------------------------------------------------------------------------

pub(crate) fn register_mutation_record_global(ctx: &mut Context) {
    let proto = ObjectInitializer::new(ctx).build();
    realm_state::set_mutation_record_proto(ctx, proto.clone());

    let ctor = NativeFunction::from_fn_ptr(|_this, _args, _ctx| {
        Err(JsError::from_native(
            JsNativeError::typ().with_message("Illegal constructor"),
        ))
    });
    let ctor_fn = FunctionObjectBuilder::new(ctx.realm(), ctor)
        .name(js_string!("MutationRecord"))
        .length(0)
        .constructor(true)
        .build();

    ctor_fn
        .define_property_or_throw(
            js_string!("prototype"),
            PropertyDescriptor::builder()
                .value(proto)
                .writable(false)
                .configurable(false)
                .enumerable(false)
                .build(),
            ctx,
        )
        .expect("define MutationRecord.prototype");

    ctx.register_global_property(
        js_string!("MutationRecord"),
        ctor_fn,
        Attribute::WRITABLE | Attribute::CONFIGURABLE,
    )
    .expect("register MutationRecord global");
}

// ---------------------------------------------------------------------------
// observe() method
// ---------------------------------------------------------------------------

fn observe_fn(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let this_obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("observe: this is not an object").into()))?;
    let mo = this_obj
        .downcast_ref::<JsMutationObserver>()
        .ok_or_else(|| JsError::from_opaque(js_string!("observe: this is not a MutationObserver").into()))?;
    let observer_index = mo.observer_index;

    // Get target node
    let target_val = args
        .first()
        .ok_or_else(|| JsError::from_native(JsNativeError::typ().with_message("observe: missing target")))?;
    let (target_node_id, target_tree) = extract_node_info(target_val)?;
    let tree_ptr = Rc::as_ptr(&target_tree) as usize;

    // Parse options
    let opts_val = args
        .get(1)
        .ok_or_else(|| JsError::from_native(JsNativeError::typ().with_message("observe: missing options")))?;
    let opts_obj = opts_val
        .as_object()
        .ok_or_else(|| JsError::from_native(JsNativeError::typ().with_message("observe: options must be an object")))?;

    let child_list = opts_obj.get(js_string!("childList"), ctx)?.to_boolean();

    let mut attributes = false;
    let mut attributes_explicitly_set = false;
    let attr_val = opts_obj.get(js_string!("attributes"), ctx)?;
    if !attr_val.is_undefined() {
        attributes = attr_val.to_boolean();
        attributes_explicitly_set = true;
    }

    let mut character_data = false;
    let mut character_data_explicitly_set = false;
    let cd_val = opts_obj.get(js_string!("characterData"), ctx)?;
    if !cd_val.is_undefined() {
        character_data = cd_val.to_boolean();
        character_data_explicitly_set = true;
    }

    let subtree = opts_obj.get(js_string!("subtree"), ctx)?.to_boolean();

    let mut attribute_old_value = false;
    let aov_val = opts_obj.get(js_string!("attributeOldValue"), ctx)?;
    if !aov_val.is_undefined() {
        attribute_old_value = aov_val.to_boolean();
    }

    let mut character_data_old_value = false;
    let cdov_val = opts_obj.get(js_string!("characterDataOldValue"), ctx)?;
    if !cdov_val.is_undefined() {
        character_data_old_value = cdov_val.to_boolean();
    }

    let mut attribute_filter: Option<Vec<String>> = None;
    let af_val = opts_obj.get(js_string!("attributeFilter"), ctx)?;
    if !af_val.is_undefined() && !af_val.is_null() {
        let af_obj = af_val.as_object().ok_or_else(|| {
            JsError::from_native(JsNativeError::typ().with_message("attributeFilter must be an array"))
        })?;
        let len = af_obj.get(js_string!("length"), ctx)?.to_u32(ctx)?;
        let mut filter = Vec::new();
        for i in 0..len {
            let item = af_obj.get(i, ctx)?;
            filter.push(item.to_string(ctx)?.to_std_string_escaped());
        }
        attribute_filter = Some(filter);
    }

    // Spec: attributeOldValue or attributeFilter implies attributes=true if not explicitly set
    if attribute_old_value && !attributes_explicitly_set {
        attributes = true;
    }
    if attribute_filter.is_some() && !attributes_explicitly_set {
        attributes = true;
    }
    // Spec: characterDataOldValue implies characterData=true if not explicitly set
    if character_data_old_value && !character_data_explicitly_set {
        character_data = true;
    }

    // Validation: at least one of childList, attributes, characterData must be true
    if !child_list && !attributes && !character_data {
        return Err(JsError::from_native(JsNativeError::typ().with_message(
            "Failed to execute 'observe' on 'MutationObserver': The options object must set at least one of 'attributes', 'characterData', or 'childList' to true.",
        )));
    }

    // Contradiction checks
    if attributes_explicitly_set && !attributes && attribute_old_value {
        return Err(JsError::from_native(JsNativeError::typ().with_message(
            "Failed to execute 'observe' on 'MutationObserver': The options object may not set 'attributeOldValue' to true when 'attributes' is false.",
        )));
    }
    if attributes_explicitly_set && !attributes && attribute_filter.is_some() {
        return Err(JsError::from_native(JsNativeError::typ().with_message(
            "Failed to execute 'observe' on 'MutationObserver': The options object may not set 'attributeFilter' when 'attributes' is false.",
        )));
    }
    if character_data_explicitly_set && !character_data && character_data_old_value {
        return Err(JsError::from_native(JsNativeError::typ().with_message(
            "Failed to execute 'observe' on 'MutationObserver': The options object may not set 'characterDataOldValue' to true when 'characterData' is false.",
        )));
    }

    let init = MutationObserverInit {
        child_list,
        attributes,
        character_data,
        subtree,
        attribute_old_value,
        character_data_old_value,
        attribute_filter,
    };

    // Add or replace registration
    {
        let state_rc = realm_state::mutation_observer_state(ctx);
        let mut state = state_rc.borrow_mut();

        let key = (tree_ptr, target_node_id);
        let regs = state.registrations.entry(key).or_default();

        // If this observer already observes this node, replace options
        if let Some(existing) = regs.iter_mut().find(|r| r.observer_index == observer_index) {
            existing.options = init;
        } else {
            regs.push(NodeRegistration {
                observer_index,
                options: init,
            });
        }
    }

    Ok(JsValue::undefined())
}

// ---------------------------------------------------------------------------
// disconnect() method
// ---------------------------------------------------------------------------

fn disconnect_fn(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let this_obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("disconnect: this is not an object").into()))?;
    let mo = this_obj
        .downcast_ref::<JsMutationObserver>()
        .ok_or_else(|| JsError::from_opaque(js_string!("disconnect: not a MutationObserver").into()))?;
    let observer_index = mo.observer_index;

    {
        let state_rc = realm_state::mutation_observer_state(ctx);
        let mut state = state_rc.borrow_mut();

        // Remove all registrations for this observer
        state.registrations.retain(|_key, regs| {
            regs.retain(|r| r.observer_index != observer_index);
            !regs.is_empty()
        });

        // Clear pending records
        if observer_index < state.observers.len() {
            state.observers[observer_index].pending_records.clear();
        }
    }

    Ok(JsValue::undefined())
}

// ---------------------------------------------------------------------------
// takeRecords() method
// ---------------------------------------------------------------------------

fn take_records_fn(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let this_obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("takeRecords: this is not an object").into()))?;
    let mo = this_obj
        .downcast_ref::<JsMutationObserver>()
        .ok_or_else(|| JsError::from_opaque(js_string!("takeRecords: not a MutationObserver").into()))?;
    let observer_index = mo.observer_index;

    let records = {
        let state_rc = realm_state::mutation_observer_state(ctx);
        let mut state = state_rc.borrow_mut();
        if observer_index < state.observers.len() {
            std::mem::take(&mut state.observers[observer_index].pending_records)
        } else {
            Vec::new()
        }
    };

    // Convert to JS array
    let arr = JsArray::new(ctx);
    for record in records {
        let js_rec = raw_record_to_js(&record, ctx)?;
        arr.push(js_rec, ctx)?;
    }

    Ok(JsValue::from(arr))
}

// ---------------------------------------------------------------------------
// Helper: extract (NodeId, tree) from a JsValue that is a Node
// ---------------------------------------------------------------------------

fn extract_node_info(val: &JsValue) -> JsResult<(NodeId, Rc<RefCell<DomTree>>)> {
    let obj = val
        .as_object()
        .ok_or_else(|| JsError::from_native(JsNativeError::typ().with_message("Not a Node")))?;
    if let Some(el) = obj.downcast_ref::<JsElement>() {
        return Ok((el.node_id, el.tree.clone()));
    }
    if let Some(doc) = obj.downcast_ref::<JsDocument>() {
        let doc_id = doc.tree.borrow().document();
        return Ok((doc_id, doc.tree.clone()));
    }
    Err(JsError::from_native(JsNativeError::typ().with_message("Not a Node")))
}

// ---------------------------------------------------------------------------
// Convert a RawMutationRecord to a JS MutationRecord object
// ---------------------------------------------------------------------------

fn raw_record_to_js(record: &RawMutationRecord, ctx: &mut Context) -> JsResult<JsValue> {
    let proto = realm_state::mutation_record_proto(ctx);

    let target = get_or_create_js_element(record.target_node_id, record.target_tree.clone(), ctx)?;
    let added = create_static_nodelist(record.added_node_ids.clone(), record.target_tree.clone(), ctx)?;
    let removed = create_static_nodelist(record.removed_node_ids.clone(), record.target_tree.clone(), ctx)?;

    let prev_sib = match record.previous_sibling_id {
        Some(id) => JsValue::from(get_or_create_js_element(id, record.target_tree.clone(), ctx)?),
        None => JsValue::null(),
    };
    let next_sib = match record.next_sibling_id {
        Some(id) => JsValue::from(get_or_create_js_element(id, record.target_tree.clone(), ctx)?),
        None => JsValue::null(),
    };

    let attr_name = record
        .attribute_name
        .as_ref()
        .map(|s| JsValue::from(js_string!(s.as_str())))
        .unwrap_or(JsValue::null());
    let attr_ns = record
        .attribute_namespace
        .as_ref()
        .map(|s| JsValue::from(js_string!(s.as_str())))
        .unwrap_or(JsValue::null());
    let old_value = record
        .old_value
        .as_ref()
        .map(|s| JsValue::from(js_string!(s.as_str())))
        .unwrap_or(JsValue::null());

    let props = Attribute::CONFIGURABLE | Attribute::ENUMERABLE;
    let obj = ObjectInitializer::new(ctx)
        .property(js_string!("type"), js_string!(record.mutation_type.as_str()), props)
        .property(js_string!("target"), target, props)
        .property(js_string!("addedNodes"), added, props)
        .property(js_string!("removedNodes"), removed, props)
        .property(js_string!("previousSibling"), prev_sib, props)
        .property(js_string!("nextSibling"), next_sib, props)
        .property(js_string!("attributeName"), attr_name, props)
        .property(js_string!("attributeNamespace"), attr_ns, props)
        .property(js_string!("oldValue"), old_value, props)
        .build();

    if let Some(p) = proto {
        obj.set_prototype(Some(p));
    }

    Ok(JsValue::from(obj))
}

// ---------------------------------------------------------------------------
// notify_mutation_observers: deliver pending records to callbacks
// ---------------------------------------------------------------------------

pub(crate) fn notify_mutation_observers(ctx: &mut Context) {
    // Collect observers with pending records
    let observers_to_notify: Vec<(JsObject, Vec<RawMutationRecord>, JsObject)> = {
        let state_rc = realm_state::mutation_observer_state(ctx);
        let mut state = state_rc.borrow_mut();
        let mut result = Vec::new();
        for entry in state.observers.iter_mut() {
            if !entry.pending_records.is_empty() {
                let records = std::mem::take(&mut entry.pending_records);
                result.push((entry.callback.clone(), records, entry.js_object.clone()));
            }
        }
        result
    };

    for (callback, records, observer_obj) in observers_to_notify {
        let arr = JsArray::new(ctx);
        for record in &records {
            if let Ok(js_rec) = raw_record_to_js(record, ctx) {
                let _ = arr.push(js_rec, ctx);
            }
        }

        // Call callback(records, observer)
        let _ = callback.call(
            &JsValue::from(observer_obj.clone()),
            &[JsValue::from(arr), JsValue::from(observer_obj)],
            ctx,
        );
    }
}

// ---------------------------------------------------------------------------
// collect_interested_observers: walk ancestors looking for registrations
// ---------------------------------------------------------------------------

fn collect_interested_observers(
    state: &MutationObserverState,
    tree: &Rc<RefCell<DomTree>>,
    node_id: NodeId,
    tree_ptr: usize,
    filter: impl Fn(&MutationObserverInit) -> bool,
) -> Vec<(usize, bool)> {
    // Returns Vec<(observer_index, should_capture_old_value)>
    let borrowed = tree.borrow();
    let mut result: Vec<(usize, bool)> = Vec::new();
    let mut seen_observers: HashSet<usize> = HashSet::new();

    // Walk from node_id up to root, checking registrations at each level
    let mut current = Some(node_id);
    let mut is_direct = true; // first iteration = direct target
    while let Some(cur_id) = current {
        let key = (tree_ptr, cur_id);
        if let Some(regs) = state.registrations.get(&key) {
            for reg in regs {
                // Direct target: always matches if filter passes
                // Ancestor: only matches if subtree is set
                if (is_direct || reg.options.subtree)
                    && filter(&reg.options)
                    && seen_observers.insert(reg.observer_index)
                {
                    let capture_old = reg.options.attribute_old_value || reg.options.character_data_old_value;
                    result.push((reg.observer_index, capture_old));
                }
            }
        }
        current = borrowed.get_node(cur_id).parent;
        is_direct = false;
    }

    result
}

// ---------------------------------------------------------------------------
// Queue functions (internal)
// ---------------------------------------------------------------------------

fn queue_attributes_mutation(
    ctx: &Context,
    tree: &Rc<RefCell<DomTree>>,
    node_id: NodeId,
    attr_name: &str,
    attr_namespace: Option<&str>,
    old_value: Option<String>,
) {
    let tree_ptr = Rc::as_ptr(tree) as usize;

    let state_rc = realm_state::mutation_observer_state(ctx);
    let mut state = state_rc.borrow_mut();

    let interested = collect_interested_observers(&state, tree, node_id, tree_ptr, |opts| {
        if !opts.attributes {
            return false;
        }
        if let Some(ref filter) = opts.attribute_filter {
            if !filter.iter().any(|f| f == attr_name) {
                return false;
            }
        }
        true
    });

    for (obs_idx, capture_old) in interested {
        let record = RawMutationRecord {
            mutation_type: "attributes".to_string(),
            target_tree: tree.clone(),
            target_node_id: node_id,
            attribute_name: Some(attr_name.to_string()),
            attribute_namespace: attr_namespace.map(|s| s.to_string()),
            old_value: if capture_old { old_value.clone() } else { None },
            added_node_ids: Vec::new(),
            removed_node_ids: Vec::new(),
            previous_sibling_id: None,
            next_sibling_id: None,
        };
        state.observers[obs_idx].pending_records.push(record);
    }
}

fn queue_character_data_mutation(
    ctx: &Context,
    tree: &Rc<RefCell<DomTree>>,
    node_id: NodeId,
    old_value: Option<String>,
) {
    let tree_ptr = Rc::as_ptr(tree) as usize;

    let state_rc = realm_state::mutation_observer_state(ctx);
    let mut state = state_rc.borrow_mut();

    let interested = collect_interested_observers(&state, tree, node_id, tree_ptr, |opts| opts.character_data);

    for (obs_idx, capture_old) in interested {
        let record = RawMutationRecord {
            mutation_type: "characterData".to_string(),
            target_tree: tree.clone(),
            target_node_id: node_id,
            attribute_name: None,
            attribute_namespace: None,
            old_value: if capture_old { old_value.clone() } else { None },
            added_node_ids: Vec::new(),
            removed_node_ids: Vec::new(),
            previous_sibling_id: None,
            next_sibling_id: None,
        };
        state.observers[obs_idx].pending_records.push(record);
    }
}

/// Queue a childList mutation record for interested observers.
pub(crate) fn queue_childlist_mutation(
    ctx: &Context,
    tree: &Rc<RefCell<DomTree>>,
    parent_id: NodeId,
    added_ids: Vec<NodeId>,
    removed_ids: Vec<NodeId>,
    prev_sibling: Option<NodeId>,
    next_sibling: Option<NodeId>,
) {
    let tree_ptr = Rc::as_ptr(tree) as usize;

    let state_rc = realm_state::mutation_observer_state(ctx);
    let mut state = state_rc.borrow_mut();

    let interested = collect_interested_observers(&state, tree, parent_id, tree_ptr, |opts| opts.child_list);

    for (obs_idx, _) in interested {
        let record = RawMutationRecord {
            mutation_type: "childList".to_string(),
            target_tree: tree.clone(),
            target_node_id: parent_id,
            attribute_name: None,
            attribute_namespace: None,
            old_value: None,
            added_node_ids: added_ids.clone(),
            removed_node_ids: removed_ids.clone(),
            previous_sibling_id: prev_sibling,
            next_sibling_id: next_sibling,
        };
        state.observers[obs_idx].pending_records.push(record);
    }
}

// ---------------------------------------------------------------------------
// Attribute wrapper functions
// ---------------------------------------------------------------------------

pub(crate) fn set_attribute_with_observer(
    ctx: &Context,
    tree: &Rc<RefCell<DomTree>>,
    node_id: NodeId,
    name: &str,
    value: &str,
) {
    let old_value = tree.borrow().get_attribute(node_id, name).map(|s| s.to_string());
    tree.borrow_mut().set_attribute(node_id, name, value);
    queue_attributes_mutation(ctx, tree, node_id, name, None, old_value);
}

pub(crate) fn remove_attribute_with_observer(ctx: &Context, tree: &Rc<RefCell<DomTree>>, node_id: NodeId, name: &str) {
    let old_value = tree.borrow().get_attribute(node_id, name).map(|s| s.to_string());
    tree.borrow_mut().remove_attribute(node_id, name);
    // Only queue if attribute actually existed
    if old_value.is_some() {
        queue_attributes_mutation(ctx, tree, node_id, name, None, old_value);
    }
}

pub(crate) fn set_attribute_ns_with_observer(
    ctx: &Context,
    tree: &Rc<RefCell<DomTree>>,
    node_id: NodeId,
    namespace: &str,
    qualified_name: &str,
    value: &str,
) {
    // Extract local name for old value lookup
    let local_name = if let Some(pos) = qualified_name.find(':') {
        &qualified_name[pos + 1..]
    } else {
        qualified_name
    };
    let old_value = {
        let borrowed = tree.borrow();
        let node = borrowed.get_node(node_id);
        if let NodeData::Element { attributes, .. } = &node.data {
            attributes
                .iter()
                .find(|a| a.local_name == local_name && a.namespace == namespace)
                .map(|a| a.value.clone())
        } else {
            None
        }
    };
    tree.borrow_mut()
        .set_attribute_ns(node_id, namespace, qualified_name, value);
    queue_attributes_mutation(ctx, tree, node_id, local_name, Some(namespace), old_value);
}

pub(crate) fn remove_attribute_ns_with_observer(
    ctx: &Context,
    tree: &Rc<RefCell<DomTree>>,
    node_id: NodeId,
    namespace: &str,
    local_name: &str,
) {
    let old_value = {
        let borrowed = tree.borrow();
        let node = borrowed.get_node(node_id);
        if let NodeData::Element { attributes, .. } = &node.data {
            attributes
                .iter()
                .find(|a| a.local_name == local_name && a.namespace == namespace)
                .map(|a| a.value.clone())
        } else {
            None
        }
    };
    tree.borrow_mut().remove_attribute_ns(node_id, namespace, local_name);
    if old_value.is_some() {
        queue_attributes_mutation(ctx, tree, node_id, local_name, Some(namespace), old_value);
    }
}

// ---------------------------------------------------------------------------
// CharacterData wrapper functions
// ---------------------------------------------------------------------------

pub(crate) fn character_data_set_with_observer(
    ctx: &Context,
    tree: &Rc<RefCell<DomTree>>,
    node_id: NodeId,
    data: &str,
) {
    let old_value = tree.borrow().character_data_get(node_id);
    tree.borrow_mut().character_data_set(node_id, data);
    queue_character_data_mutation(ctx, tree, node_id, old_value);
}

pub(crate) fn character_data_append_with_observer(
    ctx: &Context,
    tree: &Rc<RefCell<DomTree>>,
    node_id: NodeId,
    data: &str,
) {
    let old_value = tree.borrow().character_data_get(node_id);
    tree.borrow_mut().character_data_append(node_id, data);
    queue_character_data_mutation(ctx, tree, node_id, old_value);
}

pub(crate) fn character_data_delete_with_observer(
    ctx: &Context,
    tree: &Rc<RefCell<DomTree>>,
    node_id: NodeId,
    offset: usize,
    count: usize,
) -> Result<(), &'static str> {
    let old_value = tree.borrow().character_data_get(node_id);
    let result = tree.borrow_mut().character_data_delete(node_id, offset, count);
    if result.is_ok() {
        queue_character_data_mutation(ctx, tree, node_id, old_value);
    }
    result
}

pub(crate) fn character_data_insert_with_observer(
    ctx: &Context,
    tree: &Rc<RefCell<DomTree>>,
    node_id: NodeId,
    offset: usize,
    data: &str,
) -> Result<(), &'static str> {
    let old_value = tree.borrow().character_data_get(node_id);
    let result = tree.borrow_mut().character_data_insert(node_id, offset, data);
    if result.is_ok() {
        queue_character_data_mutation(ctx, tree, node_id, old_value);
    }
    result
}

pub(crate) fn character_data_replace_with_observer(
    ctx: &Context,
    tree: &Rc<RefCell<DomTree>>,
    node_id: NodeId,
    offset: usize,
    count: usize,
    data: &str,
) -> Result<(), &'static str> {
    let old_value = tree.borrow().character_data_get(node_id);
    let result = tree.borrow_mut().character_data_replace(node_id, offset, count, data);
    if result.is_ok() {
        queue_character_data_mutation(ctx, tree, node_id, old_value);
    }
    result
}
