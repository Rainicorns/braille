//! Tests for PerformanceObserver stub.
//!
//! Sites like Airbnb use PerformanceObserver for paint/LCP/layout-shift metrics.
//! Our stub must provide the class, constructor, methods, and supportedEntryTypes
//! so scripts don't crash at initialization time.

use braille_engine::Engine;

fn eval(code: &str) -> String {
    let mut engine = Engine::new();
    engine.load_html("<html><body></body></html>");
    engine.eval_js(code).unwrap_or_else(|e| format!("ERROR: {e}"))
}

#[test]
fn performance_observer_exists_as_function() {
    let result = eval("typeof PerformanceObserver");
    assert_eq!(result.trim_matches('"'), "function", "PerformanceObserver should be a function");
}

#[test]
fn performance_observer_constructor_works() {
    let result = eval(
        r#"
        var obs = new PerformanceObserver(function(list) {});
        typeof obs
    "#,
    );
    assert_eq!(result.trim_matches('"'), "object");
}

#[test]
fn performance_observer_observe_no_crash() {
    let result = eval(
        r#"
        var obs = new PerformanceObserver(function(list) {});
        obs.observe({ entryTypes: ['paint'] });
        'ok'
    "#,
    );
    assert_eq!(result.trim_matches('"'), "ok");
}

#[test]
fn performance_observer_disconnect_no_crash() {
    let result = eval(
        r#"
        var obs = new PerformanceObserver(function(list) {});
        obs.observe({ type: 'largest-contentful-paint', buffered: true });
        obs.disconnect();
        'ok'
    "#,
    );
    assert_eq!(result.trim_matches('"'), "ok");
}

#[test]
fn performance_observer_take_records_returns_array() {
    let result = eval(
        r#"
        var obs = new PerformanceObserver(function(list) {});
        var records = obs.takeRecords();
        Array.isArray(records) && records.length === 0
    "#,
    );
    assert!(result.contains("true"), "takeRecords should return empty array, got: {result}");
}

#[test]
fn performance_observer_supported_entry_types_includes_lcp() {
    let result = eval("PerformanceObserver.supportedEntryTypes.indexOf('largest-contentful-paint') >= 0");
    assert!(result.contains("true"), "supportedEntryTypes should include LCP, got: {result}");
}

#[test]
fn performance_observer_supported_entry_types_includes_layout_shift() {
    let result = eval("PerformanceObserver.supportedEntryTypes.indexOf('layout-shift') >= 0");
    assert!(
        result.contains("true"),
        "supportedEntryTypes should include layout-shift, got: {result}"
    );
}

#[test]
fn performance_observer_supported_entry_types_full_list() {
    let result = eval("JSON.stringify(PerformanceObserver.supportedEntryTypes)");
    let expected = [
        "element",
        "event",
        "first-input",
        "largest-contentful-paint",
        "layout-shift",
        "longtask",
        "mark",
        "measure",
        "navigation",
        "paint",
        "resource",
    ];
    for entry in &expected {
        assert!(
            result.contains(entry),
            "supportedEntryTypes should include '{entry}', got: {result}"
        );
    }
}
