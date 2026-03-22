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
var origFn = __braille_maybe_load_link;
var calls = [];
__braille_maybe_load_link = function(node) {
    calls.push(node ? (node.tagName + ':' + node.getAttribute('rel')) : 'null');
    origFn(node);
};

var link = document.createElement('link');
link.setAttribute('rel', 'stylesheet');
link.setAttribute('href', '/test.css');
link.onload = function() {
    document.getElementById('out').textContent = 'fired';
};
document.head.appendChild(link);

document.getElementById('out').textContent = 'calls=' + calls.join(',');
"#;

    let mut engine = Engine::new();
    let mut scripts = HashMap::new();
    scripts.insert("/t.js".to_string(), js.to_string());
    let d = engine.parse_and_collect_scripts(html);
    let e = engine.execute_scripts_lossy(&d, &FetchedResources::scripts_only(scripts));
    assert!(e.is_empty(), "{e:?}");

    let r = engine.eval_js("document.getElementById('out').textContent").unwrap();
    assert!(r.contains("LINK:stylesheet"), "should call __braille_maybe_load_link with LINK, got: {r}");

    engine.settle();

    let r2 = engine.eval_js("document.getElementById('out').textContent").unwrap();
    assert_eq!(r2, "fired", "onload should fire after settle, got: {r2}");
}
