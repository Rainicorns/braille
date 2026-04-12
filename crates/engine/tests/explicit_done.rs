use std::collections::HashMap;
use braille_engine::{Engine, FetchedResources};

/// Minimal preamble with explicit_done support
fn preamble_with_explicit_done() -> String {
    r#"
(function() {
    var results = [];
    var _explicit_done = false;
    var _completionFired = false;
    var _completion_callbacks = [];

    function _fireCompletion() {
        if (_completionFired) return;
        _completionFired = true;
        var tests = results.map(function(r) {
            return {name: r.name, status: r.status, message: r.message || ''};
        });
        var statusObj = {status: 0};
        for (var ci = 0; ci < _completion_callbacks.length; ci++) {
            try { _completion_callbacks[ci](tests, statusObj); } catch(e) {}
        }
    }

    self.setup = function(props) {
        if (props && props.explicit_done) _explicit_done = true;
    };
    self.test = function(fn, name) {
        var result = { name: name || 'test', status: 0, message: '' };
        try { fn(); } catch(e) { result.status = 1; result.message = e.message; }
        results.push(result);
    };
    self.assert_true = function(val, msg) {
        if (val !== true) throw new Error(msg || "assert_true failed");
    };
    self.done = function() {
        _fireCompletion();
    };
    self.add_completion_callback = function(fn) { _completion_callbacks.push(fn); };

    // Auto-completion (disabled when explicit_done)
    if (!_explicit_done) {
        setTimeout(function() {
            if (!_completionFired && !_explicit_done) _fireCompletion();
        }, 0);
    }
})();
"#.to_string()
}

#[test]
fn explicit_done_prevents_auto_completion() {
    let mut engine = Engine::new();

    let preamble = preamble_with_explicit_done();

    let html = r#"<!DOCTYPE html>
<html><head>
<script src="/preamble.js"></script>
</head>
<body>
<script>
    setup({explicit_done: true});

    var callbackFired = false;
    add_completion_callback(function(tests, status) {
        callbackFired = true;
    });

    test(function() {
        assert_true(true);
    }, "a passing test");
</script>
</body></html>"#;

    let mut scripts = HashMap::new();
    scripts.insert("/preamble.js".to_string(), preamble);

    let resources = FetchedResources {
        scripts,
        iframes: HashMap::new(),
        css: HashMap::new(),
    };

    let errors = engine.load_html_with_resources_lossy(html, &resources);
    eprintln!("JS errors: {:?}", errors);
    engine.settle();

    let fired = engine.eval_js("String(callbackFired)").unwrap_or_default();
    eprintln!("Completion fired before done(): {}", fired);
    assert_eq!(fired, "false", "completion should not fire before done()");

    engine.eval_js("done()").unwrap();
    engine.settle();

    let fired = engine.eval_js("String(callbackFired)").unwrap_or_default();
    eprintln!("Completion fired after done(): {}", fired);
    assert_eq!(fired, "true", "completion should fire after done()");
}
