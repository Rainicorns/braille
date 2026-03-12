use std::collections::HashMap;
use super::node::NodeId;
use super::tree::DomTree;

/// Looks up `@eN` style refs in a provided HashMap.
/// The ref_str should start with "@".
pub fn resolve_ref(ref_map: &HashMap<String, NodeId>, ref_str: &str) -> Option<NodeId> {
    if !ref_str.starts_with('@') {
        return None;
    }
    ref_map.get(ref_str).copied()
}

/// Resolves `#id` shorthand by stripping the leading "#" and calling `tree.get_element_by_id()`.
pub fn resolve_id(tree: &DomTree, id_str: &str) -> Option<NodeId> {
    if !id_str.starts_with('#') {
        return None;
    }
    let id = &id_str[1..];
    tree.get_element_by_id(id)
}

/// Finds the first element matching the tag name (case-insensitive).
/// Uses `tree.get_elements_by_tag_name()` and returns the first result.
pub fn resolve_tag(tree: &DomTree, tag: &str) -> Option<NodeId> {
    tree.get_elements_by_tag_name(tag).into_iter().next()
}

/// The main entry point. Tries resolution strategies in order:
/// - If selector starts with `@` → try `resolve_ref`
/// - If selector starts with `#` → try `resolve_id`
/// - Otherwise → try `resolve_tag`
/// Returns None if nothing matches.
pub fn resolve_selector(tree: &DomTree, ref_map: &HashMap<String, NodeId>, selector: &str) -> Option<NodeId> {
    if selector.starts_with('@') {
        resolve_ref(ref_map, selector)
    } else if selector.starts_with('#') {
        resolve_id(tree, selector)
    } else {
        resolve_tag(tree, selector)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dom::node::NodeData;

    #[test]
    fn resolve_ref_finds_valid_ref() {
        let mut ref_map = HashMap::new();
        ref_map.insert("@e1".to_string(), 42);
        ref_map.insert("@e2".to_string(), 99);

        assert_eq!(resolve_ref(&ref_map, "@e1"), Some(42));
        assert_eq!(resolve_ref(&ref_map, "@e2"), Some(99));
    }

    #[test]
    fn resolve_ref_returns_none_for_unknown_ref() {
        let mut ref_map = HashMap::new();
        ref_map.insert("@e1".to_string(), 42);

        assert_eq!(resolve_ref(&ref_map, "@e999"), None);
        assert_eq!(resolve_ref(&ref_map, "@unknown"), None);
    }

    #[test]
    fn resolve_ref_returns_none_for_non_ref_format() {
        let mut ref_map = HashMap::new();
        ref_map.insert("@e1".to_string(), 42);

        assert_eq!(resolve_ref(&ref_map, "e1"), None);
        assert_eq!(resolve_ref(&ref_map, "#myid"), None);
        assert_eq!(resolve_ref(&ref_map, "div"), None);
    }

    #[test]
    fn resolve_id_finds_element_with_id_attribute() {
        let mut tree = DomTree::new();
        let div = tree.create_element("div");
        tree.append_child(tree.document(), div);

        if let NodeData::Element { ref mut attributes, .. } = tree.get_node_mut(div).data {
            attributes.push(("id".to_string(), "myid".to_string()));
        }

        assert_eq!(resolve_id(&tree, "#myid"), Some(div));
    }

    #[test]
    fn resolve_id_returns_none_for_missing_id() {
        let mut tree = DomTree::new();
        let div = tree.create_element("div");
        tree.append_child(tree.document(), div);

        if let NodeData::Element { ref mut attributes, .. } = tree.get_node_mut(div).data {
            attributes.push(("id".to_string(), "myid".to_string()));
        }

        assert_eq!(resolve_id(&tree, "#nonexistent"), None);
    }

    #[test]
    fn resolve_id_returns_none_for_non_id_format() {
        let mut tree = DomTree::new();
        let div = tree.create_element("div");
        tree.append_child(tree.document(), div);

        if let NodeData::Element { ref mut attributes, .. } = tree.get_node_mut(div).data {
            attributes.push(("id".to_string(), "myid".to_string()));
        }

        assert_eq!(resolve_id(&tree, "myid"), None);
        assert_eq!(resolve_id(&tree, "@e1"), None);
        assert_eq!(resolve_id(&tree, "div"), None);
    }

    #[test]
    fn resolve_tag_finds_first_matching_element() {
        let mut tree = DomTree::new();
        let div1 = tree.create_element("div");
        let div2 = tree.create_element("div");
        let span = tree.create_element("span");

        tree.append_child(tree.document(), div1);
        tree.append_child(tree.document(), div2);
        tree.append_child(tree.document(), span);

        // Should return the first div
        assert_eq!(resolve_tag(&tree, "div"), Some(div1));
        assert_eq!(resolve_tag(&tree, "span"), Some(span));
    }

    #[test]
    fn resolve_tag_is_case_insensitive() {
        let mut tree = DomTree::new();
        let div = tree.create_element("div");
        tree.append_child(tree.document(), div);

        assert_eq!(resolve_tag(&tree, "div"), Some(div));
        assert_eq!(resolve_tag(&tree, "DIV"), Some(div));
        assert_eq!(resolve_tag(&tree, "Div"), Some(div));
    }

    #[test]
    fn resolve_tag_returns_none_for_missing_tag() {
        let mut tree = DomTree::new();
        let div = tree.create_element("div");
        tree.append_child(tree.document(), div);

        assert_eq!(resolve_tag(&tree, "span"), None);
        assert_eq!(resolve_tag(&tree, "nonexistent"), None);
    }

    #[test]
    fn resolve_selector_dispatches_to_ref() {
        let mut tree = DomTree::new();
        let div = tree.create_element("div");
        tree.append_child(tree.document(), div);

        let mut ref_map = HashMap::new();
        ref_map.insert("@e1".to_string(), div);

        assert_eq!(resolve_selector(&tree, &ref_map, "@e1"), Some(div));
    }

    #[test]
    fn resolve_selector_dispatches_to_id() {
        let mut tree = DomTree::new();
        let div = tree.create_element("div");
        tree.append_child(tree.document(), div);

        if let NodeData::Element { ref mut attributes, .. } = tree.get_node_mut(div).data {
            attributes.push(("id".to_string(), "myid".to_string()));
        }

        let ref_map = HashMap::new();
        assert_eq!(resolve_selector(&tree, &ref_map, "#myid"), Some(div));
    }

    #[test]
    fn resolve_selector_dispatches_to_tag() {
        let mut tree = DomTree::new();
        let div = tree.create_element("div");
        tree.append_child(tree.document(), div);

        let ref_map = HashMap::new();
        assert_eq!(resolve_selector(&tree, &ref_map, "div"), Some(div));
    }

    #[test]
    fn resolve_selector_returns_none_when_nothing_matches() {
        let tree = DomTree::new();
        let ref_map = HashMap::new();

        assert_eq!(resolve_selector(&tree, &ref_map, "@e1"), None);
        assert_eq!(resolve_selector(&tree, &ref_map, "#myid"), None);
        assert_eq!(resolve_selector(&tree, &ref_map, "div"), None);
    }

    #[test]
    fn resolve_selector_prefers_ref_over_id_and_tag() {
        let mut tree = DomTree::new();
        let div1 = tree.create_element("div");
        let div2 = tree.create_element("div");
        tree.append_child(tree.document(), div1);
        tree.append_child(tree.document(), div2);

        // Add an id attribute to div2
        if let NodeData::Element { ref mut attributes, .. } = tree.get_node_mut(div2).data {
            attributes.push(("id".to_string(), "test".to_string()));
        }

        let mut ref_map = HashMap::new();
        ref_map.insert("@e1".to_string(), div1);

        // @e1 should resolve to div1 via ref_map, not div2 or any tag
        assert_eq!(resolve_selector(&tree, &ref_map, "@e1"), Some(div1));
    }

    #[test]
    fn resolve_selector_prefers_id_over_tag() {
        let mut tree = DomTree::new();
        let div1 = tree.create_element("div");
        let div2 = tree.create_element("div");
        tree.append_child(tree.document(), div1);
        tree.append_child(tree.document(), div2);

        // Add an id attribute to div2
        if let NodeData::Element { ref mut attributes, .. } = tree.get_node_mut(div2).data {
            attributes.push(("id".to_string(), "test".to_string()));
        }

        let ref_map = HashMap::new();

        // #test should resolve to div2 via id, not div1 via tag
        assert_eq!(resolve_selector(&tree, &ref_map, "#test"), Some(div2));
    }
}
