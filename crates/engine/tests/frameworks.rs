//! Framework compatibility tests.
//!
//! Loads small apps written with framework-like patterns (React/Preact/Vue-style
//! virtual DOM, reactive state, component composition) and verifies they render
//! correctly and respond to interactions via the Engine API.
//!
//! These tests validate that the DOM API surface is sufficient for real framework
//! patterns: createElement, appendChild, innerHTML, addEventListener, event
//! dispatch, Proxy, input.value, form submission, etc.

use braille_engine::Engine;
use braille_wire::SnapMode;

fn load_framework_test(filename: &str) -> (Engine, String) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tests/frameworks")
        .join(filename);
    let html = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", path.display(), e));
    let mut engine = Engine::new();
    engine.load_html(&html);
    let snapshot = engine.snapshot(SnapMode::Accessibility);
    (engine, snapshot)
}

fn snap_framework(filename: &str) -> String {
    let (_engine, snapshot) = load_framework_test(filename);
    snapshot
}

// ---------------------------------------------------------------------------
// Preact-like counter
// ---------------------------------------------------------------------------

#[test]
fn preact_counter_initial_render() {
    let snapshot = snap_framework("preact_counter.html");

    assert!(snapshot.contains("Counter: 0"), "should show initial count");
    assert!(snapshot.contains("Current value: 0"), "should show current value");
}

#[test]
fn preact_counter_increment() {
    let (mut engine, _snapshot) = load_framework_test("preact_counter.html");

    // Click the increment button
    engine.handle_click("#inc");
    let snapshot = engine.snapshot(SnapMode::Accessibility);

    assert!(snapshot.contains("Counter: 1"), "count should be 1 after increment");
    assert!(snapshot.contains("Current value: 1"), "current value should update");
}

#[test]
fn preact_counter_multiple_clicks() {
    let (mut engine, _snapshot) = load_framework_test("preact_counter.html");

    engine.handle_click("#inc");
    engine.handle_click("#inc");
    engine.handle_click("#inc");
    engine.handle_click("#dec");

    let snapshot = engine.snapshot(SnapMode::Accessibility);

    assert!(snapshot.contains("Counter: 2"), "count should be 2");
}

// ---------------------------------------------------------------------------
// Preact-like todo
// ---------------------------------------------------------------------------

#[test]
fn preact_todo_initial_render() {
    let snapshot = snap_framework("preact_todo.html");

    assert!(snapshot.contains("Todo App"), "should show title");
    assert!(snapshot.contains("0 of 0 done"), "should show empty count");
}

#[test]
fn preact_todo_add_item() {
    let (mut engine, _snapshot) = load_framework_test("preact_todo.html");

    // Type into the input and submit
    engine.handle_type("#new-todo", "Buy groceries").unwrap();
    engine.handle_click("button[type=submit]");

    let snapshot = engine.snapshot(SnapMode::Accessibility);

    assert!(snapshot.contains("Buy groceries"), "should show added todo");
    assert!(snapshot.contains("0 of 1 done"), "should update count");
}

// ---------------------------------------------------------------------------
// React-like counter
// ---------------------------------------------------------------------------

#[test]
fn react_counter_initial_render() {
    let snapshot = snap_framework("react_counter.html");

    assert!(snapshot.contains("React Counter"), "should show title");
    assert!(snapshot.contains("Count: 0"), "should show initial count");
}

#[test]
fn react_counter_increment() {
    let (mut engine, _snapshot) = load_framework_test("react_counter.html");

    engine.handle_click("#inc");
    let snapshot = engine.snapshot(SnapMode::Accessibility);

    assert!(snapshot.contains("Count: 1"), "count should be 1");
}

#[test]
fn react_counter_decrement_and_reset() {
    let (mut engine, _snapshot) = load_framework_test("react_counter.html");

    engine.handle_click("#inc");
    engine.handle_click("#inc");
    engine.handle_click("#inc");
    engine.handle_click("#dec");

    let snapshot = engine.snapshot(SnapMode::Accessibility);
    assert!(snapshot.contains("Count: 2"), "count should be 2");

    engine.handle_click("#reset");
    let snapshot = engine.snapshot(SnapMode::Accessibility);
    assert!(snapshot.contains("Count: 0"), "count should reset to 0");
}

// ---------------------------------------------------------------------------
// React-like todo
// ---------------------------------------------------------------------------

#[test]
fn react_todo_initial_render() {
    let snapshot = snap_framework("react_todo.html");

    assert!(snapshot.contains("React Todo"), "should show title");
    assert!(snapshot.contains("0 of 0 completed"), "should show empty count");
}

#[test]
fn react_todo_add_item() {
    let (mut engine, _snapshot) = load_framework_test("react_todo.html");

    engine.handle_type("#new-todo", "Write tests").unwrap();
    engine.handle_click("button[type=submit]");

    let snapshot = engine.snapshot(SnapMode::Accessibility);

    assert!(snapshot.contains("Write tests"), "should show added todo");
    assert!(snapshot.contains("0 of 1 completed"), "should update count");
}

// ---------------------------------------------------------------------------
// Vue-like counter
// ---------------------------------------------------------------------------

#[test]
fn vue_counter_initial_render() {
    let snapshot = snap_framework("vue_counter.html");

    assert!(snapshot.contains("Vue Counter"), "should show title");
    assert!(snapshot.contains("Count: 0"), "should show initial count");
    assert!(snapshot.contains("Zero"), "should show conditional text");
}

#[test]
fn vue_counter_increment() {
    let (mut engine, _snapshot) = load_framework_test("vue_counter.html");

    engine.handle_click("#inc");
    let snapshot = engine.snapshot(SnapMode::Accessibility);

    assert!(snapshot.contains("Count: 1"), "count should be 1");
    assert!(snapshot.contains("Positive"), "should show Positive");
}

#[test]
fn vue_counter_negative() {
    let (mut engine, _snapshot) = load_framework_test("vue_counter.html");

    engine.handle_click("#dec");
    let snapshot = engine.snapshot(SnapMode::Accessibility);

    assert!(snapshot.contains("Count: -1"), "count should be -1");
    assert!(snapshot.contains("Negative"), "should show Negative");
}

// ---------------------------------------------------------------------------
// Static HTML form submit (verifies activation behavior through handle_click)
// ---------------------------------------------------------------------------

#[test]
fn static_form_submit_via_handle_click() {
    let html = r#"
    <html><body>
    <p id="result">none</p>
    <form id="myform">
      <input type="text" id="inp" value="hello">
      <button type="submit" id="btn">Go</button>
    </form>
    <script>
      document.getElementById("myform").addEventListener("submit", function(e) {
        e.preventDefault();
        document.getElementById("result").textContent = "submitted: " + document.getElementById("inp").value;
      });
    </script>
    </body></html>
    "#;
    let mut engine = Engine::new();
    engine.load_html(html);
    engine.snapshot(SnapMode::Accessibility);

    engine.handle_click("#btn");
    let snap = engine.snapshot(SnapMode::Accessibility);
    assert!(snap.contains("submitted: hello"), "form submit should have fired");
}
