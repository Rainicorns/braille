use crate::dom::node::{Node, NodeData, NodeId};

#[derive(Debug)]
pub struct DomTree {
    pub(crate) nodes: Vec<Node>,
    is_html_document: bool,
    pub url_fragment: Option<String>,
}

impl Default for DomTree {
    fn default() -> Self {
        Self::new()
    }
}

impl DomTree {
    /// Creates a new DomTree with a Document root node at index 0.
    /// Defaults to HTML document (is_html_document = true).
    pub fn new() -> Self {
        let mut tree = DomTree {
            nodes: Vec::new(),
            is_html_document: true,
            url_fragment: None,
        };
        tree.alloc_node(NodeData::Document);
        tree
    }

    /// Creates a new DomTree for an XML document (is_html_document = false).
    pub fn new_xml() -> Self {
        let mut tree = DomTree {
            nodes: Vec::new(),
            is_html_document: false,
            url_fragment: None,
        };
        tree.alloc_node(NodeData::Document);
        tree
    }

    /// Returns true if this is an HTML document, false for XML documents.
    pub fn is_html_document(&self) -> bool {
        self.is_html_document
    }

    /// Allocates a new node with the given data, no parent, no children, and no styles.
    /// Returns the NodeId of the newly created node.
    pub(crate) fn alloc_node(&mut self, data: NodeData) -> NodeId {
        let id = self.nodes.len();
        self.nodes.push(Node {
            id,
            data,
            parent: None,
            children: Vec::new(),
            computed_style: None,
            template_contents: None,
            shadow_root: None,
        });
        id
    }

    pub fn get_node(&self, id: NodeId) -> &Node {
        &self.nodes[id]
    }

    pub fn get_node_mut(&mut self, id: NodeId) -> &mut Node {
        &mut self.nodes[id]
    }

    /// Returns the index of `child` within `parent`'s children list, or None if not found.
    ///
    /// O(n) linear scan over siblings, which is acceptable for typical DOM sizes
    /// (most parent nodes have tens, not thousands, of children).
    pub(crate) fn find_child_index(&self, parent: NodeId, child: NodeId) -> Option<usize> {
        self.nodes[parent].children.iter().position(|&c| c == child)
    }

    /// The Document root is always at index 0.
    pub fn document(&self) -> NodeId {
        0
    }

    /// Finds the first `<body>` element in the tree.
    pub fn body(&self) -> Option<NodeId> {
        self.find_element_by_tag("body")
    }

    /// Finds the first `<head>` element in the tree.
    pub fn head(&self) -> Option<NodeId> {
        self.find_element_by_tag("head")
    }

    pub(crate) fn find_element_by_tag(&self, tag: &str) -> Option<NodeId> {
        let tag_lower = tag.to_ascii_lowercase();
        self.nodes.iter().find_map(|node| {
            if let NodeData::Element { ref tag_name, .. } = node.data {
                if tag_name.to_ascii_lowercase() == tag_lower {
                    return Some(node.id);
                }
            }
            None
        })
    }

    /// Returns the total number of nodes in the tree.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }
}
