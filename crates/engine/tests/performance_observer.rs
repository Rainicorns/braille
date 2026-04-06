use braille_engine::Engine;

fn eval(code: &str) -> String {
    let mut engine = Engine::new();
    engine.load_html("<html><body></body></html>");
    engine.eval_js(code).unwrap()
}

#[test]
fn performance_observer_exists() {
    let result = eval("typeof PerformanceObserver");
    assert_eq!(result, "function", "PerformanceObserver should be a function");
}

#[test]
fn observe_does_not_throw() {
    // Airbnb's real pattern: construct + observe paint entries + disconnect
    let result = eval(r#"
        try {
            var po = new PerformanceObserver(function(list) {});
            po.observe({ entryTypes: ['paint'] });
            po.disconnect();
            'ok'
        } catch(e) {
            'error: ' + e.message
        }
    "#);
    assert_eq!(result.trim(), "ok", "observe + disconnect should not throw");
}

#[test]
fn supported_entry_types_is_array() {
    let result = eval("Array.isArray(PerformanceObserver.supportedEntryTypes)");
    assert_eq!(result, "true", "supportedEntryTypes should be an array");
}

#[test]
fn take_records_returns_empty() {
    let result = eval(r#"
        var po = new PerformanceObserver(function(){});
        po.takeRecords().length
    "#);
    assert_eq!(result.trim(), "0", "takeRecords should return empty array");
}
