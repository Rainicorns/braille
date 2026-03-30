use crate::dom::find::resolve_selector;
use crate::dom::node::NodeData;
use crate::Engine;

/// WebDriver key code to (key, code) mapping.
fn webdriver_key_map(ch: char) -> (&'static str, &'static str) {
    match ch {
        '\u{E003}' => ("Backspace", "Backspace"),
        '\u{E004}' => ("Tab", "Tab"),
        '\u{E006}' | '\u{E007}' => ("Enter", "Enter"),
        '\u{E008}' => ("Shift", "ShiftLeft"),
        '\u{E009}' => ("Control", "ControlLeft"),
        '\u{E00A}' => ("Alt", "AltLeft"),
        '\u{E00C}' => ("Escape", "Escape"),
        '\u{E00D}' => (" ", "Space"),
        '\u{E010}' => ("End", "End"),
        '\u{E011}' => ("Home", "Home"),
        '\u{E012}' => ("ArrowLeft", "ArrowLeft"),
        '\u{E013}' => ("ArrowUp", "ArrowUp"),
        '\u{E014}' => ("ArrowRight", "ArrowRight"),
        '\u{E015}' => ("ArrowDown", "ArrowDown"),
        '\u{E017}' => ("Delete", "Delete"),
        _ => ("", ""),
    }
}

