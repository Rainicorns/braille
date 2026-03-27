//! DOM bridge core tests: polyfills, DOM operations, event system.
use braille_engine::Engine;
use braille_wire::SnapMode;

fn engine_with_html(html: &str) -> Engine {
    let mut e = Engine::new();
    e.load_html(html);
    e
}

// =========================================================================
// Tier 1: Polyfill collisions — our stubs must not break core-js
// =========================================================================

#[test]
fn urlsearchparams_polyfill_pattern() {
    // core-js does: uncurryThis(URLSearchParams.prototype.delete)
    let mut e = engine_with_html("<html><body></body></html>");
    let result = e.eval_js(
        "typeof URLSearchParams.prototype.delete === 'function' ? 'ok' : 'broken'"
    );
    assert_eq!(result.unwrap(), "ok");
}

#[test]
fn urlsearchparams_delete_two_args() {
    // Spec: delete(name, value) removes only entries matching both
    let mut e = engine_with_html("<html><body></body></html>");
    let result = e.eval_js(
        "var m = new URLSearchParams('a=1&a=2&b=3'); m.delete('a', '1'); m.toString()"
    );
    assert_eq!(result.unwrap(), "a=2&b=3");
}

#[test]
fn urlsearchparams_delete_one_arg() {
    let mut e = engine_with_html("<html><body></body></html>");
    let result = e.eval_js(
        "var m = new URLSearchParams('a=1&a=2&b=3'); m.delete('a'); m.toString()"
    );
    assert_eq!(result.unwrap(), "b=3");
}

#[test]
fn urlsearchparams_size() {
    let mut e = engine_with_html("<html><body></body></html>");
    let result = e.eval_js("new URLSearchParams('a=1&b=2&c=3').size");
    assert_eq!(result.unwrap(), "3");
}

#[test]
fn event_constructor_works() {
    let mut e = engine_with_html("<html><body></body></html>");
    let result = e.eval_js("new Event('click').type");
    assert_eq!(result.unwrap(), "click");
}

#[test]
fn custom_event_constructor_works() {
    let mut e = engine_with_html("<html><body></body></html>");
    let result = e.eval_js("new CustomEvent('foo', {detail: 42}).detail");
    assert_eq!(result.unwrap(), "42");
}

// =========================================================================
// Tier 2: DOM bridge — createElement, appendChild, etc. modify real DomTree
// =========================================================================

#[test]
fn create_element_and_append() {
    let mut e = engine_with_html("<html><body></body></html>");
    e.eval_js("var d = document.createElement('div'); d.textContent = 'hello'; document.body.appendChild(d);").unwrap();
    let snap = e.snapshot(SnapMode::Text);
    assert!(snap.contains("hello"), "snapshot should contain created element text, got: {snap}");
}

#[test]
fn create_text_node_and_append() {
    let mut e = engine_with_html("<html><body></body></html>");
    e.eval_js("document.body.appendChild(document.createTextNode('world'));").unwrap();
    let snap = e.snapshot(SnapMode::Text);
    assert!(snap.contains("world"), "snapshot should contain text node, got: {snap}");
}

#[test]
fn set_attribute_reflects_in_snapshot() {
    let mut e = engine_with_html("<html><body><input id='x'></body></html>");
    e.eval_js("document.getElementById('x').setAttribute('value', 'typed');").unwrap();
    let snap = e.snapshot(SnapMode::Compact);
    assert!(snap.contains("typed"), "snapshot should show input value, got: {snap}");
}

#[test]
fn get_element_by_id_returns_real_element() {
    let mut e = engine_with_html("<html><body><div id='target'>found</div></body></html>");
    let result = e.eval_js("var el = document.getElementById('target'); el ? el.textContent : 'null'");
    assert_eq!(result.unwrap(), "found");
}

#[test]
fn query_selector_works() {
    let mut e = engine_with_html("<html><body><p class='intro'>hi</p></body></html>");
    let result = e.eval_js("var el = document.querySelector('.intro'); el ? el.textContent : 'null'");
    assert_eq!(result.unwrap(), "hi");
}

#[test]
fn create_comment_in_dom() {
    let mut e = engine_with_html("<html><body></body></html>");
    // createComment should not crash and should create a real node
    let result = e.eval_js("var c = document.createComment('marker'); typeof c");
    assert_eq!(result.unwrap(), "object");
}

#[test]
fn node_contains() {
    let mut e = engine_with_html("<html><body><div id='outer'><span id='inner'>x</span></div></body></html>");
    let result = e.eval_js(
        "var outer = document.getElementById('outer'); var inner = document.getElementById('inner'); outer.contains(inner)"
    );
    assert_eq!(result.unwrap(), "true");
}

#[test]
fn node_contains_self() {
    let mut e = engine_with_html("<html><body><div id='el'>x</div></body></html>");
    let result = e.eval_js("var el = document.getElementById('el'); el.contains(el)");
    assert_eq!(result.unwrap(), "true");
}

