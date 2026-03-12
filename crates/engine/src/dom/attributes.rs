use super::node::{NodeData, NodeId};
use super::tree::DomTree;

impl DomTree {
    /// Returns the attribute value if the node is an Element and has that attribute, None otherwise.
    pub fn get_attribute(&self, node_id: NodeId, name: &str) -> Option<String> {
        let node = self.get_node(node_id);
        if let NodeData::Element { ref attributes, .. } = node.data {
            attributes
                .iter()
                .find(|(k, _)| k == name)
                .map(|(_, v)| v.clone())
        } else {
            None
        }
    }

    /// Sets the attribute on an Element node. If the attribute already exists, updates it.
    /// If not, adds it. Panics if node is not an Element.
    pub fn set_attribute(&mut self, node_id: NodeId, name: &str, value: &str) {
        let node = self.get_node_mut(node_id);
        if let NodeData::Element { ref mut attributes, .. } = node.data {
            // Try to find existing attribute and update it
            if let Some(existing) = attributes.iter_mut().find(|(k, _)| k == name) {
                existing.1 = value.to_string();
            } else {
                // Add new attribute
                attributes.push((name.to_string(), value.to_string()));
            }
        } else {
            panic!("set_attribute: node {} is not an Element", node_id);
        }
    }

    /// Removes the attribute if present. Returns true if it was removed, false if it wasn't there.
    /// Panics if node is not an Element.
    pub fn remove_attribute(&mut self, node_id: NodeId, name: &str) -> bool {
        let node = self.get_node_mut(node_id);
        if let NodeData::Element { ref mut attributes, .. } = node.data {
            let len_before = attributes.len();
            attributes.retain(|(k, _)| k != name);
            attributes.len() < len_before
        } else {
            panic!("remove_attribute: node {} is not an Element", node_id);
        }
    }

