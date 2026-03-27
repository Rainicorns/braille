use std::cell::RefCell;
use std::rc::Rc;

use boa_engine::{
    js_string,
    Context, JsError, JsObject, JsResult, JsValue,
};

use crate::dom::{DomTree, NodeData, NodeId};

use super::JsElement;
use super::super::document::JsDocument;

/// Compare nodes from different DomTrees for equality (recursive).
pub(super) fn cross_tree_is_equal_node(tree_a: &DomTree, a: NodeId, tree_b: &DomTree, b: NodeId) -> bool {
    let node_a = tree_a.get_node(a);
    let node_b = tree_b.get_node(b);

    if tree_a.node_type(a) != tree_b.node_type(b) {
        return false;
    }

    match (&node_a.data, &node_b.data) {
        (
            NodeData::Element {
                tag_name: t1,
                attributes: a1,
                namespace: ns1,
            },
            NodeData::Element {
                tag_name: t2,
                attributes: a2,
                namespace: ns2,
            },
        ) => {
            if t1 != t2 || ns1 != ns2 || a1.len() != a2.len() {
                return false;
            }
            for attr in a1 {
                if !a2
                    .iter()
                    .any(|a| a.local_name == attr.local_name && a.namespace == attr.namespace && a.value == attr.value)
                {
                    return false;
                }
            }
        }
        (
            NodeData::Doctype {
                name: n1,
                public_id: p1,
                system_id: s1,
            },
            NodeData::Doctype {
                name: n2,
                public_id: p2,
                system_id: s2,
            },
        ) => {
            if n1 != n2 || p1 != p2 || s1 != s2 {
                return false;
            }
        }
        (NodeData::Text { content: c1 }, NodeData::Text { content: c2 }) => {
            if c1 != c2 {
                return false;
            }
        }
        (NodeData::Comment { content: c1 }, NodeData::Comment { content: c2 }) => {
            if c1 != c2 {
                return false;
            }
        }
        (
            NodeData::ProcessingInstruction { target: t1, data: d1 },
            NodeData::ProcessingInstruction { target: t2, data: d2 },
        ) => {
            if t1 != t2 || d1 != d2 {
                return false;
            }
        }
        (
            NodeData::Attr {
                local_name: ln1,
                namespace: ns1,
                prefix: p1,
                value: v1,
            },
            NodeData::Attr {
                local_name: ln2,
                namespace: ns2,
                prefix: p2,
                value: v2,
            },
        ) => {
            if ln1 != ln2 || ns1 != ns2 || p1 != p2 || v1 != v2 {
                return false;
            }
        }
        (NodeData::CDATASection { content: c1 }, NodeData::CDATASection { content: c2 }) => {
            if c1 != c2 {
                return false;
            }
        }
        (NodeData::Document, NodeData::Document) => {}
        (NodeData::DocumentFragment, NodeData::DocumentFragment) => {}
        _ => return false,
    }

    if node_a.children.len() != node_b.children.len() {
        return false;
    }
    for (ca, cb) in node_a.children.iter().zip(node_b.children.iter()) {
        if !cross_tree_is_equal_node(tree_a, *ca, tree_b, *cb) {
            return false;
        }
    }
    true
}

/// Extract (NodeId, tree) from a JsValue that could be either JsElement or JsDocument.
/// Returns None if the value is not a Node-like object.
pub(crate) fn extract_node_id(val: &JsValue) -> Option<(NodeId, Rc<RefCell<DomTree>>)> {
    let obj = val.as_object()?;
    if let Some(el) = obj.downcast_ref::<JsElement>() {
        return Some((el.node_id, el.tree.clone()));
    }
    if let Some(doc) = obj.downcast_ref::<JsDocument>() {
        let doc_id = doc.tree.borrow().document();
        return Some((doc_id, doc.tree.clone()));
    }
    None
}

// ---------------------------------------------------------------------------
// Node comparison and namespace methods on JsElement
// ---------------------------------------------------------------------------

