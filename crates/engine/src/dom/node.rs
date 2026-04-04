pub type NodeId = usize;

/// Placeholder for computed CSS styles. Will be replaced with proper types
/// when the CSS property system is integrated.
pub type ComputedStyles = std::collections::HashMap<String, String>;

/// DOM string type that abstracts the internal representation of string values.
/// Today wraps `String` (UTF-8). Tomorrow can be swapped to `Vec<u8>` (WTF-8)
/// to support lone surrogates at the JS↔Rust boundary without touching callsites.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct DomString(String);

impl DomString {
    pub fn new() -> Self {
        DomString(String::new())
    }

    pub fn from_string(s: String) -> Self {
        DomString(s)
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl Default for DomString {
    fn default() -> Self {
        DomString::new()
    }
}

impl std::ops::Deref for DomString {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for DomString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl From<&str> for DomString {
    fn from(s: &str) -> Self {
        DomString(s.to_string())
    }
}

impl From<String> for DomString {
    fn from(s: String) -> Self {
        DomString(s)
    }
}

impl PartialEq<str> for DomString {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl PartialEq<&str> for DomString {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

impl PartialEq<String> for DomString {
    fn eq(&self, other: &String) -> bool {
        self.0 == *other
    }
}

impl PartialEq<DomString> for String {
    fn eq(&self, other: &DomString) -> bool {
        *self == other.0
    }
}

impl PartialEq<DomString> for &str {
    fn eq(&self, other: &DomString) -> bool {
        *self == other.0.as_str()
    }
}

/// Structured attribute storage supporting namespace-aware methods.
#[derive(Debug, Clone, PartialEq)]
pub struct DomAttribute {
    pub local_name: String,
    pub prefix: String,
    pub namespace: String,
    pub value: DomString,
}

impl DomAttribute {
    /// Convenience constructor for simple (non-namespaced) attributes.
    pub fn new(name: &str, value: &str) -> Self {
        DomAttribute {
            local_name: name.to_string(),
            prefix: String::new(),
            namespace: String::new(),
            value: DomString::from(value),
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
        prefix: Option<String>,
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
    ShadowRoot {
        mode: ShadowRootMode,
        host: NodeId,
    },
}

/// The mode of a shadow root: open (accessible via element.shadowRoot) or closed (hidden).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShadowRootMode {
    Open,
    Closed,
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
    /// For elements with an attached shadow root: the NodeId of the ShadowRoot node.
    pub shadow_root: Option<NodeId>,
}
