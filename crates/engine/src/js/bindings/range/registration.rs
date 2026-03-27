//! Range prototype construction, global registration, and factory functions.

use std::cell::Cell;
use std::cell::RefCell;
use std::rc::Rc;

use boa_engine::{
    js_string, native_function::NativeFunction,
    Context, JsObject, JsResult, JsValue,
};

use crate::dom::{DomTree, NodeId};
use crate::js::prop_desc;
use super::types::{JsRange, RangeInner};
use super::methods;

// ---------------------------------------------------------------------------
// Range.prototype — shared across all Range instances
// ---------------------------------------------------------------------------

pub(crate) fn create_range_prototype(ctx: &mut Context) -> JsObject {
    let realm = ctx.realm().clone();
    let proto = JsObject::with_null_proto();

    use prop_desc::{data_prop, readonly_accessor, readonly_constant};

    let method = |f: fn(&JsValue, &[JsValue], &mut Context) -> JsResult<JsValue>, _name: &str, _len: usize| {
        NativeFunction::from_fn_ptr(f).to_js_function(&realm)
    };

    // Methods
    type MethodEntry = (&'static str, fn(&JsValue, &[JsValue], &mut Context) -> JsResult<JsValue>, usize);
    let methods_list: &[MethodEntry] = &[
        ("setStart", methods::range_set_start, 2),
        ("setEnd", methods::range_set_end, 2),
        ("setStartBefore", methods::range_set_start_before, 1),
        ("setStartAfter", methods::range_set_start_after, 1),
        ("setEndBefore", methods::range_set_end_before, 1),
        ("setEndAfter", methods::range_set_end_after, 1),
        ("collapse", methods::range_collapse, 0),
        ("cloneRange", methods::range_clone_range, 0),
        ("selectNode", methods::range_select_node, 1),
        ("selectNodeContents", methods::range_select_node_contents, 1),
        ("deleteContents", methods::range_delete_contents, 0),
        ("extractContents", methods::range_extract_contents, 0),
        ("cloneContents", methods::range_clone_contents, 0),
        ("insertNode", methods::range_insert_node, 1),
        ("surroundContents", methods::range_surround_contents, 1),
        ("compareBoundaryPoints", methods::range_compare_boundary_points, 2),
        ("comparePoint", methods::range_compare_point, 2),
        ("isPointInRange", methods::range_is_point_in_range, 2),
        ("intersectsNode", methods::range_intersects_node, 1),
        ("detach", methods::range_detach, 0),
        ("toString", methods::range_to_string, 0),
    ];

    for &(name, f, len) in methods_list {
        proto
            .define_property_or_throw(js_string!(name), data_prop(method(f, name, len)), ctx)
            .expect("define Range method");
    }

    // Readonly accessor properties
    type AccessorEntry = (&'static str, fn(&JsValue, &[JsValue], &mut Context) -> JsResult<JsValue>);
    let accessors: &[AccessorEntry] = &[
        ("startContainer", methods::range_start_container),
        ("startOffset", methods::range_start_offset),
        ("endContainer", methods::range_end_container),
        ("endOffset", methods::range_end_offset),
        ("collapsed", methods::range_collapsed),
        ("commonAncestorContainer", methods::range_common_ancestor_container),
    ];

    for &(name, f) in accessors {
        let getter = NativeFunction::from_fn_ptr(f).to_js_function(&realm);
        proto
            .define_property_or_throw(js_string!(name), readonly_accessor(getter), ctx)
            .expect("define Range accessor");
    }

    // Constants on prototype
    proto.define_property_or_throw(js_string!("START_TO_START"), readonly_constant(0), ctx).expect("const");
    proto.define_property_or_throw(js_string!("START_TO_END"), readonly_constant(1), ctx).expect("const");
    proto.define_property_or_throw(js_string!("END_TO_END"), readonly_constant(2), ctx).expect("const");
    proto.define_property_or_throw(js_string!("END_TO_START"), readonly_constant(3), ctx).expect("const");

    proto
}

// ---------------------------------------------------------------------------
// Register Range global constructor
// ---------------------------------------------------------------------------

pub(crate) fn register_range_global(ctx: &mut Context) {
    use boa_engine::object::FunctionObjectBuilder;
    use boa_engine::property::Attribute;

    let tree = crate::js::realm_state::dom_tree(ctx);
    let proto = create_range_prototype(ctx);

    // Store prototype in RealmState
    crate::js::realm_state::set_range_proto(ctx, proto.clone());

    // Range constructor: new Range() creates range at (document, 0)
    let tree_for_ctor = tree.clone();
    let proto_for_ctor = proto.clone();
    let ctor = unsafe {
        NativeFunction::from_closure(move |_this, _args, ctx2| {
            let doc_id = tree_for_ctor.borrow().document();
            let obj = create_range_with_bounds(tree_for_ctor.clone(), doc_id, 0, doc_id, 0, ctx2)?;
            obj.set_prototype(Some(proto_for_ctor.clone()));
            Ok(obj.into())
        })
    };

    let ctor_fn = FunctionObjectBuilder::new(ctx.realm(), ctor)
        .name(js_string!("Range"))
        .length(0)
        .constructor(true)
        .build();

    // Set Range.prototype
    ctor_fn
        .define_property_or_throw(js_string!("prototype"), prop_desc::prototype_on_ctor(proto.clone()), ctx)
        .expect("Range.prototype");

    // Constants on constructor
    ctor_fn.define_property_or_throw(js_string!("START_TO_START"), prop_desc::readonly_constant(0), ctx).expect("const");
    ctor_fn.define_property_or_throw(js_string!("START_TO_END"), prop_desc::readonly_constant(1), ctx).expect("const");
    ctor_fn.define_property_or_throw(js_string!("END_TO_END"), prop_desc::readonly_constant(2), ctx).expect("const");
    ctor_fn.define_property_or_throw(js_string!("END_TO_START"), prop_desc::readonly_constant(3), ctx).expect("const");

    // Set constructor on prototype
    proto
        .define_property_or_throw(js_string!("constructor"), prop_desc::constructor_on_proto(ctor_fn.clone()), ctx)
        .expect("proto.constructor");

    // Register as global
    ctx.register_global_property(js_string!("Range"), ctor_fn, Attribute::WRITABLE | Attribute::CONFIGURABLE)
        .expect("register Range global");
}

// ---------------------------------------------------------------------------
// Factory: create_range() — used by document.createRange()
// ---------------------------------------------------------------------------

pub(crate) fn create_range(
    tree: Rc<RefCell<DomTree>>,
    document_id: NodeId,
    ctx: &mut Context,
) -> JsResult<JsObject> {
    let obj = create_range_with_bounds(tree, document_id, 0, document_id, 0, ctx)?;

    // Set prototype if available
    if let Some(proto) = crate::js::realm_state::range_proto(ctx) {
        obj.set_prototype(Some(proto));
    }

    Ok(obj)
}

// ---------------------------------------------------------------------------
// Factory: create_range_with_bounds
// ---------------------------------------------------------------------------

/// Creates a Range JsObject with specified boundaries.
/// Registers the range's inner state in the live-range registry so that
/// DOM mutations can update boundaries automatically.
pub(super) fn create_range_with_bounds(
    tree: Rc<RefCell<DomTree>>,
    start_node: NodeId,
    start_offset: usize,
    end_node: NodeId,
    end_offset: usize,
    ctx: &mut Context,
) -> JsResult<JsObject> {
    let inner = Rc::new(RangeInner {
        start_node: Cell::new(start_node),
        start_offset: Cell::new(start_offset),
        end_node: Cell::new(end_node),
        end_offset: Cell::new(end_offset),
    });

    // Register in live-range registry (weak ref so JS GC can collect)
    let registry = crate::js::realm_state::live_ranges(ctx);
    let mut reg = registry.borrow_mut();
    // Periodic cleanup: drop dead weak refs
    if reg.len() > 64 {
        reg.retain(|w| w.strong_count() > 0);
    }
    reg.push(Rc::downgrade(&inner));
    drop(reg);

    let range_data = JsRange {
        tree,
        inner,
    };

    let obj = boa_engine::object::ObjectInitializer::with_native_data(range_data, ctx).build();

    // Set prototype if available
    if let Some(proto) = crate::js::realm_state::range_proto(ctx) {
        obj.set_prototype(Some(proto));
    }

    Ok(obj)
}
