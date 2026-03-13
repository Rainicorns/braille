use super::node::{NodeData, NodeId};
use super::tree::DomTree;

impl DomTree {
    /// Walks up from node_id and returns the first ancestor that is an Element.
    /// Skips Document, Text, and Comment nodes.
    pub fn parent_element(&self, node_id: NodeId) -> Option<NodeId> {
        let mut current = self.get_node(node_id).parent?;
        loop {
            let node = self.get_node(current);
            if matches!(node.data, NodeData::Element { .. }) {
                return Some(current);
            }
            current = node.parent?;
        }
    }

    /// DOM spec `parentElement`: returns the immediate parent if it is an Element, else None.
    /// Unlike `parent_element()` (used for CSS matching), this does NOT walk up past non-Element nodes.
    pub fn dom_parent_element(&self, node_id: NodeId) -> Option<NodeId> {
        let parent_id = self.get_node(node_id).parent?;
        let parent_node = self.get_node(parent_id);
        if matches!(parent_node.data, NodeData::Element { .. }) {
            Some(parent_id)
        } else {
            None
        }
    }

    /// Walks backwards through siblings and returns the first sibling that is an Element.
    /// Returns None if there are no previous element siblings.
    pub fn prev_sibling_element(&self, node_id: NodeId) -> Option<NodeId> {
        let parent_id = self.get_node(node_id).parent?;
        let parent = self.get_node(parent_id);
        let pos = parent.children.iter().position(|&c| c == node_id)?;

        // Walk backwards from current position
        for i in (0..pos).rev() {
            let sibling_id = parent.children[i];
            let sibling = self.get_node(sibling_id);
            if matches!(sibling.data, NodeData::Element { .. }) {
                return Some(sibling_id);
            }
        }
        None
    }

    /// Walks forward through siblings and returns the first sibling that is an Element.
    /// Returns None if there are no following element siblings.
    pub fn next_sibling_element(&self, node_id: NodeId) -> Option<NodeId> {
        let parent_id = self.get_node(node_id).parent?;
        let parent = self.get_node(parent_id);
        let pos = parent.children.iter().position(|&c| c == node_id)?;

        // Walk forward from current position
        for i in (pos + 1)..parent.children.len() {
            let sibling_id = parent.children[i];
            let sibling = self.get_node(sibling_id);
            if matches!(sibling.data, NodeData::Element { .. }) {
                return Some(sibling_id);
            }
        }
        None
    }

    /// Returns true if this element's parent is the Document node (i.e., it's the root element, typically <html>).
    pub fn is_root_element(&self, node_id: NodeId) -> bool {
        let node = self.get_node(node_id);
        if !matches!(node.data, NodeData::Element { .. }) {
            return false;
        }
        match node.parent {
            Some(parent_id) => matches!(self.get_node(parent_id).data, NodeData::Document),
            None => false,
        }
    }

