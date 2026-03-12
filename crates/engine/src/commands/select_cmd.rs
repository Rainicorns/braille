use crate::Engine;
use crate::dom::NodeData;

impl Engine {
    /// Select an option in a <select> element identified by selector.
    /// `value` is matched against <option> elements' value attribute or text content.
    /// Returns Ok(()) on success, Err with message on failure.
    pub fn handle_select(&mut self, selector: &str, value: &str) -> Result<(), String> {
        // 1. Resolve selector to NodeId
        let tree = self.tree.borrow();
        let node_id = crate::dom::find::resolve_selector(&tree, &self.ref_map, selector)
            .ok_or_else(|| format!("element not found: {}", selector))?;

        // 2. Verify it's a <select> element
        let node = tree.get_node(node_id);
        match &node.data {
            NodeData::Element { tag_name, .. } => {
                if tag_name.to_ascii_lowercase() != "select" {
                    return Err(format!(
                        "select target must be <select>, got <{}>: {}",
                        tag_name.to_ascii_lowercase(),
                        selector
                    ));
                }
            }
            _ => {
                return Err(format!("select target is not an element: {}", selector));
            }
        }

        // 3. Find all <option> children/descendants
        let options = tree.find_descendants_by_tag(node_id, "option");

        if options.is_empty() {
            return Err("No <option> elements found in <select>".to_string());
        }

        // 4 & 5. Find the option matching `value` (check value attr first, then text content)
        let mut matching_option = None;
        for &option_id in &options {
            // Check value attribute first
            if let Some(option_value) = tree.get_attribute(option_id, "value") {
                if option_value == value {
                    matching_option = Some(option_id);
                    break;
                }
            } else {
                // If no value attribute, check text content
                let text_content = tree.get_text_content(option_id);
                if text_content.trim() == value {
                    matching_option = Some(option_id);
                    break;
                }
            }
        }

        let matching_option = matching_option.ok_or_else(|| {
            format!("no option matching '{}' found in <select>: {}", value, selector)
        })?;

        // Drop the immutable borrow before taking a mutable borrow
        drop(tree);
        let mut tree = self.tree.borrow_mut();

        // 6. Remove "selected" attribute from all options
        for &option_id in &options {
            tree.remove_attribute(option_id, "selected");
        }

        // 7. Set "selected" attribute on the matching option
        tree.set_attribute(matching_option, "selected", "selected");

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use braille_wire::SnapMode;

    #[test]
    fn select_by_option_value_attribute() {
        let html = r#"
        <html><body>
          <select id="country">
            <option value="us">United States</option>
            <option value="ca">Canada</option>
            <option value="mx">Mexico</option>
          </select>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);

        let result = engine.handle_select("#country", "ca");
        assert!(result.is_ok(), "select should succeed: {:?}", result);

        // Verify the correct option is selected
        let tree = engine.tree.borrow();
        let select = tree.get_element_by_id("country").unwrap();
        let options = tree.find_descendants_by_tag(select, "option");

        assert!(!tree.has_attribute(options[0], "selected"));
        assert!(tree.has_attribute(options[1], "selected"));
        assert!(!tree.has_attribute(options[2], "selected"));
    }

    #[test]
    fn select_by_option_text_content_when_no_value_attr() {
        let html = r#"
        <html><body>
          <select id="fruit">
            <option>Apple</option>
            <option>Banana</option>
            <option>Cherry</option>
          </select>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);

        let result = engine.handle_select("#fruit", "Banana");
        assert!(result.is_ok(), "select should succeed: {:?}", result);

        // Verify the correct option is selected
        let tree = engine.tree.borrow();
        let select = tree.get_element_by_id("fruit").unwrap();
        let options = tree.find_descendants_by_tag(select, "option");

