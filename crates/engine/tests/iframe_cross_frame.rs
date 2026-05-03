use std::collections::HashMap;
use braille_engine::{Engine, FetchedResources, IframeResource};

/// Minimal preamble snippet that provides setup/done/assert_true for child iframes.
/// This is a simplified version of the test runner's preamble.
fn mini_preamble() -> String {
    r#"
(function() {
    var results = [];
    var single_test_mode = false;
    var _completionFired = false;
    var _completion_callbacks = [];

    function _fireCompletion() {
        if (_completionFired) return;
        _completionFired = true;
        var tests = results.map(function(r) {
            return {name: r.name, status: r.status, message: r.message || '', PASS: 0, FAIL: 1, TIMEOUT: 2, NOTRUN: 3};
        });
        var statusObj = {status: 0, OK: 0, ERROR: 1, TIMEOUT: 2};
        for (var ci = 0; ci < _completion_callbacks.length; ci++) {
            try { _completion_callbacks[ci](tests, statusObj); } catch(e) {}
        }
        var isChild = false;
        try { isChild = typeof window !== 'undefined' && window.parent && window.parent !== window; } catch(e) {}
        if (isChild) {
            try { if (typeof window.parent.start_callback === 'function') window.parent.start_callback({}); } catch(e) {}
            for (var ti = 0; ti < tests.length; ti++) {
                var notrunCopy = {name: tests[ti].name, status: 3, message: '', PASS: 0, FAIL: 1, TIMEOUT: 2, NOTRUN: 3};
                try { if (typeof window.parent.test_state_callback === 'function') window.parent.test_state_callback(notrunCopy); } catch(e) {}
                try { if (typeof window.parent.result_callback === 'function') window.parent.result_callback(tests[ti]); } catch(e) {}
            }
            try { if (typeof window.parent.completion_callback === 'function') window.parent.completion_callback(tests, statusObj); } catch(e) {}
            try { window.parent.postMessage({type: 'start', properties: {}}, '*'); } catch(e) {}
            for (var mi = 0; mi < tests.length; mi++) {
                var nmCopy = {name: tests[mi].name, status: 3, message: '', PASS: 0, FAIL: 1, TIMEOUT: 2, NOTRUN: 3};
                try { window.parent.postMessage({type: 'test_state', test: nmCopy}, '*'); } catch(e) {}
                try { window.parent.postMessage({type: 'result', test: tests[mi]}, '*'); } catch(e) {}
            }
            try { window.parent.postMessage({type: 'complete', tests: tests, status: statusObj}, '*'); } catch(e) {}
        }
    }

    self.setup = function(props) {
        if (props && props.single_test) single_test_mode = true;
    };
    self.assert_true = function(val, msg) {
        if (val !== true) throw new Error(msg || "assert_true failed");
    };
    self.done = function() {
        if (single_test_mode && results.length === 0) {
            results.push({ name: 'test', status: 0, message: '' });
        }
        _fireCompletion();
    };
    self.add_completion_callback = function(fn) { _completion_callbacks.push(fn); };
})();
"#.to_string()
}

#[test]
fn iframe_child_calls_parent_completion_callback() {
    let mut engine = Engine::new();

    let preamble = mini_preamble();

    let html = r#"<!DOCTYPE html>
<html><body>
<div id="result">waiting</div>
<script>
    var callbackFired = false;
    function completion_callback(tests, status) {
        callbackFired = true;
        document.getElementById('result').textContent = 'done:' + tests.length;
    }
</script>
<iframe id="child" src="child.html"></iframe>
</body></html>"#;

    let child_html = r#"<!DOCTYPE html>
<html><head>
<script src="/preamble.js"></script>
</head>
<body>
<script>
setup({ single_test: true });
assert_true(true);
done();
</script>
</body></html>"#;

    let mut scripts = HashMap::new();
    scripts.insert("/preamble.js".to_string(), preamble);

    let mut iframes = HashMap::new();
    iframes.insert("child.html".to_string(), IframeResource { content: child_html.to_string(), content_type: "text/html".into() });

    let resources = FetchedResources {
        scripts,
        iframes,
        css: HashMap::new(),
    };

    let errors = engine.load_html_with_resources_lossy(html, &resources);
    eprintln!("JS errors: {:?}", errors);

    engine.settle();

    let callback_fired = engine.eval_js("String(callbackFired)").unwrap_or_default();
    eprintln!("completion_callback fired: {}", callback_fired);
    assert_eq!(callback_fired, "true");

    let result = engine.eval_js("document.getElementById('result').textContent").unwrap_or_default();
    eprintln!("Result: {}", result);
    assert_eq!(result, "done:1");
}

#[test]
fn iframe_child_postmessage_to_parent() {
    let mut engine = Engine::new();

    let preamble = mini_preamble();

    let html = r#"<!DOCTYPE html>
<html><body>
<div id="msgs"></div>
<script>
    var received = [];
    window.addEventListener('message', function(ev) {
        if (ev.data && ev.data.type) {
            received.push(ev.data.type);
        }
    });
</script>
<iframe id="child" src="child.html"></iframe>
</body></html>"#;

    let child_html = r#"<!DOCTYPE html>
<html><head>
<script src="/preamble.js"></script>
</head>
<body>
<script>
setup({ single_test: true });
assert_true(true);
done();
</script>
</body></html>"#;

    let mut scripts = HashMap::new();
    scripts.insert("/preamble.js".to_string(), preamble);

    let mut iframes = HashMap::new();
    iframes.insert("child.html".to_string(), IframeResource { content: child_html.to_string(), content_type: "text/html".into() });

    let resources = FetchedResources {
        scripts,
        iframes,
        css: HashMap::new(),
    };

    let errors = engine.load_html_with_resources_lossy(html, &resources);
    eprintln!("JS errors: {:?}", errors);

    engine.settle();

    let msgs = engine.eval_js("JSON.stringify(received)").unwrap_or_default();
    eprintln!("Messages received: {}", msgs);

    assert!(msgs.contains("start"), "should receive 'start' message");
    assert!(msgs.contains("complete"), "should receive 'complete' message");
    assert!(msgs.contains("result"), "should receive 'result' message");
}