impl JsElement {
    /// Native implementation of node.isEqualNode(other)
    pub(super) fn is_equal_node(this: &JsValue, args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
        let (this_id, tree) = extract_node_id(this)
            .ok_or_else(|| JsError::from_opaque(js_string!("isEqualNode: `this` is not a Node").into()))?;

        let other_val = match args.first() {
            Some(v) if !v.is_null() && !v.is_undefined() => v,
            _ => return Ok(JsValue::from(false)),
        };
        let (other_id, other_tree) = match extract_node_id(other_val) {
            Some(ids) => ids,
            None => return Ok(JsValue::from(false)),
        };

        // If both nodes are in the same tree, use the tree's built-in comparison
        if Rc::ptr_eq(&tree, &other_tree) {
            let result = tree.borrow().is_equal_node(this_id, other_id);
            return Ok(JsValue::from(result));
        }

        // Cross-tree comparison: compare nodes from different trees
        let result = cross_tree_is_equal_node(&tree.borrow(), this_id, &other_tree.borrow(), other_id);
        Ok(JsValue::from(result))
    }

    /// Native implementation of node.isSameNode(other)
    pub(super) fn is_same_node(this: &JsValue, args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
        let (this_id, tree) = extract_node_id(this)
            .ok_or_else(|| JsError::from_opaque(js_string!("isSameNode: `this` is not a Node").into()))?;

        let other_val = match args.first() {
            Some(v) if !v.is_null() && !v.is_undefined() => v,
            _ => return Ok(JsValue::from(false)),
        };
        let (other_id, other_tree) = match extract_node_id(other_val) {
            Some(ids) => ids,
            None => return Ok(JsValue::from(false)),
        };

        // Same node requires same tree AND same id
        let same = Rc::ptr_eq(&tree, &other_tree) && this_id == other_id;
        Ok(JsValue::from(same))
    }

    /// Native implementation of node.compareDocumentPosition(other)
    pub(super) fn compare_document_position(this: &JsValue, args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
        let (this_id, tree) = extract_node_id(this)
            .ok_or_else(|| JsError::from_opaque(js_string!("compareDocumentPosition: `this` is not a Node").into()))?;

        let other_val = args
            .first()
            .ok_or_else(|| JsError::from_opaque(js_string!("compareDocumentPosition: missing argument").into()))?;
        let (other_id, other_tree) = extract_node_id(other_val).ok_or_else(|| {
            JsError::from_opaque(js_string!("compareDocumentPosition: argument is not a Node").into())
        })?;

        // If nodes are in different trees, they're disconnected
        if !Rc::ptr_eq(&tree, &other_tree) {
            let dir = if (Rc::as_ptr(&other_tree) as usize) < (Rc::as_ptr(&tree) as usize) {
                0x02u16
            } else {
                0x04u16
            };
            return Ok(JsValue::from((0x01 | 0x20 | dir) as i32));
        }

        let result = tree.borrow().compare_document_position(this_id, other_id);
        Ok(JsValue::from(result as i32))
    }

    /// Native implementation of element.contains(other)
    pub(super) fn contains(this: &JsValue, args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
        let (this_id, tree) = extract_node_id(this)
            .ok_or_else(|| JsError::from_opaque(js_string!("contains: `this` is not a Node").into()))?;

        let other_val = match args.first() {
            Some(v) if !v.is_null() && !v.is_undefined() => v,
            _ => return Ok(JsValue::from(false)),
        };
        let (other_id, other_tree) = match extract_node_id(other_val) {
            Some(ids) => ids,
            None => return Ok(JsValue::from(false)),
        };

        // If other is from a different tree, it can't be contained
        if !Rc::ptr_eq(&tree, &other_tree) {
            return Ok(JsValue::from(false));
        }

        // Walk up from other to see if we reach this
        let tree_ref = tree.borrow();
        let mut current = other_id;
        loop {
            if current == this_id {
                return Ok(JsValue::from(true));
            }
            match tree_ref.get_node(current).parent {
                Some(parent_id) => current = parent_id,
                None => return Ok(JsValue::from(false)),
            }
        }
    }

