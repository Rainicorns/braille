use std::cell::RefCell;
use std::rc::Rc;

use boa_engine::{
    js_string,
    native_function::NativeFunction,
    object::{FunctionObjectBuilder, ObjectInitializer},
    Context, JsObject, JsResult, JsValue,
};

use crate::dom::{DomTree, NodeData, NodeId};
use crate::js::realm_state;

// ---------------------------------------------------------------------------
// Live DOMStringMap creation (for element.dataset)
// ---------------------------------------------------------------------------

/// Create a live DOMStringMap backed by the given element's data-* attributes.
/// The returned Proxy intercepts get/set/delete/ownKeys to read/write the DOM.
pub(crate) fn create_live_domstringmap(
    element_id: NodeId,
    tree: Rc<RefCell<DomTree>>,
    context: &mut Context,
) -> JsResult<JsObject> {
    use super::super::anchor_form::{camel_to_kebab, kebab_to_camel};

    let backing = ObjectInitializer::new(context).build();

    if let Some(p) = realm_state::dsm_proto(context) {
        backing.set_prototype(Some(p));
    }

    let realm = context.realm().clone();

    // getAttr(camelName) — returns attribute value or null
    let tree_get = tree.clone();
    let get_attr_fn = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let camel = args
                .first()
                .map(|v| v.to_string(ctx))
                .transpose()?
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_default();
            let kebab = camel_to_kebab(&camel);
            let attr_name = format!("data-{}", kebab);
            let t = tree_get.borrow();
            let node = t.get_node(element_id);
            match &node.data {
                NodeData::Element { attributes, .. } => {
                    for attr in attributes {
                        if attr.local_name == attr_name {
                            return Ok(JsValue::from(js_string!(attr.value.clone())));
                        }
                    }
                    Ok(JsValue::null())
                }
                _ => Ok(JsValue::null()),
            }
        })
    };

    // setAttr(camelName, value) — sets data-* attribute on element
    let tree_set = tree.clone();
    let set_attr_fn = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let camel = args
                .first()
                .map(|v| v.to_string(ctx))
                .transpose()?
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_default();
            let value = args
                .get(1)
                .map(|v| v.to_string(ctx))
                .transpose()?
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_default();
            let kebab = camel_to_kebab(&camel);
            let attr_name = format!("data-{}", kebab);
            let mut t = tree_set.borrow_mut();
            t.set_attribute(element_id, &attr_name, &value);
            Ok(JsValue::undefined())
        })
    };

    // deleteAttr(camelName) — removes data-* attribute, returns true if existed
    let tree_del = tree.clone();
    let delete_attr_fn = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let camel = args
                .first()
                .map(|v| v.to_string(ctx))
                .transpose()?
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_default();
            let kebab = camel_to_kebab(&camel);
            let attr_name = format!("data-{}", kebab);
            let mut t = tree_del.borrow_mut();
            let existed = t.get_attribute(element_id, &attr_name).is_some();
            if existed {
                t.remove_attribute(element_id, &attr_name);
            }
            Ok(JsValue::from(true))
        })
    };

    // getKeys() — returns NUL-separated camelCase keys
    let tree_keys = tree;
    let get_keys_fn = unsafe {
        NativeFunction::from_closure(move |_this, _args, _ctx| {
            let t = tree_keys.borrow();
            let node = t.get_node(element_id);
            match &node.data {
                NodeData::Element { attributes, .. } => {
                    let keys: Vec<String> = attributes
                        .iter()
                        .filter_map(|attr| {
                            attr.local_name.strip_prefix("data-").map(kebab_to_camel)
                        })
                        .collect();
                    if keys.is_empty() {
                        Ok(JsValue::null())
                    } else {
                        Ok(JsValue::from(js_string!(keys.join("\0"))))
                    }
                }
                _ => Ok(JsValue::null()),
            }
        })
    };

    let factory = realm_state::dsm_proxy_factory(context).expect("DOMStringMap proxy factory not initialized");
    let get_attr_js = FunctionObjectBuilder::new(&realm, get_attr_fn).build();
    let set_attr_js = FunctionObjectBuilder::new(&realm, set_attr_fn).build();
    let delete_attr_js = FunctionObjectBuilder::new(&realm, delete_attr_fn).build();
    let get_keys_js = FunctionObjectBuilder::new(&realm, get_keys_fn).build();

    let proxy = factory.call(
        &JsValue::undefined(),
        &[
            backing.into(),
            get_attr_js.into(),
            set_attr_js.into(),
            delete_attr_js.into(),
            get_keys_js.into(),
        ],
        context,
    )?;

    Ok(proxy.as_object().expect("DSM proxy should be an object").clone())
}
