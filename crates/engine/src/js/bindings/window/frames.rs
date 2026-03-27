use std::cell::RefCell;
use std::rc::Rc;

use boa_engine::{
    js_string,
    native_function::NativeFunction,
    object::ObjectInitializer,
    property::PropertyDescriptor,
    Context, JsResult, JsValue,
};

use crate::js::realm_state;

/// Build the frames getter closure for the window object.
pub(super) fn make_frames_getter(tree: Rc<RefCell<crate::dom::DomTree>>) -> NativeFunction {
    unsafe {
        NativeFunction::from_closure(move |_this, _args, ctx2| {
            let tree_ref = tree.borrow();
            let tree_ptr = Rc::as_ptr(&tree) as usize;

            // Collect iframe node IDs in document order
            let mut iframe_ids = Vec::new();
            let doc = tree_ref.document();
            collect_iframes(&tree_ref, doc, &mut iframe_ids);
            drop(tree_ref);

            let frames_obj = ObjectInitializer::new(ctx2).build();

            // Set numeric indices
            for (i, &nid) in iframe_ids.iter().enumerate() {
                // Ensure iframe content doc + realm is created
                let _doc_obj = super::super::element::ensure_iframe_content_doc(tree_ptr, nid, ctx2)?;

                // Look up the iframe's realm and return its real window object
                let cw = get_iframe_window(tree_ptr, nid, ctx2);

                frames_obj.define_property_or_throw(
                    js_string!(i.to_string()),
                    PropertyDescriptor::builder()
                        .value(cw)
                        .writable(true)
                        .configurable(true)
                        .enumerable(true)
                        .build(),
                    ctx2,
                )?;
            }

            // Set length
            frames_obj.define_property_or_throw(
                js_string!("length"),
                PropertyDescriptor::builder()
                    .value(JsValue::from(iframe_ids.len() as u32))
                    .writable(false)
                    .configurable(true)
                    .enumerable(true)
                    .build(),
                ctx2,
            )?;

            Ok(JsValue::from(frames_obj))
        })
    }
}

/// Build the frames getter closure for the global object.
pub(super) fn make_frames_getter_global(tree: Rc<RefCell<crate::dom::DomTree>>) -> NativeFunction {
    unsafe {
        NativeFunction::from_closure(move |_this, _args, ctx2| {
            let tree_ref = tree.borrow();
            let tree_ptr = Rc::as_ptr(&tree) as usize;

            let mut iframe_ids = Vec::new();
            let doc = tree_ref.document();
            collect_iframes(&tree_ref, doc, &mut iframe_ids);
            drop(tree_ref);

            let frames_obj = ObjectInitializer::new(ctx2).build();

            for (i, &nid) in iframe_ids.iter().enumerate() {
                let _doc_obj = super::super::element::ensure_iframe_content_doc(tree_ptr, nid, ctx2)?;
                let cw = get_iframe_window(tree_ptr, nid, ctx2);
                frames_obj.define_property_or_throw(
                    js_string!(i.to_string()),
                    PropertyDescriptor::builder()
                        .value(cw)
                        .writable(true)
                        .configurable(true)
                        .enumerable(true)
                        .build(),
                    ctx2,
                )?;
            }

            frames_obj.define_property_or_throw(
                js_string!("length"),
                PropertyDescriptor::builder()
                    .value(JsValue::from(iframe_ids.len() as u32))
                    .writable(false)
                    .configurable(true)
                    .enumerable(true)
                    .build(),
                ctx2,
            )?;

            Ok(JsValue::from(frames_obj))
        })
    }
}

/// Look up the real window object for an iframe's realm.
/// If the iframe has a realm, enters it to read its window object.
/// Falls back to a plain object with just `document` if no realm exists.
fn get_iframe_window(tree_ptr: usize, nid: crate::dom::NodeId, ctx: &mut Context) -> JsValue {
    let realms = realm_state::iframe_realms(ctx);
    let realm_opt = realms.borrow().get(&(tree_ptr, nid)).cloned();

    if let Some(realm) = realm_opt {
        // Enter the iframe realm to read its window object
        let win = realm_state::with_realm(ctx, &realm, |ctx| realm_state::window_object(ctx));
        match win {
            Some(w) => JsValue::from(w),
            None => JsValue::undefined(),
        }
    } else {
        // Fallback: no realm, create a plain object with just document
        let doc_obj = super::super::element::ensure_iframe_content_doc(tree_ptr, nid, ctx);
        match doc_obj {
            Ok(doc) => {
                let cw = ObjectInitializer::new(ctx).build();
                let _ = cw.define_property_or_throw(
                    js_string!("document"),
                    PropertyDescriptor::builder()
                        .value(JsValue::from(doc))
                        .writable(true)
                        .configurable(true)
                        .enumerable(true)
                        .build(),
                    ctx,
                );
                JsValue::from(cw)
            }
            Err(_) => JsValue::undefined(),
        }
    }
}

/// Recursively collects NodeIds of `<iframe>` elements in document order.
fn collect_iframes(tree: &crate::dom::DomTree, node_id: crate::dom::NodeId, out: &mut Vec<crate::dom::NodeId>) {
    use crate::dom::NodeData;
    let node = tree.get_node(node_id);
    if let NodeData::Element { ref tag_name, .. } = node.data {
        if tag_name == "iframe" {
            out.push(node_id);
        }
    }
    for child in tree.children(node_id) {
        collect_iframes(tree, child, out);
    }
}

/// Build the frames result object from a list of iframe IDs (shared logic).
#[allow(dead_code)]
fn build_frames_result(
    tree_ptr: usize,
    iframe_ids: &[crate::dom::NodeId],
    ctx: &mut Context,
) -> JsResult<JsValue> {
    let frames_obj = ObjectInitializer::new(ctx).build();

    for (i, &nid) in iframe_ids.iter().enumerate() {
        let _doc_obj = super::super::element::ensure_iframe_content_doc(tree_ptr, nid, ctx)?;
        let cw = get_iframe_window(tree_ptr, nid, ctx);
        frames_obj.define_property_or_throw(
            js_string!(i.to_string()),
            PropertyDescriptor::builder()
                .value(cw)
                .writable(true)
                .configurable(true)
                .enumerable(true)
                .build(),
            ctx,
        )?;
    }

    frames_obj.define_property_or_throw(
        js_string!("length"),
        PropertyDescriptor::builder()
            .value(JsValue::from(iframe_ids.len() as u32))
            .writable(false)
            .configurable(true)
            .enumerable(true)
            .build(),
        ctx,
    )?;

    Ok(JsValue::from(frames_obj))
}
