//! Per-realm state stored in Boa's `Realm::host_defined()`.
//!
//! Each iframe gets its own Boa Realm with an independent `RealmState`.
//! Native functions read state via `ctx.realm().host_defined().get::<RealmState>()`.
//! Accessor functions clone the `Rc` (or value) out immediately, releasing the
//! `host_defined()` borrow so callers never hold it across further Boa calls.


use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Instant;

use boa_engine::{Context, JsObject, JsValue};
use boa_engine::realm::Realm;
use boa_gc::{Finalize, Trace};

use crate::dom::{DomTree, NodeId};

use super::bindings::element::{DomPrototypes, NodeCache};
use super::bindings::event_target::ListenerMap;

// ---------------------------------------------------------------------------
// Type aliases (match existing crate conventions)
// ---------------------------------------------------------------------------

/// Key for on* IDL event handlers: (tree_ptr, node_id, event_name).
type OnEventKey = (usize, NodeId, String);
type OnEventMap = HashMap<OnEventKey, JsObject>;

/// Key for collection caches: (tree_ptr, node_id).
type CollectionCacheKey = (usize, NodeId);

// Re-export MutationObserverState so mutation_observer.rs can reference it.
pub(crate) use super::bindings::mutation_observer::MutationObserverState;

// ---------------------------------------------------------------------------
// RealmState — the per-realm state struct
// ---------------------------------------------------------------------------

/// All per-realm mutable state for one JS realm (main document or iframe).
///
/// Stored in `realm.host_defined()` via `HostDefined::insert()`.
/// Native functions access fields through the accessor functions below,
/// which clone the `Rc` out to release the `host_defined()` borrow.
///
/// GC safety: fields use `#[unsafe_ignore_trace]` — matching the current
/// thread-local behavior (thread-locals aren't GC-traced either). JsObjects
/// in our maps are also reachable through JS global scope.
#[derive(Trace, Finalize, boa_engine::JsData)]
pub(crate) struct RealmState {
    // -- Data stores (Rc<RefCell<...>> for independent borrowing) --
    #[unsafe_ignore_trace]
    pub(crate) node_cache: Rc<RefCell<NodeCache>>,

    #[unsafe_ignore_trace]
    pub(crate) event_listeners: Rc<RefCell<ListenerMap>>,

    #[unsafe_ignore_trace]
    pub(crate) on_event_handlers: Rc<RefCell<OnEventMap>>,

    #[unsafe_ignore_trace]
    #[allow(clippy::type_complexity)]
    pub(crate) iframe_content_docs: Rc<RefCell<HashMap<(usize, NodeId), Rc<RefCell<DomTree>>>>>,

    #[unsafe_ignore_trace]
    pub(crate) iframe_src_content: Rc<RefCell<HashMap<String, String>>>,

    #[unsafe_ignore_trace]
    pub(crate) mutation_observer_state: Rc<RefCell<MutationObserverState>>,

    #[unsafe_ignore_trace]
    pub(crate) child_nodes_cache: Rc<RefCell<HashMap<CollectionCacheKey, JsObject>>>,

    #[unsafe_ignore_trace]
    pub(crate) children_cache: Rc<RefCell<HashMap<CollectionCacheKey, JsObject>>>,

    // -- Prototype/factory caches (RefCell<Option<...>>) --
    #[unsafe_ignore_trace]
    pub(crate) dom_prototypes: RefCell<Option<DomPrototypes>>,

    #[unsafe_ignore_trace]
    pub(crate) nodelist_proto: RefCell<Option<JsObject>>,

    #[unsafe_ignore_trace]
    pub(crate) htmlcollection_proto: RefCell<Option<JsObject>>,

    #[unsafe_ignore_trace]
    pub(crate) nl_proxy_factory: RefCell<Option<JsObject>>,

    #[unsafe_ignore_trace]
    pub(crate) hc_proxy_factory: RefCell<Option<JsObject>>,

    #[unsafe_ignore_trace]
    pub(crate) domimpl_proto: RefCell<Option<JsObject>>,

