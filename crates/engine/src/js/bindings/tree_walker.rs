use std::cell::{Cell, RefCell};
use std::rc::Rc;

use boa_engine::{
    js_string,
    native_function::NativeFunction,
    object::ObjectInitializer,
    property::Attribute,
    Context, JsData, JsResult, JsValue,
};
use boa_gc::{Finalize, Trace};

use crate::dom::{DomTree, NodeId};

use super::element::get_or_create_js_element;

/// Native data for TreeWalker instances.
#[derive(Debug, Trace, Finalize, JsData)]
pub(crate) struct JsTreeWalker {
    #[unsafe_ignore_trace]
    tree: Rc<RefCell<DomTree>>,
    #[unsafe_ignore_trace]
    root: NodeId,
    #[unsafe_ignore_trace]
    current_node: Cell<NodeId>,
    what_to_show: u32,
    filter: JsValue,
}

/// Register `document.createTreeWalker(root, whatToShow?, filter?)`.
pub(crate) fn register_create_tree_walker(
    doc_obj: &boa_engine::JsObject,
    tree: Rc<RefCell<DomTree>>,
    ctx: &mut Context,
) {
    let tree_for_closure = tree;
    let create_tw = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            // root argument is required
            let root_val = args.first().ok_or_else(|| {
                boa_engine::JsError::from_native(
                    boa_engine::JsNativeError::typ()
                        .with_message("Failed to execute 'createTreeWalker': 1 argument required"),
                )
            })?;
            let root_obj = root_val.as_object().ok_or_else(|| {
                boa_engine::JsError::from_native(
                    boa_engine::JsNativeError::typ()
                        .with_message("Failed to execute 'createTreeWalker': parameter 1 is not a Node"),
                )
            })?;
            let root_id = {
                let el = root_obj
                    .downcast_ref::<super::element::JsElement>()
                    .ok_or_else(|| {
                        boa_engine::JsError::from_native(
                            boa_engine::JsNativeError::typ()
                                .with_message("createTreeWalker: root is not a Node"),
                        )
                    })?;
                el.node_id
            };

            // whatToShow (default: 0xFFFFFFFF = SHOW_ALL)
            let what_to_show = args
                .get(1)
                .filter(|v| !v.is_undefined())
                .map(|v| v.to_u32(ctx))
                .transpose()?
                .unwrap_or(0xFFFFFFFF);

            // filter (default: null)
            let filter = args.get(2).cloned().unwrap_or(JsValue::null());

            let tw = JsTreeWalker {
                tree: tree_for_closure.clone(),
                root: root_id,
                current_node: Cell::new(root_id),
                what_to_show,
                filter: filter.clone(),
            };

            let realm = ctx.realm().clone();
            let tw_obj = ObjectInitializer::with_native_data(tw, ctx)
                .accessor(
                    js_string!("root"),
                    Some(NativeFunction::from_fn_ptr(tw_get_root).to_js_function(&realm)),
                    None,
                    Attribute::CONFIGURABLE | Attribute::ENUMERABLE,
                )
                .accessor(
                    js_string!("currentNode"),
                    Some(NativeFunction::from_fn_ptr(tw_get_current_node).to_js_function(&realm)),
                    Some(NativeFunction::from_fn_ptr(tw_set_current_node).to_js_function(&realm)),
                    Attribute::CONFIGURABLE | Attribute::ENUMERABLE,
                )
                .accessor(
                    js_string!("whatToShow"),
                    Some(NativeFunction::from_fn_ptr(tw_get_what_to_show).to_js_function(&realm)),
                    None,
                    Attribute::CONFIGURABLE | Attribute::ENUMERABLE,
                )
                .accessor(
                    js_string!("filter"),
                    Some(NativeFunction::from_fn_ptr(tw_get_filter).to_js_function(&realm)),
                    None,
                    Attribute::CONFIGURABLE | Attribute::ENUMERABLE,
                )
                .function(
                    NativeFunction::from_fn_ptr(tw_stub_null),
                    js_string!("parentNode"),
                    0,
                )
                .function(
                    NativeFunction::from_fn_ptr(tw_stub_null),
                    js_string!("firstChild"),
                    0,
                )
                .function(
                    NativeFunction::from_fn_ptr(tw_stub_null),
                    js_string!("lastChild"),
                    0,
                )
                .function(
                    NativeFunction::from_fn_ptr(tw_stub_null),
                    js_string!("previousSibling"),
                    0,
                )
                .function(
                    NativeFunction::from_fn_ptr(tw_stub_null),
                    js_string!("nextSibling"),
                    0,
                )
                .function(
                    NativeFunction::from_fn_ptr(tw_stub_null),
                    js_string!("previousNode"),
                    0,
                )
                .function(
                    NativeFunction::from_fn_ptr(tw_stub_null),
                    js_string!("nextNode"),
                    0,
                )
                .build();

            Ok(JsValue::from(tw_obj))
        })
    };

    doc_obj
        .set(
            js_string!("createTreeWalker"),
            create_tw.to_js_function(ctx.realm()),
            false,
            ctx,
        )
        .expect("set createTreeWalker on document");
}

fn tw_get_root(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this.as_object().unwrap();
    let tw = obj.downcast_ref::<JsTreeWalker>().unwrap();
    let root_id = tw.root;
    let tree = tw.tree.clone();
    drop(tw);
    let js_el = get_or_create_js_element(root_id, tree, ctx)?;
    Ok(JsValue::from(js_el))
}

fn tw_get_current_node(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this.as_object().unwrap();
    let tw = obj.downcast_ref::<JsTreeWalker>().unwrap();
    let current = tw.current_node.get();
    let tree = tw.tree.clone();
    drop(tw);
    let js_el = get_or_create_js_element(current, tree, ctx)?;
    Ok(JsValue::from(js_el))
}

fn tw_set_current_node(this: &JsValue, args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    let node_val = args.first().ok_or_else(|| {
        boa_engine::JsError::from_native(
            boa_engine::JsNativeError::typ().with_message("TreeWalker.currentNode setter: value required"),
        )
    })?;
    let node_obj = node_val.as_object().ok_or_else(|| {
        boa_engine::JsError::from_native(
            boa_engine::JsNativeError::typ().with_message("TreeWalker.currentNode setter: not a Node"),
        )
    })?;
    let node_id = {
        let el = node_obj.downcast_ref::<super::element::JsElement>().ok_or_else(|| {
            boa_engine::JsError::from_native(
                boa_engine::JsNativeError::typ().with_message("TreeWalker.currentNode setter: not a Node"),
            )
        })?;
        el.node_id
    };

    let obj = this.as_object().unwrap();
    let tw = obj.downcast_ref::<JsTreeWalker>().unwrap();
    tw.current_node.set(node_id);
    Ok(JsValue::undefined())
}

fn tw_get_what_to_show(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this.as_object().unwrap();
    let tw = obj.downcast_ref::<JsTreeWalker>().unwrap();
    Ok(JsValue::from(tw.what_to_show))
}

fn tw_get_filter(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this.as_object().unwrap();
    let tw = obj.downcast_ref::<JsTreeWalker>().unwrap();
    Ok(tw.filter.clone())
}

fn tw_stub_null(_this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    Ok(JsValue::null())
}
