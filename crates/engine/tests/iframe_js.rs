use std::collections::HashMap;

use braille_engine::{Engine, FetchedResources};

/// Helper: load HTML with pre-fetched iframe content, settle, then eval JS.
fn engine_with_iframes(html: &str, iframes: HashMap<String, String>) -> Engine {
    let mut engine = Engine::new();
    let fetched = FetchedResources {
        scripts: HashMap::new(),
        iframes,
    };
    engine.load_html_with_resources(html, &fetched);
    engine.settle();
    engine
}

// ---------------------------------------------------------------------------
// 1. Iframe src content loads and scripts execute
// ---------------------------------------------------------------------------

#[test]
fn iframe_src_content_loads() {
    let html = r#"<html><body>
        <iframe id="f1" src="https://challenge.example.com/frame.html"></iframe>
        <script>
            var received = null;
            window.addEventListener('message', function(e) {
                received = e.data;
            });
        </script>
    </body></html>"#;

    let mut iframes = HashMap::new();
    iframes.insert(
        "https://challenge.example.com/frame.html".to_string(),
        r#"<html><body><script>parent.postMessage('iframe-loaded', '*');</script></body></html>"#
            .to_string(),
    );

    let mut engine = engine_with_iframes(html, iframes);
    engine.settle();

    let result = engine.eval_js("received").unwrap();
    assert_eq!(result, "iframe-loaded", "Parent should receive message from iframe");
}

// ---------------------------------------------------------------------------
// 2. contentWindow exists
// ---------------------------------------------------------------------------

#[test]
fn iframe_contentwindow_exists() {
    let html = r#"<html><body>
        <iframe id="f1" src="https://example.com/frame.html"></iframe>
    </body></html>"#;

    let mut iframes = HashMap::new();
    iframes.insert(
        "https://example.com/frame.html".to_string(),
        "<html><body></body></html>".to_string(),
    );

    let mut engine = engine_with_iframes(html, iframes);
    engine.settle();

    let result = engine
        .eval_js("document.getElementById('f1').contentWindow !== null")
        .unwrap();
    assert_eq!(result, "true", "contentWindow should not be null");
}

// ---------------------------------------------------------------------------
// 3. contentDocument exists
// ---------------------------------------------------------------------------

#[test]
fn iframe_contentdocument_exists() {
    let html = r#"<html><body>
        <iframe id="f1" src="https://example.com/frame.html"></iframe>
    </body></html>"#;

    let mut iframes = HashMap::new();
    iframes.insert(
        "https://example.com/frame.html".to_string(),
        "<html><body></body></html>".to_string(),
    );

    let mut engine = engine_with_iframes(html, iframes);
    engine.settle();

    let result = engine
        .eval_js("document.getElementById('f1').contentDocument !== null")
        .unwrap();
    assert_eq!(result, "true", "contentDocument should not be null");

    let result = engine
        .eval_js("document.getElementById('f1').contentDocument.nodeType")
        .unwrap();
    assert_eq!(result, "9", "contentDocument should be a document node");
}

// ---------------------------------------------------------------------------
// 4. Iframe document creates elements
// ---------------------------------------------------------------------------

#[test]
fn iframe_document_creates_elements() {
    let html = r#"<html><body>
        <iframe id="f1" src="https://example.com/frame.html"></iframe>
    </body></html>"#;

    let mut iframes = HashMap::new();
    iframes.insert(
        "https://example.com/frame.html".to_string(),
        "<html><body></body></html>".to_string(),
    );

    let mut engine = engine_with_iframes(html, iframes);
    engine.settle();

    let result = engine
        .eval_js("var d = document.getElementById('f1').contentDocument; var el = d.createElement('div'); el.tagName")
        .unwrap();
    assert_eq!(result, "DIV");
}

// ---------------------------------------------------------------------------
// 5. Scripts run in isolation (no variable leaking to parent)
// ---------------------------------------------------------------------------

