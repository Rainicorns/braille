//! Tests for settle loop properties: termination, idempotency, and timer behavior.
//!
//! The settle loop (Engine::settle / settle_inner) is the core scheduling mechanism:
//! flush microtasks, deliver MutationObserver records, fire ready timers, and
//! advance virtual time. These tests verify structural properties that must hold
//! regardless of specific content.

use braille_engine::Engine;
use braille_wire::SnapMode;

fn engine_with_html(html: &str) -> Engine {
    let mut e = Engine::new();
    e.load_html(html);
    e
}

// ===========================================================================
// Termination
// ===========================================================================

#[test]
fn settle_terminates_with_no_js() {
    let mut e = engine_with_html("<html><body><p>Static page</p></body></html>");
    // settle() should return immediately — no timers, no microtasks
    e.settle();
    let snap = e.snapshot(SnapMode::Text);
    assert!(snap.contains("Static page"));
}

#[test]
fn settle_terminates_with_resolved_promise() {
    let mut e = engine_with_html(r#"<html><body>
        <div id="result">waiting</div>
        <script>
            Promise.resolve().then(function() {
                document.getElementById('result').textContent = 'resolved';
            });
        </script>
    </body></html>"#);

    e.settle();
    let result = e.eval_js("document.getElementById('result').textContent").unwrap();
    assert_eq!(result, "resolved", "Promise.resolve should execute during settle");
}

#[test]
fn settle_terminates_with_set_timeout_zero() {
    let mut e = engine_with_html(r#"<html><body>
        <div id="result">waiting</div>
        <script>
            setTimeout(function() {
                document.getElementById('result').textContent = 'fired';
            }, 0);
        </script>
    </body></html>"#);

    e.settle();
    let result = e.eval_js("document.getElementById('result').textContent").unwrap();
    assert_eq!(result, "fired", "setTimeout(0) should fire during settle");
}

#[test]
fn settle_terminates_with_timer_bomb() {
    // A page that schedules 100 setTimeout(0) calls — settle should handle this
    // without hanging due to the 100-iteration cap
    let mut e = engine_with_html(r#"<html><body>
        <div id="count">0</div>
        <script>
            var count = 0;
            function tick() {
                count++;
                document.getElementById('count').textContent = String(count);
                if (count < 100) setTimeout(tick, 0);
            }
            setTimeout(tick, 0);
        </script>
    </body></html>"#);

    e.settle();
    // The settle loop has a 100-iteration cap and 1-second time budget,
    // so it should terminate even with many timers. The exact count depends
    // on how many iterations complete within the budget.
    let count = e.eval_js("document.getElementById('count').textContent").unwrap();
    let n: i32 = count.parse().unwrap_or(0);
    assert!(n > 0, "at least some timers should have fired: count={}", n);
}

// ===========================================================================
// Idempotency (settling a settled engine is a no-op)
// ===========================================================================

#[test]
fn settle_is_idempotent_on_static_page() {
    let mut e = engine_with_html(r#"<html><body>
        <p>Hello World</p>
        <style>p { color: red; }</style>
    </body></html>"#);

    let snap1 = e.snapshot(SnapMode::Text);
    e.settle();
    let snap2 = e.snapshot(SnapMode::Text);
    e.settle();
    let snap3 = e.snapshot(SnapMode::Text);

    assert_eq!(snap1, snap2, "settle should not change a static page");
    assert_eq!(snap2, snap3, "multiple settles should be identical");
}

#[test]
fn settle_is_idempotent_after_js_completes() {
    let mut e = engine_with_html(r#"<html><body>
        <div id="out">initial</div>
        <script>
            document.getElementById('out').textContent = 'modified';
        </script>
    </body></html>"#);

    e.settle();
    let snap1 = e.snapshot(SnapMode::Text);
    e.settle();
    let snap2 = e.snapshot(SnapMode::Text);

    assert_eq!(snap1, snap2, "second settle should not change anything");
    assert!(snap1.contains("modified"), "JS modification should be reflected");
}

#[test]
fn settle_is_idempotent_after_timeout_completes() {
    let mut e = engine_with_html(r#"<html><body>
        <div id="out">waiting</div>
        <script>
            setTimeout(function() {
                document.getElementById('out').textContent = 'done';
            }, 10);
        </script>
    </body></html>"#);

    e.settle();
    let snap1 = e.snapshot(SnapMode::Text);
    e.settle();
    let snap2 = e.snapshot(SnapMode::Text);

    assert_eq!(snap1, snap2, "settle after completed timeout should be no-op");
}

// ===========================================================================
// Timer Behavior
// ===========================================================================

#[test]
fn set_timeout_fires_in_order() {
    let mut e = engine_with_html(r#"<html><body>
        <div id="log"></div>
        <script>
            var log = document.getElementById('log');
            setTimeout(function() { log.textContent += 'A'; }, 10);
            setTimeout(function() { log.textContent += 'B'; }, 20);
            setTimeout(function() { log.textContent += 'C'; }, 30);
        </script>
    </body></html>"#);

    e.settle();
    let result = e.eval_js("document.getElementById('log').textContent").unwrap();
    assert_eq!(result, "ABC", "timers should fire in delay order: {}", result);
}

#[test]
fn set_interval_fires_during_settle() {
    let mut e = engine_with_html(r#"<html><body>
        <div id="ticks">0</div>
        <script>
            var ticks = 0;
            var intervalId = setInterval(function() {
                ticks++;
                document.getElementById('ticks').textContent = String(ticks);
                if (ticks >= 3) clearInterval(intervalId);
            }, 50);
        </script>
    </body></html>"#);

    e.settle();
    let result = e.eval_js("document.getElementById('ticks').textContent").unwrap();
    let n: i32 = result.parse().unwrap_or(0);
    assert!(n >= 3, "setInterval should fire at least 3 times before clearInterval: {}", n);
}

#[test]
fn request_animation_frame_fires_during_settle() {
    let mut e = engine_with_html(r#"<html><body>
        <div id="raf">waiting</div>
        <script>
            requestAnimationFrame(function() {
                document.getElementById('raf').textContent = 'painted';
            });
        </script>
    </body></html>"#);

    e.settle();
    let result = e.eval_js("document.getElementById('raf').textContent").unwrap();
    assert_eq!(result, "painted", "requestAnimationFrame should fire during settle: {}", result);
}

// ===========================================================================
// Settle With No Runtime
// ===========================================================================

#[test]
fn settle_with_no_runtime_does_not_panic() {
    let mut e = Engine::new();
    // No load_html called — no runtime exists
    e.settle();
    // Should not panic
}

// ===========================================================================
// Long-Running Timer Beyond Budget
// ===========================================================================

#[test]
fn settle_respects_time_budget() {
    // A timer at 5000ms should NOT fire during settle (budget is 1000ms)
    let mut e = engine_with_html(r#"<html><body>
        <div id="result">waiting</div>
        <script>
            setTimeout(function() {
                document.getElementById('result').textContent = 'fired';
            }, 5000);
        </script>
    </body></html>"#);

    e.settle();
    let result = e.eval_js("document.getElementById('result').textContent").unwrap();
    assert_eq!(result, "waiting", "5-second timer should NOT fire within 1-second budget: {}", result);
}

// ===========================================================================
// MutationObserver During Settle
// ===========================================================================

#[test]
fn mutation_observer_fires_during_settle() {
    let mut e = engine_with_html(r#"<html><body>
        <div id="target"></div>
        <div id="log"></div>
        <script>
            var log = document.getElementById('log');
            var observer = new MutationObserver(function(mutations) {
                for (var i = 0; i < mutations.length; i++) {
                    log.textContent += 'mutation:' + mutations[i].type + ';';
                }
            });
            observer.observe(document.getElementById('target'), { childList: true });

            // Trigger a mutation
            document.getElementById('target').appendChild(document.createElement('span'));
        </script>
    </body></html>"#);

    e.settle();
    let result = e.eval_js("document.getElementById('log').textContent").unwrap();
    assert!(result.contains("mutation:childList"),
        "MutationObserver should fire during settle: {}", result);
}
