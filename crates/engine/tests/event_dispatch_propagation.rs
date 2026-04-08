//! Tests for DOM event dispatch: capture, target, bubble phases,
//! stopPropagation, event.target vs event.currentTarget, and
//! event listener ordering. Real web patterns that frameworks rely on.

use braille_engine::Engine;

fn engine_with_html(html: &str) -> Engine {
    let mut e = Engine::new();
    e.load_html(html);
    e
}

// ===========================================================================
// Capture → Target → Bubble Ordering
// ===========================================================================

#[test]
fn event_phases_fire_in_correct_order() {
    let mut e = engine_with_html(r#"<html><body>
        <div id="outer">
            <div id="inner">
                <button id="btn">Click</button>
            </div>
        </div>
        <div id="log"></div>
        <script>
            var log = document.getElementById('log');
            var outer = document.getElementById('outer');
            var inner = document.getElementById('inner');
            var btn = document.getElementById('btn');

            // Capture phase listeners
            outer.addEventListener('click', function(e) {
                log.textContent += 'outer-capture(' + e.eventPhase + ');';
            }, true);
            inner.addEventListener('click', function(e) {
                log.textContent += 'inner-capture(' + e.eventPhase + ');';
            }, true);

            // Bubble phase listeners
            outer.addEventListener('click', function(e) {
                log.textContent += 'outer-bubble(' + e.eventPhase + ');';
            }, false);
            inner.addEventListener('click', function(e) {
                log.textContent += 'inner-bubble(' + e.eventPhase + ');';
            }, false);

            // Target listeners
            btn.addEventListener('click', function(e) {
                log.textContent += 'target(' + e.eventPhase + ');';
            }, false);
        </script>
    </body></html>"#);

    e.handle_click("#btn");
    e.settle();

    let result = e.eval_js("document.getElementById('log').textContent").unwrap();

    // eventPhase: 1=CAPTURING_PHASE, 2=AT_TARGET, 3=BUBBLING_PHASE
    // Expected order: outer-capture(1) → inner-capture(1) → target(2) → inner-bubble(3) → outer-bubble(3)
    assert!(result.contains("outer-capture"),
        "outer capture should fire: {}", result);
    assert!(result.contains("target"),
        "target phase should fire: {}", result);
    assert!(result.contains("outer-bubble"),
        "outer bubble should fire: {}", result);

    // Verify ordering: capture before target before bubble
    if let (Some(cap_pos), Some(tgt_pos), Some(bub_pos)) = (
        result.find("outer-capture"),
        result.find("target("),
        result.find("outer-bubble"),
    ) {
        assert!(cap_pos < tgt_pos, "capture should come before target");
        assert!(tgt_pos < bub_pos, "target should come before bubble");
    }
}

// ===========================================================================
// stopPropagation
// ===========================================================================

#[test]
fn stop_propagation_prevents_bubble() {
    let mut e = engine_with_html(r#"<html><body>
        <div id="outer">
            <button id="btn">Click</button>
        </div>
        <div id="log"></div>
        <script>
            var log = document.getElementById('log');

            document.getElementById('btn').addEventListener('click', function(e) {
                log.textContent += 'target;';
                e.stopPropagation();
            });

            document.getElementById('outer').addEventListener('click', function(e) {
                log.textContent += 'outer-bubble;';
            });
        </script>
    </body></html>"#);

    e.handle_click("#btn");
    e.settle();

    let result = e.eval_js("document.getElementById('log').textContent").unwrap();
    assert!(result.contains("target"), "target handler should fire: {}", result);
    assert!(!result.contains("outer-bubble"),
        "stopPropagation should prevent bubble to outer: {}", result);
}

#[test]
fn stop_propagation_in_capture_prevents_target() {
    let mut e = engine_with_html(r#"<html><body>
        <div id="outer">
            <button id="btn">Click</button>
        </div>
        <div id="log"></div>
        <script>
            var log = document.getElementById('log');

            document.getElementById('outer').addEventListener('click', function(e) {
                log.textContent += 'capture-stop;';
                e.stopPropagation();
            }, true);

            document.getElementById('btn').addEventListener('click', function(e) {
                log.textContent += 'target;';
            });
        </script>
    </body></html>"#);

    e.handle_click("#btn");
    e.settle();

    let result = e.eval_js("document.getElementById('log').textContent").unwrap();
    assert!(result.contains("capture-stop"), "capture handler should fire: {}", result);
    assert!(!result.contains("target"),
        "stopPropagation in capture should prevent target: {}", result);
}

// ===========================================================================
// stopImmediatePropagation
// ===========================================================================

#[test]
fn stop_immediate_propagation_prevents_sibling_listeners() {
    let mut e = engine_with_html(r#"<html><body>
        <button id="btn">Click</button>
        <div id="log"></div>
        <script>
            var log = document.getElementById('log');
            var btn = document.getElementById('btn');

            btn.addEventListener('click', function(e) {
                log.textContent += 'first;';
                e.stopImmediatePropagation();
            });

            btn.addEventListener('click', function(e) {
                log.textContent += 'second;';
            });
        </script>
    </body></html>"#);

    e.handle_click("#btn");
    e.settle();

    let result = e.eval_js("document.getElementById('log').textContent").unwrap();
    assert!(result.contains("first"), "first handler should fire: {}", result);
    assert!(!result.contains("second"),
        "stopImmediatePropagation should prevent second handler: {}", result);
}

// ===========================================================================
// event.target vs event.currentTarget
// ===========================================================================

#[test]
fn event_target_is_clicked_element() {
    let mut e = engine_with_html(r#"<html><body>
        <div id="outer">
            <span id="inner">Click me</span>
        </div>
        <div id="log"></div>
        <script>
            var log = document.getElementById('log');

            document.getElementById('outer').addEventListener('click', function(e) {
                log.textContent = 'target=' + e.target.id + ',currentTarget=' + e.currentTarget.id;
            });
        </script>
    </body></html>"#);

    e.handle_click("#inner");
    e.settle();

    let result = e.eval_js("document.getElementById('log').textContent").unwrap();
    assert!(result.contains("target=inner"),
        "event.target should be the clicked element (inner): {}", result);
    assert!(result.contains("currentTarget=outer"),
        "event.currentTarget should be the listener element (outer): {}", result);
}

// ===========================================================================
// Event Delegation Pattern
// ===========================================================================

#[test]
fn event_delegation_works() {
    // Common React/framework pattern: single listener on parent, dispatch by target
    let mut e = engine_with_html(r#"<html><body>
        <ul id="list">
            <li id="item1">Item 1</li>
            <li id="item2">Item 2</li>
            <li id="item3">Item 3</li>
        </ul>
        <div id="selected">none</div>
        <script>
            document.getElementById('list').addEventListener('click', function(e) {
                if (e.target.tagName === 'LI') {
                    document.getElementById('selected').textContent = e.target.id;
                }
            });
        </script>
    </body></html>"#);

    e.handle_click("#item2");
    e.settle();

    let result = e.eval_js("document.getElementById('selected').textContent").unwrap();
    assert_eq!(result, "item2", "event delegation should identify correct target: {}", result);
}

// ===========================================================================
// preventDefault
// ===========================================================================

#[test]
fn prevent_default_sets_default_prevented() {
    let mut e = engine_with_html(r#"<html><body>
        <button id="btn">Click</button>
        <div id="result"></div>
        <script>
            document.getElementById('btn').addEventListener('click', function(e) {
                e.preventDefault();
                document.getElementById('result').textContent = String(e.defaultPrevented);
            });
        </script>
    </body></html>"#);

    e.handle_click("#btn");
    e.settle();

    let result = e.eval_js("document.getElementById('result').textContent").unwrap();
    assert_eq!(result, "true", "preventDefault should set defaultPrevented: {}", result);
}

// ===========================================================================
// Multiple Event Types
// ===========================================================================

#[test]
fn different_event_types_dispatch_independently() {
    let mut e = engine_with_html(r#"<html><body>
        <div id="target"></div>
        <div id="log"></div>
        <script>
            var log = document.getElementById('log');
            var target = document.getElementById('target');

            target.addEventListener('click', function() {
                log.textContent += 'click;';
            });
            target.addEventListener('mousedown', function() {
                log.textContent += 'mousedown;';
            });
            target.addEventListener('mouseup', function() {
                log.textContent += 'mouseup;';
            });

            // Dispatch click (which should fire mousedown, mouseup, click in real browsers,
            // but via element.click() it may only fire the click event)
            target.click();
        </script>
    </body></html>"#);

    e.settle();
    let result = e.eval_js("document.getElementById('log').textContent").unwrap();
    assert!(result.contains("click"), "click handler should fire: {}", result);
}

// ===========================================================================
// Custom Events
// ===========================================================================

#[test]
fn custom_event_dispatch_and_bubble() {
    let mut e = engine_with_html(r#"<html><body>
        <div id="outer">
            <div id="inner"></div>
        </div>
        <div id="log"></div>
        <script>
            var log = document.getElementById('log');

            document.getElementById('outer').addEventListener('my-event', function(e) {
                log.textContent += 'outer:' + e.detail + ';';
            });

            document.getElementById('inner').addEventListener('my-event', function(e) {
                log.textContent += 'inner:' + e.detail + ';';
            });

            var event = new CustomEvent('my-event', { bubbles: true, detail: 'hello' });
            document.getElementById('inner').dispatchEvent(event);
        </script>
    </body></html>"#);

    e.settle();
    let result = e.eval_js("document.getElementById('log').textContent").unwrap();
    assert!(result.contains("inner:hello"), "custom event should fire on target: {}", result);
    assert!(result.contains("outer:hello"), "custom event should bubble to parent: {}", result);
}

#[test]
fn custom_event_no_bubble_when_not_set() {
    let mut e = engine_with_html(r#"<html><body>
        <div id="outer">
            <div id="inner"></div>
        </div>
        <div id="log"></div>
        <script>
            var log = document.getElementById('log');

            document.getElementById('outer').addEventListener('my-event', function() {
                log.textContent += 'outer;';
            });

            document.getElementById('inner').addEventListener('my-event', function() {
                log.textContent += 'inner;';
            });

            // bubbles: false (default)
            var event = new CustomEvent('my-event', { detail: 'test' });
            document.getElementById('inner').dispatchEvent(event);
        </script>
    </body></html>"#);

    e.settle();
    let result = e.eval_js("document.getElementById('log').textContent").unwrap();
    assert!(result.contains("inner"), "non-bubbling event should fire on target: {}", result);
    assert!(!result.contains("outer"),
        "non-bubbling event should NOT reach parent: {}", result);
}
