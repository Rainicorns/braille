use std::collections::HashMap;

use braille_engine::{Engine, FetchedResources, IframeResource};

/// Helper: load HTML with pre-fetched iframe content, settle, return engine.
fn engine_with_iframes(html: &str, iframes: HashMap<String, IframeResource>) -> Engine {
    let mut engine = Engine::new();
    let fetched = FetchedResources {
        scripts: HashMap::new(),
        iframes,
        css: HashMap::new(),
    };
    engine.load_html_with_resources_lossy(html, &fetched);
    engine.settle();
    engine
}

// ---------------------------------------------------------------------------
// 1. Parent receives message from iframe via parent.postMessage
// ---------------------------------------------------------------------------

#[test]
fn parent_receives_message_from_iframe() {
    let html = r#"<!DOCTYPE html>
<html><body>
<script>
    var received = null;
    window.addEventListener('message', function(e) {
        received = e.data;
    });
</script>
<iframe id="f1" src="https://challenge.example.com/frame.html"></iframe>
</body></html>"#;

    let mut iframes = HashMap::new();
    iframes.insert(
        "https://challenge.example.com/frame.html".to_string(),
        IframeResource { content: r#"<html><body><script>window.parent.postMessage({type:'hello'}, '*');</script></body></html>"#.to_string(), content_type: "text/html".into() },
    );

    let mut engine = engine_with_iframes(html, iframes);
    engine.settle();

    let result = engine.eval_js("JSON.stringify(received)").unwrap();
    eprintln!("parent received: {}", result);
    assert_eq!(result, r#"{"type":"hello"}"#, "Parent should receive message from iframe");
}

// ---------------------------------------------------------------------------
// 2. Iframe receives message from parent via contentWindow.postMessage
// ---------------------------------------------------------------------------

#[test]
fn iframe_receives_message_from_parent() {
    let html = r#"<!DOCTYPE html>
<html><body>
<iframe id="f1" src="https://example.com/frame.html"></iframe>
</body></html>"#;

    let mut iframes = HashMap::new();
    iframes.insert(
        "https://example.com/frame.html".to_string(),
        // Must use window.X (not var) so it's accessible from parent via contentWindow
        IframeResource { content: r#"<html><body><script>
            window.iframeReceived = null;
            window.addEventListener('message', function(e) {
                window.iframeReceived = e.data;
            });
        </script></body></html>"#.to_string(), content_type: "text/html".into() },
    );

    let mut engine = engine_with_iframes(html, iframes);

    // Post to iframe AFTER it's loaded (realistic pattern — parent posts in response to load)
    engine.eval_js(
        "document.getElementById('f1').contentWindow.postMessage({type:'ping'}, '*')"
    ).unwrap();
    engine.settle();

    // Read the iframe's received value
    let result = engine.eval_js(
        "JSON.stringify(document.getElementById('f1').contentWindow.iframeReceived)"
    ).unwrap();
    eprintln!("iframe received: {}", result);
    assert_eq!(result, r#"{"type":"ping"}"#, "Iframe should receive message from parent");
}

// ---------------------------------------------------------------------------
// 3. Bidirectional message exchange (Proton challenge pattern)
// ---------------------------------------------------------------------------

#[test]
fn bidirectional_message_exchange() {
    let html = r#"<!DOCTYPE html>
<html><body>
<script>
    var parentMessages = [];
    window.addEventListener('message', function(e) {
        parentMessages.push(e.data);
        // When we get 'init' from iframe, respond with 'challenge'
        if (e.data && e.data.type === 'init') {
            var iframe = document.getElementById('challenge');
            iframe.contentWindow.postMessage({type:'challenge', value:42}, '*');
        }
    });
</script>
<iframe id="challenge" src="https://challenge.example.com/v4/html"></iframe>
</body></html>"#;

    let mut iframes = HashMap::new();
    iframes.insert(
        "https://challenge.example.com/v4/html".to_string(),
        // Must use window.X (not var) so values are accessible via contentWindow
        IframeResource { content: r#"<html><body><script>
            window.challengeMessages = [];
            window.addEventListener('message', function(e) {
                window.challengeMessages.push(e.data);
                // When we get 'challenge' from parent, respond with 'solution'
                if (e.data && e.data.type === 'challenge') {
                    window.parent.postMessage({type:'solution', answer: e.data.value * 2}, '*');
                }
            });
            // Send init to parent
            window.parent.postMessage({type:'init'}, '*');
        </script></body></html>"#.to_string(), content_type: "text/html".into() },
    );

    let mut engine = engine_with_iframes(html, iframes);
    // Extra settles to let the full message chain resolve
    engine.settle();
    engine.settle();
    engine.settle();

    let parent_msgs = engine.eval_js("JSON.stringify(parentMessages)").unwrap();
    eprintln!("parent messages: {}", parent_msgs);

    // Parent should have received: init from iframe, then solution from iframe
    assert!(parent_msgs.contains(r#""type":"init"#), "Parent should receive init: {parent_msgs}");
    assert!(parent_msgs.contains(r#""type":"solution"#), "Parent should receive solution: {parent_msgs}");

    // Iframe should have received: challenge from parent
    let iframe_msgs = engine.eval_js(
        "JSON.stringify(document.getElementById('challenge').contentWindow.challengeMessages)"
    ).unwrap();
    eprintln!("iframe messages: {}", iframe_msgs);
    assert!(iframe_msgs.contains(r#""type":"challenge"#), "Iframe should receive challenge: {iframe_msgs}");
}

// ---------------------------------------------------------------------------
// 4. MessageEvent has correct source and origin
// ---------------------------------------------------------------------------

#[test]
fn message_event_has_correct_source_and_origin() {
    let html = r#"<!DOCTYPE html>
<html><body>
<script>
    var msgSource = null;
    var msgOrigin = null;
    window.addEventListener('message', function(e) {
        msgSource = e.source;
        msgOrigin = e.origin;
    });
</script>
<iframe id="f1" src="https://example.com/frame.html"></iframe>
</body></html>"#;

    let mut iframes = HashMap::new();
    iframes.insert(
        "https://example.com/frame.html".to_string(),
        IframeResource { content: r#"<html><body><script>
            window.parent.postMessage('hello', '*');
        </script></body></html>"#.to_string(), content_type: "text/html".into() },
    );

    let mut engine = engine_with_iframes(html, iframes);
    engine.settle();

    // source should be the iframe's window (not null)
    let source_check = engine.eval_js("msgSource !== null").unwrap();
    assert_eq!(source_check, "true", "event.source should not be null");

    // source should be the iframe's contentWindow
    let source_is_iframe = engine.eval_js(
        "msgSource === document.getElementById('f1').contentWindow"
    ).unwrap();
    eprintln!("source === iframe.contentWindow: {}", source_is_iframe);
    assert_eq!(source_is_iframe, "true", "event.source should be the iframe's contentWindow");
}

// ---------------------------------------------------------------------------
// 5. Structured clone preserves data
// ---------------------------------------------------------------------------

#[test]
fn structured_clone_preserves_data() {
    let html = r#"<!DOCTYPE html>
<html><body>
<script>
    var received = null;
    window.addEventListener('message', function(e) {
        received = e.data;
    });
</script>
<iframe id="f1" src="https://example.com/frame.html"></iframe>
</body></html>"#;

    let mut iframes = HashMap::new();
    iframes.insert(
        "https://example.com/frame.html".to_string(),
        IframeResource { content: r#"<html><body><script>
            window.parent.postMessage({
                str: 'hello',
                num: 42,
                bool: true,
                nil: null,
                arr: [1, 'two', {three: 3}],
                nested: {a: {b: {c: 'deep'}}}
            }, '*');
        </script></body></html>"#.to_string(), content_type: "text/html".into() },
    );

    let mut engine = engine_with_iframes(html, iframes);
    engine.settle();

    let result = engine.eval_js("received.str").unwrap();
    assert_eq!(result, "hello");

    let result = engine.eval_js("received.num").unwrap();
    assert_eq!(result, "42");

    let result = engine.eval_js("received.bool").unwrap();
    assert_eq!(result, "true");

    let result = engine.eval_js("received.nil").unwrap();
    assert_eq!(result, "null");

    let result = engine.eval_js("received.arr[2].three").unwrap();
    assert_eq!(result, "3");

    let result = engine.eval_js("received.nested.a.b.c").unwrap();
    assert_eq!(result, "deep");
}
