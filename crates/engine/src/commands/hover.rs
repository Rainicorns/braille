use crate::dom::find::resolve_selector;
use crate::dom::node::NodeData;
use crate::Engine;

impl Engine {
    /// Hover over an element identified by selector.
    /// Dispatches mouseenter, mouseover, and mousemove events.
    pub fn handle_hover(&mut self, selector: &str) -> Result<(), String> {
        let node_id = {
            let tree = self.tree.borrow();
            match resolve_selector(&tree, &self.ref_map, selector) {
                Some(id) => id,
                None => return Err(format!("element not found: {}", selector)),
            }
        };

        {
            let tree = self.tree.borrow();
            let node = tree.get_node(node_id);
            if !matches!(node.data, NodeData::Element { .. }) {
                return Err(format!(
                    "hover target is not an element: {}",
                    selector
                ));
            }
        }

        if let Some(runtime) = self.runtime.as_mut() {
            runtime.fire_mouse_events(node_id, &["mouseenter", "mouseover", "mousemove"]);
        }

        self.settle();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use braille_wire::SnapMode;

    #[test]
    fn hover_dispatches_mouse_events() {
        let html = r#"<html><body>
            <div id="target">Hover me</div>
            <script>
                window.__events = [];
                var el = document.getElementById('target');
                el.addEventListener('mouseenter', function() { window.__events.push('mouseenter'); });
                el.addEventListener('mouseover', function() { window.__events.push('mouseover'); });
                el.addEventListener('mousemove', function() { window.__events.push('mousemove'); });
            </script>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        engine.snapshot(SnapMode::Accessibility);

        engine.handle_hover("#target").unwrap();

        let events = engine.eval_js("JSON.stringify(window.__events)").unwrap();
        assert_eq!(events, r#"["mouseenter","mouseover","mousemove"]"#);
    }

    #[test]
    fn hover_mouseenter_does_not_bubble() {
        let html = r#"<html><body>
            <div id="parent">
                <span id="child">Hover me</span>
            </div>
            <script>
                window.__parentEvents = [];
                var parent = document.getElementById('parent');
                parent.addEventListener('mouseenter', function() { window.__parentEvents.push('mouseenter'); });
                parent.addEventListener('mouseover', function() { window.__parentEvents.push('mouseover'); });
            </script>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        engine.snapshot(SnapMode::Accessibility);

        engine.handle_hover("#child").unwrap();

        let events = engine.eval_js("JSON.stringify(window.__parentEvents)").unwrap();
        // mouseover bubbles, mouseenter does not
        assert_eq!(events, r#"["mouseover"]"#);
    }

    #[test]
    fn hover_invalid_selector_returns_error() {
        let html = r#"<html><body><div>Hello</div></body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        engine.snapshot(SnapMode::Accessibility);

        let result = engine.handle_hover("#nope");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("element not found"));
    }
}
