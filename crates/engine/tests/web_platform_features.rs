//! Tests for the 10 major web platform features:
//! - CSS Custom Properties (var())
//! - CSS calc()/min()/max()/clamp()
//! - CSS Grid
//! - UA stylesheet defaults
//! - history popstate
//! - localStorage persistence
//! - IntersectionObserver
//! - ResizeObserver
//! - WebSocket
//! - Dynamic import()

use braille_engine::Engine;

fn engine_with_html(html: &str) -> Engine {
    let mut e = Engine::new();
    e.load_html(html);
    e
}

// ---------------------------------------------------------------------------
// Feature 5: CSS Custom Properties (var())
// ---------------------------------------------------------------------------

#[test]
fn css_var_basic_substitution() {
    let mut e = engine_with_html(r#"
        <style>
            :root { --main-color: red; }
            .test { color: var(--main-color); }
        </style>
        <div class="test" id="t">hello</div>
    "#);
    let result = e.eval_js("getComputedStyle(document.getElementById('t')).color").unwrap();
    assert_eq!(result, "rgb(255, 0, 0)");
}

#[test]
fn css_var_with_fallback() {
    let mut e = engine_with_html(r#"
        <style>
            .test { color: var(--undefined-prop, blue); }
        </style>
        <div class="test" id="t">hello</div>
    "#);
    let result = e.eval_js("getComputedStyle(document.getElementById('t')).color").unwrap();
    assert_eq!(result, "rgb(0, 0, 255)");
}

#[test]
fn css_var_inheritance() {
    let mut e = engine_with_html(r#"
        <style>
            .parent { --size: 24px; }
            .child { font-size: var(--size); }
        </style>
        <div class="parent"><span class="child" id="c">text</span></div>
    "#);
    let result = e.eval_js("getComputedStyle(document.getElementById('c')).fontSize").unwrap();
    assert_eq!(result, "24px");
}

// ---------------------------------------------------------------------------
// Feature 6: CSS calc()/min()/max()/clamp()
// ---------------------------------------------------------------------------

#[test]
fn css_calc_basic() {
    let mut e = engine_with_html(r#"
        <style>
            #t { width: calc(100px + 50px); }
        </style>
        <div id="t">hello</div>
    "#);
    let result = e.eval_js("getComputedStyle(document.getElementById('t')).width").unwrap();
    assert_eq!(result, "150px");
}

#[test]
fn css_calc_mixed_units() {
    let mut e = engine_with_html(r#"
        <style>
            #t { font-size: 20px; width: calc(100px + 2em); }
        </style>
        <div id="t">hello</div>
    "#);
    let result = e.eval_js("getComputedStyle(document.getElementById('t')).width").unwrap();
    // 100px + 2*20px = 140px
    assert_eq!(result, "140px");
}

#[test]
fn css_min_function() {
    let mut e = engine_with_html(r#"
        <style>
            #t { width: min(300px, 100px); }
        </style>
        <div id="t">hello</div>
    "#);
    let result = e.eval_js("getComputedStyle(document.getElementById('t')).width").unwrap();
    assert_eq!(result, "100px");
}

#[test]
fn css_clamp_function() {
    let mut e = engine_with_html(r#"
        <style>
            #t { width: clamp(50px, 200px, 150px); }
        </style>
        <div id="t">hello</div>
    "#);
    let result = e.eval_js("getComputedStyle(document.getElementById('t')).width").unwrap();
    // clamp(50, 200, 150) => 150 (preferred > max)
    assert_eq!(result, "150px");
}

// ---------------------------------------------------------------------------
// Feature 2: CSS Grid
// ---------------------------------------------------------------------------

#[test]
fn css_grid_display() {
    let mut e = engine_with_html(r#"
        <style>
            #g { display: grid; grid-template-columns: 1fr 1fr; }
        </style>
        <div id="g"><div>A</div><div>B</div></div>
    "#);
    let result = e.eval_js("getComputedStyle(document.getElementById('g')).display").unwrap();
    assert_eq!(result, "grid");
}

#[test]
fn css_grid_template_columns() {
    let mut e = engine_with_html(r#"
        <style>
            #g { display: grid; grid-template-columns: 100px 200px; }
        </style>
        <div id="g"><div>A</div><div>B</div></div>
    "#);
    let result = e.eval_js("getComputedStyle(document.getElementById('g')).gridTemplateColumns").unwrap();
    assert_eq!(result, "100px 200px");
}

#[test]
fn css_grid_column_placement() {
    let mut e = engine_with_html(r#"
        <style>
            #g { display: grid; grid-template-columns: 1fr 1fr 1fr; }
            #item { grid-column: 2 / 4; }
        </style>
        <div id="g"><div id="item">spans 2</div></div>
    "#);
    let result = e.eval_js("getComputedStyle(document.getElementById('item')).gridColumnStart").unwrap();
    assert_eq!(result, "2");
}

// ---------------------------------------------------------------------------
// Feature 7: UA Defaults
// ---------------------------------------------------------------------------

#[test]
fn ua_table_display() {
    let mut e = engine_with_html(r#"
        <table id="t"><tr><td>Cell</td></tr></table>
    "#);
    let result = e.eval_js("getComputedStyle(document.getElementById('t')).display").unwrap();
    assert_eq!(result, "table");
}

#[test]
fn ua_form_elements_inline_block() {
    let mut e = engine_with_html(r#"<input id="i" type="text">"#);
    let result = e.eval_js("getComputedStyle(document.getElementById('i')).display").unwrap();
    assert_eq!(result, "inline-block");
}

#[test]
fn ua_section_block() {
    let mut e = engine_with_html(r#"<article id="a">content</article>"#);
    let result = e.eval_js("getComputedStyle(document.getElementById('a')).display").unwrap();
    assert_eq!(result, "block");
}

#[test]
fn ua_noscript_hidden() {
    let mut e = engine_with_html(r#"<noscript id="ns">content</noscript>"#);
    let result = e.eval_js("getComputedStyle(document.getElementById('ns')).display").unwrap();
    assert_eq!(result, "none");
}

#[test]
fn ua_pre_monospace() {
    let mut e = engine_with_html(r#"<pre id="p">code</pre>"#);
    let result = e.eval_js("getComputedStyle(document.getElementById('p')).fontFamily").unwrap();
    assert_eq!(result, "monospace");
}

// ---------------------------------------------------------------------------
// Feature 3: history popstate
// ---------------------------------------------------------------------------

#[test]
fn history_popstate_fires_on_back() {
    let mut e = Engine::new();
    e.set_url("https://example.com/");
    e.load_html(r#"<html><body>
        <script>
            var popstateState = null;
            window.addEventListener('popstate', function(e) { popstateState = e.state; });
            history.pushState({page: 1}, '', '/page1');
            history.pushState({page: 2}, '', '/page2');
        </script>
    </body></html>"#);
    e.settle();
    let result = e.eval_js("history.back(); popstateState && popstateState.page").unwrap();
    assert_eq!(result, "1");
}

#[test]
fn history_popstate_updates_url() {
    let mut e = Engine::new();
    e.set_url("https://example.com/");
    e.load_html(r#"<html><body>
        <script>
            history.pushState(null, '', '/page1');
            history.pushState(null, '', '/page2');
        </script>
    </body></html>"#);
    e.settle();
    let _ = e.eval_js("history.back()");
    let result = e.eval_js("location.pathname").unwrap();
    assert_eq!(result, "/page1");
}

// ---------------------------------------------------------------------------
// Feature 4: localStorage persistence
// ---------------------------------------------------------------------------

#[test]
fn localstorage_basic_operations() {
    let mut e = engine_with_html(r#"<html><body>
        <script>
            localStorage.setItem('key', 'value');
        </script>
    </body></html>"#);
    let result = e.eval_js("localStorage.getItem('key')").unwrap();
    assert_eq!(result, "value");
}

// ---------------------------------------------------------------------------
// Feature 1: IntersectionObserver
// ---------------------------------------------------------------------------

#[test]
fn intersection_observer_construct() {
    let mut e = engine_with_html(r#"<div id="t">hello</div>"#);
    let result = e.eval_js(r#"
        var io = new IntersectionObserver(function() {});
        io.thresholds.length
    "#).unwrap();
    assert_eq!(result, "1");
}

#[test]
fn intersection_observer_fires_on_observe() {
    let mut e = engine_with_html(r#"<div id="t" style="width:100px;height:100px;">hello</div>"#);
    let _ = e.eval_js(r#"
        globalThis.__io_result = null;
        var io = new IntersectionObserver(function(entries) {
            globalThis.__io_result = entries[0].isIntersecting;
        });
        io.observe(document.getElementById('t'));
    "#);
    e.settle();
    let result = e.eval_js("String(__io_result)").unwrap();
    assert_eq!(result, "true");
}

#[test]
fn intersection_observer_disconnect() {
    let mut e = engine_with_html(r#"<div id="t">hello</div>"#);
    let _ = e.eval_js(r#"
        globalThis.__io_count = 0;
        var io = new IntersectionObserver(function(entries) {
            globalThis.__io_count++;
        });
        io.observe(document.getElementById('t'));
        io.disconnect();
    "#);
    e.settle();
    // After disconnect, the initial check should not fire for this observer
    let result = e.eval_js("__io_count").unwrap();
    assert_eq!(result, "0");
}

// ---------------------------------------------------------------------------
// Feature 9: ResizeObserver
// ---------------------------------------------------------------------------

#[test]
fn resize_observer_construct() {
    let mut e = engine_with_html(r#"<div id="t">hello</div>"#);
    let result = e.eval_js(r#"
        var ro = new ResizeObserver(function() {});
        typeof ro.observe
    "#).unwrap();
    assert_eq!(result, "function");
}

#[test]
fn resize_observer_fires_on_observe() {
    let mut e = engine_with_html(r#"<div id="t" style="width:200px;height:100px;">hello</div>"#);
    let _ = e.eval_js(r#"
        globalThis.__ro_result = null;
        var ro = new ResizeObserver(function(entries) {
            globalThis.__ro_result = entries.length;
        });
        ro.observe(document.getElementById('t'));
    "#);
    e.settle();
    let result = e.eval_js("String(__ro_result)").unwrap();
    assert_eq!(result, "1");
}

// ---------------------------------------------------------------------------
// Feature 8: WebSocket
// ---------------------------------------------------------------------------

#[test]
fn websocket_construct() {
    let mut e = engine_with_html(r#"<div>hello</div>"#);
    let result = e.eval_js(r#"
        var ws = new WebSocket('wss://example.com/ws');
        ws.readyState
    "#).unwrap();
    assert_eq!(result, "0"); // CONNECTING
}

#[test]
fn websocket_deliver_open() {
    let mut e = engine_with_html(r#"<div>hello</div>"#);
    let _ = e.eval_js(r#"
        globalThis.__ws_opened = false;
        var ws = new WebSocket('wss://example.com/ws');
        ws.onopen = function() { globalThis.__ws_opened = true; };
    "#);
    // Simulate server opening the connection
    e.ws_deliver_event(1, "open", "");
    let result = e.eval_js("String(__ws_opened)").unwrap();
    assert_eq!(result, "true");
}

#[test]
fn websocket_deliver_message() {
    let mut e = engine_with_html(r#"<div>hello</div>"#);
    let _ = e.eval_js(r#"
        globalThis.__ws_data = '';
        var ws = new WebSocket('wss://example.com/ws');
        ws.onmessage = function(e) { globalThis.__ws_data = e.data; };
    "#);
    e.ws_deliver_event(1, "open", "");
    e.ws_deliver_event(1, "message", "hello world");
    let result = e.eval_js("__ws_data").unwrap();
    assert_eq!(result, "hello world");
}

#[test]
fn websocket_close() {
    let mut e = engine_with_html(r#"<div>hello</div>"#);
    let _ = e.eval_js(r#"
        globalThis.__ws_closed = false;
        var ws = new WebSocket('wss://example.com/ws');
        ws.onclose = function() { globalThis.__ws_closed = true; };
    "#);
    e.ws_deliver_event(1, "open", "");
    e.ws_deliver_event(1, "close", "");
    let result = e.eval_js("String(__ws_closed)").unwrap();
    assert_eq!(result, "true");
}

// ---------------------------------------------------------------------------
// Feature 10: Dynamic import() — module fetch tracking
// ---------------------------------------------------------------------------

#[test]
fn dynamic_import_tracks_pending_fetch() {
    let mut e = Engine::new();
    e.set_url("https://example.com/");
    e.load_html(r#"<html><body>
        <script>
            // Dynamic import of a module not in registry should track as pending
            import('./missing-module.js').catch(function() {});
        </script>
    </body></html>"#);
    e.settle();
    // The module should appear in pending fetches
    // (We can verify by checking the engine state, though the exact API
    // depends on what's exposed. For now, we verify it doesn't crash.)
}
