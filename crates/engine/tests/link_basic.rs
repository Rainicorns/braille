use std::collections::HashMap;
use braille_engine::{Engine, FetchedResources};

#[test]
fn link_element_tagname() {
    let html = r#"<!DOCTYPE html><html><head></head><body>
<div id="out"></div>
<script src="/t.js"></script>
</body></html>"#;

    let js = r#"
var link = document.createElement('link');
document.getElementById('out').textContent =
    'tagName=' + link.tagName +
    ' nodeType=' + link.nodeType +
    ' nid=' + link.__nid;
"#;

    let mut engine = Engine::new();
    let mut scripts = HashMap::new();
    scripts.insert("/t.js".to_string(), js.to_string());
    let d = engine.parse_and_collect_scripts(html);
    let e = engine.execute_scripts_lossy(&d, &FetchedResources::scripts_only(scripts));
    assert!(e.is_empty(), "{e:?}");

    let r = engine.eval_js("document.getElementById('out').textContent").unwrap();
    assert!(r.contains("tagName=LINK"), "got: {r}");
}

#[test]
fn link_appendchild_triggers_maybe_load() {
    let html = r#"<!DOCTYPE html><html><head></head><body>
<div id="out">before</div>
<script src="/t.js"></script>
</body></html>"#;

    let js = r#"
window.__linkCalls = [];
window.__linkFired = false;
var origFn = __braille_maybe_load_link;
__braille_maybe_load_link = function(node) {
    window.__linkCalls.push(node ? (node.tagName + ':' + node.getAttribute('rel')) : 'null');
    origFn(node);
};

var link = document.createElement('link');
link.setAttribute('rel', 'stylesheet');
link.setAttribute('href', '/test.css');
link.onload = function() {
    window.__linkFired = true;
};
document.head.appendChild(link);
"#;

    let mut engine = Engine::new();
    let mut scripts = HashMap::new();
    scripts.insert("/t.js".to_string(), js.to_string());
    let d = engine.parse_and_collect_scripts(html);
    let e = engine.execute_scripts_lossy(&d, &FetchedResources::scripts_only(scripts));
    assert!(e.is_empty(), "{e:?}");

    // Verify __braille_maybe_load_link was called with the LINK element
    let calls = engine.eval_js("window.__linkCalls.join(',')").unwrap();
    assert!(calls.contains("LINK:stylesheet"), "should call __braille_maybe_load_link with LINK, got: {calls}");

    // onload fires via setTimeout(0) — may have already fired during execute_scripts_lossy's
    // load cycle, or will fire on settle. Either way it must be true after settle.
    engine.settle();
    let fired = engine.eval_js("window.__linkFired").unwrap();
    assert_eq!(fired, "true", "onload should have fired");
}
