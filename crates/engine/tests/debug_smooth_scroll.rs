use braille_engine::Engine;

#[test]
fn smooth_scroll_input_basic() {
    let mut engine = Engine::new();
    let html = r#"<!DOCTYPE html>
<style>
  #inp { width: 100px; height: 50px; }
</style>
<input type="text" id="inp" value="qwertyuiopasddfghjklzxcvbnmqwertyuiopasddfghjklzxcvbnmqwer">
<script>
var inp = document.getElementById('inp');
window.__debug = [];
window.__debug.push('scrollWidth=' + inp.scrollWidth);
window.__debug.push('clientWidth=' + inp.clientWidth);
window.__debug.push('initialScrollLeft=' + inp.scrollLeft);

var scrollendCount = 0;
inp.addEventListener('scrollend', function() {
    scrollendCount++;
    window.__debug.push('scrollend#' + scrollendCount + ' scrollLeft=' + inp.scrollLeft);
});

inp.scrollTo({ left: 10, behavior: 'smooth' });
window.__debug.push('afterScrollTo scrollLeft=' + inp.scrollLeft);
</script>"#;
    engine.load_html(html);
    engine.settle();

    let debug = engine.eval_js("__debug.join('\\n')").unwrap();
    eprintln!("=== DEBUG ===\n{}", debug);

    let final_sl = engine.eval_js("document.getElementById('inp').scrollLeft").unwrap();
    eprintln!("Final scrollLeft: {}", final_sl);
    assert_eq!(final_sl, "10");
}

/// Reproduces the cleanup-scrollend-leak bug: when cleanup's scrollend fires
/// via setTimeout(0), it can be caught by the next subtest's listener if cleanup
/// promises aren't properly awaited.
#[test]
fn smooth_scroll_input_cleanup_leak() {
    let mut engine = Engine::new();
    let html = r#"<!DOCTYPE html>
<style>
  #inp { width: 100px; height: 50px; }
</style>
<input type="text" id="inp" value="qwertyuiopasddfghjklzxcvbnmqwertyuiopasddfghjklzxcvbnmqwer">
<script>
var inp = document.getElementById('inp');
window.__debug = [];

// Simulate: previous subtest left scrollLeft=10, cleanup resets to 0
inp.scrollLeft = 10;

// Wait for the cleanup scrollend to fire, then start "next subtest"
// This is the race condition: cleanup scrollend via setTimeout(0)
// can leak into the next test's scrollend listener.
var cleanupScrollendFired = false;

// Register cleanup: set scrollLeft=0, wait for scrollend
var cleanupPromise = new Promise(function(resolve) {
    inp.addEventListener('scrollend', function() {
        cleanupScrollendFired = true;
        window.__debug.push('cleanup scrollend fired, scrollLeft=' + inp.scrollLeft);
        resolve();
    }, { once: true });
    inp.scrollLeft = 0;
});

// Simulate the bug: start next subtest WITHOUT awaiting cleanup promise
var nextSubtestScrollendPromise = new Promise(function(resolve) {
    inp.addEventListener('scrollend', function() {
        window.__debug.push('subtest scrollend fired, scrollLeft=' + inp.scrollLeft);
        resolve();
    }, { once: true });
});

inp.scrollTo({ left: 10, behavior: 'smooth' });
window.__debug.push('started smooth scrollTo, cleanupScrollendFired=' + cleanupScrollendFired);

Promise.all([cleanupPromise, nextSubtestScrollendPromise]).then(function() {
    window.__debug.push('final scrollLeft=' + inp.scrollLeft);
});
</script>"#;
    engine.load_html(html);
    engine.settle();

    let debug = engine.eval_js("__debug.join('\\n')").unwrap();
    eprintln!("=== CLEANUP LEAK DEBUG ===\n{}", debug);

    let final_sl = engine.eval_js("document.getElementById('inp').scrollLeft").unwrap();
    eprintln!("Final scrollLeft: {}", final_sl);
    assert_eq!(final_sl, "10");
}
