pub type NodeId = usize;

/// Placeholder for computed CSS styles. Will be replaced with proper types
/// when the CSS property system is integrated.
pub type ComputedStyles = std::collections::HashMap<String, String>;

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
        attributes: Vec<(String, String)>,
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
        namespace: String,   // "" = null
        prefix: String,      // "" = null
        value: String,
    },
    DocumentFragment,
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
