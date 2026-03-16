use crate::dom::find::resolve_selector;
use crate::dom::node::NodeData;
use crate::Engine;

impl Engine {
    /// Type text into an element identified by selector.
    /// For <input> elements: sets the "value" attribute.
    /// For <textarea> elements: sets the text content (children).
    /// Returns Ok(()) on success, Err with message on failure.
    pub fn handle_type(&mut self, selector: &str, text: &str) -> Result<(), String> {
        // 1. Resolve selector to NodeId
        let node_id = {
            let tree = self.tree.borrow();
            match resolve_selector(&tree, &self.ref_map, selector) {
                Some(id) => id,
                None => return Err(format!("element not found: {}", selector)),
            }
        };

        // 2. Check element tag name and handle accordingly
        let tree = self.tree.borrow();
        let node = tree.get_node(node_id);
        match &node.data {
            NodeData::Element { tag_name, .. } => {
                let tag_lower = tag_name.to_ascii_lowercase();

                // 3. If <input>: set_attribute(node_id, "value", text)
                if tag_lower == "input" {
                    drop(tree);
                    let mut tree_mut = self.tree.borrow_mut();
                    tree_mut.set_attribute(node_id, "value", text);
                    return Ok(());
                }

                // 4. If <textarea>: set_text_content(node_id, text)
                if tag_lower == "textarea" {
                    drop(tree);
                    let mut tree_mut = self.tree.borrow_mut();
                    tree_mut.set_text_content(node_id, text);
                    return Ok(());
                }

                // 5. Otherwise: return error with actual tag
                Err(format!(
                    "type target must be <input> or <textarea>, got <{}>: {}",
                    tag_lower, selector
                ))
            }
            _ => {
                // Non-element nodes cannot receive text input
                Err(format!("type target is not an element: {}", selector))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use braille_wire::SnapMode;

    #[test]
    fn type_into_input_sets_value_attribute() {
        let html = r#"
        <html><body>
          <input id="username" type="text">
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        engine.snapshot(SnapMode::Accessibility);

        let result = engine.handle_type("#username", "john_doe");
        assert!(result.is_ok(), "Expected Ok, got {:?}", result);

        // Verify the value attribute was set
        let tree = engine.tree.borrow();
        let node_id = tree.get_element_by_id("username").unwrap();
        assert_eq!(tree.get_attribute(node_id, "value"), Some("john_doe".to_string()));
    }

    #[test]
    fn type_into_input_with_explicit_type_text_sets_value_attribute() {
        let html = r#"
        <html><body>
          <input id="email" type="email">
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        engine.snapshot(SnapMode::Accessibility);

        let result = engine.handle_type("#email", "test@example.com");
        assert!(result.is_ok(), "Expected Ok, got {:?}", result);

        // Verify the value attribute was set
        let tree = engine.tree.borrow();
        let node_id = tree.get_element_by_id("email").unwrap();
        assert_eq!(
            tree.get_attribute(node_id, "value"),
            Some("test@example.com".to_string())
        );
    }

    #[test]
    fn type_into_textarea_sets_text_content() {
        let html = r#"
        <html><body>
          <textarea id="comments"></textarea>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        engine.snapshot(SnapMode::Accessibility);

        let result = engine.handle_type("#comments", "This is a comment");
        assert!(result.is_ok(), "Expected Ok, got {:?}", result);

        // Verify the text content was set
        let tree = engine.tree.borrow();
        let node_id = tree.get_element_by_id("comments").unwrap();
        assert_eq!(tree.get_text_content(node_id), "This is a comment");
    }

    #[test]
    fn type_into_div_returns_error() {
        let html = r#"
        <html><body>
          <div id="content">Hello</div>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        engine.snapshot(SnapMode::Accessibility);

        let result = engine.handle_type("#content", "New text");
        assert!(result.is_err(), "Expected error, got {:?}", result);

        let err = result.unwrap_err();
        assert!(
            err.contains("type target must be <input> or <textarea>"),
            "Error should describe valid targets, got: {}",
            err
        );
        assert!(
            err.contains("got <div>"),
            "Error should mention actual tag name, got: {}",
            err
        );
        assert!(err.contains("#content"), "Error should mention selector, got: {}", err);
    }

    #[test]
    fn type_with_invalid_selector_returns_error() {
        let html = r#"
        <html><body>
          <input id="username" type="text">
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        engine.snapshot(SnapMode::Accessibility);

        let result = engine.handle_type("#nonexistent", "text");
        assert!(result.is_err(), "Expected error, got {:?}", result);

        let err = result.unwrap_err();
        assert!(
            err.contains("element not found"),
            "Error should mention element not found, got: {}",
            err
        );
        assert!(
            err.contains("#nonexistent"),
            "Error should include selector, got: {}",
            err
        );
    }

    #[test]
    fn type_overwrites_existing_value() {
        let html = r#"
        <html><body>
          <input id="username" type="text" value="old_value">
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        engine.snapshot(SnapMode::Accessibility);

        // Verify initial value
        {
            let tree = engine.tree.borrow();
            let node_id = tree.get_element_by_id("username").unwrap();
            assert_eq!(tree.get_attribute(node_id, "value"), Some("old_value".to_string()));
        }

        // Type new value
        let result = engine.handle_type("#username", "new_value");
        assert!(result.is_ok(), "Expected Ok, got {:?}", result);

        // Verify the value was overwritten
        let tree = engine.tree.borrow();
        let node_id = tree.get_element_by_id("username").unwrap();
        assert_eq!(tree.get_attribute(node_id, "value"), Some("new_value".to_string()));
    }

    #[test]
    fn type_into_input_using_tag_selector() {
        let html = r#"
        <html><body>
          <input type="text">
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        engine.snapshot(SnapMode::Accessibility);

        let result = engine.handle_type("input", "typed text");
        assert!(result.is_ok(), "Expected Ok, got {:?}", result);

        // Verify the value attribute was set on the first input
        let tree = engine.tree.borrow();
        let inputs = tree.get_elements_by_tag_name("input");
        assert_eq!(inputs.len(), 1);
        assert_eq!(tree.get_attribute(inputs[0], "value"), Some("typed text".to_string()));
    }

    #[test]
    fn type_into_textarea_using_ref_selector() {
        let html = r#"
        <html><body>
          <textarea>Initial content</textarea>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        engine.snapshot(SnapMode::Accessibility);

        // Note: textarea should appear in ref_map if it's an interactive element
        // For now, use tag selector
        let result = engine.handle_type("textarea", "New content");
        assert!(result.is_ok(), "Expected Ok, got {:?}", result);

        let tree = engine.tree.borrow();
        let textareas = tree.get_elements_by_tag_name("textarea");
        assert_eq!(textareas.len(), 1);
        assert_eq!(tree.get_text_content(textareas[0]), "New content");
    }

    #[test]
    fn type_empty_string_into_input() {
        let html = r#"
        <html><body>
          <input id="clear-me" type="text" value="something">
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        engine.snapshot(SnapMode::Accessibility);

        let result = engine.handle_type("#clear-me", "");
        assert!(result.is_ok(), "Expected Ok, got {:?}", result);

        let tree = engine.tree.borrow();
        let node_id = tree.get_element_by_id("clear-me").unwrap();
        assert_eq!(tree.get_attribute(node_id, "value"), Some("".to_string()));
    }

    #[test]
    fn type_empty_string_into_textarea() {
        let html = r#"
        <html><body>
          <textarea id="clear-me">Initial text</textarea>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        engine.snapshot(SnapMode::Accessibility);

        let result = engine.handle_type("#clear-me", "");
        assert!(result.is_ok(), "Expected Ok, got {:?}", result);

        let tree = engine.tree.borrow();
        let node_id = tree.get_element_by_id("clear-me").unwrap();
        assert_eq!(tree.get_text_content(node_id), "");
    }

    #[test]
    fn type_multiline_text_into_textarea() {
        let html = r#"
        <html><body>
          <textarea id="bio"></textarea>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        engine.snapshot(SnapMode::Accessibility);

        let multiline_text = "Line 1\nLine 2\nLine 3";
        let result = engine.handle_type("#bio", multiline_text);
        assert!(result.is_ok(), "Expected Ok, got {:?}", result);

        let tree = engine.tree.borrow();
        let node_id = tree.get_element_by_id("bio").unwrap();
        assert_eq!(tree.get_text_content(node_id), multiline_text);
    }

    #[test]
    fn type_into_input_without_type_attribute() {
        let html = r#"
        <html><body>
          <input id="default-input">
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        engine.snapshot(SnapMode::Accessibility);

        // Input without type attribute defaults to type="text"
        let result = engine.handle_type("#default-input", "default text");
        assert!(result.is_ok(), "Expected Ok, got {:?}", result);

        let tree = engine.tree.borrow();
        let node_id = tree.get_element_by_id("default-input").unwrap();
        assert_eq!(tree.get_attribute(node_id, "value"), Some("default text".to_string()));
    }

    #[test]
    fn type_into_paragraph_returns_error() {
        let html = r#"
        <html><body>
          <p id="para">Paragraph</p>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        engine.snapshot(SnapMode::Accessibility);

        let result = engine.handle_type("#para", "text");
        assert!(result.is_err(), "Expected error, got {:?}", result);

        let err = result.unwrap_err();
        assert!(
            err.contains("type target must be <input> or <textarea>"),
            "Error should describe valid targets, got: {}",
            err
        );
        assert!(
            err.contains("got <p>"),
            "Error should mention actual tag name, got: {}",
            err
        );
    }

    #[test]
    fn type_on_non_element_node_returns_error() {
        let html = r#"
        <html><body>
          <p>Just text</p>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        engine.snapshot(SnapMode::Accessibility);

        // Manually insert a text node ID into ref_map
        let text_node_id = {
            let tree = engine.tree.borrow();
            let p_nodes = tree.get_elements_by_tag_name("p");
            let p_node = tree.get_node(p_nodes[0]);
            p_node.children[0]
        };
        engine.ref_map.insert("@text".to_string(), text_node_id);

        let result = engine.handle_type("@text", "hello");
        assert!(result.is_err(), "Expected error, got {:?}", result);

        let err = result.unwrap_err();
        assert!(
            err.contains("type target is not an element"),
            "Error should say 'type target is not an element', got: {}",
            err
        );
        assert!(err.contains("@text"), "Error should contain selector, got: {}", err);
    }
}
