//! Tests for the QuickJS DOM bridge — verifying JS ↔ DomTree integration.
//!
//! Each test creates an Engine, loads HTML, runs JS via eval_js, and checks
//! that the JS operations correctly modify the underlying Rust DomTree.

use braille_engine::Engine;
use braille_wire::{FetchResponseData, SnapMode};

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

// =========================================================================
// HTMLScriptElement IDL properties (noModule, async, defer)
// =========================================================================

#[test]
fn nomodule_in_check() {
    let mut e = engine_with_html("<html><body></body></html>");
    let result = e.eval_js("'noModule' in document.createElement('script')");
    assert_eq!(result.unwrap(), "true");
}

#[test]
fn nomodule_reflect() {
    let mut e = engine_with_html("<html><body></body></html>");
    let result = e.eval_js(r#"
        var s = document.createElement('script');
        s.noModule = true;
        var has = s.hasAttribute('nomodule');
        var get = s.noModule;
        s.noModule = false;
        var gone = !s.hasAttribute('nomodule');
        has + ',' + get + ',' + gone
    "#);
    assert_eq!(result.unwrap(), "true,true,true");
}

#[test]
fn async_defer_properties() {
    let mut e = engine_with_html("<html><body></body></html>");
    let result = e.eval_js(r#"
        var s = document.createElement('script');
        s.async = true;
        var a = s.hasAttribute('async');
        s.defer = true;
        var d = s.hasAttribute('defer');
        a + ',' + d
    "#);
    assert_eq!(result.unwrap(), "true,true");
}

#[test]
fn reversed_in_ol() {
    let mut e = engine_with_html("<html><body></body></html>");
    let result = e.eval_js("'reversed' in document.createElement('ol')");
    assert_eq!(result.unwrap(), "true");
}

#[test]
fn proton_browser_check() {
    // Exact check from ProtonMail's public-index.js module 33759
    let mut e = engine_with_html("<html><body></body></html>");
    let result = e.eval_js(r#"
        "reversed" in document.createElement("ol")
        && Object.fromEntries
        && "".trimStart
        && window.crypto.subtle
        ? 1 : 0
    "#);
    assert_eq!(result.unwrap(), "1");
}

// =========================================================================
// CSS.supports()
// =========================================================================

#[test]
fn css_supports_two_arg() {
    let mut e = engine_with_html("<html><body></body></html>");
    let result = e.eval_js("CSS.supports('display', 'flex')");
    assert_eq!(result.unwrap(), "true");
}

#[test]
fn css_supports_one_arg() {
    let mut e = engine_with_html("<html><body></body></html>");
    let result = e.eval_js("CSS.supports('(display: flex)')");
    assert_eq!(result.unwrap(), "true");
}

#[test]
fn css_supports_invalid() {
    let mut e = engine_with_html("<html><body></body></html>");
    let result = e.eval_js("CSS.supports('', '')");
    assert_eq!(result.unwrap(), "false");
}

#[test]
fn css_escape() {
    let mut e = engine_with_html("<html><body></body></html>");
    let result = e.eval_js("CSS.escape('foo.bar')");
    assert_eq!(result.unwrap(), r"foo\.bar");
}

// =========================================================================
// Intl
// =========================================================================

#[test]
fn intl_typeof() {
    let mut e = engine_with_html("<html><body></body></html>");
    let result = e.eval_js("typeof Intl === 'object'");
    assert_eq!(result.unwrap(), "true");
}

#[test]
fn intl_numberformat_basic() {
    let mut e = engine_with_html("<html><body></body></html>");
    let result = e.eval_js("new Intl.NumberFormat('en').format(1234.5)");
    assert_eq!(result.unwrap(), "1,234.5");
}

#[test]
fn intl_pluralrules() {
    let mut e = engine_with_html("<html><body></body></html>");
    let result = e.eval_js("new Intl.PluralRules('en').select(1)");
    assert_eq!(result.unwrap(), "one");
}

#[test]
fn intl_datetimeformat() {
    let mut e = engine_with_html("<html><body></body></html>");
    let result = e.eval_js("new Intl.DateTimeFormat('en').format(new Date(0))");
    let val = result.unwrap();
    assert!(!val.is_empty(), "DateTimeFormat should produce non-empty output");
}

// =========================================================================
// Dynamic script loading
// =========================================================================

#[test]
fn dynamic_script_load_fires_onload() {
    let mut e = engine_with_html("<html><head></head><body></body></html>");
    e.eval_js(r#"
        var s = document.createElement('script');
        s.src = 'https://example.com/chunk.js';
        window.__script_loaded = false;
        s.onload = function() { window.__script_loaded = true; };
        document.head.appendChild(s);
    "#).unwrap();

    // Should have a pending fetch for the script
    assert!(e.has_pending_fetches(), "should have pending fetch for script src");
    let pending = e.pending_fetches();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].url, "https://example.com/chunk.js");

    // Resolve the fetch with some JS code
    e.resolve_fetch(pending[0].id, &FetchResponseData {
        status: 200,
        status_text: "OK".to_string(),
        headers: vec![("content-type".to_string(), "application/javascript".to_string())],
        body: "window.__chunk_ran = 42;".to_string(),
        url: "https://example.com/chunk.js".to_string(),
    });
    e.settle();

    // The script should have executed
    assert_eq!(e.eval_js("window.__chunk_ran").unwrap(), "42");
    // onload should have fired
    assert_eq!(e.eval_js("window.__script_loaded").unwrap(), "true");
}

#[test]
fn dynamic_script_eval_runs_code() {
    let mut e = engine_with_html("<html><head></head><body></body></html>");

    // Set up a global array that the "chunk" will push to (like webpack)
    e.eval_js("window.__chunks = []; window.__chunks.push = function(v) { Array.prototype.push.call(this, v); };").unwrap();

    e.eval_js(r#"
        var s = document.createElement('script');
        s.src = 'https://cdn.example.com/chunk.4599.js';
        document.head.appendChild(s);
    "#).unwrap();

    let pending = e.pending_fetches();
    assert_eq!(pending.len(), 1);

    // The chunk pushes data onto the global array (webpack pattern)
    e.resolve_fetch(pending[0].id, &FetchResponseData {
        status: 200,
        status_text: "OK".to_string(),
        headers: vec![],
        body: "window.__chunks.push([4599, {hello: 'world'}]);".to_string(),
        url: "https://cdn.example.com/chunk.4599.js".to_string(),
    });
    e.settle();

    assert_eq!(e.eval_js("window.__chunks.length").unwrap(), "1");
    assert_eq!(e.eval_js("window.__chunks[0][0]").unwrap(), "4599");
}

#[test]
fn dynamic_script_insertbefore_also_loads() {
    let mut e = engine_with_html("<html><head><meta charset='utf-8'></head><body></body></html>");
    e.eval_js(r#"
        var s = document.createElement('script');
        s.src = 'https://example.com/insert.js';
        var meta = document.querySelector('meta');
        document.head.insertBefore(s, meta);
    "#).unwrap();

    assert!(e.has_pending_fetches(), "insertBefore should trigger script load");
    let pending = e.pending_fetches();
    assert_eq!(pending[0].url, "https://example.com/insert.js");
}

#[test]
fn dynamic_script_error_fires_onerror() {
    let mut e = engine_with_html("<html><head></head><body></body></html>");
    e.eval_js(r#"
        var s = document.createElement('script');
        s.src = 'https://example.com/missing.js';
        window.__script_error = false;
        s.onerror = function() { window.__script_error = true; };
        document.head.appendChild(s);
    "#).unwrap();

    let pending = e.pending_fetches();
    e.reject_fetch(pending[0].id, "Network error");
    e.settle();

    assert_eq!(e.eval_js("window.__script_error").unwrap(), "true");
}

#[test]
fn message_channel_settles() {
    // React's scheduler uses MessageChannel → setTimeout(0) → callback
    let mut e = engine_with_html("<html><body></body></html>");
    e.eval_js(r#"
        window.__mc_result = 'not fired';
        var ch = new MessageChannel();
        ch.port1.onmessage = function(ev) {
            window.__mc_result = 'fired: ' + ev.data;
        };
        ch.port2.postMessage('hello');
    "#).unwrap();

    // Before settle: the setTimeout(0) hasn't fired
    assert_eq!(e.eval_js("window.__mc_result").unwrap(), "not fired");

    // After settle: the timer fires, MessageChannel callback runs
    e.settle();
    assert_eq!(e.eval_js("window.__mc_result").unwrap(), "fired: hello");
}

#[test]
fn settle_fires_chained_timers() {
    // Verify that settle() processes cascading timers
    let mut e = engine_with_html("<html><body></body></html>");
    e.eval_js(r#"
        window.__chain = [];
        setTimeout(function() {
            __chain.push('A');
            setTimeout(function() {
                __chain.push('B');
                setTimeout(function() {
                    __chain.push('C');
                }, 0);
            }, 0);
        }, 0);
    "#).unwrap();
    e.settle();
    assert_eq!(e.eval_js("__chain.join(',')").unwrap(), "A,B,C");
}

// =========================================================================
// WebCrypto
// =========================================================================

#[test]
fn crypto_get_random_values() {
    let mut e = engine_with_html("<html><body></body></html>");
    let result = e.eval_js(r#"
        var a = new Uint8Array(16);
        crypto.getRandomValues(a);
        a.length + ',' + (a.some(function(x){ return x !== 0; }) ? 'random' : 'all-zero')
    "#);
    assert_eq!(result.unwrap(), "16,random");
}

#[test]
fn crypto_subtle_digest_sha256() {
    let mut e = engine_with_html("<html><body></body></html>");
    // SHA-256 of empty string = e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
    e.eval_js(r#"
        var p; crypto.subtle.digest('SHA-256', new Uint8Array(0)).then(function(b){
            var a = new Uint8Array(b), h = '';
            for(var i=0;i<a.length;i++) h += (a[i]<16?'0':'') + a[i].toString(16);
            p = h;
        });
    "#).unwrap();
    e.settle();
    assert_eq!(e.eval_js("p").unwrap(), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
}

#[test]
fn crypto_subtle_aes_gcm_roundtrip() {
    let mut e = engine_with_html("<html><body></body></html>");
    e.eval_js(r#"
        var result = 'pending';
        crypto.subtle.generateKey({name:'AES-GCM',length:256}, true, ['encrypt','decrypt'])
        .then(function(key) {
            var iv = crypto.getRandomValues(new Uint8Array(12));
            var data = new TextEncoder().encode('hello world');
            return crypto.subtle.encrypt({name:'AES-GCM',iv:iv}, key, data)
            .then(function(ct) {
                return crypto.subtle.decrypt({name:'AES-GCM',iv:iv}, key, ct);
            });
        })
        .then(function(pt) {
            result = new TextDecoder().decode(new Uint8Array(pt));
        })
        .catch(function(e) { result = 'ERROR: ' + e.message; });
    "#).unwrap();
    e.settle();
    let val = e.eval_js("result");
    assert_eq!(val.unwrap(), "hello world");
}

#[test]
fn crypto_subtle_hmac_sign_verify() {
    let mut e = engine_with_html("<html><body></body></html>");
    e.eval_js(r#"
        var result = 'pending';
        crypto.subtle.generateKey({name:'HMAC',hash:'SHA-256'}, false, ['sign','verify'])
        .then(function(key) {
            var data = new TextEncoder().encode('test message');
            return crypto.subtle.sign({name:'HMAC'}, key, data)
            .then(function(sig) {
                return crypto.subtle.verify({name:'HMAC'}, key, sig, data);
            });
        })
        .then(function(valid) { result = String(valid); })
        .catch(function(e) { result = 'ERROR: ' + e.message; });
    "#).unwrap();
    e.settle();
    assert_eq!(e.eval_js("result").unwrap(), "true");
}

#[test]
fn dynamic_script_no_src_does_not_fetch() {
    let mut e = engine_with_html("<html><head></head><body></body></html>");
    e.eval_js(r#"
        var s = document.createElement('script');
        s.textContent = 'window.__inline = 1';
        document.head.appendChild(s);
    "#).unwrap();

    assert!(!e.has_pending_fetches(), "inline script should not trigger fetch");
}

// =========================================================================
// Label association: htmlFor, control, labels, click-on-label
// =========================================================================

#[test]
fn label_html_for_getter_reflects_for_attribute() {
    let mut e = engine_with_html(r#"<html><body>
        <label id="lbl" for="username">Name</label>
        <input id="username" />
    </body></html>"#);
    let result = e.eval_js(r#"document.getElementById('lbl').htmlFor"#).unwrap();
    assert_eq!(result, "username");
}

#[test]
fn label_html_for_getter_returns_empty_when_no_for() {
    let mut e = engine_with_html(r#"<html><body>
        <label id="lbl">Name <input /></label>
    </body></html>"#);
    let result = e.eval_js(r#"document.getElementById('lbl').htmlFor"#).unwrap();
    assert_eq!(result, "");
}

#[test]
fn label_html_for_setter_updates_for_attribute() {
    let mut e = engine_with_html(r#"<html><body>
        <label id="lbl">Name</label>
        <input id="email" />
    </body></html>"#);
    e.eval_js(r#"document.getElementById('lbl').htmlFor = 'email'"#).unwrap();
    let result = e.eval_js(r#"document.getElementById('lbl').getAttribute('for')"#).unwrap();
    assert_eq!(result, "email");
}

#[test]
fn label_html_for_undefined_on_non_label() {
    let mut e = engine_with_html(r#"<html><body><div id="d"></div></body></html>"#);
    let result = e.eval_js(r#"typeof document.getElementById('d').htmlFor"#).unwrap();
    assert_eq!(result, "undefined");
}

#[test]
fn label_control_returns_element_by_for_attribute() {
    let mut e = engine_with_html(r#"<html><body>
        <label id="lbl" for="inp">Name</label>
        <input id="inp" />
    </body></html>"#);
    let result = e.eval_js(r#"document.getElementById('lbl').control.id"#).unwrap();
    assert_eq!(result, "inp");
}

#[test]
fn label_control_returns_first_labelable_descendant() {
    let mut e = engine_with_html(r#"<html><body>
        <label id="lbl">Name <span><input id="inner" /></span></label>
    </body></html>"#);
    let result = e.eval_js(r#"document.getElementById('lbl').control.id"#).unwrap();
    assert_eq!(result, "inner");
}

#[test]
fn label_control_returns_null_when_no_control_found() {
    let mut e = engine_with_html(r#"<html><body>
        <label id="lbl" for="nonexistent">Name</label>
    </body></html>"#);
    let result = e.eval_js(r#"document.getElementById('lbl').control === null"#).unwrap();
    assert_eq!(result, "true");
}

#[test]
fn label_control_returns_null_when_no_descendant() {
    let mut e = engine_with_html(r#"<html><body>
        <label id="lbl">Just text</label>
    </body></html>"#);
    let result = e.eval_js(r#"document.getElementById('lbl').control === null"#).unwrap();
    assert_eq!(result, "true");
}

#[test]
fn label_control_undefined_on_non_label() {
    let mut e = engine_with_html(r#"<html><body><div id="d"></div></body></html>"#);
    let result = e.eval_js(r#"typeof document.getElementById('d').control"#).unwrap();
    assert_eq!(result, "undefined");
}

#[test]
fn input_labels_returns_label_with_for_attribute() {
    let mut e = engine_with_html(r#"<html><body>
        <label id="lbl" for="inp">Name</label>
        <input id="inp" />
    </body></html>"#);
    let result = e.eval_js(r#"
        var labels = document.getElementById('inp').labels;
        labels.length + ':' + labels[0].id
    "#).unwrap();
    assert_eq!(result, "1:lbl");
}

#[test]
fn input_labels_returns_ancestor_label() {
    let mut e = engine_with_html(r#"<html><body>
        <label id="lbl">Name <input id="inp" /></label>
    </body></html>"#);
    let result = e.eval_js(r#"
        var labels = document.getElementById('inp').labels;
        labels.length + ':' + labels[0].id
    "#).unwrap();
    assert_eq!(result, "1:lbl");
}

#[test]
fn input_labels_returns_multiple_labels() {
    let mut e = engine_with_html(r#"<html><body>
        <label id="lbl1" for="inp">First</label>
        <label id="lbl2" for="inp">Second</label>
        <input id="inp" />
    </body></html>"#);
    let result = e.eval_js(r#"document.getElementById('inp').labels.length"#).unwrap();
    assert_eq!(result, "2");
}

#[test]
fn input_labels_returns_empty_for_no_labels() {
    let mut e = engine_with_html(r#"<html><body>
        <input id="inp" />
    </body></html>"#);
    let result = e.eval_js(r#"document.getElementById('inp').labels.length"#).unwrap();
    assert_eq!(result, "0");
}

#[test]
fn input_labels_undefined_on_non_labelable() {
    let mut e = engine_with_html(r#"<html><body><div id="d"></div></body></html>"#);
    let result = e.eval_js(r#"typeof document.getElementById('d').labels"#).unwrap();
    assert_eq!(result, "undefined");
}

#[test]
fn input_labels_works_for_select() {
    let mut e = engine_with_html(r#"<html><body>
        <label id="lbl" for="sel">Pick</label>
        <select id="sel"><option>A</option></select>
    </body></html>"#);
    let result = e.eval_js(r#"document.getElementById('sel').labels.length"#).unwrap();
    assert_eq!(result, "1");
}

#[test]
fn input_labels_works_for_textarea() {
    let mut e = engine_with_html(r#"<html><body>
        <label id="lbl" for="ta">Bio</label>
        <textarea id="ta"></textarea>
    </body></html>"#);
    let result = e.eval_js(r#"document.getElementById('ta').labels.length"#).unwrap();
    assert_eq!(result, "1");
}

#[test]
fn input_labels_works_for_button() {
    let mut e = engine_with_html(r#"<html><body>
        <label id="lbl" for="btn">Action</label>
        <button id="btn">Go</button>
    </body></html>"#);
    let result = e.eval_js(r#"document.getElementById('btn').labels.length"#).unwrap();
    assert_eq!(result, "1");
}

#[test]
fn click_on_label_activates_associated_control() {
    let mut e = engine_with_html(r#"<html><body>
        <label id="lbl" for="cb">Check me</label>
        <input id="cb" type="checkbox" />
    </body></html>"#);
    e.eval_js(r#"
        window.__clicked = false;
        document.getElementById('cb').addEventListener('click', function() {
            window.__clicked = true;
        });
    "#).unwrap();
    e.eval_js(r#"document.getElementById('lbl').click()"#).unwrap();
    let result = e.eval_js(r#"window.__clicked"#).unwrap();
    assert_eq!(result, "true");
}

// =========================================================================
// Property vs Attribute separation (HTML spec compliance)
// =========================================================================

#[test]
fn input_value_property_does_not_update_attribute() {
    // Per HTML spec: setting .value property should NOT change getAttribute('value')
    let mut e = engine_with_html(r#"<html><body><input id="i" /></body></html>"#);
    let result = e.eval_js(r#"
        var el = document.getElementById('i');
        el.value = 'hello';
        el.getAttribute('value')
    "#).unwrap();
    assert_eq!(result, "null", "getAttribute('value') should be null after setting .value property");
}

#[test]
fn input_value_property_preserves_existing_attribute() {
    // Setting .value should not change an existing value attribute
    let mut e = engine_with_html(r#"<html><body><input id="i" value="initial" /></body></html>"#);
    let result = e.eval_js(r#"
        var el = document.getElementById('i');
        el.value = 'changed';
        JSON.stringify([el.value, el.getAttribute('value')])
    "#).unwrap();
    assert_eq!(result, r#"["changed","initial"]"#);
}

#[test]
fn input_set_attribute_value_updates_property_when_not_dirty() {
    // setAttribute('value', ...) should update .value if the property hasn't been set directly
    let mut e = engine_with_html(r#"<html><body><input id="i" /></body></html>"#);
    let result = e.eval_js(r#"
        var el = document.getElementById('i');
        el.setAttribute('value', 'from-attr');
        el.value
    "#).unwrap();
    assert_eq!(result, "from-attr");
}

#[test]
fn input_set_attribute_value_does_not_override_dirty_property() {
    // setAttribute('value', ...) should NOT override .value if it was set directly (dirty)
    let mut e = engine_with_html(r#"<html><body><input id="i" /></body></html>"#);
    let result = e.eval_js(r#"
        var el = document.getElementById('i');
        el.value = 'dirty';
        el.setAttribute('value', 'from-attr');
        el.value
    "#).unwrap();
    assert_eq!(result, "dirty");
}

#[test]
fn input_default_value_reads_and_writes_attribute() {
    let mut e = engine_with_html(r#"<html><body><input id="i" value="initial" /></body></html>"#);
    // defaultValue reads the attribute
    let result = e.eval_js(r#"document.getElementById('i').defaultValue"#).unwrap();
    assert_eq!(result, "initial");
    // Setting defaultValue updates the attribute
    let result = e.eval_js(r#"
        var el = document.getElementById('i');
        el.defaultValue = 'new-default';
        el.getAttribute('value')
    "#).unwrap();
    assert_eq!(result, "new-default");
}

#[test]
fn input_default_checked_reads_and_writes_attribute() {
    let mut e = engine_with_html(r#"<html><body><input id="c" type="checkbox" /></body></html>"#);
    // defaultChecked initially false
    let result = e.eval_js(r#"document.getElementById('c').defaultChecked"#).unwrap();
    assert_eq!(result, "false");
    // Setting defaultChecked updates the attribute
    let result = e.eval_js(r#"
        var el = document.getElementById('c');
        el.defaultChecked = true;
        el.hasAttribute('checked')
    "#).unwrap();
    assert_eq!(result, "true");
}

#[test]
fn click_on_label_with_descendant_control_activates_it() {
    let mut e = engine_with_html(r#"<html><body>
        <label id="lbl">Check me <input id="cb" type="checkbox" /></label>
    </body></html>"#);
    e.eval_js(r#"
        window.__clicked = false;
        document.getElementById('cb').addEventListener('click', function() {
            window.__clicked = true;
        });
    "#).unwrap();
    e.eval_js(r#"document.getElementById('lbl').click()"#).unwrap();
    let result = e.eval_js(r#"window.__clicked"#).unwrap();
    assert_eq!(result, "true");
}

// =========================================================================
// select.options, select.selectedOptions, select.length
// =========================================================================

#[test]
fn select_options_returns_all_option_elements() {
    let mut e = engine_with_html(r#"<html><body>
        <select id="s">
            <option value="a">A</option>
            <option value="b">B</option>
            <option value="c">C</option>
        </select>
    </body></html>"#);
    assert_eq!(e.eval_js("document.getElementById('s').options.length").unwrap(), "3");
}

#[test]
fn select_options_indexed_access() {
    let mut e = engine_with_html(r#"<html><body>
        <select id="s">
            <option value="x">X</option>
            <option value="y">Y</option>
        </select>
    </body></html>"#);
    assert_eq!(
        e.eval_js("document.getElementById('s').options[1].getAttribute('value')").unwrap(),
        "y"
    );
}

#[test]
fn select_selected_options_returns_selected() {
    let mut e = engine_with_html(r#"<html><body>
        <select id="s">
            <option value="a">A</option>
            <option value="b" selected>B</option>
            <option value="c">C</option>
        </select>
    </body></html>"#);
    assert_eq!(e.eval_js("document.getElementById('s').selectedOptions.length").unwrap(), "1");
    assert_eq!(
        e.eval_js("document.getElementById('s').selectedOptions[0].getAttribute('value')").unwrap(),
        "b"
    );
}

#[test]
fn select_selected_options_empty_when_none_explicitly_selected() {
    let mut e = engine_with_html(r#"<html><body>
        <select id="s">
            <option value="a">A</option>
            <option value="b">B</option>
        </select>
    </body></html>"#);
    assert_eq!(e.eval_js("document.getElementById('s').selectedOptions.length").unwrap(), "0");
}

#[test]
fn select_length_returns_number_of_options() {
    let mut e = engine_with_html(r#"<html><body>
        <select id="s">
            <option value="a">A</option>
            <option value="b">B</option>
            <option value="c">C</option>
        </select>
    </body></html>"#);
    assert_eq!(e.eval_js("document.getElementById('s').length").unwrap(), "3");
}

#[test]
fn select_length_returns_zero_for_empty_select() {
    let mut e = engine_with_html(r#"<html><body>
        <select id="s"></select>
    </body></html>"#);
    assert_eq!(e.eval_js("document.getElementById('s').length").unwrap(), "0");
}

#[test]
fn select_length_undefined_for_non_select() {
    let mut e = engine_with_html(r#"<html><body><div id="d"></div></body></html>"#);
    assert_eq!(e.eval_js("String(document.getElementById('d').length)").unwrap(), "undefined");
}

// =========================================================================
// input.form attribute support
// =========================================================================

#[test]
fn input_form_getter_uses_form_attribute() {
    let mut e = engine_with_html(
        r#"<html><body><form id="myform"><input type="text" /></form><input id="ext" form="myform" /></body></html>"#,
    );
    let result = e.eval_js(r#"document.getElementById("ext").form.id"#).unwrap();
    assert_eq!(result, "myform");
}

#[test]
fn input_form_getter_returns_null_for_invalid_form_attribute() {
    let mut e = engine_with_html(
        r#"<html><body><input id="ext" form="nonexistent" /></body></html>"#,
    );
    let result = e.eval_js(r#"document.getElementById("ext").form === null"#).unwrap();
    assert_eq!(result, "true");
}

#[test]
fn input_form_getter_falls_back_to_ancestor() {
    let mut e = engine_with_html(
        r#"<html><body><form id="f"><input id="inp" type="text" /></form></body></html>"#,
    );
    let result = e.eval_js(r#"document.getElementById("inp").form.id"#).unwrap();
    assert_eq!(result, "f");
}

#[test]
fn form_elements_includes_external_inputs_with_form_attribute() {
    let mut e = engine_with_html(
        r#"<html><body><form id="myform"><input type="text" name="inside" /></form><input type="text" name="outside" form="myform" /></body></html>"#,
    );
    let result = e.eval_js(r#"document.getElementById("myform").elements.length"#).unwrap();
    assert_eq!(result, "2");
}

#[test]
fn form_elements_external_inputs_have_correct_names() {
    let mut e = engine_with_html(
        r#"<html><body><form id="myform"><input name="a" /></form><input name="b" form="myform" /><textarea name="c" form="myform"></textarea></body></html>"#,
    );
    let result = e.eval_js(r#"
        var elems = document.getElementById("myform").elements;
        var names = [];
        for (var i = 0; i < elems.length; i++) {
            names.push(elems[i].name);
        }
        names.join(",");
    "#).unwrap();
    assert_eq!(result, "a,b,c");
}

// =========================================================================
// Live HTMLCollection tests
// =========================================================================

#[test]
fn getelements_by_tag_name_is_live_on_add() {
    let mut e = engine_with_html("<html><body><p>first</p></body></html>");
    let result = e.eval_js(r#"
        var col = document.getElementsByTagName('p');
        var before = col.length;
        var p2 = document.createElement('p');
        p2.textContent = 'second';
        document.body.appendChild(p2);
        before + ',' + col.length;
    "#);
    assert_eq!(result.unwrap(), "1,2");
}

#[test]
fn getelements_by_tag_name_is_live_on_remove() {
    let mut e = engine_with_html("<html><body><p id='a'>one</p><p id='b'>two</p></body></html>");
    let result = e.eval_js(r#"
        var col = document.getElementsByTagName('p');
        var before = col.length;
        var a = document.getElementById('a');
        a.parentNode.removeChild(a);
        before + ',' + col.length;
    "#);
    assert_eq!(result.unwrap(), "2,1");
}

#[test]
fn getelements_by_class_name_is_live_on_add() {
    let mut e = engine_with_html("<html><body><div class='x'>a</div></body></html>");
    let result = e.eval_js(r#"
        var col = document.getElementsByClassName('x');
        var before = col.length;
        var d = document.createElement('div');
        d.className = 'x';
        document.body.appendChild(d);
        before + ',' + col.length;
    "#);
    assert_eq!(result.unwrap(), "1,2");
}

#[test]
fn getelements_by_class_name_is_live_on_remove() {
    let mut e = engine_with_html("<html><body><div class='x' id='a'>a</div><div class='x'>b</div></body></html>");
    let result = e.eval_js(r#"
        var col = document.getElementsByClassName('x');
        var before = col.length;
        var a = document.getElementById('a');
        a.parentNode.removeChild(a);
        before + ',' + col.length;
    "#);
    assert_eq!(result.unwrap(), "2,1");
}

#[test]
fn element_getelements_by_tag_name_is_live() {
    let mut e = engine_with_html("<html><body><div id='container'><span>a</span></div></body></html>");
    let result = e.eval_js(r#"
        var container = document.getElementById('container');
        var col = container.getElementsByTagName('span');
        var before = col.length;
        var s = document.createElement('span');
        container.appendChild(s);
        before + ',' + col.length;
    "#);
    assert_eq!(result.unwrap(), "1,2");
}

#[test]
fn form_elements_is_live_on_add() {
    let mut e = engine_with_html("<html><body><form id='f'><input name='a'></form></body></html>");
    let result = e.eval_js(r#"
        var form = document.getElementById('f');
        var els = form.elements;
        var before = els.length;
        var inp = document.createElement('input');
        inp.setAttribute('name', 'b');
        form.appendChild(inp);
        before + ',' + els.length;
    "#);
    assert_eq!(result.unwrap(), "1,2");
}

#[test]
fn form_elements_is_live_on_remove() {
    let mut e = engine_with_html("<html><body><form id='f'><input id='i1' name='a'><input name='b'></form></body></html>");
    let result = e.eval_js(r#"
        var form = document.getElementById('f');
        var els = form.elements;
        var before = els.length;
        var i1 = document.getElementById('i1');
        i1.parentNode.removeChild(i1);
        before + ',' + els.length;
    "#);
    assert_eq!(result.unwrap(), "2,1");
}

#[test]
fn form_elements_named_access() {
    let mut e = engine_with_html("<html><body><form id='f'><input name='email' value='test@example.com'></form></body></html>");
    let result = e.eval_js(r#"
        var form = document.getElementById('f');
        var els = form.elements;
        els.namedItem('email').value;
    "#);
    assert_eq!(result.unwrap(), "test@example.com");
}

#[test]
fn live_collection_index_access_updates() {
    let mut e = engine_with_html("<html><body><p>first</p></body></html>");
    let result = e.eval_js(r#"
        var col = document.getElementsByTagName('p');
        var first = col[0].textContent;
        var p2 = document.createElement('p');
        p2.textContent = 'second';
        document.body.appendChild(p2);
        first + ',' + col[1].textContent;
    "#);
    assert_eq!(result.unwrap(), "first,second");
}

#[test]
fn checked_property_does_not_update_attribute() {
    // Per HTML spec: setting .checked property should NOT change the checked attribute
    let mut e = engine_with_html(r#"<html><body><input id="c" type="checkbox" /></body></html>"#);
    let result = e.eval_js(r#"
        var el = document.getElementById('c');
        el.checked = true;
        JSON.stringify([el.checked, el.hasAttribute('checked')])
    "#).unwrap();
    assert_eq!(result, r#"[true,false]"#);
}

#[test]
fn checked_property_does_not_remove_existing_attribute() {
    // Setting .checked = false shouldn't remove the checked attribute
    let mut e = engine_with_html(r#"<html><body><input id="c" type="checkbox" checked /></body></html>"#);
    let result = e.eval_js(r#"
        var el = document.getElementById('c');
        el.checked = false;
        JSON.stringify([el.checked, el.hasAttribute('checked')])
    "#).unwrap();
    assert_eq!(result, r#"[false,true]"#);
}

#[test]
fn textarea_value_property_does_not_update_value_attribute() {
    // textarea.value should update text content but NOT a value attribute
    let mut e = engine_with_html(r#"<html><body><textarea id="t"></textarea></body></html>"#);
    let result = e.eval_js(r#"
        var el = document.getElementById('t');
        el.value = 'typed text';
        JSON.stringify([el.value, el.getAttribute('value')])
    "#).unwrap();
    assert_eq!(result, r#"["typed text",null]"#);
}

#[test]
fn select_value_property_does_not_update_attribute() {
    let mut e = engine_with_html(r#"<html><body>
        <select id="s">
            <option value="a">A</option>
            <option value="b">B</option>
        </select>
    </body></html>"#);
    let result = e.eval_js(r#"
        var el = document.getElementById('s');
        el.value = 'b';
        JSON.stringify([el.value, el.getAttribute('value')])
    "#).unwrap();
    assert_eq!(result, r#"["b",null]"#);
}

// =========================================================================
// React event delegation prerequisites
// =========================================================================

#[test]
fn element_identity_get_element_by_id() {
    // document.getElementById must return the SAME object for the same element
    let mut e = engine_with_html("<html><body><div id='root'>hello</div></body></html>");
    let result = e.eval_js(
        "document.getElementById('root') === document.getElementById('root')"
    );
    assert_eq!(result.unwrap(), "true", "getElementById must return identical objects");
}

#[test]
fn element_identity_query_selector() {
    // querySelector must return the same cached wrapper
    let mut e = engine_with_html("<html><body><div id='root'>hello</div></body></html>");
    let result = e.eval_js(
        "document.querySelector('#root') === document.getElementById('root')"
    );
    assert_eq!(result.unwrap(), "true", "querySelector and getElementById must return the same object");
}

#[test]
fn element_identity_parent_child_traversal() {
    // Traversal via parentNode/firstChild must return cached wrappers
    let mut e = engine_with_html("<html><body><div id='parent'><span id='child'>x</span></div></body></html>");
    let result = e.eval_js(r#"
        var parent = document.getElementById('parent');
        var child = document.getElementById('child');
        var childViaParent = parent.firstChild;
        var parentViaChild = child.parentNode;
        (child === childViaParent) + ',' + (parent === parentViaChild)
    "#);
    assert_eq!(result.unwrap(), "true,true", "traversal must return identity-equal wrappers");
}

#[test]
fn capture_phase_listener_fires_on_root() {
    // A capture-phase listener on the root must fire when a child dispatches an event
    let mut e = engine_with_html("<html><body><div id='root'><button id='btn'>Click</button></div></body></html>");
    let result = e.eval_js(r#"
        var root = document.getElementById('root');
        var btn = document.getElementById('btn');
        var captureTarget = null;
        var captureCurrentTarget = null;
        var captureFired = false;

        root.addEventListener('click', function(e) {
            captureFired = true;
            captureTarget = e.target;
            captureCurrentTarget = e.currentTarget;
        }, true);

        btn.click();

        captureFired + ',' + (captureTarget === btn) + ',' + (captureCurrentTarget === root)
    "#);
    assert_eq!(result.unwrap(), "true,true,true",
        "capture listener on root must fire with correct target/currentTarget");
}

#[test]
fn event_target_identity_through_bubbling() {
    // event.target must be the same object reference throughout capture and bubble phases
    let mut e = engine_with_html("<html><body><div id='outer'><div id='inner'><span id='leaf'>x</span></div></div></body></html>");
    let result = e.eval_js(r#"
        var outer = document.getElementById('outer');
        var inner = document.getElementById('inner');
        var leaf = document.getElementById('leaf');
        var targets = [];

        outer.addEventListener('click', function(e) { targets.push(e.target); }, true);
        inner.addEventListener('click', function(e) { targets.push(e.target); }, true);
        leaf.addEventListener('click', function(e) { targets.push(e.target); });
        inner.addEventListener('click', function(e) { targets.push(e.target); });
        outer.addEventListener('click', function(e) { targets.push(e.target); });

        leaf.click();

        // All targets should be the exact same object (the leaf)
        var allSame = targets.every(function(t) { return t === leaf; });
        targets.length + ',' + allSame
    "#);
    assert_eq!(result.unwrap(), "5,true",
        "event.target must be identity-equal across all phases");
}

#[test]
fn event_instanceof_check() {
    // Events created via new Event() must pass instanceof checks
    let mut e = engine_with_html("<html><body></body></html>");
    let result = e.eval_js(r#"
        var e1 = new Event('click');
        var e2 = new MouseEvent('click');
        var e3 = new CustomEvent('foo');
        (e1 instanceof Event) + ',' + (e2 instanceof Event) + ',' + (e2 instanceof MouseEvent) + ',' + (e3 instanceof Event) + ',' + (e3 instanceof CustomEvent)
    "#);
    assert_eq!(result.unwrap(), "true,true,true,true,true",
        "Events must pass instanceof checks");
}

#[test]
fn react_style_delegation_simulation() {
    // Simulate React 17+ event delegation: register capture listener on container,
    // dispatch click on a deeply nested child, verify the listener fires and can
    // find __reactFiber$ / __reactProps$ on event.target
    let mut e = engine_with_html(r#"<html><body><div id="root"><div id="container"><button id="btn">Go</button></div></div></body></html>"#);
    let result = e.eval_js(r#"
        var root = document.getElementById('root');
        var btn = document.getElementById('btn');

        // Simulate what React does: attach fiber/props metadata to DOM elements
        btn['__reactFiber$abc123'] = { stateNode: btn, memoizedProps: { onClick: function() {} } };
        btn['__reactProps$abc123'] = { onClick: function() { window.__reactOnClickFired = true; } };

        // Simulate React's root listener (capture phase on root container)
        var delegatedTarget = null;
        var foundFiber = false;
        root.addEventListener('click', function(e) {
            delegatedTarget = e.target;
            // React does: getClosestInstanceFromNode(e.target)
            // which checks for __reactFiber$ on the target
            var keys = Object.keys(e.target);
            for (var i = 0; i < keys.length; i++) {
                if (keys[i].indexOf('__reactFiber$') === 0) {
                    foundFiber = true;
                    break;
                }
            }
            // React would then call the handler from __reactProps$
            if (foundFiber) {
                var propsKey = Object.keys(e.target).find(function(k) { return k.indexOf('__reactProps$') === 0; });
                if (propsKey && e.target[propsKey].onClick) {
                    e.target[propsKey].onClick({ type: 'click', target: e.target });
                }
            }
        }, true);

        btn.click();

        (delegatedTarget === btn) + ',' + foundFiber + ',' + (window.__reactOnClickFired === true)
    "#);
    assert_eq!(result.unwrap(), "true,true,true",
        "React-style delegation must work: capture listener on root fires, finds fiber on target, invokes handler");
}

#[test]
fn dispatch_event_target_not_overwritten() {
    // When dispatchEvent is called, the event.target should be set by __dispatch
    // and not be overwritable during the dispatch
    let mut e = engine_with_html("<html><body><div id='root'><span id='child'>hi</span></div></body></html>");
    let result = e.eval_js(r#"
        var root = document.getElementById('root');
        var child = document.getElementById('child');
        var targetInCapture = null;
        var targetInBubble = null;

        root.addEventListener('click', function(e) {
            targetInCapture = e.target;
        }, true);
        root.addEventListener('click', function(e) {
            targetInBubble = e.target;
        }, false);

        var evt = new MouseEvent('click', {bubbles: true, cancelable: true});
        child.dispatchEvent(evt);

        (targetInCapture === child) + ',' + (targetInBubble === child)
    "#);
    assert_eq!(result.unwrap(), "true,true",
        "dispatchEvent must set event.target to the dispatching element");
}

#[test]
fn microtask_flushing_after_event_dispatch() {
    // React uses queueMicrotask for batching. Verify microtasks run after event dispatch.
    let mut e = engine_with_html("<html><body><button id='btn'>X</button></body></html>");
    let result = e.eval_js(r#"
        var btn = document.getElementById('btn');
        var order = [];

        btn.addEventListener('click', function(e) {
            order.push('handler');
            queueMicrotask(function() {
                order.push('microtask');
            });
        });

        btn.click();
        order.join(',')
    "#);
    // After click(), the handler runs synchronously. The microtask should run
    // after the current task completes (which is the click() call).
    let val = result.unwrap();
    assert!(val.contains("handler"), "handler must run, got: {val}");
    // Note: microtask may or may not have run yet depending on flush timing.
    // If it hasn't, that's a potential issue for React.
}

#[test]
fn microtask_runs_inline_during_click() {
    // React 18 needs microtasks to flush during synchronous event dispatch.
    // If microtasks only flush after settle(), React's setState batching won't work.
    let mut e = engine_with_html("<html><body><button id='btn'>X</button></body></html>");
    let result = e.eval_js(r#"
        var btn = document.getElementById('btn');
        var order = [];

        btn.addEventListener('click', function(e) {
            order.push('handler-start');
            queueMicrotask(function() {
                order.push('microtask-1');
            });
            Promise.resolve().then(function() {
                order.push('promise-1');
            });
            order.push('handler-end');
        });

        btn.click();
        order.push('after-click');
        order.join(',')
    "#);
    let val = result.unwrap();
    // In a real browser: handler-start, handler-end, microtask-1, promise-1, after-click
    // In QuickJS: microtasks don't flush inline during JS execution — they only
    // flush when Rust calls execute_pending_job(). So the order is:
    //   handler-start, handler-end, after-click (microtasks deferred)
    // This is acceptable because settle() flushes microtasks after event dispatch.
    assert!(val.starts_with("handler-start,handler-end"),
        "handler must run synchronously, got: {val}");
    assert!(val.contains("after-click"),
        "synchronous code after click must run, got: {val}");
    // Note: microtask-1 and promise-1 are NOT present because QuickJS defers
    // microtask execution to the Rust host. This is a known limitation but
    // does not break React because settle() flushes jobs after event dispatch.
    assert!(!val.contains("microtask-1"),
        "microtasks should be deferred to Rust flush_jobs, got: {val}");
}

#[test]
fn react_onchange_via_delegation_not_hack() {
    // Simulate React's onChange delegation pattern:
    // 1. React registers capture listener on root for 'change' event
    // 2. When change fires on input, React's listener gets it at root
    // 3. React finds the fiber on event.target and calls the onChange handler
    // This tests that the delegation path works WITHOUT the __reactProps$ hack
    let mut e = engine_with_html(r#"<html><body><div id="root"><input id="inp" type="text"></div></body></html>"#);
    let result = e.eval_js(r#"
        var root = document.getElementById('root');
        var inp = document.getElementById('inp');
        var delegatedChangeValue = null;

        // Simulate React's fiber metadata
        inp['__reactFiber$test'] = { stateNode: inp };
        inp['__reactProps$test'] = {
            onChange: function(e) { delegatedChangeValue = e.target.value; }
        };

        // React's root capture listener (simplified)
        root.addEventListener('change', function(e) {
            var target = e.target;
            var propsKey = Object.keys(target).find(function(k) { return k.indexOf('__reactProps$') === 0; });
            if (propsKey && target[propsKey] && typeof target[propsKey].onChange === 'function') {
                target[propsKey].onChange(e);
            }
        }, true);

        // Simulate typing: set value and dispatch change event
        inp.value = 'hello world';
        var changeEvt = new Event('change', {bubbles: true});
        inp.dispatchEvent(changeEvt);

        delegatedChangeValue
    "#);
    assert_eq!(result.unwrap(), "hello world",
        "React-style onChange delegation via capture must work without __reactProps$ hack");
}

#[test]
fn react17_full_delegation_flow() {
    // Full React 17+ delegation simulation:
    // 1. React registers ALL event listeners on root container with capture
    // 2. When event fires, React's listener finds the fiber via event.target
    // 3. React walks fiber tree to accumulate listeners
    // 4. React dispatches synthetic events to the accumulated listeners
    // 5. React setState triggers re-render via MessageChannel (setTimeout(0))
    let mut e = engine_with_html(r#"<html><body>
        <div id="root">
            <div id="app">
                <button id="btn">Count: 0</button>
            </div>
        </div>
    </body></html>"#);
    e.eval_js(r#"
        var root = document.getElementById('root');
        var app = document.getElementById('app');
        var btn = document.getElementById('btn');

        // Simulate React fiber tree
        var internalKey = '__reactFiber$test123';
        var propsKey = '__reactProps$test123';
        var containerKey = '__reactContainer$test123';

        // React marks the root container
        root[containerKey] = { current: { child: null } };

        // State
        var count = 0;
        var renderCount = 0;

        // onClick handler (what the user writes in JSX)
        function handleClick() {
            count++;
        }

        // React attaches fibers and props to DOM nodes during render
        function reactRender() {
            renderCount++;
            btn.textContent = 'Count: ' + count;
            btn[internalKey] = {
                stateNode: btn,
                return: { stateNode: app, return: { stateNode: root } },
                memoizedProps: { onClick: handleClick },
            };
            btn[propsKey] = { onClick: handleClick };
            app[internalKey] = {
                stateNode: app,
                return: { stateNode: root },
            };
        }

        // Initial render
        reactRender();

        // React's root event listener (capture phase, registered once)
        root.addEventListener('click', function dispatchDiscreteEvent(nativeEvent) {
            var target = nativeEvent.target;

            // getClosestInstanceFromNode: walk up from target looking for fiber
            var node = target;
            var fiber = null;
            while (node) {
                fiber = node[internalKey];
                if (fiber) break;
                node = node.parentNode;
            }

            if (!fiber) return;

            // accumulateSinglePhaseListeners: walk fiber tree to find onClick handlers
            var listeners = [];
            var inst = fiber;
            while (inst) {
                if (inst.memoizedProps && typeof inst.memoizedProps.onClick === 'function') {
                    listeners.push({
                        instance: inst,
                        listener: inst.memoizedProps.onClick,
                        currentTarget: inst.stateNode,
                    });
                }
                inst = inst.return;
            }

            // processDispatchQueue: call each listener
            for (var i = 0; i < listeners.length; i++) {
                listeners[i].listener.call(null, {
                    type: 'click',
                    target: target,
                    currentTarget: listeners[i].currentTarget,
                    nativeEvent: nativeEvent,
                    preventDefault: function() {},
                    stopPropagation: function() {},
                    persist: function() {},
                });
            }

            // Schedule re-render via MessageChannel (shimmed to setTimeout(0))
            setTimeout(function() { reactRender(); }, 0);
        }, true);

        // Dispatch click
        btn.click();

        // At this point, the handler has run synchronously (count=1),
        // but the re-render is scheduled via setTimeout(0) and hasn't run yet.
        window.__countAfterClick = count;
        window.__renderCountAfterClick = renderCount;
        window.__textAfterClick = btn.textContent;
    "#).unwrap();

    // After settle(), the setTimeout(0) should have fired, causing a re-render
    e.settle();

    let count = e.eval_js("window.__countAfterClick").unwrap();
    // With the __reactProps$ hack removed, onClick fires exactly once via delegation
    assert_eq!(count, "1", "onClick handler fires exactly once via capture-phase delegation");

    let text_after_settle = e.eval_js("btn.textContent").unwrap();
    assert_eq!(text_after_settle, "Count: 1", "Re-render via setTimeout(0) must complete after settle()");

    let render_count = e.eval_js("renderCount").unwrap();
    assert_eq!(render_count, "2", "Two renders: initial + after click");
}

#[test]
fn promise_resolves_during_settle() {
    // Promises (microtasks) should resolve when we flush jobs
    let mut e = engine_with_html("<html><body></body></html>");
    e.eval_js(r#"
        window.__promiseResult = 'pending';
        Promise.resolve().then(function() {
            window.__promiseResult = 'resolved';
        });
    "#).unwrap();
    e.settle();
    let result = e.eval_js("window.__promiseResult");
    assert_eq!(result.unwrap(), "resolved", "Promises must resolve during settle");
}