#[test]
fn node_contains_false() {
    let mut e = engine_with_html("<html><body><div id='a'>a</div><div id='b'>b</div></body></html>");
    let result = e.eval_js(
        "var a = document.getElementById('a'); var b = document.getElementById('b'); a.contains(b)"
    );
    assert_eq!(result.unwrap(), "false");
}

#[test]
fn element_closest() {
    let mut e = engine_with_html("<html><body><div class='wrap'><span id='inner'>x</span></div></body></html>");
    let result = e.eval_js(
        "var inner = document.getElementById('inner'); var wrap = inner.closest('.wrap'); wrap ? wrap.tagName : 'null'"
    );
    assert_eq!(result.unwrap(), "DIV");
}

#[test]
fn element_closest_no_match() {
    let mut e = engine_with_html("<html><body><span id='el'>x</span></body></html>");
    let result = e.eval_js("document.getElementById('el').closest('.nonexistent') === null");
    assert_eq!(result.unwrap(), "true");
}

#[test]
fn remove_child_from_dom() {
    let mut e = engine_with_html("<html><body><div id='parent'><span id='child'>gone</span></div></body></html>");
    e.eval_js("var p = document.getElementById('parent'); var c = document.getElementById('child'); p.removeChild(c);").unwrap();
    let snap = e.snapshot(SnapMode::Text);
    assert!(!snap.contains("gone"), "removed child should not appear in snapshot, got: {snap}");
}

#[test]
fn innerhtml_setter() {
    let mut e = engine_with_html("<html><body><div id='target'></div></body></html>");
    e.eval_js("document.getElementById('target').innerHTML = '<b>bold</b>';").unwrap();
    let snap = e.snapshot(SnapMode::Text);
    assert!(snap.contains("bold"), "innerHTML should render in snapshot, got: {snap}");
}

#[test]
fn dataset_read() {
    let mut e = engine_with_html("<html><body><div id='el' data-foo='bar'></div></body></html>");
    let result = e.eval_js("document.getElementById('el').dataset.foo");
    assert_eq!(result.unwrap(), "bar");
}

// =========================================================================
// Tier 3: Event system
// =========================================================================

#[test]
fn add_event_listener_and_dispatch() {
    let mut e = engine_with_html("<html><body><button id='btn'>click me</button></body></html>");
    e.eval_js("var clicked = false; document.getElementById('btn').addEventListener('click', function() { clicked = true; });").unwrap();
    e.eval_js("document.getElementById('btn').click();").unwrap();
    let result = e.eval_js("clicked");
    assert_eq!(result.unwrap(), "true");
}

#[test]
fn event_bubbles_to_parent() {
    let mut e = engine_with_html("<html><body><div id='parent'><button id='btn'>x</button></div></body></html>");
    e.eval_js("var heard = false; document.getElementById('parent').addEventListener('click', function() { heard = true; });").unwrap();
    e.eval_js("document.getElementById('btn').click();").unwrap();
    let result = e.eval_js("heard");
    assert_eq!(result.unwrap(), "true");
}

#[test]
fn event_bubbles_to_document() {
    let mut e = engine_with_html("<html><body><button id='btn'>x</button></body></html>");
    e.eval_js("var docHeard = false; document.addEventListener('click', function() { docHeard = true; });").unwrap();
    e.eval_js("document.getElementById('btn').click();").unwrap();
    let result = e.eval_js("docHeard");
    assert_eq!(result.unwrap(), "true");
}

#[test]
fn window_add_event_listener() {
    let mut e = engine_with_html("<html><body></body></html>");
    e.eval_js("var winHeard = false; window.addEventListener('click', function() { winHeard = true; });").unwrap();
    // Dispatch a click on body — should bubble to window
    e.eval_js("document.body.click();").unwrap();
    let result = e.eval_js("winHeard");
    assert_eq!(result.unwrap(), "true");
}

#[test]
fn stop_propagation_prevents_bubbling() {
    let mut e = engine_with_html("<html><body><div id='parent'><button id='btn'>x</button></div></body></html>");
    e.eval_js("var parentHeard = false; document.getElementById('parent').addEventListener('click', function() { parentHeard = true; });").unwrap();
    e.eval_js("document.getElementById('btn').addEventListener('click', function(e) { e.stopPropagation(); });").unwrap();
    e.eval_js("document.getElementById('btn').click();").unwrap();
    let result = e.eval_js("parentHeard");
    assert_eq!(result.unwrap(), "false");
}

#[test]
fn handle_type_fires_input_event() {
    let mut e = engine_with_html("<html><body><input id='name' type='text'></body></html>");
    e.snapshot(SnapMode::Compact); // populate refs
    e.eval_js("var inputFired = false; document.getElementById('name').addEventListener('input', function() { inputFired = true; });").unwrap();
    let _ = e.handle_type("#name", "alice");
    let result = e.eval_js("inputFired");
    assert_eq!(result.unwrap(), "true");
}

