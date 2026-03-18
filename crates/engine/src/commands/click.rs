use std::rc::Rc;

use crate::dom::find::resolve_selector;
use crate::dom::node::NodeData;
use crate::js::bindings::element::get_or_create_js_element;
use crate::Engine;
use boa_engine::{js_string, JsValue};
use braille_wire::{EngineAction, HttpMethod, NavigateRequest};

impl Engine {
    /// Handle a click on an element identified by selector.
    ///
    /// Dispatches a real DOM click event (with full capture/bubble propagation and
    /// activation behavior) by calling `element.click()` through the JS runtime.
    /// After the event has been dispatched and all listeners have run, checks
    /// whether the target is an `<a>` with an href and returns a Navigate action
    /// if so.
    pub fn handle_click(&mut self, selector: &str) -> EngineAction {
        // 1. Resolve selector to NodeId
        let node_id = {
            let tree = self.tree.borrow();
            match resolve_selector(&tree, &self.ref_map, selector) {
                Some(id) => id,
                None => return EngineAction::Error(format!("element not found: {}", selector)),
            }
        };

        // 2. Verify it's an element
        {
            let tree = self.tree.borrow();
            let node = tree.get_node(node_id);
            if !matches!(node.data, NodeData::Element { .. }) {
                return EngineAction::Error(format!("click target is not an element: {}", selector));
            }
        }

        // 3. Dispatch a real click event via JS element.click()
        if let Some(runtime) = self.runtime.as_mut() {
            let tree = Rc::clone(&self.tree);
            let ctx = &mut runtime.context;
            let el_obj = get_or_create_js_element(node_id, tree, ctx)
                .unwrap_or_else(|e| panic!("handle_click: failed to get JS element: {e}"));
            let click_fn = el_obj
                .get(js_string!("click"), ctx)
                .unwrap_or_else(|e| panic!("handle_click: failed to get click method: {e}"));
            if let Some(click_obj) = click_fn.as_object() {
                let _ = click_obj.call(&JsValue::from(el_obj), &[], ctx);
            }
        }

        // 3b. Settle: flush microtasks, MO records, recompute CSS
        self.settle();

        // 4. After event dispatch, check if this is a navigable <a> element
        let tree = self.tree.borrow();
        let node = tree.get_node(node_id);
        if let NodeData::Element { tag_name, .. } = &node.data {
            if tag_name.eq_ignore_ascii_case("a") {
                if let Some(href) = tree.get_attribute(node_id, "href") {
                    return EngineAction::Navigate(NavigateRequest {
                        url: href,
                        method: HttpMethod::Get,
                        body: None,
                        content_type: None,
                    });
                }
            }
        }

        EngineAction::None
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
                assert!(
                    msg.contains("element not found"),
                    "Error message should contain 'element not found', got: {}",
                    msg
                );
                assert!(
                    msg.contains("#nonexistent"),
                    "Error message should contain selector, got: {}",
                    msg
                );
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
                assert!(
                    msg.contains("click target is not an element"),
                    "Error should mention 'click target is not an element', got: {}",
                    msg
                );
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
