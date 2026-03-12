use super::node::{Node, NodeData, NodeId};

#[derive(Debug)]
pub struct DomTree {
    nodes: Vec<Node>,
}

impl DomTree {
    /// Creates a new DomTree with a Document root node at index 0.
    pub fn new() -> Self {
        let root = Node {
            id: 0,
            data: NodeData::Document,
            parent: None,
            children: Vec::new(),
            computed_style: None,
            template_contents: None,
        };
        DomTree { nodes: vec![root] }
    }

    /// Allocates a new Element node (unattached) and returns its NodeId.
    pub fn create_element(&mut self, tag_name: &str) -> NodeId {
        let id = self.nodes.len();
        self.nodes.push(Node {
            id,
            data: NodeData::Element {
                tag_name: tag_name.to_string(),
                attributes: Vec::new(),
                namespace: String::new(),
            },
            parent: None,
            children: Vec::new(),
            computed_style: None,
            template_contents: None,
        });
        id
    }

    /// Allocates a new Text node (unattached) and returns its NodeId.
    pub fn create_text(&mut self, content: &str) -> NodeId {
        let id = self.nodes.len();
        self.nodes.push(Node {
            id,
            data: NodeData::Text {
                content: content.to_string(),
            },
            parent: None,
            children: Vec::new(),
            computed_style: None,
            template_contents: None,
        });
        id
    }

    /// Appends `child` as the last child of `parent`.
    /// If the child already has a parent, it is first removed from that parent.
    pub fn append_child(&mut self, parent: NodeId, child: NodeId) {
        // Detach from current parent if any.
        if let Some(old_parent) = self.nodes[child].parent {
            self.nodes[old_parent].children.retain(|&c| c != child);
        }
        self.nodes[child].parent = Some(parent);
        self.nodes[parent].children.push(child);
    }

    /// Removes `child` from `parent`'s children list and clears the child's parent.
    pub fn remove_child(&mut self, parent: NodeId, child: NodeId) {
        self.nodes[parent].children.retain(|&c| c != child);
        self.nodes[child].parent = None;
    }

    pub fn get_node(&self, id: NodeId) -> &Node {
        &self.nodes[id]
    }

    pub fn get_node_mut(&mut self, id: NodeId) -> &mut Node {
        &mut self.nodes[id]
    }

