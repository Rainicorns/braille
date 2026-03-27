use crate::dom::node::{DomAttribute, NodeData, NodeId};

use super::DomTree;

impl DomTree {
    // -----------------------------------------------------------------------
    // Namespace lookup algorithms (DOM spec section 4.4.8)
    // -----------------------------------------------------------------------

    const XMLNS_NS: &str = "http://www.w3.org/2000/xmlns/";
    const XML_NS: &str = "http://www.w3.org/XML/1998/namespace";

    /// Implements the "locate a namespace" algorithm from the DOM spec.
    /// `prefix` is `None` for null/empty prefix.
    /// Returns `Some(namespace_uri)` or `None` for null.
    pub fn locate_namespace(&self, node_id: NodeId, prefix: Option<&str>) -> Option<String> {
        let node = self.get_node(node_id);
        match &node.data {
            NodeData::Element {
                tag_name,
                namespace,
                attributes,
            } => self.locate_namespace_for_element(node_id, tag_name, namespace, attributes, prefix),
            NodeData::Document => {
                // Delegate to document element if it exists
                for &child_id in &node.children {
                    if matches!(self.get_node(child_id).data, NodeData::Element { .. }) {
                        return self.locate_namespace(child_id, prefix);
                    }
                }
                None
            }
            NodeData::Doctype { .. } | NodeData::DocumentFragment | NodeData::ShadowRoot { .. } => None,
            NodeData::Attr { .. } => {
                // Attr nodes: delegate to ownerElement.
                // In this implementation, Attr nodes don't track their ownerElement,
                // so we return null (None) for disconnected Attrs.
                // If the Attr has a parent (which currently doesn't happen), delegate to it.
                if let Some(parent_id) = node.parent {
                    if matches!(self.get_node(parent_id).data, NodeData::Element { .. }) {
                        return self.locate_namespace(parent_id, prefix);
                    }
                }
                None
            }
            NodeData::Text { .. }
            | NodeData::CDATASection { .. }
            | NodeData::Comment { .. }
            | NodeData::ProcessingInstruction { .. } => {
                // Delegate to parent element
                if let Some(parent_id) = node.parent {
                    if matches!(self.get_node(parent_id).data, NodeData::Element { .. }) {
                        return self.locate_namespace(parent_id, prefix);
                    }
                }
                None
            }
        }
    }

    /// Core "locate a namespace" for Element nodes.
    fn locate_namespace_for_element(
        &self,
        node_id: NodeId,
        tag_name: &str,
        namespace: &str,
        attributes: &[DomAttribute],
        prefix: Option<&str>,
    ) -> Option<String> {
        // Step 1: If element's namespace is not empty and element's prefix matches, return namespace
        if !namespace.is_empty() {
            let elem_prefix = tag_name.find(':').map(|pos| &tag_name[..pos]);
            if elem_prefix == prefix {
                return Some(namespace.to_string());
            }
        }

        // Step 2: Check xmlns attributes
        for attr in attributes {
            if attr.namespace != Self::XMLNS_NS {
                continue;
            }
            if let Some(pfx) = prefix {
                // Looking for xmlns:pfx attribute
                if attr.prefix == "xmlns" && attr.local_name == pfx {
                    return if attr.value.is_empty() {
                        None
                    } else {
                        Some(attr.value.clone())
                    };
                }
            } else {
                // Looking for xmlns attribute (no prefix) for null/empty prefix lookup
                if attr.prefix.is_empty() && attr.local_name == "xmlns" {
                    return if attr.value.is_empty() {
                        None
                    } else {
                        Some(attr.value.clone())
                    };
                }
            }
        }

        // Step 3: Built-in xml prefix
        if prefix == Some("xml") {
            return Some(Self::XML_NS.to_string());
        }

        // Step 4: Built-in xmlns prefix
        if prefix == Some("xmlns") {
            return Some(Self::XMLNS_NS.to_string());
        }

        // Step 5: Recurse to parent element
        let node = self.get_node(node_id);
        if let Some(parent_id) = node.parent {
            if matches!(self.get_node(parent_id).data, NodeData::Element { .. }) {
                return self.locate_namespace(parent_id, prefix);
            }
        }

        // Step 6: Not found
        None
    }

    /// Implements the "locate a namespace prefix" algorithm from the DOM spec.
    /// `namespace` is the namespace URI to look up.
    /// Returns `Some(prefix)` or `None`.
    pub fn locate_prefix(&self, node_id: NodeId, namespace: &str) -> Option<String> {
        let node = self.get_node(node_id);
        match &node.data {
            NodeData::Element {
                tag_name,
                namespace: elem_ns,
                attributes,
            } => self.locate_prefix_for_element(node_id, tag_name, elem_ns, attributes, namespace),
            NodeData::Document => {
                // Delegate to document element if it exists
                for &child_id in &node.children {
                    if matches!(self.get_node(child_id).data, NodeData::Element { .. }) {
                        return self.locate_prefix(child_id, namespace);
                    }
                }
                None
            }
            NodeData::Doctype { .. } | NodeData::DocumentFragment | NodeData::ShadowRoot { .. } => None,
            NodeData::Attr { .. } => {
                // Delegate to ownerElement if known (currently always None)
                if let Some(parent_id) = node.parent {
                    if matches!(self.get_node(parent_id).data, NodeData::Element { .. }) {
                        return self.locate_prefix(parent_id, namespace);
                    }
                }
                None
            }
            NodeData::Text { .. }
            | NodeData::CDATASection { .. }
            | NodeData::Comment { .. }
            | NodeData::ProcessingInstruction { .. } => {
                // Delegate to parent element
                if let Some(parent_id) = node.parent {
                    if matches!(self.get_node(parent_id).data, NodeData::Element { .. }) {
                        return self.locate_prefix(parent_id, namespace);
                    }
                }
                None
            }
        }
    }

    /// Core "locate a namespace prefix" for Element nodes.
    fn locate_prefix_for_element(
        &self,
        node_id: NodeId,
        tag_name: &str,
        elem_ns: &str,
        attributes: &[DomAttribute],
        namespace: &str,
    ) -> Option<String> {
        // Step 1: If element's namespace matches and element has a prefix, return the prefix
        if !elem_ns.is_empty() && elem_ns == namespace {
            if let Some(pos) = tag_name.find(':') {
                let pfx = &tag_name[..pos];
                if !pfx.is_empty() {
                    return Some(pfx.to_string());
                }
            }
        }

        // Step 2: Check xmlns:* attributes whose value matches the namespace
        for attr in attributes {
            if attr.namespace == Self::XMLNS_NS && attr.prefix == "xmlns" && attr.value == namespace {
                return Some(attr.local_name.clone());
            }
        }

        // Step 3: Recurse to parent element
        let node = self.get_node(node_id);
        if let Some(parent_id) = node.parent {
            if matches!(self.get_node(parent_id).data, NodeData::Element { .. }) {
                return self.locate_prefix(parent_id, namespace);
            }
        }

        // Step 4: Not found
        None
    }
}