    /// Returns true if the Element has that attribute. Panics if node is not an Element.
    pub fn has_attribute(&self, node_id: NodeId, name: &str) -> bool {
        let node = self.get_node(node_id);
        if let NodeData::Element { ref attributes, .. } = node.data {
            attributes.iter().any(|(k, _)| k == name)
        } else {
            panic!("has_attribute: node {} is not an Element", node_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_attribute_returns_some_for_existing_attr() {
        let mut tree = DomTree::new();
        let div = tree.create_element_with_attrs("div", vec![
            ("class".to_string(), "container".to_string()),
            ("id".to_string(), "main".to_string()),
        ]);

        assert_eq!(tree.get_attribute(div, "class"), Some("container".to_string()));
        assert_eq!(tree.get_attribute(div, "id"), Some("main".to_string()));
    }

    #[test]
    fn get_attribute_returns_none_for_missing_attr() {
        let mut tree = DomTree::new();
        let div = tree.create_element_with_attrs("div", vec![
            ("class".to_string(), "container".to_string()),
        ]);

        assert_eq!(tree.get_attribute(div, "id"), None);
        assert_eq!(tree.get_attribute(div, "data-value"), None);
    }

    #[test]
    fn get_attribute_returns_none_for_non_element_node() {
        let mut tree = DomTree::new();
        let text = tree.create_text("hello");
        let comment = tree.create_comment("comment");

        assert_eq!(tree.get_attribute(text, "class"), None);
        assert_eq!(tree.get_attribute(comment, "id"), None);
        assert_eq!(tree.get_attribute(tree.document(), "attr"), None);
    }

    #[test]
    fn set_attribute_creates_new_attribute() {
        let mut tree = DomTree::new();
        let div = tree.create_element("div");

        tree.set_attribute(div, "class", "container");
        tree.set_attribute(div, "id", "main");

        assert_eq!(tree.get_attribute(div, "class"), Some("container".to_string()));
        assert_eq!(tree.get_attribute(div, "id"), Some("main".to_string()));
    }

    #[test]
    fn set_attribute_updates_existing_attribute() {
        let mut tree = DomTree::new();
        let div = tree.create_element_with_attrs("div", vec![
            ("class".to_string(), "old-value".to_string()),
        ]);

        assert_eq!(tree.get_attribute(div, "class"), Some("old-value".to_string()));

        tree.set_attribute(div, "class", "new-value");

        assert_eq!(tree.get_attribute(div, "class"), Some("new-value".to_string()));

        // Verify that we didn't add a duplicate attribute
        let node = tree.get_node(div);
        if let NodeData::Element { ref attributes, .. } = node.data {
            let class_count = attributes.iter().filter(|(k, _)| k == "class").count();
            assert_eq!(class_count, 1);
        }
    }

    #[test]
    #[should_panic(expected = "set_attribute: node")]
    fn set_attribute_panics_on_text_node() {
        let mut tree = DomTree::new();
        let text = tree.create_text("hello");
        tree.set_attribute(text, "class", "value");
    }

    #[test]
    #[should_panic(expected = "set_attribute: node")]
    fn set_attribute_panics_on_document() {
        let mut tree = DomTree::new();
        tree.set_attribute(tree.document(), "class", "value");
    }

    #[test]
    fn remove_attribute_removes_existing_returns_true() {
        let mut tree = DomTree::new();
        let div = tree.create_element_with_attrs("div", vec![
            ("class".to_string(), "container".to_string()),
            ("id".to_string(), "main".to_string()),
        ]);

        assert_eq!(tree.get_attribute(div, "class"), Some("container".to_string()));

        let removed = tree.remove_attribute(div, "class");

        assert!(removed);
        assert_eq!(tree.get_attribute(div, "class"), None);
        assert_eq!(tree.get_attribute(div, "id"), Some("main".to_string()));
    }

    #[test]
    fn remove_attribute_returns_false_for_missing() {
        let mut tree = DomTree::new();
        let div = tree.create_element_with_attrs("div", vec![
            ("class".to_string(), "container".to_string()),
        ]);

        let removed = tree.remove_attribute(div, "id");

        assert!(!removed);
        assert_eq!(tree.get_attribute(div, "class"), Some("container".to_string()));
    }

    #[test]
    #[should_panic(expected = "remove_attribute: node")]
    fn remove_attribute_panics_on_text_node() {
        let mut tree = DomTree::new();
        let text = tree.create_text("hello");
        tree.remove_attribute(text, "class");
    }

    #[test]
    #[should_panic(expected = "remove_attribute: node")]
    fn remove_attribute_panics_on_comment() {
        let mut tree = DomTree::new();
        let comment = tree.create_comment("test");
        tree.remove_attribute(comment, "class");
    }

    #[test]
    fn has_attribute_returns_true_for_existing() {
        let mut tree = DomTree::new();
        let div = tree.create_element_with_attrs("div", vec![
            ("class".to_string(), "container".to_string()),
            ("id".to_string(), "main".to_string()),
        ]);

        assert!(tree.has_attribute(div, "class"));
        assert!(tree.has_attribute(div, "id"));
    }

    #[test]
    fn has_attribute_returns_false_for_missing() {
        let mut tree = DomTree::new();
        let div = tree.create_element_with_attrs("div", vec![
            ("class".to_string(), "container".to_string()),
        ]);

        assert!(!tree.has_attribute(div, "id"));
        assert!(!tree.has_attribute(div, "data-value"));
    }

    #[test]
    #[should_panic(expected = "has_attribute: node")]
    fn has_attribute_panics_on_document() {
        let tree = DomTree::new();
        tree.has_attribute(tree.document(), "class");
    }

    #[test]
    #[should_panic(expected = "has_attribute: node")]
    fn has_attribute_panics_on_text_node() {
        let mut tree = DomTree::new();
        let text = tree.create_text("hello");
        tree.has_attribute(text, "class");
    }

    #[test]
    fn attribute_workflow_integration() {
        let mut tree = DomTree::new();
        let div = tree.create_element("div");

        // Initially no attributes
        assert!(!tree.has_attribute(div, "class"));
        assert_eq!(tree.get_attribute(div, "class"), None);

        // Set an attribute
        tree.set_attribute(div, "class", "container");
        assert!(tree.has_attribute(div, "class"));
        assert_eq!(tree.get_attribute(div, "class"), Some("container".to_string()));

        // Update the attribute
        tree.set_attribute(div, "class", "wrapper");
        assert!(tree.has_attribute(div, "class"));
        assert_eq!(tree.get_attribute(div, "class"), Some("wrapper".to_string()));

        // Add another attribute
        tree.set_attribute(div, "id", "main");
        assert!(tree.has_attribute(div, "id"));
        assert_eq!(tree.get_attribute(div, "id"), Some("main".to_string()));

        // Remove first attribute
        assert!(tree.remove_attribute(div, "class"));
        assert!(!tree.has_attribute(div, "class"));
        assert_eq!(tree.get_attribute(div, "class"), None);

        // Second attribute should still be there
        assert!(tree.has_attribute(div, "id"));
        assert_eq!(tree.get_attribute(div, "id"), Some("main".to_string()));

        // Try removing non-existent attribute
        assert!(!tree.remove_attribute(div, "data-value"));
    }
}
