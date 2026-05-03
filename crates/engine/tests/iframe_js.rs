use std::collections::HashMap;

use braille_engine::{Engine, FetchedResources, IframeResource};

/// Helper: load HTML with pre-fetched iframe content, settle, then eval JS.
fn engine_with_iframes(html: &str, iframes: HashMap<String, IframeResource>) -> Engine {
    let mut engine = Engine::new();
    let fetched = FetchedResources {
        scripts: HashMap::new(),
        iframes,
        css: HashMap::new(),
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
        IframeResource { content: r#"<html><body><script>parent.postMessage('iframe-loaded', '*');</script></body></html>"#.to_string(), content_type: "text/html".into() },
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
        IframeResource { content: "<html><body></body></html>".to_string(), content_type: "text/html".into() },
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
        IframeResource { content: "<html><body></body></html>".to_string(), content_type: "text/html".into() },
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
        IframeResource { content: "<html><body></body></html>".to_string(), content_type: "text/html".into() },
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
        IframeResource { content: r#"<html><body><script>var iframeSecret = 'set-by-iframe';</script></body></html>"#.to_string(), content_type: "text/html".into() },
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
        IframeResource { content: r#"<html><body><script>
            window.addEventListener('message', function(e) {
                // Echo back what we received
                parent.postMessage('got:' + e.data, '*');
            });
        </script></body></html>"#.to_string(), content_type: "text/html".into() },
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
        IframeResource { content: r#"<html><body><script>
            var gotResponse = false;
            window.addEventListener('message', function(e) {
                if (e.data === 'response') {
                    gotResponse = true;
                    parent.postMessage('done', '*');
                }
            });
            parent.postMessage('request', '*');
        </script></body></html>"#.to_string(), content_type: "text/html".into() },
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
        IframeResource { content: "<html><body></body></html>".to_string(), content_type: "text/html".into() },
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
        IframeResource { content: r#"<html><body><script>parent.postMessage('hello', '*');</script></body></html>"#.to_string(), content_type: "text/html".into() },
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
        IframeResource { content: r#"<html><body><script>
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
        </script></body></html>"#.to_string(), content_type: "text/html".into() },
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

// ---------------------------------------------------------------------------
// 12. Dynamic iframe (createElement + appendChild) gets contentDocument
// ---------------------------------------------------------------------------

#[test]
fn dynamic_iframe_gets_content_document() {
    let html = r#"<html><body><script>
        var iframe = document.createElement('iframe');
        document.body.appendChild(iframe);
        window.__hasDoc = iframe.contentDocument !== null;
        window.__hasBody = iframe.contentDocument && iframe.contentDocument.body !== null;
        window.__docType = iframe.contentDocument ? iframe.contentDocument.nodeType : -1;
    </script></body></html>"#;

    let mut engine = Engine::new();
    engine.load_html(html);
    engine.settle();

    let has_doc = engine.eval_js("window.__hasDoc").unwrap();
    assert_eq!(has_doc, "true", "Dynamic iframe should have contentDocument");

    let has_body = engine.eval_js("window.__hasBody").unwrap();
    assert_eq!(has_body, "true", "Dynamic iframe contentDocument should have body");

    let doc_type = engine.eval_js("window.__docType").unwrap();
    assert_eq!(doc_type, "9", "contentDocument nodeType should be 9");
}

// ---------------------------------------------------------------------------
// 13. Script appended to iframe contentDocument.body executes
// ---------------------------------------------------------------------------

#[test]
fn script_appended_to_iframe_executes() {
    let html = r#"<html><body><script>
        window.__iframeResult = 'not set';
        var iframe = document.createElement('iframe');
        document.body.appendChild(iframe);
        var script = document.createElement('script');
        script.textContent = "parent.postMessage('script-ran', '*');";
        iframe.contentDocument.body.appendChild(script);
        window.addEventListener('message', function(e) {
            if (e.data === 'script-ran') window.__iframeResult = 'executed';
        });
    </script></body></html>"#;

    let mut engine = Engine::new();
    engine.load_html(html);
    engine.settle();

    let result = engine.eval_js("window.__iframeResult").unwrap();
    assert_eq!(result, "executed", "Script appended to iframe should execute");
}

// ---------------------------------------------------------------------------
// 14. Script in iframe sees iframe's document, not parent's
// ---------------------------------------------------------------------------

#[test]
fn iframe_script_sees_iframe_document() {
    let html = r#"<html><body>
        <div id="parent-marker"></div>
        <script>
            window.__iframeSeesParentMarker = 'unknown';
            window.__iframeSelfCheck = 'unknown';
            var iframe = document.createElement('iframe');
            document.body.appendChild(iframe);
            var script = document.createElement('script');
            script.textContent = "var marker = document.getElementById('parent-marker'); parent.postMessage({hasMarker: !!marker, bodyTag: document.body.tagName}, '*');";
            iframe.contentDocument.body.appendChild(script);
            window.addEventListener('message', function(e) {
                if (e.data && e.data.hasMarker !== undefined) {
                    window.__iframeSeesParentMarker = e.data.hasMarker;
                    window.__iframeSelfCheck = e.data.bodyTag;
                }
            });
        </script>
    </body></html>"#;

    let mut engine = Engine::new();
    engine.load_html(html);
    engine.settle();

    let sees_marker = engine.eval_js("window.__iframeSeesParentMarker").unwrap();
    assert_eq!(sees_marker, "false", "Iframe script should NOT see parent's #parent-marker");

    let body_tag = engine.eval_js("window.__iframeSelfCheck").unwrap();
    assert_eq!(body_tag, "BODY", "Iframe document should have a body element");
}

// ---------------------------------------------------------------------------
// 15. WPT pattern: createElement iframe + contentDocument.body.appendChild(script)
// ---------------------------------------------------------------------------

#[test]
fn wpt_iframe_content_document_script_pattern() {
    let html = r#"<html><body><script>
        window.__results = [];
        var child = document.createElement("iframe");
        document.body.appendChild(child);

        var script = document.createElement("script");
        script.textContent = "window.__results.push('child-script-ran');";
        child.contentDocument.body.appendChild(script);
    </script></body></html>"#;

    let mut engine = Engine::new();
    engine.load_html(html);
    engine.settle();

    let result = engine.eval_js("window.__results.join(',')").unwrap();
    eprintln!("results: {}", result);
    // The script runs in iframe context where window.__results doesn't exist,
    // so we check via a different mechanism
    let has_content_doc = engine
        .eval_js("document.querySelector('iframe').contentDocument !== null")
        .unwrap();
    assert_eq!(has_content_doc, "true", "iframe should have contentDocument");

    let has_body = engine
        .eval_js("document.querySelector('iframe').contentDocument.body !== null")
        .unwrap();
    assert_eq!(has_body, "true", "iframe contentDocument should have body");
}

// ---------------------------------------------------------------------------
// window.frames collection
// ---------------------------------------------------------------------------

#[test]
fn window_frames_returns_iframe_contentwindows() {
    let html = r#"<html><body>
        <iframe id="f1" src="https://example.com/a.html"></iframe>
        <iframe id="f2" src="https://example.com/b.html"></iframe>
    </body></html>"#;

    let mut iframes = HashMap::new();
    iframes.insert(
        "https://example.com/a.html".to_string(),
        IframeResource { content: "<html><body><p>A</p></body></html>".to_string(), content_type: "text/html".into() },
    );
    iframes.insert(
        "https://example.com/b.html".to_string(),
        IframeResource { content: "<html><body><p>B</p></body></html>".to_string(), content_type: "text/html".into() },
    );

    let mut engine = engine_with_iframes(html, iframes);
    engine.settle();

    let typeof_frames = engine.eval_js("typeof window.frames").unwrap();
    eprintln!("typeof frames: {}", typeof_frames);

    let frames_len = engine.eval_js("window.frames.length").unwrap();
    eprintln!("frames.length: {}", frames_len);
    assert_eq!(frames_len, "2", "window.frames should have 2 entries");

    let frame0_is_cw = engine
        .eval_js("window.frames[0] === document.getElementById('f1').contentWindow")
        .unwrap();
    eprintln!("frames[0] === f1.contentWindow: {}", frame0_is_cw);
    assert_eq!(frame0_is_cw, "true", "frames[0] should be f1's contentWindow");

    let frame1_is_cw = engine
        .eval_js("window.frames[1] === document.getElementById('f2').contentWindow")
        .unwrap();
    assert_eq!(frame1_is_cw, "true", "frames[1] should be f2's contentWindow");
}

#[test]
fn window_length_counts_iframes() {
    let html = r#"<html><body>
        <iframe src="https://example.com/a.html"></iframe>
    </body></html>"#;

    let mut iframes = HashMap::new();
    iframes.insert(
        "https://example.com/a.html".to_string(),
        IframeResource { content: "<html><body></body></html>".to_string(), content_type: "text/html".into() },
    );

    let mut engine = engine_with_iframes(html, iframes);
    engine.settle();

    let len = engine.eval_js("window.length").unwrap();
    assert_eq!(len, "1", "window.length should count iframes");
}

#[test]
fn window_frames_accessible_in_onload() {
    let html = r#"<html><body>
        <iframe src="https://example.com/frame.html"></iframe>
        <script>
            window.testResult = 'not set';
            window.onload = function() {
                window.testResult = 'frames_len=' + window.frames.length;
                if (window.frames.length > 0 && window.frames[0].document) {
                    window.testResult += ',has_doc=true';
                }
            };
        </script>
    </body></html>"#;

    let mut iframes = HashMap::new();
    iframes.insert(
        "https://example.com/frame.html".to_string(),
        IframeResource { content: "<html><body><p>hello</p></body></html>".to_string(), content_type: "text/html".into() },
    );

    let mut engine = Engine::new();
    let fetched = FetchedResources {
        scripts: HashMap::new(),
        iframes,
        css: HashMap::new(),
    };
    engine.load_html_with_resources(html, &fetched);
    engine.settle();

    let result = engine.eval_js("window.testResult").unwrap();
    eprintln!("testResult: {}", result);
    assert_eq!(result, "frames_len=1,has_doc=true", "frames should be accessible in onload with document");
}

#[test]
fn window_frames_xml_iframe_document_access() {
    // Verify frames[0].document.documentElement is accessible for XML iframes
    let html = r#"<!doctype html>
<iframe src="test.xml"></iframe>
<script>
  window.testResult = 'not set';
  window.onload = function() {
    var f = window.frames;
    if (f.length > 0 && f[0].document && f[0].document.documentElement) {
      window.testResult = 'tag=' + f[0].document.documentElement.tagName;
    }
  };
</script>"#;

    let mut iframes = HashMap::new();
    iframes.insert("test.xml".to_string(), IframeResource { content: "<root/>".to_string(), content_type: "application/xml".into() });

    let mut engine = Engine::new();
    let fetched = FetchedResources {
        scripts: HashMap::new(),
        iframes,
        css: HashMap::new(),
    };
    engine.load_html_with_resources(html, &fetched);
    engine.settle();

    let result = engine.eval_js("window.testResult").unwrap();
    eprintln!("testResult: {}", result);
    assert!(result.starts_with("tag="), "should access iframe document element: {}", result);
}