    /// Native implementation of node.lookupNamespaceURI(prefix)
    /// Returns the namespace URI associated with the given prefix by walking ancestors.
    pub(super) fn lookup_namespace_uri(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
        let (node_id, tree) = extract_node_id(this)
            .ok_or_else(|| JsError::from_opaque(js_string!("lookupNamespaceURI: `this` is not a Node").into()))?;

        // Per spec: if prefix is null or empty string, treat as null (None)
        let prefix_arg = args.first().cloned().unwrap_or(JsValue::undefined());
        let prefix: Option<String> = if prefix_arg.is_null() || prefix_arg.is_undefined() {
            None
        } else {
            let s = prefix_arg.to_string(ctx)?.to_std_string_escaped();
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        };

        // For Attr nodes, the spec says delegate to ownerElement.
        // Attr nodes in the tree don't have a parent, so check the shared attr_node_cache
        // to find the owning element.
        let effective_node_id = {
            let t = tree.borrow();
            if matches!(t.get_node(node_id).data, crate::dom::NodeData::Attr { .. }) {
                let cache = crate::js::realm_state::attr_node_cache(ctx);
                let c = cache.borrow();
                c.iter()
                    .find(|(_, &nid)| nid == node_id)
                    .map(|((_, el_id, _), _)| *el_id)
                    .unwrap_or(node_id)
            } else {
                node_id
            }
        };

        let tree_ref = tree.borrow();
        let result = tree_ref.locate_namespace(effective_node_id, prefix.as_deref());

        match result {
            Some(ns) => Ok(JsValue::from(js_string!(ns))),
            None => Ok(JsValue::null()),
        }
    }

    /// Native implementation of node.lookupPrefix(namespace)
    /// Returns the prefix for the given namespace URI by walking ancestors.
    pub(super) fn lookup_prefix(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
        let (node_id, tree) = extract_node_id(this)
            .ok_or_else(|| JsError::from_opaque(js_string!("lookupPrefix: `this` is not a Node").into()))?;

        // Per spec: if namespace is null or empty string, return null immediately
        let ns_arg = args.first().cloned().unwrap_or(JsValue::undefined());
        if ns_arg.is_null() || ns_arg.is_undefined() {
            return Ok(JsValue::null());
        }
        let namespace = ns_arg.to_string(ctx)?.to_std_string_escaped();
        if namespace.is_empty() {
            return Ok(JsValue::null());
        }

        let tree_ref = tree.borrow();
        let result = tree_ref.locate_prefix(node_id, &namespace);

        match result {
            Some(pfx) => Ok(JsValue::from(js_string!(pfx))),
            None => Ok(JsValue::null()),
        }
    }

    /// Native implementation of node.isDefaultNamespace(namespace)
    /// Returns true if the given namespace is the default namespace.
    pub(super) fn is_default_namespace(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
        let (node_id, tree) = extract_node_id(this)
            .ok_or_else(|| JsError::from_opaque(js_string!("isDefaultNamespace: `this` is not a Node").into()))?;

        // Per spec: if namespace is null or empty string, treat as null (None)
        let ns_arg = args.first().cloned().unwrap_or(JsValue::undefined());
        let namespace: Option<String> = if ns_arg.is_null() || ns_arg.is_undefined() {
            None
        } else {
            let s = ns_arg.to_string(ctx)?.to_std_string_escaped();
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        };

        // Get the default namespace (prefix = null)
        let tree_ref = tree.borrow();
        let default_ns = tree_ref.locate_namespace(node_id, None);

        // Compare: both None (null) -> true, both Some with same value -> true
        Ok(JsValue::from(default_ns == namespace))
    }
}

// ---------------------------------------------------------------------------
// Standalone node comparison functions (for use on document object too)
// ---------------------------------------------------------------------------

pub(crate) fn node_is_equal_node(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    JsElement::is_equal_node(this, args, ctx)
}

pub(crate) fn node_is_same_node(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    JsElement::is_same_node(this, args, ctx)
}

pub(crate) fn node_compare_document_position(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    JsElement::compare_document_position(this, args, ctx)
}

pub(crate) fn node_contains(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    JsElement::contains(this, args, ctx)
}

pub(crate) fn node_lookup_namespace_uri(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    JsElement::lookup_namespace_uri(this, args, ctx)
}

pub(crate) fn node_lookup_prefix(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    JsElement::lookup_prefix(this, args, ctx)
}

pub(crate) fn node_is_default_namespace(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    JsElement::is_default_namespace(this, args, ctx)
}