impl Engine {
    /// Send key events to an element identified by selector.
    /// `keys` is a string of characters — each character produces a keydown/keypress/keyup
    /// sequence. WebDriver special key codes (U+E0xx) are mapped to named keys.
    pub fn handle_send_keys(&mut self, selector: &str, keys: &str) -> Result<(), String> {
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
                    "send_keys target is not an element: {}",
                    selector
                ));
            }
        }

        if let Some(runtime) = self.runtime.as_mut() {
            for ch in keys.chars() {
                let (key, code) = webdriver_key_map(ch);
                let (key, code) = if key.is_empty() {
                    (ch.to_string(), format!("Key{}", ch.to_ascii_uppercase()))
                } else {
                    (key.to_string(), code.to_string())
                };

                runtime.fire_keyboard_event(node_id, "keydown", &key, &code);
                runtime.fire_keyboard_event(node_id, "keypress", &key, &code);

                // Move cursor for navigation keys on input/textarea
                let is_nav = matches!(
                    key.as_str(),
                    "ArrowRight" | "ArrowLeft" | "Home" | "End" | "ArrowUp" | "ArrowDown"
                );
                if is_nav {
                    runtime.move_input_cursor(node_id, &key);
                }

                runtime.fire_keyboard_event(node_id, "keyup", &key, &code);
            }
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
    fn send_keys_dispatches_keyboard_events() {
        let html = r#"<html><body>
            <input id="i" type="text" value="hello">
            <script>
                window.__events = [];
                var el = document.getElementById('i');
                el.addEventListener('keydown', function(e) {
                    window.__events.push('keydown:' + e.key);
                });
                el.addEventListener('keypress', function(e) {
                    window.__events.push('keypress:' + e.key);
                });
                el.addEventListener('keyup', function(e) {
                    window.__events.push('keyup:' + e.key);
                });
            </script>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        engine.snapshot(SnapMode::Accessibility);

        engine.handle_send_keys("#i", "a").unwrap();

        let events = engine.eval_js("JSON.stringify(window.__events)").unwrap();
        assert_eq!(events, r#"["keydown:a","keypress:a","keyup:a"]"#);
    }

    #[test]
    fn send_keys_arrow_right_moves_cursor() {
        let html = r#"<html><body>
            <input id="i" type="text" value="hello">
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        engine.snapshot(SnapMode::Accessibility);

        // selectionStart defaults to 0
        let pos = engine.eval_js("document.getElementById('i').selectionStart").unwrap();
        assert_eq!(pos, "0");

        // Send ArrowRight
        engine
            .handle_send_keys("#i", "\u{E014}")
            .unwrap();

        let pos = engine.eval_js("document.getElementById('i').selectionStart").unwrap();
        assert_eq!(pos, "1");
    }

    #[test]
    fn send_keys_arrow_right_at_end_stays() {
        let html = r#"<html><body>
            <input id="i" type="text" value="ab">
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        engine.snapshot(SnapMode::Accessibility);

        // Move to end
        engine.handle_send_keys("#i", "\u{E014}\u{E014}").unwrap();
        let pos = engine.eval_js("document.getElementById('i').selectionStart").unwrap();
        assert_eq!(pos, "2");

        // One more ArrowRight — should stay at 2
        engine.handle_send_keys("#i", "\u{E014}").unwrap();
        let pos = engine.eval_js("document.getElementById('i').selectionStart").unwrap();
        assert_eq!(pos, "2");
    }

    #[test]
    fn send_keys_arrow_left_moves_cursor() {
        let html = r#"<html><body>
            <input id="i" type="text" value="hello">
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        engine.snapshot(SnapMode::Accessibility);

        // Move right first, then left
        engine
            .handle_send_keys("#i", "\u{E014}\u{E014}\u{E012}")
            .unwrap();

        let pos = engine.eval_js("document.getElementById('i').selectionStart").unwrap();
        assert_eq!(pos, "1");
    }

    #[test]
    fn send_keys_home_end_keys() {
        let html = r#"<html><body>
            <input id="i" type="text" value="hello">
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        engine.snapshot(SnapMode::Accessibility);

        // End key
        engine.handle_send_keys("#i", "\u{E010}").unwrap();
        let pos = engine.eval_js("document.getElementById('i').selectionStart").unwrap();
        assert_eq!(pos, "5");

        // Home key
        engine.handle_send_keys("#i", "\u{E011}").unwrap();
        let pos = engine.eval_js("document.getElementById('i').selectionStart").unwrap();
        assert_eq!(pos, "0");
    }

    #[test]
    fn send_keys_updates_scroll_left_on_overflow() {
        let html = r#"<html><body>
            <input id="i" type="text" style="width: 40px"
                   value="Fooooooooooooooooooooooooooo">
            <script>
                window.__scrolled = false;
                document.getElementById('i').addEventListener('scroll', function() {
                    window.__scrolled = true;
                });
            </script>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        engine.snapshot(SnapMode::Accessibility);

        // scrollLeft starts at 0
        let sl = engine.eval_js("document.getElementById('i').scrollLeft").unwrap();
        assert_eq!(sl, "0");

        // Send enough ArrowRight keys to overflow the 40px width (at 8px/char, >5 chars)
        for _ in 0..10 {
            engine.handle_send_keys("#i", "\u{E014}").unwrap();
        }

        let sl: f64 = engine
            .eval_js("document.getElementById('i').scrollLeft")
            .unwrap()
            .parse()
            .unwrap_or(0.0);
        assert!(sl > 0.0, "scrollLeft should be > 0 after overflow, got {}", sl);

        let scrolled = engine.eval_js("window.__scrolled").unwrap();
        assert_eq!(scrolled, "true", "scroll event should have fired");
    }

    #[test]
    fn send_keys_selection_start_end_on_textarea() {
        let html = r#"<html><body><textarea id="t">hello world</textarea></body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        engine.snapshot(SnapMode::Accessibility);

        let pos = engine.eval_js("document.getElementById('t').selectionStart").unwrap();
        assert_eq!(pos, "0");

        engine
            .handle_send_keys("#t", "\u{E014}\u{E014}\u{E014}")
            .unwrap();

        let pos = engine.eval_js("document.getElementById('t').selectionStart").unwrap();
        assert_eq!(pos, "3");
    }

    #[test]
    fn send_keys_invalid_selector_returns_error() {
        let html = r#"<html><body><input type="text"></body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        engine.snapshot(SnapMode::Accessibility);

        let result = engine.handle_send_keys("#nope", "a");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("element not found"));
    }

    #[test]
    fn set_selection_range_works() {
        let html = r#"<html><body>
            <input id="i" type="text" value="hello world">
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        engine.snapshot(SnapMode::Accessibility);

        engine
            .eval_js("document.getElementById('i').setSelectionRange(3, 7)")
            .unwrap();

        let start = engine.eval_js("document.getElementById('i').selectionStart").unwrap();
        let end = engine.eval_js("document.getElementById('i').selectionEnd").unwrap();
        assert_eq!(start, "3");
        assert_eq!(end, "7");
    }

    #[test]
    fn scroll_left_and_width_for_overflow_input() {
        // Simulates the WPT test pattern: arrow right through long text in narrow input
        let html = r#"<html><body>
            <input id="i" type="text" style="width: 50px"
                   value="Fooooooooooooooooooooooooooooooooooooooooooooooooo">
            <script>
                window.__scrollCount = 0;
                document.getElementById('i').addEventListener('scroll', function() {
                    window.__scrollCount++;
                });
            </script>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        engine.snapshot(SnapMode::Accessibility);

        let width = engine.eval_js("document.getElementById('i').getBoundingClientRect().width").unwrap();
        eprintln!("input width = {}", width);
        let val_len = engine.eval_js("document.getElementById('i').value.length").unwrap();
        eprintln!("value length = {}", val_len);
        let sel = engine.eval_js("document.getElementById('i').selectionStart").unwrap();
        eprintln!("initial selectionStart = {}", sel);
        let sl = engine.eval_js("document.getElementById('i').scrollLeft").unwrap();
        eprintln!("initial scrollLeft = {}", sl);

        assert_eq!(sl, "0", "scrollLeft should start at 0");

        // Arrow right until scrollLeft > 0 (simulating the WPT while loop)
        let mut iterations = 0;
        loop {
            engine.handle_send_keys("#i", "\u{E014}").unwrap();
            iterations += 1;
            let sl = engine.eval_js("document.getElementById('i').scrollLeft").unwrap();
            if sl != "0" {
                eprintln!("scrollLeft became {} after {} arrow rights", sl, iterations);
                break;
            }
            if iterations > 100 {
                panic!("scrollLeft never became nonzero after 100 ArrowRight presses");
            }
        }

        let scroll_count = engine.eval_js("window.__scrollCount").unwrap();
        eprintln!("scroll events fired: {}", scroll_count);
        assert!(scroll_count.parse::<i32>().unwrap() > 0, "scroll event should have fired");
    }

    #[test]
    fn select_method_selects_all() {
        let html = r#"<html><body>
            <input id="i" type="text" value="hello">
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        engine.snapshot(SnapMode::Accessibility);

        engine.eval_js("document.getElementById('i').select()").unwrap();

        let start = engine.eval_js("document.getElementById('i').selectionStart").unwrap();
        let end = engine.eval_js("document.getElementById('i').selectionEnd").unwrap();
        assert_eq!(start, "0");
        assert_eq!(end, "5");
    }

    #[test]
    fn scroll_top_clamps_to_max() {
        let html = r#"<html><body>
            <div id="container" style="overflow:scroll; height:100px; width:100px">
                <div style="height:200px; width:200px"></div>
            </div>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        engine.snapshot(SnapMode::Accessibility);

        let sh = engine.eval_js("document.getElementById('container').scrollHeight").unwrap();
        eprintln!("scrollHeight = {}", sh);
        let ch = engine.eval_js("document.getElementById('container').clientHeight").unwrap();
        eprintln!("clientHeight = {}", ch);
        let sw = engine.eval_js("document.getElementById('container').scrollWidth").unwrap();
        eprintln!("scrollWidth = {}", sw);
        let cw = engine.eval_js("document.getElementById('container').clientWidth").unwrap();
        eprintln!("clientWidth = {}", cw);

        engine.eval_js("document.getElementById('container').scrollTop = 1000").unwrap();
        let st = engine.eval_js("document.getElementById('container').scrollTop").unwrap();
        eprintln!("scrollTop after set 1000 = {}", st);

        // scrollHeight=200, clientHeight=100, max=100
        // so scrollTop should clamp to 100
        assert_eq!(st, "100");
    }
}