    #[unsafe_ignore_trace]
    pub(crate) mutation_record_proto: RefCell<Option<JsObject>>,

    #[unsafe_ignore_trace]
    pub(crate) is_trusted_getter: RefCell<Option<JsObject>>,

    // -- Singleton state --
    #[unsafe_ignore_trace]
    pub(crate) dom_tree: Rc<RefCell<DomTree>>,

    #[unsafe_ignore_trace]
    pub(crate) window_object: RefCell<Option<JsObject>>,

    #[unsafe_ignore_trace]
    pub(crate) current_event: RefCell<Option<JsObject>>,

    #[unsafe_ignore_trace]
    pub(crate) dispatch_target: RefCell<Option<JsValue>>,

    #[unsafe_ignore_trace]
    pub(crate) creation_time: Instant,
}

impl RealmState {
    /// Create a new `RealmState` for a realm backed by the given `DomTree`.
    pub(crate) fn new(tree: Rc<RefCell<DomTree>>) -> Self {
        Self {
            node_cache: Rc::new(RefCell::new(HashMap::new())),
            event_listeners: Rc::new(RefCell::new(HashMap::new())),
            on_event_handlers: Rc::new(RefCell::new(HashMap::new())),
            iframe_content_docs: Rc::new(RefCell::new(HashMap::new())),
            iframe_src_content: Rc::new(RefCell::new(HashMap::new())),
            mutation_observer_state: Rc::new(RefCell::new(MutationObserverState::new())),
            child_nodes_cache: Rc::new(RefCell::new(HashMap::new())),
            children_cache: Rc::new(RefCell::new(HashMap::new())),
            dom_prototypes: RefCell::new(None),
            nodelist_proto: RefCell::new(None),
            htmlcollection_proto: RefCell::new(None),
            nl_proxy_factory: RefCell::new(None),
            hc_proxy_factory: RefCell::new(None),
            domimpl_proto: RefCell::new(None),
            mutation_record_proto: RefCell::new(None),
            is_trusted_getter: RefCell::new(None),
            dom_tree: tree,
            window_object: RefCell::new(None),
            current_event: RefCell::new(None),
            dispatch_target: RefCell::new(None),
            creation_time: Instant::now(),
        }
    }
}

// ---------------------------------------------------------------------------
// Accessor functions — clone Rc or value out, releasing host_defined() borrow
// ---------------------------------------------------------------------------
//
// Pattern: get the GcRef<HostDefined>, extract the field, drop the GcRef.
// For `Rc<RefCell<...>>` fields: `.clone()` copies the Rc (cheap).
// For `RefCell<Option<T>>` fields: save to a local so the temporary `Ref`
// is dropped before the `GcRef`, avoiding Rust 2021 drop-order issues.

macro_rules! rc_accessor {
    ($name:ident, $field:ident, $ty:ty) => {
        #[allow(clippy::type_complexity)]
        pub(crate) fn $name(ctx: &Context) -> $ty {
            let hd = ctx.realm().host_defined();
            hd.get::<RealmState>().expect("RealmState not initialized").$field.clone()
        }
    };
}

macro_rules! option_accessor {
    ($getter:ident, $setter:ident, $field:ident, $ty:ty) => {
        pub(crate) fn $getter(ctx: &Context) -> Option<$ty> {
            let hd = ctx.realm().host_defined();
            let val = hd.get::<RealmState>().expect("RealmState not initialized").$field.borrow().clone();
            val
        }
        pub(crate) fn $setter(ctx: &Context, v: $ty) {
            let hd = ctx.realm().host_defined();
            let state = hd.get::<RealmState>().expect("RealmState not initialized");
            *state.$field.borrow_mut() = Some(v);
        }
    };
}

// -- Data store accessors (clone Rc out) --

