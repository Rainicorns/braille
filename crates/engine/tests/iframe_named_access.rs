use std::collections::HashMap;
use braille_engine::{Engine, FetchedResources, IframeResource};

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

#[test]
fn iframe_name_creates_window_property() {
    let html = r#"<!DOCTYPE html>
<html><body>
<iframe name="myFrame" id="f1" src="empty.html"></iframe>
</body></html>"#;

    let mut iframes = HashMap::new();
    iframes.insert("empty.html".to_string(), IframeResource { content: "<html><body></body></html>".to_string(), content_type: "text/html".into() });
    let mut engine = engine_with_iframes(html, iframes);
    engine.settle();

    let by_name = engine.eval_js("typeof myFrame").unwrap();
    eprintln!("myFrame by name: {}", by_name);
    assert_eq!(by_name, "object", "window.myFrame should resolve to iframe contentWindow");

    let is_cw = engine.eval_js("myFrame === document.getElementById('f1').contentWindow").unwrap();
    eprintln!("myFrame === contentWindow: {}", is_cw);
    assert_eq!(is_cw, "true", "named access should return the contentWindow");
}

#[test]
fn frames_indexed_access() {
    let html = r#"<!DOCTYPE html>
<html><body>
<iframe id="f1" src="empty.html"></iframe>
</body></html>"#;

    let mut iframes = HashMap::new();
    iframes.insert("empty.html".to_string(), IframeResource { content: "<html><body></body></html>".to_string(), content_type: "text/html".into() });
    let mut engine = engine_with_iframes(html, iframes);
    engine.settle();

    let f0_type = engine.eval_js("typeof frames[0]").unwrap();
    eprintln!("frames[0] type: {}", f0_type);
    assert_eq!(f0_type, "object", "frames[0] should be the first iframe's contentWindow");

    let has_post = engine.eval_js("typeof frames[0].postMessage").unwrap();
    eprintln!("frames[0].postMessage type: {}", has_post);
    assert_eq!(has_post, "function", "frames[0].postMessage should be a function");
}

#[test]
fn cross_realm_object_creation() {
    let html = r#"<!DOCTYPE html>
<html><body>
<iframe name="otherRealm" id="f1" src="empty.html"></iframe>
</body></html>"#;

    let mut iframes = HashMap::new();
    iframes.insert("empty.html".to_string(), IframeResource { content: "<html><body></body></html>".to_string(), content_type: "text/html".into() });
    let mut engine = engine_with_iframes(html, iframes);
    engine.settle();

    // Create object using iframe's constructor
    let result = engine.eval_js("typeof otherRealm.Object").unwrap();
    eprintln!("otherRealm.Object type: {}", result);
    assert_eq!(result, "function", "iframe window should have Object constructor");

    let obj = engine.eval_js("var o = new otherRealm.Object(); o.x = 42; o.x").unwrap();
    assert_eq!(obj, "42", "cross-realm object should work");
}

#[test]
fn document_capture_listeners_fire() {
    let mut engine = Engine::new();
    engine.load_html(r#"<!DOCTYPE html>
<html><body>
<script>
    var captureOrder = [];
    document.addEventListener('test', function() { captureOrder.push('capture'); }, true);
    document.addEventListener('test', function() { captureOrder.push('bubble'); }, false);
</script>
</body></html>"#);
    engine.settle();

    engine.eval_js("document.dispatchEvent(new Event('test', {bubbles: true}))").unwrap();
    engine.settle();

    let order = engine.eval_js("captureOrder.join(',')").unwrap();
    eprintln!("listener order: {}", order);
    assert_eq!(order, "capture,bubble", "capture listeners should fire before bubble on document.dispatchEvent");
}