    /// Searches the entire tree for an Element whose "id" attribute matches `id`.
    pub fn get_element_by_id(&self, id: &str) -> Option<NodeId> {
        self.nodes.iter().find_map(|node| {
            if let NodeData::Element { ref attributes, .. } = node.data {
                if attributes.iter().any(|(k, v)| k == "id" && v == id) {
                    return Some(node.id);
                }
            }
            None
        })
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

    /// Recursively collects all text content from Text node descendants.
    pub fn get_text_content(&self, node_id: NodeId) -> String {
        let node = &self.nodes[node_id];
        match &node.data {
            NodeData::Text { content } => content.clone(),
            _ => {
                let mut result = String::new();
                for &child_id in &node.children {
                    result.push_str(&self.get_text_content(child_id));
                }
                result
            }
        }
    }

    /// Removes all children of the node and replaces them with a single Text child.
    pub fn set_text_content(&mut self, node_id: NodeId, text: &str) {
        // Collect children to detach.
        let children: Vec<NodeId> = self.nodes[node_id].children.clone();
        for child_id in children {
            self.nodes[child_id].parent = None;
        }
        self.nodes[node_id].children.clear();

        // Create and append a new text node.
        let text_id = self.create_text(text);
        self.append_child(node_id, text_id);
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

    /// Allocates a new Comment node (unattached) and returns its NodeId.
    pub fn create_comment(&mut self, content: &str) -> NodeId {
        let id = self.nodes.len();
        self.nodes.push(Node {
            id,
            data: NodeData::Comment {
                content: content.to_string(),
            },
            parent: None,
            children: Vec::new(),
            computed_style: None,
            template_contents: None,
        });
        id
    }

    /// Allocates a new DocumentFragment node (unattached) and returns its NodeId.
    pub fn create_document_fragment(&mut self) -> NodeId {
        let id = self.nodes.len();
        self.nodes.push(Node {
            id,
            data: NodeData::DocumentFragment,
            parent: None,
            children: Vec::new(),
            computed_style: None,
            template_contents: None,
        });
        id
    }

    /// Allocates a new Element node with attributes (unattached) and returns its NodeId.
    pub fn create_element_with_attrs(
        &mut self,
        tag_name: &str,
        attributes: Vec<(String, String)>,
    ) -> NodeId {
        let id = self.nodes.len();
        self.nodes.push(Node {
            id,
            data: NodeData::Element {
                tag_name: tag_name.to_string(),
                attributes,
                namespace: String::new(),
            },
            parent: None,
            children: Vec::new(),
            computed_style: None,
            template_contents: None,
        });
        id
    }

    /// Allocates a new Doctype node (unattached) and returns its NodeId.
    pub fn create_doctype(&mut self, name: &str, public_id: &str, system_id: &str) -> NodeId {
        let id = self.nodes.len();
        self.nodes.push(Node {
            id,
            data: NodeData::Doctype {
                name: name.to_string(),
                public_id: public_id.to_string(),
                system_id: system_id.to_string(),
            },
            parent: None,
            children: Vec::new(),
            computed_style: None,
            template_contents: None,
        });
        id
    }

    /// Allocates a new Element node with namespace (unattached) and returns its NodeId.
    pub fn create_element_ns(
        &mut self,
        tag_name: &str,
        attributes: Vec<(String, String)>,
        namespace: &str,
    ) -> NodeId {
        let id = self.nodes.len();
        self.nodes.push(Node {
            id,
            data: NodeData::Element {
                tag_name: tag_name.to_string(),
                attributes,
                namespace: namespace.to_string(),
            },
            parent: None,
            children: Vec::new(),
            computed_style: None,
            template_contents: None,
        });
        id
    }

    /// Creates a template content fragment node (Document-like container).
    /// Returns the NodeId of the new fragment.
    pub fn create_template_contents(&mut self) -> NodeId {
        let id = self.nodes.len();
        self.nodes.push(Node {
            id,
            data: NodeData::Document, // content fragment acts like a document fragment
            parent: None,
            children: Vec::new(),
            computed_style: None,
            template_contents: None,
        });
        id
    }

    /// Inserts `child` as a sibling immediately before `sibling`.
    /// If `child` already has a parent, it is first detached.
    pub fn insert_before(&mut self, sibling: NodeId, child: NodeId) {
        let parent = self.nodes[sibling]
            .parent
            .expect("insert_before: sibling has no parent");

        // Detach child from current parent if any.
        if let Some(old_parent) = self.nodes[child].parent {
            self.nodes[old_parent].children.retain(|&c| c != child);
        }

        // Find sibling's position in parent's children list and insert before it.
        let pos = self.nodes[parent]
            .children
            .iter()
            .position(|&c| c == sibling)
            .expect("insert_before: sibling not found in parent's children");
        self.nodes[parent].children.insert(pos, child);
        self.nodes[child].parent = Some(parent);
    }

    /// Removes a node from its parent (if it has one).
    pub fn remove_from_parent(&mut self, target: NodeId) {
        if let Some(parent) = self.nodes[target].parent {
            self.nodes[parent].children.retain(|&c| c != target);
            self.nodes[target].parent = None;
        }
    }

    /// Moves all children of `source` to become children of `new_parent`.
    pub fn reparent_children(&mut self, source: NodeId, new_parent: NodeId) {
        let children: Vec<NodeId> = self.nodes[source].children.clone();
        self.nodes[source].children.clear();
        for child_id in children {
            self.nodes[child_id].parent = Some(new_parent);
            self.nodes[new_parent].children.push(child_id);
        }
    }

    /// Returns the total number of nodes in the tree.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// If the node is a Text node, appends `extra` to its content. Returns true if successful.
    pub fn append_to_text(&mut self, node_id: NodeId, extra: &str) -> bool {
        if let NodeData::Text { ref mut content } = self.nodes[node_id].data {
            content.push_str(extra);
            true
        } else {
            false
        }
    }


    /// Inserts new_child before reference_child in parent's children list.
    /// If new_child already has a parent, it is first detached.
    /// Panics if reference_child is not found in parent's children.
    pub fn insert_child_before(
        &mut self,
        parent: NodeId,
        new_child: NodeId,
        reference_child: NodeId,
    ) {
        // Detach new_child from its current parent if any.
        if let Some(old_parent) = self.nodes[new_child].parent {
            self.nodes[old_parent].children.retain(|&c| c != new_child);
        }

        // Find reference_child position in parent's children list and insert before it.
        let pos = self.nodes[parent]
            .children
            .iter()
            .position(|&c| c == reference_child)
            .expect("insert_child_before: reference_child not found in parent's children");
        self.nodes[parent].children.insert(pos, new_child);
        self.nodes[new_child].parent = Some(parent);
    }

    /// Replaces old_child with new_child in parent's children list.
    /// If new_child already has a parent, it is first detached.
    /// Clears old_child's parent. Panics if old_child is not in parent's children.
    pub fn replace_child(
        &mut self,
        parent: NodeId,
        new_child: NodeId,
        old_child: NodeId,
    ) {
        // Detach new_child from its current parent if any.
        if let Some(old_parent) = self.nodes[new_child].parent {
            self.nodes[old_parent].children.retain(|&c| c != new_child);
        }

        // Find old_child position and replace it.
        let pos = self.nodes[parent]
            .children
            .iter()
            .position(|&c| c == old_child)
            .expect("replace_child: old_child not found in parent's children");
        self.nodes[parent].children[pos] = new_child;
        self.nodes[new_child].parent = Some(parent);
        self.nodes[old_child].parent = None;
    }

    /// Clones a node and optionally all its descendants.
    /// The cloned node has no parent (unattached).
    /// If deep is true, all children are recursively cloned and attached to the clone.
    pub fn clone_node(&mut self, node_id: NodeId, deep: bool) -> NodeId {
        let data = self.nodes[node_id].data.clone();
        let new_id = self.nodes.len();
        self.nodes.push(Node {
            id: new_id,
            data,
            parent: None,
            children: Vec::new(),
            computed_style: None,
            template_contents: None,
        });

        if deep {
            let child_ids: Vec<NodeId> = self.nodes[node_id].children.clone();
            for child_id in child_ids {
                let cloned_child = self.clone_node(child_id, true);
                self.append_child(new_id, cloned_child);
            }
        }

        new_id
    }

    fn find_element_by_tag(&self, tag: &str) -> Option<NodeId> {
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

    fn is_void_element(tag: &str) -> bool {
        matches!(tag.to_ascii_lowercase().as_str(),
            "area"|"base"|"br"|"col"|"embed"|"hr"|"img"|"input"|
            "link"|"meta"|"param"|"source"|"track"|"wbr")
    }

    fn escape_html(text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        for ch in text.chars() {
            match ch {
                '&' => out.push_str("&amp;"),
                '<' => out.push_str("&lt;"),
                '>' => out.push_str("&gt;"),
                '"' => out.push_str("&quot;"),
                _ => out.push(ch),
            }
        }
        out
    }

    pub fn serialize_children_html(&self, nid: NodeId) -> String {
        let mut out = String::new();
        for &child in &self.nodes[nid].children {
            out.push_str(&self.serialize_node_html(child));
        }
        out
    }

    pub fn serialize_node_html(&self, nid: NodeId) -> String {
        let nd = &self.nodes[nid];
        match &nd.data {
            NodeData::Text { content } => Self::escape_html(content),
            NodeData::Comment { content } => format!("<!--{}-->", content),
            NodeData::Doctype { name, .. } => format!("<!DOCTYPE {}>", name),
            NodeData::Element { tag_name, attributes, .. } => {
                let mut o = String::new();
                o.push('<');
                o.push_str(tag_name);
                for (k, v) in attributes {
                    o.push(' ');
                    o.push_str(k);
                    o.push_str("=\"");
                    o.push_str(&Self::escape_html(v));
                    o.push('"');
                }
                o.push('>');
                if Self::is_void_element(tag_name) { return o; }
                for &c in &nd.children {
                    o.push_str(&self.serialize_node_html(c));
                }
                o.push_str("</");
                o.push_str(tag_name);
                o.push('>');
                o
            }
            NodeData::Document | NodeData::DocumentFragment => self.serialize_children_html(nid),
        }
    }

    pub fn clear_children(&mut self, nid: NodeId) {
        let children: Vec<NodeId> = self.nodes[nid].children.clone();
        for child_id in children {
            self.nodes[child_id].parent = None;
        }
        self.nodes[nid].children.clear();
    }

    pub fn import_subtree(&mut self, source: &DomTree, src_nid: NodeId) -> NodeId {
        let src_node = source.get_node(src_nid);
        let new_id = match &src_node.data {
            NodeData::Element { tag_name, attributes, namespace } => {
                self.create_element_ns(tag_name, attributes.clone(), namespace)
            }
            NodeData::Text { content } => self.create_text(content),
            NodeData::Comment { content } => self.create_comment(content),
            NodeData::Doctype { name, public_id, system_id } => {
                self.create_doctype(name, public_id, system_id)
            }
            NodeData::Document => panic!("cannot import Document node"),
            NodeData::DocumentFragment => panic!("cannot import DocumentFragment node"),
        };
        let src_children: Vec<NodeId> = src_node.children.clone();
        for &child_id in &src_children {
            let new_child = self.import_subtree(source, child_id);
            self.append_child(new_id, new_child);
        }
        new_id
    }

    pub fn insert_after(&mut self, sibling: NodeId, child: NodeId) {
        let parent = self.nodes[sibling]
            .parent
            .expect("insert_after: sibling has no parent");
        if let Some(old_parent) = self.nodes[child].parent {
            self.nodes[old_parent].children.retain(|&c| c != child);
        }
        let pos = self.nodes[parent]
            .children
            .iter()
            .position(|&c| c == sibling)
            .expect("insert_after: sibling not found");
        self.nodes[parent].children.insert(pos + 1, child);
        self.nodes[child].parent = Some(parent);
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_tree_has_document_root() {
        let tree = DomTree::new();
        let root = tree.get_node(tree.document());
        assert!(matches!(root.data, NodeData::Document));
        assert_eq!(root.id, 0);
        assert!(root.parent.is_none());
        assert!(root.children.is_empty());
    }

    #[test]
    fn create_and_append_children() {
        let mut tree = DomTree::new();
        let html = tree.create_element("html");
        let body = tree.create_element("body");
        let p = tree.create_element("p");

        tree.append_child(tree.document(), html);
        tree.append_child(html, body);
        tree.append_child(body, p);

        // Verify parent links
        assert_eq!(tree.get_node(html).parent, Some(0));
        assert_eq!(tree.get_node(body).parent, Some(html));
        assert_eq!(tree.get_node(p).parent, Some(body));

        // Verify children lists
        assert_eq!(tree.get_node(tree.document()).children, vec![html]);
        assert_eq!(tree.get_node(html).children, vec![body]);
        assert_eq!(tree.get_node(body).children, vec![p]);
    }

    #[test]
    fn append_child_detaches_from_old_parent() {
        let mut tree = DomTree::new();
        let div1 = tree.create_element("div");
        let div2 = tree.create_element("div");
        let span = tree.create_element("span");

        tree.append_child(tree.document(), div1);
        tree.append_child(tree.document(), div2);
        tree.append_child(div1, span);

        assert_eq!(tree.get_node(span).parent, Some(div1));
        assert_eq!(tree.get_node(div1).children, vec![span]);

        // Move span from div1 to div2
        tree.append_child(div2, span);

        assert_eq!(tree.get_node(span).parent, Some(div2));
        assert_eq!(tree.get_node(div2).children, vec![span]);
        assert!(tree.get_node(div1).children.is_empty());
    }

    #[test]
    fn remove_child_clears_relationship() {
        let mut tree = DomTree::new();
        let div = tree.create_element("div");
        let span = tree.create_element("span");

        tree.append_child(tree.document(), div);
        tree.append_child(div, span);

        assert_eq!(tree.get_node(div).children, vec![span]);
        assert_eq!(tree.get_node(span).parent, Some(div));

        tree.remove_child(div, span);

        assert!(tree.get_node(div).children.is_empty());
        assert!(tree.get_node(span).parent.is_none());
    }

    #[test]
    fn get_element_by_id_finds_matching_attribute() {
        let mut tree = DomTree::new();
        let div = tree.create_element("div");
        tree.append_child(tree.document(), div);

        // Add an "id" attribute
        if let NodeData::Element { ref mut attributes, .. } = tree.get_node_mut(div).data {
            attributes.push(("id".to_string(), "main".to_string()));
        }

        assert_eq!(tree.get_element_by_id("main"), Some(div));
        assert_eq!(tree.get_element_by_id("nonexistent"), None);
    }

    #[test]
    fn get_elements_by_tag_name_is_case_insensitive() {
        let mut tree = DomTree::new();
        let div1 = tree.create_element("div");
        let div2 = tree.create_element("DIV");
        let span = tree.create_element("span");

        tree.append_child(tree.document(), div1);
        tree.append_child(tree.document(), div2);
        tree.append_child(tree.document(), span);

        let divs = tree.get_elements_by_tag_name("div");
        assert_eq!(divs, vec![div1, div2]);

        let spans = tree.get_elements_by_tag_name("SPAN");
        assert_eq!(spans, vec![span]);
    }

    #[test]
    fn get_text_content_collects_recursively() {
        let mut tree = DomTree::new();
        let div = tree.create_element("div");
        let t1 = tree.create_text("Hello, ");
        let span = tree.create_element("span");
        let t2 = tree.create_text("world");
        let t3 = tree.create_text("!");

        tree.append_child(tree.document(), div);
        tree.append_child(div, t1);
        tree.append_child(div, span);
        tree.append_child(span, t2);
        tree.append_child(div, t3);

        assert_eq!(tree.get_text_content(div), "Hello, world!");
        assert_eq!(tree.get_text_content(span), "world");
    }

    #[test]
    fn set_text_content_replaces_children() {
        let mut tree = DomTree::new();
        let div = tree.create_element("div");
        let span = tree.create_element("span");
        let t1 = tree.create_text("old text");

        tree.append_child(tree.document(), div);
        tree.append_child(div, span);
        tree.append_child(span, t1);

        tree.set_text_content(div, "new text");

        assert_eq!(tree.get_text_content(div), "new text");
        // The old span should be detached
        assert!(tree.get_node(span).parent.is_none());
        // div should have exactly one child (the new text node)
        assert_eq!(tree.get_node(div).children.len(), 1);
    }

    #[test]
    fn body_and_head_find_elements() {
        let mut tree = DomTree::new();
        let html = tree.create_element("html");
        let head = tree.create_element("head");
        let body = tree.create_element("body");

        tree.append_child(tree.document(), html);
        tree.append_child(html, head);
        tree.append_child(html, body);

        assert_eq!(tree.head(), Some(head));
        assert_eq!(tree.body(), Some(body));
    }

    #[test]
    fn body_and_head_return_none_when_absent() {
        let tree = DomTree::new();
        assert_eq!(tree.head(), None);
        assert_eq!(tree.body(), None);
    }

    #[test]
    fn insert_child_before_inserts_at_correct_position() {
        let mut tree = DomTree::new();
        let div = tree.create_element("div");
        let a = tree.create_element("a");
        let b = tree.create_element("b");
        let c = tree.create_element("c");

        tree.append_child(tree.document(), div);
        tree.append_child(div, a);
        tree.append_child(div, c);

        tree.insert_child_before(div, b, c);

        assert_eq!(tree.get_node(div).children, vec![a, b, c]);
        assert_eq!(tree.get_node(b).parent, Some(div));
    }

    #[test]
    fn insert_child_before_detaches_from_old_parent() {
        let mut tree = DomTree::new();
        let div1 = tree.create_element("div");
        let div2 = tree.create_element("div");
        let a = tree.create_element("a");
        let b = tree.create_element("b");

        tree.append_child(tree.document(), div1);
        tree.append_child(tree.document(), div2);
        tree.append_child(div1, a);
        tree.append_child(div2, b);

        tree.insert_child_before(div2, a, b);

        assert!(tree.get_node(div1).children.is_empty());
        assert_eq!(tree.get_node(div2).children, vec![a, b]);
        assert_eq!(tree.get_node(a).parent, Some(div2));
    }

    #[test]
    fn replace_child_swaps_correctly() {
        let mut tree = DomTree::new();
        let div = tree.create_element("div");
        let old = tree.create_element("old");
        let new_el = tree.create_element("new");

        tree.append_child(tree.document(), div);
        tree.append_child(div, old);

        tree.replace_child(div, new_el, old);

        assert_eq!(tree.get_node(div).children, vec![new_el]);
        assert_eq!(tree.get_node(new_el).parent, Some(div));
        assert!(tree.get_node(old).parent.is_none());
    }

    #[test]
    fn replace_child_detaches_new_child_from_old_parent() {
        let mut tree = DomTree::new();
        let div1 = tree.create_element("div");
        let div2 = tree.create_element("div");
        let old_child = tree.create_element("old");
        let new_child = tree.create_element("new");

        tree.append_child(tree.document(), div1);
        tree.append_child(tree.document(), div2);
        tree.append_child(div1, new_child);
        tree.append_child(div2, old_child);

        tree.replace_child(div2, new_child, old_child);

        assert!(tree.get_node(div1).children.is_empty());
        assert_eq!(tree.get_node(div2).children, vec![new_child]);
        assert_eq!(tree.get_node(new_child).parent, Some(div2));
        assert!(tree.get_node(old_child).parent.is_none());
    }

    #[test]
    fn clone_node_shallow_no_children() {
        let mut tree = DomTree::new();
        let div = tree.create_element_with_attrs("div", vec![
            ("class".to_string(), "container".to_string()),
        ]);
        let span = tree.create_element("span");
        tree.append_child(div, span);

        let cloned = tree.clone_node(div, false);

        assert_ne!(cloned, div);
        assert!(tree.get_node(cloned).children.is_empty());
        assert!(tree.get_node(cloned).parent.is_none());
        match &tree.get_node(cloned).data {
            NodeData::Element { tag_name, attributes, .. } => {
                assert_eq!(tag_name, "div");
                assert_eq!(attributes, &vec![("class".to_string(), "container".to_string())]);
            }
            _ => panic!("expected Element"),
        }
    }

    #[test]
    fn clone_node_deep_clones_descendants() {
        let mut tree = DomTree::new();
        let div = tree.create_element("div");
        let span = tree.create_element("span");
        let text = tree.create_text("hello");

        tree.append_child(div, span);
        tree.append_child(span, text);

        let cloned = tree.clone_node(div, true);

        assert_eq!(tree.get_node(cloned).children.len(), 1);
        let cloned_span = tree.get_node(cloned).children[0];
        assert_ne!(cloned_span, span);
        assert_eq!(tree.get_node(cloned_span).children.len(), 1);
        let cloned_text = tree.get_node(cloned_span).children[0];
        assert_ne!(cloned_text, text);
        assert_eq!(tree.get_text_content(cloned), "hello");
        assert!(tree.get_node(cloned).parent.is_none());
        assert_eq!(tree.get_node(cloned_span).parent, Some(cloned));
        assert_eq!(tree.get_node(cloned_text).parent, Some(cloned_span));
    }

    #[test]
    fn clone_node_preserves_attributes() {
        let mut tree = DomTree::new();
        let div = tree.create_element_with_attrs("div", vec![
            ("id".to_string(), "main".to_string()),
            ("class".to_string(), "container".to_string()),
            ("data-x".to_string(), "42".to_string()),
        ]);

        let cloned = tree.clone_node(div, false);

        match &tree.get_node(cloned).data {
            NodeData::Element { tag_name, attributes, .. } => {
                assert_eq!(tag_name, "div");
                assert_eq!(attributes.len(), 3);
                assert!(attributes.contains(&("id".to_string(), "main".to_string())));
                assert!(attributes.contains(&("class".to_string(), "container".to_string())));
                assert!(attributes.contains(&("data-x".to_string(), "42".to_string())));
            }
            _ => panic!("expected Element"),
        }
    }

    #[test]
    fn clone_node_text_node() {
        let mut tree = DomTree::new();
        let text = tree.create_text("hello world");

        let cloned = tree.clone_node(text, false);

        assert_ne!(cloned, text);
        match &tree.get_node(cloned).data {
            NodeData::Text { content } => assert_eq!(content, "hello world"),
            _ => panic!("expected Text"),
        }
        assert!(tree.get_node(cloned).parent.is_none());
    }
}