rc_accessor!(node_cache, node_cache, Rc<RefCell<NodeCache>>);
rc_accessor!(event_listeners, event_listeners, Rc<RefCell<ListenerMap>>);
rc_accessor!(on_event_handlers, on_event_handlers, Rc<RefCell<OnEventMap>>);
rc_accessor!(iframe_content_docs, iframe_content_docs, Rc<RefCell<HashMap<(usize, NodeId), Rc<RefCell<DomTree>>>>>);
rc_accessor!(iframe_src_content, iframe_src_content, Rc<RefCell<HashMap<String, String>>>);
rc_accessor!(mutation_observer_state, mutation_observer_state, Rc<RefCell<MutationObserverState>>);
rc_accessor!(child_nodes_cache, child_nodes_cache, Rc<RefCell<HashMap<CollectionCacheKey, JsObject>>>);
rc_accessor!(children_cache, children_cache, Rc<RefCell<HashMap<CollectionCacheKey, JsObject>>>);
rc_accessor!(dom_tree, dom_tree, Rc<RefCell<DomTree>>);

// -- Prototype/factory cache accessors (clone Option<T> out) --

option_accessor!(nodelist_proto, set_nodelist_proto, nodelist_proto, JsObject);
option_accessor!(htmlcollection_proto, set_htmlcollection_proto, htmlcollection_proto, JsObject);
option_accessor!(nl_proxy_factory, set_nl_proxy_factory, nl_proxy_factory, JsObject);
option_accessor!(hc_proxy_factory, set_hc_proxy_factory, hc_proxy_factory, JsObject);
option_accessor!(domimpl_proto, set_domimpl_proto, domimpl_proto, JsObject);
option_accessor!(mutation_record_proto, set_mutation_record_proto, mutation_record_proto, JsObject);
option_accessor!(is_trusted_getter, set_is_trusted_getter, is_trusted_getter, JsObject);
option_accessor!(window_object, set_window_object, window_object, JsObject);

// -- DomPrototypes (special: clone returns the whole struct) --

pub(crate) fn dom_prototypes(ctx: &Context) -> Option<DomPrototypes> {
    let hd = ctx.realm().host_defined();
    let val = hd.get::<RealmState>().expect("RealmState not initialized").dom_prototypes.borrow().clone();
    val
}

pub(crate) fn set_dom_prototypes(ctx: &Context, protos: DomPrototypes) {
    let hd = ctx.realm().host_defined();
    let state = hd.get::<RealmState>().expect("RealmState not initialized");
    *state.dom_prototypes.borrow_mut() = Some(protos);
}

// -- Singleton state accessors --

pub(crate) fn current_event(ctx: &Context) -> Option<JsObject> {
    let hd = ctx.realm().host_defined();
    let val = hd.get::<RealmState>().expect("RealmState not initialized").current_event.borrow().clone();
    val
}

pub(crate) fn set_current_event(ctx: &Context, event: Option<JsObject>) {
    let hd = ctx.realm().host_defined();
    let state = hd.get::<RealmState>().expect("RealmState not initialized");
    *state.current_event.borrow_mut() = event;
}

pub(crate) fn dispatch_target(ctx: &Context) -> Option<JsValue> {
    let hd = ctx.realm().host_defined();
    let val = hd.get::<RealmState>().expect("RealmState not initialized").dispatch_target.borrow().clone();
    val
}

pub(crate) fn set_dispatch_target(ctx: &Context, target: Option<JsValue>) {
    let hd = ctx.realm().host_defined();
    let state = hd.get::<RealmState>().expect("RealmState not initialized");
    *state.dispatch_target.borrow_mut() = target;
}

pub(crate) fn creation_time(ctx: &Context) -> Instant {
    let hd = ctx.realm().host_defined();
    hd.get::<RealmState>().expect("RealmState not initialized").creation_time
}

// ---------------------------------------------------------------------------
// with_realm — execute a closure in a different realm's context
// ---------------------------------------------------------------------------

#[allow(dead_code)] // Will be used in Phase 5 (iframe realm creation)
pub(crate) fn with_realm<F, R>(ctx: &mut Context, realm: &Realm, f: F) -> R
where
    F: FnOnce(&mut Context) -> R,
{
    let old_realm = ctx.enter_realm(realm.clone());
    let result = f(ctx);
    ctx.enter_realm(old_realm);
    result
}