    /// Returns only Element children, filtering out Text and Comment nodes.
    pub fn element_children(&self, node_id: NodeId) -> Vec<NodeId> {
        let node = self.get_node(node_id);
        node.children
            .iter()
            .copied()
            .filter(|&child_id| {
                matches!(self.get_node(child_id).data, NodeData::Element { .. })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parent_element_skips_non_element_nodes() {
        let mut tree = DomTree::new();
        let html = tree.create_element("html");
        let body = tree.create_element("body");
        let text = tree.create_text("some text");
        let span = tree.create_element("span");

        tree.append_child(tree.document(), html);
        tree.append_child(html, body);
        tree.append_child(body, text);
        tree.append_child(text, span); // span's parent is text node

        // span's parent_element should skip the text node and return body
        assert_eq!(tree.parent_element(span), Some(body));
    }

    #[test]
    fn parent_element_returns_none_at_document_root() {
        let mut tree = DomTree::new();
        let html = tree.create_element("html");
        tree.append_child(tree.document(), html);

        // html's parent is Document, so parent_element should return None
        assert_eq!(tree.parent_element(html), None);
    }

    #[test]
    fn prev_sibling_element_skips_text_nodes() {
        let mut tree = DomTree::new();
        let div = tree.create_element("div");
        let span1 = tree.create_element("span");
        let text = tree.create_text("text");
        let comment = tree.create_comment("comment");
        let span2 = tree.create_element("span");

        tree.append_child(tree.document(), div);
        tree.append_child(div, span1);
        tree.append_child(div, text);
        tree.append_child(div, comment);
        tree.append_child(div, span2);

        // span2's prev element sibling should skip text and comment, returning span1
        assert_eq!(tree.prev_sibling_element(span2), Some(span1));
    }

    #[test]
    fn prev_sibling_element_returns_none_for_first_sibling() {
        let mut tree = DomTree::new();
        let div = tree.create_element("div");
        let span = tree.create_element("span");
        let text = tree.create_text("text");

        tree.append_child(tree.document(), div);
        tree.append_child(div, span);
        tree.append_child(div, text);

        // span is the first (and only) element sibling
        assert_eq!(tree.prev_sibling_element(span), None);
    }

    #[test]
    fn next_sibling_element_skips_text_nodes() {
        let mut tree = DomTree::new();
        let div = tree.create_element("div");
        let span1 = tree.create_element("span");
        let text = tree.create_text("text");
        let comment = tree.create_comment("comment");
        let span2 = tree.create_element("span");

        tree.append_child(tree.document(), div);
        tree.append_child(div, span1);
        tree.append_child(div, text);
        tree.append_child(div, comment);
        tree.append_child(div, span2);

        // span1's next element sibling should skip text and comment, returning span2
        assert_eq!(tree.next_sibling_element(span1), Some(span2));
    }

    #[test]
    fn next_sibling_element_returns_none_for_last_sibling() {
        let mut tree = DomTree::new();
        let div = tree.create_element("div");
        let text = tree.create_text("text");
        let span = tree.create_element("span");

        tree.append_child(tree.document(), div);
        tree.append_child(div, text);
        tree.append_child(div, span);

        // span is the last (and only) element sibling
        assert_eq!(tree.next_sibling_element(span), None);
    }

    #[test]
    fn is_root_element_returns_true_for_html_false_for_body() {
        let mut tree = DomTree::new();
        let html = tree.create_element("html");
        let body = tree.create_element("body");

        tree.append_child(tree.document(), html);
        tree.append_child(html, body);

        assert!(tree.is_root_element(html));
        assert!(!tree.is_root_element(body));
    }

    #[test]
    fn is_root_element_returns_false_for_non_elements() {
        let mut tree = DomTree::new();
        let text = tree.create_text("text");
        tree.append_child(tree.document(), text);

        // Even though text is a direct child of Document, it's not an element
        assert!(!tree.is_root_element(text));
    }

    #[test]
    fn element_children_filters_out_text_nodes() {
        let mut tree = DomTree::new();
        let div = tree.create_element("div");
        let span1 = tree.create_element("span");
        let text = tree.create_text("text");
        let span2 = tree.create_element("span");
        let comment = tree.create_comment("comment");

        tree.append_child(tree.document(), div);
        tree.append_child(div, span1);
        tree.append_child(div, text);
        tree.append_child(div, span2);
        tree.append_child(div, comment);

        let element_kids = tree.element_children(div);
        assert_eq!(element_kids, vec![span1, span2]);
    }

    #[test]
    fn computed_style_defaults_to_none_for_new_nodes() {
        let mut tree = DomTree::new();
        let div = tree.create_element("div");
        let text = tree.create_text("text");

        assert!(tree.get_node(div).computed_style.is_none());
        assert!(tree.get_node(text).computed_style.is_none());
        assert!(tree.get_node(tree.document()).computed_style.is_none());
    }
}
