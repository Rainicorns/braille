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

// =========================================================================
// getElementsByTagName namespace-aware case sensitivity
// =========================================================================

#[test]
fn getelementsbytagname_html_ns_case_insensitive() {
    // HTML-namespace elements match case-insensitively
    let mut e = engine_with_html("<html><body></body></html>");
    let result = e.eval_js(r#"
        (function() {
            var div = document.createElement('div');
            var a1 = document.createElementNS('http://www.w3.org/1999/xhtml', 'a');
            var a2 = document.createElementNS('http://www.w3.org/1999/xhtml', 'A');
            div.appendChild(a1);
            div.appendChild(a2);
            var list = div.getElementsByTagName('A');
            return list.length;
        })()
    "#).unwrap();
    // Per spec: only the input is lowercased. XHTML 'a' matches (stored 'a' == lowered 'a'),
    // but XHTML 'A' does NOT match (stored 'A' != lowered 'a'). Only parser-created elements
    // have lowercased tag names; createElementNS preserves case.
    assert_eq!(result, "1", "getElementsByTagName('A') lowercases input but not element name for XHTML namespace");
}

#[test]
fn getelementsbytagname_non_html_ns_case_sensitive() {
    // Non-HTML-namespace elements match case-sensitively
    let mut e = engine_with_html("<html><body></body></html>");
    let result = e.eval_js(r#"
        (function() {
            var div = document.createElement('div');
            var a1 = document.createElementNS('', 'a');
            var a2 = document.createElementNS('', 'A');
            div.appendChild(a1);
            div.appendChild(a2);
            var list = div.getElementsByTagName('A');
            return list.length;
        })()
    "#).unwrap();
    assert_eq!(result, "1", "getElementsByTagName('A') should only match 'A' (case-sensitive) for non-HTML namespace");
}

#[test]
fn getelementsbytagname_mixed_namespaces() {
    // The WPT test pattern: mixed XHTML and null-namespace elements
    let mut e = engine_with_html("<html><body></body></html>");
    let result = e.eval_js(r#"
        (function() {
            var parent = document.createElement('div');
            var child1 = document.createElementNS('http://www.w3.org/1999/xhtml', 'a');
            var child2 = document.createElementNS('http://www.w3.org/1999/xhtml', 'A');
            var child3 = document.createElementNS('', 'a');
            var child4 = document.createElementNS('', 'A');
            parent.appendChild(child1);
            parent.appendChild(child2);
            parent.appendChild(child3);
            parent.appendChild(child4);

            var list = parent.getElementsByTagName('A');
            // XHTML 'a' matches (lowercased 'A' = 'a'), XHTML 'A' matches (lowercased 'A' = 'a'),
            // null-ns 'a' doesn't match ('a' != 'A'), null-ns 'A' matches ('A' == 'A')
            var names = [];
            for (var i = 0; i < list.length; i++) {
                names.push(list[i].textContent || list[i].tagName);
            }
            return list.length + ':' + names.join(',');
        })()
    "#).unwrap();
    // child1 (xhtml:a, stored "a" == lowered "a"), child4 (null:A, "A" == "A") = 2 matches
    // child2 (xhtml:A, stored "A" != lowered "a") does NOT match
    // child3 (null:a, "a" != "A") does NOT match
    assert!(result.starts_with("2:"), "Expected 2 matches for mixed namespaces, got: {}", result);
}


#[test]
fn getelementsbytagname_wpt_change_document_htmlness_flow() {
    // Simulates the full WPT Element-getElementsByTagName-change-document-HTMLNess test flow
    use std::collections::HashMap;
    use braille_engine::FetchedResources;

    let html = r#"<!doctype html>
<iframe src="test.xml"></iframe>
<script>
  window.debugSteps = [];
  window.onload = function() {
    try {
      var parent = document.createElement("div");
      var child1 = document.createElementNS("http://www.w3.org/1999/xhtml", "a");
      var child2 = document.createElementNS("http://www.w3.org/1999/xhtml", "A");
      var child3 = document.createElementNS("", "a");
      var child4 = document.createElementNS("", "A");
      parent.appendChild(child1);
      parent.appendChild(child2);
      parent.appendChild(child3);
      parent.appendChild(child4);

      var list = parent.getElementsByTagName("A");
      debugSteps.push('list.length=' + list.length);
      debugSteps.push('list[0]===child1: ' + (list[0] === child1));
      debugSteps.push('list[1]===child4: ' + (list[1] === child4));

      debugSteps.push('frames.length=' + frames.length);
      frames[0].document.documentElement.appendChild(parent);
      debugSteps.push('moved to iframe doc');

      // list was created in HTML context — should still show HTML lowercasing
      debugSteps.push('list after move len=' + list.length);
      debugSteps.push('list[0]===child1 after move: ' + (list[0] === child1));
      debugSteps.push('list[1]===child4 after move: ' + (list[1] === child4));

      // New list created in XML context — case-sensitive matching
      var list2 = parent.getElementsByTagName("A");
      debugSteps.push('list2.length=' + list2.length);
      debugSteps.push('list2[0]===child2: ' + (list2[0] === child2));
      debugSteps.push('list2[1]===child4: ' + (list2[1] === child4));

      // Re-append children (blow away caches)
      parent.appendChild(child1);
      parent.appendChild(child2);
      parent.appendChild(child3);
      parent.appendChild(child4);
      debugSteps.push('after reappend list.length=' + list.length);
      debugSteps.push('list[0]===child1: ' + (list[0] === child1));
      debugSteps.push('list[1]===child4: ' + (list[1] === child4));
      debugSteps.push('list2.length=' + list2.length);
      debugSteps.push('list2[0]===child2: ' + (list2[0] === child2));
      debugSteps.push('list2[1]===child4: ' + (list2[1] === child4));

      debugSteps.push('DONE');
    } catch(e) {
      debugSteps.push('ERROR: ' + e.toString());
    }
  };
</script>"#;

    let mut iframes = HashMap::new();
    iframes.insert("test.xml".to_string(), "<root/>".to_string());

    let mut engine = Engine::new();
    let fetched = FetchedResources {
        scripts: HashMap::new(),
        iframes,
    };
    engine.load_html_with_resources(html, &fetched);
    engine.settle();

    let log = engine.eval_js("JSON.stringify(window.debugSteps)").unwrap();
    eprintln!("debugSteps: {}", log);

    // First assertion: list should contain [child1, child4] (2 elements)
    assert!(log.contains("\"list.length=2\""), "getElementsByTagName should return 2 elements, got: {}", log);
    assert!(log.contains("\"DONE\""), "onload should complete without error, got: {}", log);
}

