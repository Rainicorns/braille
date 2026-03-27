#[cfg(test)]
mod tests {
    use crate::dom::node::{DomAttribute, NodeData};
    use crate::dom::tree::DomTree;

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
            attributes.push(DomAttribute::new("id", "main"));
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
        let div = tree.create_element_with_attrs("div", vec![DomAttribute::new("class", "container")]);
        let span = tree.create_element("span");
        tree.append_child(div, span);

        let cloned = tree.clone_node(div, false);

        assert_ne!(cloned, div);
        assert!(tree.get_node(cloned).children.is_empty());
        assert!(tree.get_node(cloned).parent.is_none());
        match &tree.get_node(cloned).data {
            NodeData::Element {
                tag_name, attributes, ..
            } => {
                assert_eq!(tag_name, "div");
                assert_eq!(attributes, &vec![DomAttribute::new("class", "container")]);
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
        let div = tree.create_element_with_attrs(
            "div",
            vec![
                DomAttribute::new("id", "main"),
                DomAttribute::new("class", "container"),
                DomAttribute::new("data-x", "42"),
            ],
        );

        let cloned = tree.clone_node(div, false);

        match &tree.get_node(cloned).data {
            NodeData::Element {
                tag_name, attributes, ..
            } => {
                assert_eq!(tag_name, "div");
                assert_eq!(attributes.len(), 3);
                assert!(attributes.contains(&DomAttribute::new("id", "main")));
                assert!(attributes.contains(&DomAttribute::new("class", "container")));
                assert!(attributes.contains(&DomAttribute::new("data-x", "42")));
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

    #[test]
    fn deep_nesting_does_not_stack_overflow() {
        // 10,000 nested divs — verifies iterative tree walking
        let mut tree = DomTree::new();
        let root = tree.create_element("div");
        tree.append_child(tree.document(), root);
        let mut parent = root;
        for _ in 0..10_000 {
            let child = tree.create_element("div");
            tree.append_child(parent, child);
            parent = child;
        }
        // Add text at the deepest level
        let text = tree.create_text("deep");
        tree.append_child(parent, text);

        // All these should complete without stack overflow
        let text_content = tree.get_text_content(root);
        assert_eq!(text_content, "deep");

        let html = tree.serialize_node_html(root);
        assert!(html.contains("deep"));
        assert!(html.contains("<div>"));

        let cloned = tree.clone_node(root, true);
        assert_eq!(tree.get_text_content(cloned), "deep");

        let equal = tree.is_equal_node(root, cloned);
        assert!(equal);

        tree.normalize(root);
        assert_eq!(tree.get_text_content(root), "deep");
    }

    // --- compareDocumentPosition tests ---

    const DISCONNECTED: u16 = 0x01;
    const PRECEDING: u16 = 0x02;
    const FOLLOWING: u16 = 0x04;
    const CONTAINS: u16 = 0x08;
    const CONTAINED_BY: u16 = 0x10;
    const IMPLEMENTATION_SPECIFIC: u16 = 0x20;

    #[test]
    fn compare_document_position_same_node() {
        let mut tree = DomTree::new();
        let div = tree.create_element("div");
        tree.append_child(tree.document(), div);
        assert_eq!(tree.compare_document_position(div, div), 0);
    }

    #[test]
    fn compare_document_position_parent_child() {
        let mut tree = DomTree::new();
        let parent = tree.create_element("div");
        let child = tree.create_element("span");
        tree.append_child(tree.document(), parent);
        tree.append_child(parent, child);

        // parent.compareDocumentPosition(child): child is contained by parent, follows
        let result = tree.compare_document_position(parent, child);
        assert_eq!(result, CONTAINED_BY | FOLLOWING);

        // child.compareDocumentPosition(parent): parent contains child, precedes
        let result = tree.compare_document_position(child, parent);
        assert_eq!(result, CONTAINS | PRECEDING);
    }

    #[test]
    fn compare_document_position_sibling_order() {
        let mut tree = DomTree::new();
        let parent = tree.create_element("div");
        let first = tree.create_element("span");
        let second = tree.create_element("p");
        tree.append_child(tree.document(), parent);
        tree.append_child(parent, first);
        tree.append_child(parent, second);

        // first.compareDocumentPosition(second): second follows first
        let result = tree.compare_document_position(first, second);
        assert_eq!(result, FOLLOWING);

        // second.compareDocumentPosition(first): first precedes second
        let result = tree.compare_document_position(second, first);
        assert_eq!(result, PRECEDING);
    }

    #[test]
    fn compare_document_position_disconnected() {
        let mut tree = DomTree::new();
        let a = tree.create_element("div");
        let b = tree.create_element("span");
        // Both detached (not appended to document), but they share no common root
        // Actually in DomTree, detached nodes have no parent, so root_of(a) == a, root_of(b) == b

        let result = tree.compare_document_position(a, b);
        assert!(result & DISCONNECTED != 0);
        assert!(result & IMPLEMENTATION_SPECIFIC != 0);
        // Must be either PRECEDING or FOLLOWING but not both
        assert!((result & PRECEDING != 0) ^ (result & FOLLOWING != 0));
    }

    #[test]
    fn compare_document_position_deep_ancestor() {
        let mut tree = DomTree::new();
        let root = tree.create_element("div");
        let mid = tree.create_element("section");
        let deep = tree.create_element("span");
        tree.append_child(tree.document(), root);
        tree.append_child(root, mid);
        tree.append_child(mid, deep);

        // root.compareDocumentPosition(deep): deep is a descendant
        let result = tree.compare_document_position(root, deep);
        assert_eq!(result, CONTAINED_BY | FOLLOWING);

        // deep.compareDocumentPosition(root): root is an ancestor
        let result = tree.compare_document_position(deep, root);
        assert_eq!(result, CONTAINS | PRECEDING);
    }
}
