//! Tests for React-like controlled inputs with inputValueTracking,
//! event delegation via capture phase, and onBlur validation.

use braille_engine::Engine;
use braille_wire::SnapMode;

const HTML: &str = include_str!("../../../tests/fixtures/react_controlled_input.html");

fn setup() -> Engine {
    let mut engine = Engine::new();
    engine.load_html(HTML);
    engine.snapshot(SnapMode::Compact);
    engine
}

#[test]
fn controlled_input_renders() {
    let mut engine = setup();
    let snap = engine.snapshot(SnapMode::Compact);
    assert!(snap.contains("Signup"), "should render signup heading");
    assert!(snap.contains("Username"), "should render username label");
    assert!(snap.contains("Password"), "should render password label");
    assert!(snap.contains("Create Account"), "should render submit button");
}

#[test]
fn type_into_controlled_input_updates_value() {
    let mut engine = setup();

    engine.handle_type("#username", "testuser").unwrap();
    engine.settle();

    let snap = engine.snapshot(SnapMode::Compact);
    // The controlled input should show the typed value
    assert!(
        snap.contains("=\"testuser\""),
        "username should show typed value, got:\n{snap}"
    );
}

#[test]
fn type_into_password_controlled_input() {
    let mut engine = setup();

    engine.handle_type("#password", "secret123").unwrap();
    engine.settle();

    let snap = engine.snapshot(SnapMode::Compact);
    assert!(
        snap.contains("=\"secret123\""),
        "password should show typed value, got:\n{snap}"
    );
}

#[test]
fn input_without_type_attribute_defaults_to_text() {
    let mut engine = setup();

    // The username input has no type attribute but should still be typeable
    // (HTML spec: <input> without type defaults to type="text")
    engine.handle_type("#username", "hello").unwrap();
    engine.settle();

    let snap = engine.snapshot(SnapMode::Compact);
    assert!(
        snap.contains("=\"hello\""),
        "input without type should accept text, got:\n{snap}"
    );
}

#[test]
fn click_submit_with_empty_fields_shows_validation_error() {
    let mut engine = setup();

    engine.handle_click("#submit");
    engine.settle();

    let snap = engine.snapshot(SnapMode::Compact);
    assert!(
        snap.contains("Username required"),
        "should show validation error for empty username, got:\n{snap}"
    );
}

#[test]
fn click_submit_with_short_password_shows_error() {
    let mut engine = setup();

    engine.handle_type("#username", "testuser").unwrap();
    engine.settle();
    engine.handle_type("#password", "abc").unwrap();
    engine.settle();
    engine.handle_click("#submit");
    engine.settle();

    let snap = engine.snapshot(SnapMode::Compact);
    assert!(
        snap.contains("Password required"),
        "should show password validation error, got:\n{snap}"
    );
}

#[test]
fn click_submit_with_valid_fields_shows_success() {
    let mut engine = setup();

    engine.handle_type("#username", "testuser").unwrap();
    engine.settle();
    engine.handle_type("#password", "secret123").unwrap();
    engine.settle();
    engine.handle_click("#submit");
    engine.settle();

    let snap = engine.snapshot(SnapMode::Compact);
    assert!(
        snap.contains("Account created"),
        "should show success message, got:\n{snap}"
    );
}

#[test]
fn blur_validation_fires_for_short_username() {
    let mut engine = setup();

    engine.handle_type("#username", "ab").unwrap();
    engine.settle();

    let snap = engine.snapshot(SnapMode::Compact);
    assert!(
        snap.contains("Username too short"),
        "should show onBlur validation error for 2-char username, got:\n{snap}"
    );
}

#[test]
fn value_tracking_detects_change_from_rust_set_attribute() {
    let mut engine = setup();

    // Type a value — this sets the value via Rust's set_attribute,
    // bypassing React's tracked setter. The _valueTracker should
    // still detect the change.
    engine.handle_type("#username", "alice").unwrap();
    engine.settle();

    let snap = engine.snapshot(SnapMode::Compact);
    assert!(
        snap.contains("=\"alice\""),
        "value tracking should detect change from set_attribute, got:\n{snap}"
    );

    // Type a different value — should update
    engine.handle_type("#username", "bob").unwrap();
    engine.settle();

    let snap = engine.snapshot(SnapMode::Compact);
    assert!(
        snap.contains("=\"bob\""),
        "should update to new value, got:\n{snap}"
    );
}
