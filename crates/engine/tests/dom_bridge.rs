//! Tests for the QuickJS DOM bridge — verifying JS ↔ DomTree integration.
//!
//! Each test creates an Engine, loads HTML, runs JS via eval_js, and checks
//! that the JS operations correctly modify the underlying Rust DomTree.

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

// =========================================================================
// Tier 4: DOM bridge completeness — React reconciler requirements
// =========================================================================

#[test]
fn childnodes_includes_text_nodes() {
    let mut e = engine_with_html("<html><body><div id='d'>hello<span>world</span></div></body></html>");
    let result = e.eval_js(
        "var d = document.getElementById('d'); d.childNodes.length"
    );
    assert_eq!(result.unwrap(), "2", "childNodes should include text + element");
    let result = e.eval_js("document.getElementById('d').childNodes[0].nodeType");
    assert_eq!(result.unwrap(), "3", "first childNode should be text (nodeType 3)");
}

#[test]
fn firstchild_lastchild() {
    let mut e = engine_with_html("<html><body><div id='d'><span>a</span><b>b</b></div></body></html>");
    let result = e.eval_js("document.getElementById('d').firstChild.tagName");
    assert_eq!(result.unwrap(), "SPAN");
    let result = e.eval_js("document.getElementById('d').lastChild.tagName");
    assert_eq!(result.unwrap(), "B");
}

#[test]
fn firstchild_lastchild_null() {
    let mut e = engine_with_html("<html><body><div id='d'></div></body></html>");
    let result = e.eval_js("document.getElementById('d').firstChild === null");
    assert_eq!(result.unwrap(), "true");
    let result = e.eval_js("document.getElementById('d').lastChild === null");
    assert_eq!(result.unwrap(), "true");
}

#[test]
fn nextsibling_previoussibling() {
    let mut e = engine_with_html("<html><body><div id='p'><span id='a'>a</span><b id='b'>b</b><i id='c'>c</i></div></body></html>");
    let result = e.eval_js("document.getElementById('a').nextSibling.tagName");
    assert_eq!(result.unwrap(), "B");
    let result = e.eval_js("document.getElementById('c').previousSibling.tagName");
    assert_eq!(result.unwrap(), "B");
    let result = e.eval_js("document.getElementById('a').previousSibling === null");
    assert_eq!(result.unwrap(), "true");
    let result = e.eval_js("document.getElementById('c').nextSibling === null");
    assert_eq!(result.unwrap(), "true");
}

#[test]
fn text_node_data_property() {
    let mut e = engine_with_html("<html><body><div id='d'>hello</div></body></html>");
    let result = e.eval_js("document.getElementById('d').firstChild.data");
    assert_eq!(result.unwrap(), "hello");
    // set data
    e.eval_js("document.getElementById('d').firstChild.data = 'world';").unwrap();
    let result = e.eval_js("document.getElementById('d').firstChild.data");
    assert_eq!(result.unwrap(), "world");
}

#[test]
fn nodevalue_for_text_and_comment() {
    let mut e = engine_with_html("<html><body><div id='d'>text</div></body></html>");
    // text node nodeValue
    let result = e.eval_js("document.getElementById('d').firstChild.nodeValue");
    assert_eq!(result.unwrap(), "text");
    // element nodeValue is null
    let result = e.eval_js("document.getElementById('d').nodeValue === null");
    assert_eq!(result.unwrap(), "true");
    // comment nodeValue
    let result = e.eval_js("document.createComment('hi').nodeValue");
    assert_eq!(result.unwrap(), "hi");
}

#[test]
fn clone_node_shallow() {
    let mut e = engine_with_html("<html><body><div id='d' class='x'><span>child</span></div></body></html>");
    let result = e.eval_js(
        "var orig = document.getElementById('d'); var cl = orig.cloneNode(false); cl.tagName + '|' + cl.getAttribute('class') + '|' + cl.childNodes.length"
    );
    assert_eq!(result.unwrap(), "DIV|x|0");
}

#[test]
fn clone_node_deep() {
    let mut e = engine_with_html("<html><body><div id='d'><span>child</span></div></body></html>");
    let result = e.eval_js(
        "var cl = document.getElementById('d').cloneNode(true); cl.childNodes.length + '|' + cl.firstChild.tagName"
    );
    assert_eq!(result.unwrap(), "1|SPAN");
}

#[test]
fn replace_child() {
    let mut e = engine_with_html("<html><body><div id='p'><span id='old'>old</span></div></body></html>");
    e.eval_js("var p = document.getElementById('p'); var n = document.createElement('b'); n.textContent = 'new'; p.replaceChild(n, document.getElementById('old'));").unwrap();
    let snap = e.snapshot(SnapMode::Text);
    assert!(snap.contains("new"), "replaced child should appear: {snap}");
    assert!(!snap.contains("old"), "old child should be gone: {snap}");
}

#[test]
fn document_fragment_transfers_children() {
    let mut e = engine_with_html("<html><body><div id='target'></div></body></html>");
    e.eval_js(r#"
        var frag = document.createDocumentFragment();
        var a = document.createElement('span'); a.textContent = 'aaa';
        var b = document.createElement('span'); b.textContent = 'bbb';
        frag.appendChild(a);
        frag.appendChild(b);
        document.getElementById('target').appendChild(frag);
    "#).unwrap();
    let snap = e.snapshot(SnapMode::Text);
    assert!(snap.contains("aaa"), "fragment child a should appear: {snap}");
    assert!(snap.contains("bbb"), "fragment child b should appear: {snap}");
    // Fragment should now be empty
    let result = e.eval_js("frag.childNodes.length");
    assert_eq!(result.unwrap(), "0");
}

#[test]
fn innerhtml_getter() {
    let mut e = engine_with_html("<html><body><div id='d'><b>bold</b> text</div></body></html>");
    let result = e.eval_js("document.getElementById('d').innerHTML");
    let html = result.unwrap();
    assert!(html.contains("<b>bold</b>"), "innerHTML should contain <b>bold</b>, got: {html}");
    assert!(html.contains("text"), "innerHTML should contain text, got: {html}");
}

#[test]
fn matches_selector() {
    let mut e = engine_with_html("<html><body><div id='d' class='foo bar'></div></body></html>");
    let result = e.eval_js("document.getElementById('d').matches('.foo')");
    assert_eq!(result.unwrap(), "true");
    let result = e.eval_js("document.getElementById('d').matches('.baz')");
    assert_eq!(result.unwrap(), "false");
    let result = e.eval_js("document.getElementById('d').matches('div.bar')");
    assert_eq!(result.unwrap(), "true");
}

#[test]
fn has_child_nodes() {
    let mut e = engine_with_html("<html><body><div id='full'><span>x</span></div><div id='empty'></div></body></html>");
    let result = e.eval_js("document.getElementById('full').hasChildNodes()");
    assert_eq!(result.unwrap(), "true");
    let result = e.eval_js("document.getElementById('empty').hasChildNodes()");
    assert_eq!(result.unwrap(), "false");
}
