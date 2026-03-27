use crate::dom::node::{NodeData, NodeId};

use super::DomTree;

impl DomTree {
    /// Searches the document tree in tree order (DFS) for the first Element whose
    /// "id" attribute matches `id`. Only visits nodes connected to the document root,
    /// so disconnected/detached nodes are excluded. Per spec, empty string never matches.
    pub fn get_element_by_id(&self, id: &str) -> Option<NodeId> {
        if id.is_empty() {
            return None;
        }
        // DFS walk starting from document root (node 0) gives tree order
        // and automatically excludes disconnected nodes.
        let mut stack = vec![0usize]; // document node
        while let Some(node_id) = stack.pop() {
            let node = &self.nodes[node_id];
            if let NodeData::Element { ref attributes, .. } = node.data {
                if attributes.iter().any(|a| a.local_name == "id" && a.value == id) {
                    return Some(node_id);
                }
            }
            // Push children in reverse order so first child is popped first (DFS pre-order)
            for &child in node.children.iter().rev() {
                stack.push(child);
            }
        }
        None
    }

    /// Returns all Element nodes whose tag_name matches `tag` (case-insensitive).
    pub fn get_elements_by_tag_name(&self, tag: &str) -> Vec<NodeId> {
        let tag_lower = tag.to_ascii_lowercase();
        self.nodes
            .iter()
            .filter_map(|node| {
                if let NodeData::Element { ref tag_name, .. } = node.data {
                    if tag_name.to_ascii_lowercase() == tag_lower {
                        return Some(node.id);
                    }
                }
                None
            })
            .collect()
    }

    /// Returns the textContent of a node per the DOM spec:
    /// - Text / Comment: return the node's data
    /// - Element / DocumentFragment: concatenation of all descendant Text node data
    /// - Document / Doctype: return empty string (JS layer returns null)
    pub fn get_text_content(&self, node_id: NodeId) -> String {
        let node = &self.nodes[node_id];
        match &node.data {
            NodeData::Text { content } | NodeData::CDATASection { content } => content.clone(),
            NodeData::Comment { content } => content.clone(),
            NodeData::ProcessingInstruction { data, .. } => data.clone(),
            NodeData::Attr { value, .. } => value.clone(),
            NodeData::Element { .. } | NodeData::DocumentFragment | NodeData::ShadowRoot { .. } => {
                let mut result = String::new();
                self.collect_descendant_text(node_id, &mut result);
                result
            }
            _ => {
                // Document / Doctype: empty string at tree level
                String::new()
            }
        }
    }

    /// Iteratively collects text content from all descendant Text nodes.
    /// Per spec, only Text node data is included (not Comment or PI).
    fn collect_descendant_text(&self, node_id: NodeId, result: &mut String) {
        let mut stack: Vec<NodeId> = self.nodes[node_id].children.iter().rev().copied().collect();
        while let Some(child_id) = stack.pop() {
            match &self.nodes[child_id].data {
                NodeData::Text { content } | NodeData::CDATASection { content } => result.push_str(content),
                NodeData::Element { .. } | NodeData::DocumentFragment | NodeData::ShadowRoot { .. } => {
                    for &grandchild in self.nodes[child_id].children.iter().rev() {
                        stack.push(grandchild);
                    }
                }
                // Skip Comment, Doctype, Document
                _ => {}
            }
        }
    }

    /// Returns the concatenation of all contiguous Text node siblings' data,
    /// including this node, in document order.
    ///
    /// Walks backwards from this node through previous siblings (stopping at
    /// non-Text nodes), then forwards through next siblings (stopping at non-Text
    /// nodes), collecting all text content.
    pub fn whole_text(&self, node_id: NodeId) -> String {
        // Verify this is a Text node
        if !matches!(&self.nodes[node_id].data, NodeData::Text { .. }) {
            return String::new();
        }

        let parent = self.nodes[node_id].parent;

        // If no parent, wholeText is just this node's text
        if parent.is_none() {
            return match &self.nodes[node_id].data {
                NodeData::Text { content } => content.clone(),
                _ => String::new(),
            };
        }

        let parent_id = parent.unwrap();
        let siblings = &self.nodes[parent_id].children;

        // Find position of this node in parent's children
        let pos = match self.find_child_index(parent_id, node_id) {
            Some(p) => p,
            None => {
                return match &self.nodes[node_id].data {
                    NodeData::Text { content } => content.clone(),
                    _ => String::new(),
                };
            }
        };

        // Walk backwards to find the start of the contiguous Text run
        let mut start = pos;
        while start > 0 {
            let prev = siblings[start - 1];
            if matches!(&self.nodes[prev].data, NodeData::Text { .. }) {
                start -= 1;
            } else {
                break;
            }
        }

        // Walk forwards to find the end of the contiguous Text run
        let mut end = pos;
        while end + 1 < siblings.len() {
            let next = siblings[end + 1];
            if matches!(&self.nodes[next].data, NodeData::Text { .. }) {
                end += 1;
            } else {
                break;
            }
        }

        // Concatenate all text content in the range [start..=end]
        let mut result = String::new();
        for sib in siblings.iter().take(end + 1).skip(start) {
            if let NodeData::Text { content } = &self.nodes[*sib].data {
                result.push_str(content);
            }
        }

        result
    }
}
