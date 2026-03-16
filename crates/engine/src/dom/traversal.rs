use super::node::{NodeData, NodeId};
use super::tree::DomTree;

impl DomTree {
    /// Returns the parent NodeId if the node has one.
    pub fn get_parent(&self, node_id: NodeId) -> Option<NodeId> {
        self.get_node(node_id).parent
    }

    /// Walks up the tree from node_id, returns the first ancestor Element whose tag_name matches (case-insensitive).
    /// Does NOT check the node itself, starts from parent.
    pub fn find_ancestor(&self, node_id: NodeId, tag: &str) -> Option<NodeId> {
        let tag_lower = tag.to_ascii_lowercase();
        let mut current = self.get_parent(node_id);

        while let Some(current_id) = current {
            let node = self.get_node(current_id);
            if let NodeData::Element { ref tag_name, .. } = node.data {
                if tag_name.to_ascii_lowercase() == tag_lower {
                    return Some(current_id);
                }
            }
            current = node.parent;
        }

        None
    }

    /// Recursively finds all descendant Elements matching the tag (case-insensitive).
    /// Does NOT include the root itself.
    pub fn find_descendants_by_tag(&self, root: NodeId, tag: &str) -> Vec<NodeId> {
        let tag_lower = tag.to_ascii_lowercase();
        let mut results = Vec::new();
        self.find_descendants_by_tag_recursive(root, &tag_lower, &mut results);
        results
    }

    fn find_descendants_by_tag_recursive(&self, node_id: NodeId, tag_lower: &str, results: &mut Vec<NodeId>) {
        let node = self.get_node(node_id);
        for &child_id in &node.children {
            let child = self.get_node(child_id);
            if let NodeData::Element { ref tag_name, .. } = child.data {
                if tag_name.to_ascii_lowercase() == tag_lower {
                    results.push(child_id);
                }
            }
            self.find_descendants_by_tag_recursive(child_id, tag_lower, results);
        }
    }

    /// Returns first child if any.
    pub fn first_child(&self, node_id: NodeId) -> Option<NodeId> {
        self.get_node(node_id).children.first().copied()
    }

    /// Returns last child if any.
    pub fn last_child(&self, node_id: NodeId) -> Option<NodeId> {
        self.get_node(node_id).children.last().copied()
    }

    /// Returns the next sibling in parent's children list.
    pub fn next_sibling(&self, node_id: NodeId) -> Option<NodeId> {
        let node = self.get_node(node_id);
        let parent_id = node.parent?;
        let parent = self.get_node(parent_id);

        let pos = parent.children.iter().position(|&c| c == node_id)?;
        parent.children.get(pos + 1).copied()
    }

    /// Returns the previous sibling in parent's children list.
    pub fn prev_sibling(&self, node_id: NodeId) -> Option<NodeId> {
        let node = self.get_node(node_id);
        let parent_id = node.parent?;
        let parent = self.get_node(parent_id);

        let pos = parent.children.iter().position(|&c| c == node_id)?;
        if pos > 0 {
            parent.children.get(pos - 1).copied()
        } else {
            None
        }
    }

