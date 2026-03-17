pub type NodeId = usize;

/// Placeholder for computed CSS styles. Will be replaced with proper types
/// when the CSS property system is integrated.
pub type ComputedStyles = std::collections::HashMap<String, String>;

/// Structured attribute storage supporting namespace-aware methods.
#[derive(Debug, Clone, PartialEq)]
pub struct DomAttribute {
    pub local_name: String,
    pub prefix: String,
    pub namespace: String,
    pub value: String,
}

impl DomAttribute {
    /// Convenience constructor for simple (non-namespaced) attributes.
    pub fn new(name: &str, value: &str) -> Self {
        DomAttribute {
            local_name: name.to_string(),
            prefix: String::new(),
            namespace: String::new(),
            value: value.to_string(),
        }
    }

    /// Returns true if this attribute matches the given namespace and local name.
    pub fn matches_ns(&self, namespace: &str, local_name: &str) -> bool {
        self.namespace == namespace && self.local_name == local_name
    }

    /// Returns the qualified name: "prefix:localName" if prefix is non-empty,
    /// otherwise just localName.
    pub fn qualified_name(&self) -> String {
        if self.prefix.is_empty() {
            self.local_name.clone()
        } else {
            format!("{}:{}", self.prefix, self.local_name)
        }
    }
}

#[derive(Debug, Clone)]
pub enum NodeData {
    Document,
    Doctype {
        name: String,
        public_id: String,
        system_id: String,
    },
    Element {
        tag_name: String,
        attributes: Vec<DomAttribute>,
        namespace: String,
    },
    Text {
        content: String,
    },
    Comment {
        content: String,
    },
    ProcessingInstruction {
        target: String,
        data: String,
    },
    Attr {
        local_name: String,
        namespace: String, // "" = null
        prefix: String,    // "" = null
        value: String,
    },
    DocumentFragment,
    CDATASection {
        content: String,
    },
}

#[derive(Debug, Clone)]
pub struct Node {
    pub id: NodeId,
    pub data: NodeData,
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
    pub computed_style: Option<ComputedStyles>,
    /// For `<template>` elements: the NodeId of the associated content fragment.
    pub template_contents: Option<NodeId>,
}
