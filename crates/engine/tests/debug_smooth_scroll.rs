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

#[test]
fn debug_scroll_snap_values() {
    let mut engine = Engine::new();
    let html = r#"<!DOCTYPE html>
<html><body>
<style>
.scroller {
    scroll-snap-type: x mandatory;
    overflow-x: auto;
    overflow-y: hidden;
    position: relative;
    height: 500px;
    width: 500px;
}
.box {
    scroll-snap-align: start;
    width: 400px;
    position: absolute;
    top: 200px;
}
#box1 { background-color: red; height: 500px; }
#box2 { background-color: yellow; height: 300px; left: 700.5px; }
#box3 { background-color: blue; height: 100px; left: 1400px; }
</style>
<div id="scroller" class="scroller">
    <div class="box" id="box1">1</div>
    <div class="box" id="box2">2</div>
    <div class="box" id="box3">3</div>
</div>
<script>
window.__d = [];
var sc = document.getElementById('scroller');
var b1 = document.getElementById('box1');
var b2 = document.getElementById('box2');
var b3 = document.getElementById('box3');
__d.push('scroller rect: ' + JSON.stringify(sc.getBoundingClientRect()));
__d.push('box1 rect: ' + JSON.stringify(b1.getBoundingClientRect()));
__d.push('box2 rect: ' + JSON.stringify(b2.getBoundingClientRect()));
__d.push('box3 rect: ' + JSON.stringify(b3.getBoundingClientRect()));
__d.push('box2.offsetLeft: ' + b2.offsetLeft);
__d.push('scrollWidth: ' + sc.scrollWidth);
__d.push('clientWidth: ' + sc.clientWidth);
__d.push('maxScroll: ' + (sc.scrollWidth - sc.clientWidth));

var scrollendCount = 0;
sc.addEventListener('scrollend', function() { scrollendCount++; __d.push('scrollend#' + scrollendCount + ' scrollLeft=' + sc.scrollLeft); });

__d.push('box2 computed left: ' + getComputedStyle(b2).left);
__d.push('box2 computed top: ' + getComputedStyle(b2).top);
__d.push('box2 computed position: ' + getComputedStyle(b2).position);
__d.push('scroller computed snap-type: ' + getComputedStyle(sc).getPropertyValue('scroll-snap-type'));
__d.push('box2 computed snap-align: ' + getComputedStyle(b2).getPropertyValue('scroll-snap-align'));

// Replicate the WPT test flow
var expected_scroll_left = b2.offsetLeft;
var target_offset = b2.offsetLeft + b2.clientWidth / 2;
__d.push('expected_scroll_left: ' + expected_scroll_left);
__d.push('target_offset: ' + target_offset);

// promise_test style: async/await
var scrollendReceived = false;
var scrollendPromise = new Promise(function(resolve, reject) {
    var timeout = setTimeout(function() {
        reject('No scrollend received in 500ms');
    }, 500);
    sc.addEventListener('scrollend', function(evt) {
        clearTimeout(timeout);
        scrollendReceived = true;
        __d.push('scrollend received! scrollLeft=' + sc.scrollLeft);
        resolve(evt);
    }, { once: true });
});

sc.scrollTo({ left: target_offset });
__d.push('after scrollTo(' + target_offset + ') scrollLeft: ' + sc.scrollLeft);
__d.push('scrollendReceived sync: ' + scrollendReceived);

scrollendPromise.then(function() {
    __d.push('promise resolved, scrollLeft=' + sc.scrollLeft);
}).catch(function(e) {
    __d.push('promise rejected: ' + e);
});
</script>
</body></html>"#;
    engine.load_html(html);
    engine.settle();

    let debug = engine.eval_js("__d.join('\\n')").unwrap();
    eprintln!("=== SNAP DEBUG ===\n{}", debug);
}