    /// Returns a clone of the children Vec.
    pub fn children(&self, node_id: NodeId) -> Vec<NodeId> {
        self.get_node(node_id).children.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_parent_returns_parent() {
        let mut tree = DomTree::new();
        let div = tree.create_element("div");
        let span = tree.create_element("span");

        tree.append_child(tree.document(), div);
        tree.append_child(div, span);

        assert_eq!(tree.get_parent(span), Some(div));
        assert_eq!(tree.get_parent(div), Some(tree.document()));
    }

    #[test]
    fn get_parent_returns_none_for_root() {
        let tree = DomTree::new();
        assert_eq!(tree.get_parent(tree.document()), None);
    }

    #[test]
    fn get_parent_returns_none_for_detached_node() {
        let mut tree = DomTree::new();
        let div = tree.create_element("div");
        assert_eq!(tree.get_parent(div), None);
    }

    #[test]
    fn find_ancestor_finds_matching_ancestor() {
        let mut tree = DomTree::new();
        let html = tree.create_element("html");
        let body = tree.create_element("body");
        let div = tree.create_element("div");
        let span = tree.create_element("span");

        tree.append_child(tree.document(), html);
        tree.append_child(html, body);
        tree.append_child(body, div);
        tree.append_child(div, span);

        assert_eq!(tree.find_ancestor(span, "div"), Some(div));
        assert_eq!(tree.find_ancestor(span, "body"), Some(body));
        assert_eq!(tree.find_ancestor(span, "html"), Some(html));
    }

    #[test]
    fn find_ancestor_is_case_insensitive() {
        let mut tree = DomTree::new();
        let html = tree.create_element("HTML");
        let div = tree.create_element("div");

        tree.append_child(tree.document(), html);
        tree.append_child(html, div);

        assert_eq!(tree.find_ancestor(div, "html"), Some(html));
        assert_eq!(tree.find_ancestor(div, "HTML"), Some(html));
        assert_eq!(tree.find_ancestor(div, "HtMl"), Some(html));
    }

    #[test]
    fn find_ancestor_skips_non_matching() {
        let mut tree = DomTree::new();
        let html = tree.create_element("html");
        let body = tree.create_element("body");
        let div = tree.create_element("div");
        let span = tree.create_element("span");

        tree.append_child(tree.document(), html);
        tree.append_child(html, body);
        tree.append_child(body, div);
        tree.append_child(div, span);

        // Looking for 'ul' should skip div and body, return None
        assert_eq!(tree.find_ancestor(span, "ul"), None);
    }

    #[test]
    fn find_ancestor_returns_none_at_root() {
        let mut tree = DomTree::new();
        let html = tree.create_element("html");
        tree.append_child(tree.document(), html);

        // Document is not an Element, so can't match
        assert_eq!(tree.find_ancestor(html, "document"), None);
        assert_eq!(tree.find_ancestor(html, "html"), None);
    }

    #[test]
    fn find_ancestor_does_not_check_self() {
        let mut tree = DomTree::new();
        let div = tree.create_element("div");
        tree.append_child(tree.document(), div);

        // Should not find itself
        assert_eq!(tree.find_ancestor(div, "div"), None);
    }

    #[test]
    fn find_descendants_by_tag_collects_all_matches() {
        let mut tree = DomTree::new();
        let html = tree.create_element("html");
        let body = tree.create_element("body");
        let div1 = tree.create_element("div");
        let div2 = tree.create_element("div");
        let span = tree.create_element("span");
        let div3 = tree.create_element("div");

        tree.append_child(tree.document(), html);
        tree.append_child(html, body);
        tree.append_child(body, div1);
        tree.append_child(body, div2);
        tree.append_child(div1, span);
        tree.append_child(span, div3);

        let divs = tree.find_descendants_by_tag(body, "div");
        assert_eq!(divs.len(), 3);
        assert!(divs.contains(&div1));
        assert!(divs.contains(&div2));
        assert!(divs.contains(&div3));
    }

    #[test]
    fn find_descendants_by_tag_excludes_root() {
        let mut tree = DomTree::new();
        let div = tree.create_element("div");
        let span = tree.create_element("span");

        tree.append_child(tree.document(), div);
        tree.append_child(div, span);

        let divs = tree.find_descendants_by_tag(div, "div");
        assert_eq!(divs.len(), 0);
    }

    #[test]
    fn find_descendants_by_tag_is_case_insensitive() {
        let mut tree = DomTree::new();
        let body = tree.create_element("body");
        let div1 = tree.create_element("DIV");
        let div2 = tree.create_element("div");

        tree.append_child(tree.document(), body);
        tree.append_child(body, div1);
        tree.append_child(body, div2);

        let divs = tree.find_descendants_by_tag(body, "div");
        assert_eq!(divs.len(), 2);
        assert!(divs.contains(&div1));
        assert!(divs.contains(&div2));
    }

    #[test]
    fn find_descendants_by_tag_returns_empty_when_no_matches() {
        let mut tree = DomTree::new();
        let div = tree.create_element("div");
        let span = tree.create_element("span");

        tree.append_child(tree.document(), div);
        tree.append_child(div, span);

        let ps = tree.find_descendants_by_tag(div, "p");
        assert_eq!(ps.len(), 0);
    }

    #[test]
    fn first_child_returns_first() {
        let mut tree = DomTree::new();
        let div = tree.create_element("div");
        let span1 = tree.create_element("span");
        let span2 = tree.create_element("span");
        let span3 = tree.create_element("span");

        tree.append_child(tree.document(), div);
        tree.append_child(div, span1);
        tree.append_child(div, span2);
        tree.append_child(div, span3);

        assert_eq!(tree.first_child(div), Some(span1));
    }

    #[test]
    fn first_child_returns_none_when_empty() {
        let mut tree = DomTree::new();
        let div = tree.create_element("div");
        tree.append_child(tree.document(), div);

        assert_eq!(tree.first_child(div), None);
    }

    #[test]
    fn last_child_returns_last() {
        let mut tree = DomTree::new();
        let div = tree.create_element("div");
        let span1 = tree.create_element("span");
        let span2 = tree.create_element("span");
        let span3 = tree.create_element("span");

        tree.append_child(tree.document(), div);
        tree.append_child(div, span1);
        tree.append_child(div, span2);
        tree.append_child(div, span3);

        assert_eq!(tree.last_child(div), Some(span3));
    }

    #[test]
    fn last_child_returns_none_when_empty() {
        let mut tree = DomTree::new();
        let div = tree.create_element("div");
        tree.append_child(tree.document(), div);

        assert_eq!(tree.last_child(div), None);
    }

    #[test]
    fn first_and_last_child_same_for_single_child() {
        let mut tree = DomTree::new();
        let div = tree.create_element("div");
        let span = tree.create_element("span");

        tree.append_child(tree.document(), div);
        tree.append_child(div, span);

        assert_eq!(tree.first_child(div), Some(span));
        assert_eq!(tree.last_child(div), Some(span));
    }

    #[test]
    fn next_sibling_returns_next() {
        let mut tree = DomTree::new();
        let div = tree.create_element("div");
        let span1 = tree.create_element("span");
        let span2 = tree.create_element("span");
        let span3 = tree.create_element("span");

        tree.append_child(tree.document(), div);
        tree.append_child(div, span1);
        tree.append_child(div, span2);
        tree.append_child(div, span3);

        assert_eq!(tree.next_sibling(span1), Some(span2));
        assert_eq!(tree.next_sibling(span2), Some(span3));
    }

    #[test]
    fn next_sibling_returns_none_at_end() {
        let mut tree = DomTree::new();
        let div = tree.create_element("div");
        let span1 = tree.create_element("span");
        let span2 = tree.create_element("span");

        tree.append_child(tree.document(), div);
        tree.append_child(div, span1);
        tree.append_child(div, span2);

        assert_eq!(tree.next_sibling(span2), None);
    }

    #[test]
    fn next_sibling_returns_none_for_detached_node() {
        let mut tree = DomTree::new();
        let div = tree.create_element("div");

        assert_eq!(tree.next_sibling(div), None);
    }

    #[test]
    fn prev_sibling_returns_previous() {
        let mut tree = DomTree::new();
        let div = tree.create_element("div");
        let span1 = tree.create_element("span");
        let span2 = tree.create_element("span");
        let span3 = tree.create_element("span");

        tree.append_child(tree.document(), div);
        tree.append_child(div, span1);
        tree.append_child(div, span2);
        tree.append_child(div, span3);

        assert_eq!(tree.prev_sibling(span3), Some(span2));
        assert_eq!(tree.prev_sibling(span2), Some(span1));
    }

    #[test]
    fn prev_sibling_returns_none_at_start() {
        let mut tree = DomTree::new();
        let div = tree.create_element("div");
        let span1 = tree.create_element("span");
        let span2 = tree.create_element("span");

        tree.append_child(tree.document(), div);
        tree.append_child(div, span1);
        tree.append_child(div, span2);

        assert_eq!(tree.prev_sibling(span1), None);
    }

    #[test]
    fn prev_sibling_returns_none_for_detached_node() {
        let mut tree = DomTree::new();
        let div = tree.create_element("div");

        assert_eq!(tree.prev_sibling(div), None);
    }

    #[test]
    fn children_returns_clone_of_children_list() {
        let mut tree = DomTree::new();
        let div = tree.create_element("div");
        let span1 = tree.create_element("span");
        let span2 = tree.create_element("span");
        let span3 = tree.create_element("span");

        tree.append_child(tree.document(), div);
        tree.append_child(div, span1);
        tree.append_child(div, span2);
        tree.append_child(div, span3);

        let children = tree.children(div);
        assert_eq!(children, vec![span1, span2, span3]);
    }

    #[test]
    fn children_returns_empty_vec_when_no_children() {
        let mut tree = DomTree::new();
        let div = tree.create_element("div");
        tree.append_child(tree.document(), div);

        let children = tree.children(div);
        assert_eq!(children, Vec::<NodeId>::new());
    }

    #[test]
    fn children_is_cloned_not_referenced() {
        let mut tree = DomTree::new();
        let div = tree.create_element("div");
        let span1 = tree.create_element("span");

        tree.append_child(tree.document(), div);
        tree.append_child(div, span1);

        let mut children = tree.children(div);
        children.push(999); // Modify the returned vec

        // Original should be unchanged
        assert_eq!(tree.get_node(div).children, vec![span1]);
    }
}
