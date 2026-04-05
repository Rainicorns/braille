//! Security tests for JS injection via string interpolation in keyboard/cursor events.

use braille_engine::Engine;
use braille_wire::SnapMode;

fn engine_with_input() -> Engine {
    let html = r#"<html><body>
        <input id="i" type="text" value="hello">
    </body></html>"#;
    let mut e = Engine::new();
    e.load_html(html);
    e.snapshot(SnapMode::Accessibility);
    e
}

#[test]
fn keyboard_event_key_injection() {
    let mut e = engine_with_input();

    // A malicious key value that attempts to break out of the single-quoted string
    // in fire_keyboard_event's format! call and execute arbitrary JS.
    e.fire_keyboard_event_on("#i", "keydown", "' + (window.__pwned=true) + '", "KeyA")
        .unwrap();

    let result = e.eval_js("typeof window.__pwned").unwrap();
    assert_eq!(
        result, "undefined",
        "JS injection via key parameter should not execute: window.__pwned = {}",
        e.eval_js("window.__pwned").unwrap_or_default()
    );
}

#[test]
fn keyboard_event_code_injection() {
    let mut e = engine_with_input();

    e.fire_keyboard_event_on("#i", "keydown", "a", "' + (window.__pwned=true) + '")
        .unwrap();

    let result = e.eval_js("typeof window.__pwned").unwrap();
    assert_eq!(
        result, "undefined",
        "JS injection via code parameter should not execute"
    );
}

#[test]
fn keyboard_event_type_injection() {
    let mut e = engine_with_input();

    e.fire_keyboard_event_on("#i", "' + (window.__pwned=true) + '", "a", "KeyA")
        .unwrap();

    let result = e.eval_js("typeof window.__pwned").unwrap();
    assert_eq!(
        result, "undefined",
        "JS injection via event_type parameter should not execute"
    );
}

#[test]
fn move_input_cursor_key_injection() {
    let mut e = engine_with_input();

    // move_input_cursor also interpolates key into single-quoted strings
    e.move_input_cursor_on("#i", "' + (window.__pwned=true) + '")
        .unwrap();

    let result = e.eval_js("typeof window.__pwned").unwrap();
    assert_eq!(
        result, "undefined",
        "JS injection via move_input_cursor key parameter should not execute"
    );
}
