use crate::Engine;
use crate::dom::node::NodeData;
use crate::dom::find::resolve_selector;

impl Engine {
    /// Focus an element identified by selector.
    /// Returns Ok(()) on success, Err with message on failure.
    pub fn handle_focus(&mut self, selector: &str) -> Result<(), String> {
        // 1. Resolve selector to NodeId
        let tree = self.tree.borrow();
        let node_id = resolve_selector(&tree, &self.ref_map, selector)
            .ok_or_else(|| format!("focus target not found: {}", selector))?;

        // 2. Verify it's a focusable element
        let node = tree.get_node(node_id);
        let is_focusable = match &node.data {
            NodeData::Element { tag_name, attributes } => {
                let tag = tag_name.to_ascii_lowercase();
                // Standard focusable elements
                if matches!(tag.as_str(), "input" | "textarea" | "select" | "button" | "a") {
                    true
                } else {
                    // Check for tabindex attribute
                    attributes.iter().any(|(k, _)| k == "tabindex")
                }
            }
            _ => false,
        };

        if !is_focusable {
            return Err(format!("element is not focusable (not an interactive element and no tabindex): {}", selector));
        }

        // 3. Set focus
        drop(tree);
        self.focused_element = Some(node_id);

        Ok(())
    }

    /// Clear focus
    pub fn handle_blur(&mut self) {
        self.focused_element = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use braille_wire::SnapMode;

    #[test]
    fn focus_on_input_succeeds() {
        let html = r#"
        <html><body>
          <input type="text" id="myinput">
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        engine.snapshot(SnapMode::Accessibility);

        let result = engine.handle_focus("@e1");
        assert!(result.is_ok(), "focus on input should succeed");
        assert!(engine.focused_element.is_some(), "focused_element should be set");
    }

    #[test]
    fn focus_on_button_succeeds() {
        let html = r#"
        <html><body>
          <button id="mybtn">Click me</button>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        engine.snapshot(SnapMode::Accessibility);

        let result = engine.handle_focus("#mybtn");
        assert!(result.is_ok(), "focus on button should succeed");
        assert!(engine.focused_element.is_some(), "focused_element should be set");
    }

    #[test]
    fn focus_on_link_succeeds() {
        let html = r#"
        <html><body>
          <a href="/home">Home</a>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        engine.snapshot(SnapMode::Accessibility);

        let result = engine.handle_focus("@e1");
        assert!(result.is_ok(), "focus on link should succeed");
    }

    #[test]
    fn focus_on_textarea_succeeds() {
        let html = r#"
        <html><body>
          <textarea id="mytext"></textarea>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        engine.snapshot(SnapMode::Accessibility);

        let result = engine.handle_focus("#mytext");
        assert!(result.is_ok(), "focus on textarea should succeed");
    }

    #[test]
    fn focus_on_select_succeeds() {
        let html = r#"
        <html><body>
          <select id="myselect"></select>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        engine.snapshot(SnapMode::Accessibility);

        let result = engine.handle_focus("#myselect");
        assert!(result.is_ok(), "focus on select should succeed");
    }

    #[test]
    fn focus_on_element_with_tabindex_succeeds() {
        let html = r#"
        <html><body>
          <div id="focusable" tabindex="0">Focusable div</div>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        engine.snapshot(SnapMode::Accessibility);

        let result = engine.handle_focus("#focusable");
        assert!(result.is_ok(), "focus on element with tabindex should succeed");
    }

    #[test]
    fn focus_on_non_focusable_div_fails() {
        let html = r#"
        <html><body>
          <div id="notfocusable">Not focusable</div>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        engine.snapshot(SnapMode::Accessibility);

        let result = engine.handle_focus("#notfocusable");
        assert!(result.is_err(), "focus on non-focusable div should fail");
        let err = result.unwrap_err();
        assert!(err.contains("not focusable"), "error should mention not focusable, got: {}", err);
        assert!(err.contains("#notfocusable"), "error should include selector, got: {}", err);
    }

    #[test]
    fn focus_on_non_focusable_paragraph_fails() {
        let html = r#"
        <html><body>
          <p id="para">Paragraph</p>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        engine.snapshot(SnapMode::Accessibility);

        let result = engine.handle_focus("#para");
        assert!(result.is_err(), "focus on paragraph should fail");
    }

    #[test]
    fn focus_on_nonexistent_element_fails() {
        let html = r#"
        <html><body>
          <input type="text">
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        engine.snapshot(SnapMode::Accessibility);

        let result = engine.handle_focus("#nonexistent");
        assert!(result.is_err(), "focus on nonexistent element should fail");
        let err = result.unwrap_err();
        assert!(err.contains("focus target not found"), "error should say 'focus target not found', got: {}", err);
        assert!(err.contains("#nonexistent"), "error should include selector, got: {}", err);
    }

    #[test]
    fn focus_shows_in_a11y_output() {
        let html = r#"
        <html><body>
          <input type="text" id="myinput">
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        engine.snapshot(SnapMode::Accessibility);
        engine.handle_focus("@e1").unwrap();

        let snapshot = engine.snapshot(SnapMode::Accessibility);
        assert!(snapshot.contains("[focused]"), "snapshot should show focused marker: {}", snapshot);
    }

    #[test]
    fn focus_changes_when_moved() {
        let html = r#"
        <html><body>
          <input type="text" id="input1">
          <input type="text" id="input2">
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        engine.snapshot(SnapMode::Accessibility);

        // Focus first input
        engine.handle_focus("@e1").unwrap();
        let snapshot1 = engine.snapshot(SnapMode::Accessibility);
        assert!(snapshot1.contains("input[type=text] @e1 [focused]"), "first snapshot should show @e1 focused: {}", snapshot1);

        // Focus second input
        engine.handle_focus("@e2").unwrap();
        let snapshot2 = engine.snapshot(SnapMode::Accessibility);
        assert!(snapshot2.contains("input[type=text] @e2 [focused]"), "second snapshot should show @e2 focused: {}", snapshot2);
        assert!(!snapshot2.contains("input[type=text] @e1 [focused]"), "second snapshot should not show @e1 focused: {}", snapshot2);
    }

    #[test]
    fn blur_clears_focus() {
        let html = r#"
        <html><body>
          <input type="text" id="myinput">
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        engine.snapshot(SnapMode::Accessibility);

        engine.handle_focus("@e1").unwrap();
        assert!(engine.focused_element.is_some(), "should have focus");

        engine.handle_blur();
        assert!(engine.focused_element.is_none(), "focus should be cleared");

        let snapshot = engine.snapshot(SnapMode::Accessibility);
        assert!(!snapshot.contains("[focused]"), "snapshot should not show focused marker after blur: {}", snapshot);
    }

    #[test]
    fn load_new_html_resets_focus() {
        let html1 = r#"
        <html><body>
          <input type="text" id="input1">
        </body></html>"#;

        let html2 = r#"
        <html><body>
          <input type="text" id="input2">
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html1);
        engine.snapshot(SnapMode::Accessibility);
        engine.handle_focus("@e1").unwrap();

        assert!(engine.focused_element.is_some(), "should have focus after first load");

        // Load new HTML
        engine.load_html(html2);
        assert!(engine.focused_element.is_none(), "focus should be reset after loading new HTML");

        let snapshot = engine.snapshot(SnapMode::Accessibility);
        assert!(!snapshot.contains("[focused]"), "new page should not show focused marker: {}", snapshot);
    }

    #[test]
    fn focus_on_non_element_node_fails() {
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

        let result = engine.handle_focus("@text");
        assert!(result.is_err(), "focus on non-element node should fail");
        let err = result.unwrap_err();
        assert!(err.contains("not focusable"), "error should mention not focusable, got: {}", err);
    }
}