#[test]
fn iframe_scripts_run_in_isolation() {
    let html = r#"<html><body>
        <iframe id="f1" src="https://example.com/frame.html"></iframe>
        <script>
            var iframeSecret = 'not set';
        </script>
    </body></html>"#;

    let mut iframes = HashMap::new();
    iframes.insert(
        "https://example.com/frame.html".to_string(),
        r#"<html><body><script>var iframeSecret = 'set-by-iframe';</script></body></html>"#
            .to_string(),
    );

    let mut engine = engine_with_iframes(html, iframes);
    engine.settle();

    let result = engine.eval_js("iframeSecret").unwrap();
    assert_eq!(
        result, "not set",
        "Iframe variable should not leak to parent scope"
    );
}

// ---------------------------------------------------------------------------
// 6. Parent to iframe postMessage
// ---------------------------------------------------------------------------

#[test]
fn iframe_postmessage_parent_to_child() {
    let html = r#"<html><body>
        <iframe id="f1" src="https://example.com/frame.html"></iframe>
        <script>
            var childReceived = null;
        </script>
    </body></html>"#;

    let mut iframes = HashMap::new();
    iframes.insert(
        "https://example.com/frame.html".to_string(),
        r#"<html><body><script>
            window.addEventListener('message', function(e) {
                // Echo back what we received
                parent.postMessage('got:' + e.data, '*');
            });
        </script></body></html>"#
            .to_string(),
    );

    let mut engine = engine_with_iframes(html, iframes);
    engine.settle();

    // Set up parent listener
    engine.eval_js(r#"
        var fromChild = null;
        window.addEventListener('message', function(e) {
            if (typeof e.data === 'string' && e.data.indexOf('got:') === 0) fromChild = e.data;
        });
    "#).unwrap();

    // Send message from parent to iframe
    engine
        .eval_js("document.getElementById('f1').contentWindow.postMessage('ping', '*')")
        .unwrap();
    engine.settle();

    let result = engine.eval_js("fromChild").unwrap();
    assert_eq!(result, "got:ping", "Iframe should receive and echo parent message");
}

// ---------------------------------------------------------------------------
// 7. Bidirectional postMessage
// ---------------------------------------------------------------------------

#[test]
fn iframe_postmessage_bidirectional() {
    let html = r#"<html><body>
        <iframe id="f1" src="https://example.com/frame.html"></iframe>
        <script>
            var responses = [];
            window.addEventListener('message', function(e) {
                if (e.data === 'request') {
                    document.getElementById('f1').contentWindow.postMessage('response', '*');
                } else {
                    responses.push(e.data);
                }
            });
        </script>
    </body></html>"#;

    let mut iframes = HashMap::new();
    iframes.insert(
        "https://example.com/frame.html".to_string(),
        r#"<html><body><script>
            var gotResponse = false;
            window.addEventListener('message', function(e) {
                if (e.data === 'response') {
                    gotResponse = true;
                    parent.postMessage('done', '*');
                }
            });
            parent.postMessage('request', '*');
        </script></body></html>"#
            .to_string(),
    );

    let mut engine = engine_with_iframes(html, iframes);
    // Need multiple settles to process the async message chain
    engine.settle();
    engine.settle();
    engine.settle();

    let result = engine.eval_js("responses.join(',')").unwrap();
    assert_eq!(result, "done", "Full bidirectional message exchange should complete");
}

// ---------------------------------------------------------------------------
// 8. iframe.onload fires
// ---------------------------------------------------------------------------

#[test]
fn iframe_onload_fires() {
    let html = r#"<html><body>
        <iframe id="f1" src="https://example.com/frame.html" onload="window.__iframeLoaded = true"></iframe>
        <script>
            window.__iframeLoaded = false;
        </script>
    </body></html>"#;

    let mut iframes = HashMap::new();
    iframes.insert(
        "https://example.com/frame.html".to_string(),
        "<html><body></body></html>".to_string(),
    );

    let mut engine = engine_with_iframes(html, iframes);
    engine.settle();

    let result = engine.eval_js("window.__iframeLoaded").unwrap();
    assert_eq!(result, "true", "iframe onload should fire");
}

// ---------------------------------------------------------------------------
// 9. MessageEvent has source and origin
// ---------------------------------------------------------------------------

#[test]
fn iframe_messageevent_has_source() {
    let html = r#"<html><body>
        <iframe id="f1" src="https://example.com/frame.html"></iframe>
        <script>
            var msgSource = null;
            var msgOrigin = null;
            window.addEventListener('message', function(e) {
                msgSource = e.source;
                msgOrigin = e.origin;
            });
        </script>
    </body></html>"#;

    let mut iframes = HashMap::new();
    iframes.insert(
        "https://example.com/frame.html".to_string(),
        r#"<html><body><script>parent.postMessage('hello', '*');</script></body></html>"#
            .to_string(),
    );

    let mut engine = engine_with_iframes(html, iframes);
    engine.settle();

    let source_exists = engine.eval_js("msgSource !== null && msgSource !== undefined").unwrap();
    assert_eq!(source_exists, "true", "event.source should be set");

    let is_iframe_window = engine
        .eval_js("msgSource === document.getElementById('f1').contentWindow")
        .unwrap();
    assert_eq!(
        is_iframe_window, "true",
        "event.source should be the iframe's contentWindow"
    );
}

// ---------------------------------------------------------------------------
// 10. window.postMessage delivers to own listeners
// ---------------------------------------------------------------------------

#[test]
fn window_postmessage_self() {
    let html = r#"<html><body>
        <script>
            var selfMsg = null;
            window.addEventListener('message', function(e) {
                selfMsg = e.data;
            });
            window.postMessage('hello-self', '*');
        </script>
    </body></html>"#;

    let mut engine = Engine::new();
    engine.load_html(html);
    engine.settle();

    let result = engine.eval_js("selfMsg").unwrap();
    assert_eq!(result, "hello-self", "window.postMessage should deliver to own listeners");
}

// ---------------------------------------------------------------------------
// 11. Proton challenge iframe flow simulation
// ---------------------------------------------------------------------------

#[test]
fn proton_challenge_iframe_flow() {
    let html = r#"<html><body>
        <iframe id="challenge" src="https://account-api.proton.me/challenge/v4/html"></iframe>
        <script>
            var messages = [];
            window.addEventListener('message', function(e) {
                messages.push(e.data);
                var iframe = document.getElementById('challenge');
                if (e.data && e.data.type === 'init') {
                    iframe.contentWindow.postMessage({type: 'env.loaded'}, '*');
                } else if (e.data && e.data.type === 'onload') {
                    iframe.contentWindow.postMessage({type: 'submit.broadcast'}, '*');
                }
            });
        </script>
    </body></html>"#;

    let mut iframes = HashMap::new();
    iframes.insert(
        "https://account-api.proton.me/challenge/v4/html".to_string(),
        r#"<html><body><script>
            // Phase 1: iframe announces itself
            parent.postMessage({type: 'init'}, '*');

            // Listen for commands from parent
            window.addEventListener('message', function(e) {
                if (e.data && e.data.type === 'env.loaded') {
                    parent.postMessage({type: 'onload'}, '*');
                } else if (e.data && e.data.type === 'submit.broadcast') {
                    parent.postMessage({type: 'child.message.data', data: {fingerprint: 'test-fp-123'}}, '*');
                }
            });
        </script></body></html>"#
            .to_string(),
    );

    let mut engine = engine_with_iframes(html, iframes);
    // Multiple settles needed for the async message chain
    for _ in 0..5 {
        engine.settle();
    }

    let count = engine.eval_js("messages.length").unwrap();
    eprintln!("Messages received: {}", count);

    let types = engine
        .eval_js("messages.map(function(m) { return m.type; }).join(',')")
        .unwrap();
    eprintln!("Message types: {}", types);

    assert_eq!(
        types, "init,onload,child.message.data",
        "Full Proton challenge protocol should complete"
    );

    // Verify fingerprint data was received
    let fingerprint = engine
        .eval_js("messages[2].data.fingerprint")
        .unwrap();
    assert_eq!(fingerprint, "test-fp-123");
}
