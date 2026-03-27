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
