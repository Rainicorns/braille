pub type NodeId = usize;

/// Placeholder for computed CSS styles. Will be replaced with proper types
/// when the CSS property system is integrated.
pub type ComputedStyles = std::collections::HashMap<String, String>;

#[derive(Debug, Clone)]
pub enum NodeData {
    Document,
    Element {
        tag_name: String,
        attributes: Vec<(String, String)>,
    },
    Text {
        content: String,
    },
    Comment {
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
}
