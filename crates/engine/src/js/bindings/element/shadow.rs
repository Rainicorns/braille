use boa_engine::{
    js_string,
    Context, JsNativeError, JsResult, JsValue,
};

use crate::dom::node::ShadowRootMode;
use crate::dom::NodeData;

use super::cache::get_or_create_js_element;
use super::JsElement;

// ---------------------------------------------------------------------------
// Shadow DOM methods on JsElement
// ---------------------------------------------------------------------------

impl JsElement {
    /// element.attachShadow({mode: 'open'|'closed'})
    pub(super) fn attach_shadow(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
        extract_element!(el, this, "attachShadow");

        // Parse options
        let options = args
            .first()
            .and_then(|v| v.as_object())
            .ok_or_else(|| {
                JsNativeError::typ().with_message("attachShadow: argument must be an object with a 'mode' property")
            })?;
        let mode_val = options.get(js_string!("mode"), ctx)?;
        let mode_str = mode_val
            .as_string()
            .ok_or_else(|| JsNativeError::typ().with_message("attachShadow: mode must be a string"))?
            .to_std_string_escaped();
        let mode = match mode_str.as_str() {
            "open" => ShadowRootMode::Open,
            "closed" => ShadowRootMode::Closed,
            _ => {
                return Err(JsNativeError::typ()
                    .with_message("attachShadow: mode must be 'open' or 'closed'")
                    .into())
            }
        };

        let tree_rc = el.tree.clone();
        let host_id = el.node_id;

        // Validate: must be an Element, must be a valid shadow host tag
        {
            let tree = tree_rc.borrow();
            let node = tree.get_node(host_id);
            match &node.data {
                NodeData::Element { tag_name, .. } => {
                    let local = tag_name.to_ascii_lowercase();
                    let is_custom = local.contains('-');
                    if !is_custom && !VALID_SHADOW_HOST_TAGS.contains(&local.as_str()) {
                        return Err(JsNativeError::typ()
                            .with_message(format!(
                                "NotSupportedError: '{}' is not a valid element for attachShadow",
                                tag_name
                            ))
                            .into());
                    }
                }
                _ => {
                    return Err(JsNativeError::typ()
                        .with_message("attachShadow: this is not an Element")
                        .into())
                }
            }
            // Check for existing shadow root
            if node.shadow_root.is_some() {
                return Err(JsNativeError::typ()
                    .with_message("NotSupportedError: element already has a shadow root")
                    .into());
            }
        }

        // Create the shadow root
        let shadow_id = tree_rc.borrow_mut().create_shadow_root(mode, host_id);
        let js_obj = get_or_create_js_element(shadow_id, tree_rc, ctx)?;
        Ok(js_obj.into())
    }

    /// element.shadowRoot getter — returns ShadowRoot if mode is open, null if closed or none
    pub(super) fn get_shadow_root(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
        extract_element!(el, this, "shadowRoot getter");

        let tree_rc = el.tree.clone();
        let tree = tree_rc.borrow();
        let node = tree.get_node(el.node_id);

        if let Some(shadow_id) = node.shadow_root {
            if let NodeData::ShadowRoot { mode, .. } = tree.get_node(shadow_id).data {
                if mode == ShadowRootMode::Open {
                    drop(tree);
                    let js_obj = get_or_create_js_element(shadow_id, tree_rc, ctx)?;
                    return Ok(js_obj.into());
                }
            }
        }
        Ok(JsValue::null())
    }
}

/// Valid shadow host tag names per spec.
pub(super) const VALID_SHADOW_HOST_TAGS: &[&str] = &[
    "article",
    "aside",
    "blockquote",
    "body",
    "div",
    "footer",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "header",
    "main",
    "nav",
    "p",
    "section",
    "span",
];
