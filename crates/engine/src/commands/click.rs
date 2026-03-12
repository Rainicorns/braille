use crate::Engine;
use crate::dom::find::resolve_selector;
use crate::dom::node::NodeData;
use braille_wire::{EngineAction, NavigateRequest, HttpMethod};

impl Engine {
    /// Handle a click on an element identified by selector.
    /// If the element is a link (<a> with href), returns Navigate action.
    /// If the element is a submit button inside a form, delegates to form submission.
    /// Otherwise returns None action.
    pub fn handle_click(&mut self, selector: &str) -> EngineAction {
        // 1. Resolve selector to NodeId using self.ref_map and self.tree
        let tree = self.tree.borrow();
        let node_id = match resolve_selector(&tree, &self.ref_map, selector) {
            Some(id) => id,
            None => return EngineAction::Error(format!("element not found: {}", selector)),
        };

        // 2. Check the element type
        let node = tree.get_node(node_id);
        match &node.data {
            NodeData::Element { tag_name, .. } => {
                let tag_lower = tag_name.to_ascii_lowercase();

                // 3. If it's an <a> element, read href attribute and return Navigate
                if tag_lower == "a" {
                    if let Some(href) = tree.get_attribute(node_id, "href") {
                        return EngineAction::Navigate(NavigateRequest {
                            url: href,
                            method: HttpMethod::Get,
                            body: None,
                            content_type: None,
                        });
                    } else {
                        // <a> without href is not a clickable link
                        return EngineAction::None;
                    }
                }

                // 4. If it's a <button> or <input type="submit">, return None for now
                // (A-1C handles form submission)
                if tag_lower == "button" {
                    return EngineAction::None;
                }

                if tag_lower == "input" {
                    if let Some(input_type) = tree.get_attribute(node_id, "type") {
                        if input_type.to_ascii_lowercase() == "submit" {
                            return EngineAction::None;
                        }
                    }
                }

                // 5. Otherwise return None - not a clickable element
                EngineAction::None
            }
            _ => {
                // Non-element nodes are not clickable
                EngineAction::Error(format!("click target is not an element: {}", selector))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use braille_wire::SnapMode;

    #[test]
    fn click_on_link_with_href_returns_navigate() {
        let html = r#"
        <html><body>
          <a href="/about">About</a>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        engine.snapshot(SnapMode::Accessibility);

        let action = engine.handle_click("a");

        match action {
            EngineAction::Navigate(req) => {
                assert_eq!(req.url, "/about");
                assert_eq!(req.method, HttpMethod::Get);
                assert_eq!(req.body, None);
                assert_eq!(req.content_type, None);
            }
            _ => panic!("Expected Navigate action, got {:?}", action),
        }
    }

    #[test]
    fn click_on_link_without_href_returns_none() {
        let html = r#"
        <html><body>
          <a>Not a link</a>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        engine.snapshot(SnapMode::Accessibility);

        let action = engine.handle_click("a");

        match action {
            EngineAction::None => {}
            _ => panic!("Expected None action, got {:?}", action),
        }
    }

    #[test]
    fn click_on_non_link_element_returns_none() {
        let html = r#"
        <html><body>
          <div id="mydiv">Click me</div>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        engine.snapshot(SnapMode::Accessibility);

        let action = engine.handle_click("#mydiv");

        match action {
            EngineAction::None => {}
            _ => panic!("Expected None action, got {:?}", action),
        }
    }

    #[test]
    fn click_with_invalid_selector_returns_error() {
        let html = r#"
        <html><body>
          <a href="/home">Home</a>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        engine.snapshot(SnapMode::Accessibility);

        let action = engine.handle_click("#nonexistent");

        match action {
            EngineAction::Error(msg) => {
                assert!(msg.contains("element not found"), "Error message should contain 'element not found', got: {}", msg);
                assert!(msg.contains("#nonexistent"), "Error message should contain selector, got: {}", msg);
            }
            _ => panic!("Expected Error action, got {:?}", action),
        }
    }

    #[test]
    fn click_on_button_returns_none() {
        let html = r#"
        <html><body>
          <button>Submit</button>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        engine.snapshot(SnapMode::Accessibility);

        let action = engine.handle_click("button");

        match action {
            EngineAction::None => {}
            _ => panic!("Expected None action for button, got {:?}", action),
        }
    }

    #[test]
    fn click_on_submit_input_returns_none() {
        let html = r#"
        <html><body>
          <input type="submit" value="Submit">
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        engine.snapshot(SnapMode::Accessibility);

        let action = engine.handle_click("input");

        match action {
            EngineAction::None => {}
            _ => panic!("Expected None action for submit input, got {:?}", action),
        }
    }

    #[test]
    fn click_on_link_using_ref_selector() {
        let html = r#"
        <html><body>
          <a href="/page1">Page 1</a>
          <a href="/page2">Page 2</a>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        engine.snapshot(SnapMode::Accessibility);

        // Click on @e1 (first link)
        let action = engine.handle_click("@e1");

        match action {
            EngineAction::Navigate(req) => {
                assert_eq!(req.url, "/page1");
            }
            _ => panic!("Expected Navigate action, got {:?}", action),
        }
    }

    #[test]
    fn click_on_link_using_id_selector() {
        let html = r#"
        <html><body>
          <a id="home-link" href="/home">Home</a>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        engine.snapshot(SnapMode::Accessibility);

        let action = engine.handle_click("#home-link");

        match action {
            EngineAction::Navigate(req) => {
                assert_eq!(req.url, "/home");
            }
            _ => panic!("Expected Navigate action, got {:?}", action),
        }
    }

    #[test]
    fn click_on_link_with_absolute_url() {
        let html = r#"
        <html><body>
          <a href="https://example.com/page">External</a>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        engine.snapshot(SnapMode::Accessibility);

        let action = engine.handle_click("a");

        match action {
            EngineAction::Navigate(req) => {
                assert_eq!(req.url, "https://example.com/page");
            }
            _ => panic!("Expected Navigate action, got {:?}", action),
        }
    }

    #[test]
    fn click_on_non_interactive_element_returns_none() {
        let html = r#"
        <html><body>
          <p>Just text</p>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        engine.snapshot(SnapMode::Accessibility);

        // Try to click on paragraph (which won't be in ref_map but could be selected by tag)
        let action = engine.handle_click("p");

        match action {
            EngineAction::None => {}
            _ => panic!("Expected None action for paragraph, got {:?}", action),
        }
    }

    #[test]
    fn click_on_non_element_node_returns_error() {
        let html = r#"
        <html><body>
          <p>Just text</p>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        engine.snapshot(SnapMode::Accessibility);

        // Manually insert a text node ID into the ref_map to simulate
        // a non-element being resolved
        let text_node_id = {
            let tree = engine.tree.borrow();
            let p_nodes = tree.get_elements_by_tag_name("p");
            let p_node = tree.get_node(p_nodes[0]);
            // The first child of <p> should be a text node
            p_node.children[0]
        };
        engine.ref_map.insert("@text".to_string(), text_node_id);

        let action = engine.handle_click("@text");

        match action {
            EngineAction::Error(msg) => {
                assert!(msg.contains("click target is not an element"), "Error should mention 'click target is not an element', got: {}", msg);
                assert!(msg.contains("@text"), "Error should contain selector, got: {}", msg);
            }
            _ => panic!("Expected Error action for non-element node, got {:?}", action),
        }
    }

    #[test]
    fn click_on_input_type_text_returns_none() {
        let html = r#"
        <html><body>
          <input type="text" name="username">
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        engine.snapshot(SnapMode::Accessibility);

        let action = engine.handle_click("input");

        match action {
            EngineAction::None => {}
            _ => panic!("Expected None action for text input, got {:?}", action),
        }
    }

    #[test]
    fn click_on_link_with_fragment_href() {
        let html = r##"
        <html><body>
          <a href="#section1">Section 1</a>
        </body></html>"##;

        let mut engine = Engine::new();
        engine.load_html(html);
        engine.snapshot(SnapMode::Accessibility);

        let action = engine.handle_click("a");

        match action {
            EngineAction::Navigate(req) => {
                assert_eq!(req.url, "#section1");
            }
            _ => panic!("Expected Navigate action, got {:?}", action),
        }
    }
}