        assert!(!tree.has_attribute(options[0], "selected"));
        assert!(tree.has_attribute(options[1], "selected"));
        assert!(!tree.has_attribute(options[2], "selected"));
    }

    #[test]
    fn select_on_non_select_element_returns_error() {
        let html = r#"
        <html><body>
          <div id="notselect">
            <option>Test</option>
          </div>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);

        let result = engine.handle_select("#notselect", "Test");
        assert!(result.is_err(), "should error on non-select element");
        let err = result.unwrap_err();
        assert!(
            err.contains("select target must be <select>"),
            "error should mention it must be a select element, got: {}", err
        );
        assert!(
            err.contains("got <div>"),
            "error should mention actual tag, got: {}", err
        );
        assert!(
            err.contains("#notselect"),
            "error should include selector, got: {}", err
        );
    }

    #[test]
    fn select_with_no_matching_option_returns_error() {
        let html = r#"
        <html><body>
          <select id="country">
            <option value="us">United States</option>
            <option value="ca">Canada</option>
          </select>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);

        let result = engine.handle_select("#country", "uk");
        assert!(result.is_err(), "should error when no matching option");
        let err = result.unwrap_err();
        assert!(
            err.contains("no option matching 'uk'"),
            "error should mention the value searched for, got: {}", err
        );
        assert!(
            err.contains("found in <select>"),
            "error should mention it was in a select, got: {}", err
        );
        assert!(
            err.contains("#country"),
            "error should include selector, got: {}", err
        );
    }

    #[test]
    fn select_clears_previously_selected_option() {
        let html = r#"
        <html><body>
          <select id="country">
            <option value="us" selected>United States</option>
            <option value="ca">Canada</option>
            <option value="mx">Mexico</option>
          </select>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);

        // Verify initial state
        {
            let tree = engine.tree.borrow();
            let select = tree.get_element_by_id("country").unwrap();
            let options = tree.find_descendants_by_tag(select, "option");
            assert!(tree.has_attribute(options[0], "selected"));
        }

        // Select a different option
        let result = engine.handle_select("#country", "mx");
        assert!(result.is_ok(), "select should succeed: {:?}", result);

        // Verify the selection changed
        let tree = engine.tree.borrow();
        let select = tree.get_element_by_id("country").unwrap();
        let options = tree.find_descendants_by_tag(select, "option");

        assert!(!tree.has_attribute(options[0], "selected"), "first option should not be selected");
        assert!(!tree.has_attribute(options[1], "selected"), "second option should not be selected");
        assert!(tree.has_attribute(options[2], "selected"), "third option should be selected");
    }

    #[test]
    fn select_with_invalid_selector_returns_error() {
        let html = r#"
        <html><body>
          <select id="country">
            <option value="us">United States</option>
          </select>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);

        let result = engine.handle_select("#nonexistent", "us");
        assert!(result.is_err(), "should error with invalid selector");
        let err = result.unwrap_err();
        assert!(
            err.contains("element not found"),
            "error should mention element not found, got: {}", err
        );
        assert!(
            err.contains("#nonexistent"),
            "error should include selector, got: {}", err
        );
    }

    #[test]
    fn select_using_ref_selector() {
        let html = r#"
        <html><body>
          <select>
            <option value="a">Option A</option>
            <option value="b">Option B</option>
          </select>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        engine.snapshot(SnapMode::Accessibility);

        // The select element should be @e1
        let result = engine.handle_select("@e1", "b");
        assert!(result.is_ok(), "select with @ref should succeed: {:?}", result);

        let tree = engine.tree.borrow();
        let select_ref = engine.resolve_ref("@e1").unwrap();
        let options = tree.find_descendants_by_tag(select_ref, "option");

        assert!(!tree.has_attribute(options[0], "selected"));
        assert!(tree.has_attribute(options[1], "selected"));
    }

    #[test]
    fn select_using_tag_selector() {
        let html = r#"
        <html><body>
          <select>
            <option value="x">X</option>
            <option value="y">Y</option>
          </select>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);

        let result = engine.handle_select("select", "y");
        assert!(result.is_ok(), "select with tag selector should succeed: {:?}", result);

        let tree = engine.tree.borrow();
        let select = tree.get_elements_by_tag_name("select").into_iter().next().unwrap();
        let options = tree.find_descendants_by_tag(select, "option");

        assert!(!tree.has_attribute(options[0], "selected"));
        assert!(tree.has_attribute(options[1], "selected"));
    }

    #[test]
    fn select_with_no_options_returns_error() {
        let html = r#"
        <html><body>
          <select id="empty">
          </select>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);

        let result = engine.handle_select("#empty", "anything");
        assert!(result.is_err(), "should error when select has no options");
        assert!(
            result.unwrap_err().contains("No <option> elements found"),
            "error should mention no options found"
        );
    }

    #[test]
    fn select_with_mixed_value_and_text_options() {
        let html = r#"
        <html><body>
          <select id="mixed">
            <option value="val1">Text 1</option>
            <option>Text 2</option>
            <option value="val3">Text 3</option>
          </select>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);

        // Select by value attribute
        let result = engine.handle_select("#mixed", "val1");
        assert!(result.is_ok(), "select by value should succeed: {:?}", result);

        {
            let tree = engine.tree.borrow();
            let select = tree.get_element_by_id("mixed").unwrap();
            let options = tree.find_descendants_by_tag(select, "option");
            assert!(tree.has_attribute(options[0], "selected"));
        }

        // Select by text content (for option without value attribute)
        let result = engine.handle_select("#mixed", "Text 2");
        assert!(result.is_ok(), "select by text should succeed: {:?}", result);

        {
            let tree = engine.tree.borrow();
            let select = tree.get_element_by_id("mixed").unwrap();
            let options = tree.find_descendants_by_tag(select, "option");
            assert!(!tree.has_attribute(options[0], "selected"));
            assert!(tree.has_attribute(options[1], "selected"));
            assert!(!tree.has_attribute(options[2], "selected"));
        }
    }

    #[test]
    fn select_on_non_element_node_returns_error() {
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

        let result = engine.handle_select("@text", "anything");
        assert!(result.is_err(), "should error on non-element node");
        let err = result.unwrap_err();
        assert!(
            err.contains("select target is not an element"),
            "error should say 'select target is not an element', got: {}", err
        );
        assert!(
            err.contains("@text"),
            "error should include selector, got: {}", err
        );
    }

    #[test]
    fn select_prefers_value_attribute_over_text_content() {
        let html = r#"
        <html><body>
          <select id="test">
            <option value="different">Same Text</option>
            <option>Same Text</option>
          </select>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);

        // When matching "different", should match first option by value attribute
        let result = engine.handle_select("#test", "different");
        assert!(result.is_ok(), "select should succeed: {:?}", result);

        {
            let tree = engine.tree.borrow();
            let select = tree.get_element_by_id("test").unwrap();
            let options = tree.find_descendants_by_tag(select, "option");
            assert!(tree.has_attribute(options[0], "selected"));
            assert!(!tree.has_attribute(options[1], "selected"));
        }

        // When matching "Same Text", should match first option with value="different" first
        // because value attribute is checked before text content
        let result = engine.handle_select("#test", "Same Text");
        assert!(result.is_ok(), "select should succeed: {:?}", result);

        {
            let tree = engine.tree.borrow();
            let select = tree.get_element_by_id("test").unwrap();
            let options = tree.find_descendants_by_tag(select, "option");
            // Should match the second option because first option's value is "different"
            assert!(!tree.has_attribute(options[0], "selected"));
            assert!(tree.has_attribute(options[1], "selected"));
        }
    }
}
