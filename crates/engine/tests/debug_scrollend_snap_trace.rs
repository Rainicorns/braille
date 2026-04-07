use braille_engine::Engine;

#[test]
fn trace_scrollend_snap_subtest2() {
    let mut engine = Engine::new();

    // Minimal version of the scrollend-event-fired-after-snap test
    // Focus on subtest 2 flow
    engine.load_html(r#"<!DOCTYPE html>
<style>
#scroller {
  width: 500px; height: 500px; overflow: scroll;
  scroll-snap-type: both mandatory;
  position: absolute;
}
#space { width: 2000px; height: 2000px; }
.target { width: 200px; height: 200px; scroll-snap-align: start; position: absolute; }
</style>
<div id="scroller">
  <div id="space"></div>
  <div class="target" style="left:0;top:0;"></div>
  <div class="target" style="left:80px;top:80px;"></div>
  <div class="target" style="left:200px;top:200px;"></div>
</div>
<script>
globalThis.__eventLog = [];
var scroller = document.getElementById("scroller");
scroller.addEventListener("scroll", function() {
    __eventLog.push("scroll(scrollTop=" + scroller.scrollTop + ",scroll_end_arrived=" + globalThis.__scroll_end_arrived + ")");
});
scroller.addEventListener("scrollend", function() {
    __eventLog.push("scrollend(scrollTop=" + scroller.scrollTop + ")");
});
globalThis.__scroll_end_arrived = false;
globalThis.__scroll_arrived_after_scroll_end = false;
</script>"#);

    // Simulate subtest 1: scroll down
    engine.eval_js(r#"
        scroller.scrollTo(0, 50);
    "#).unwrap();
    engine.settle();

    eprintln!("After subtest1 scroll+settle: log={}", engine.eval_js("JSON.stringify(__eventLog)").unwrap());
    eprintln!("scrollTop={}", engine.eval_js("scroller.scrollTop").unwrap());

    // Now simulate subtest 2 reset: scrollTo(0,0), then reset flags, then scroll again
    engine.eval_js(r#"
        __eventLog = [];
        __scroll_end_arrived = false;
        __scroll_arrived_after_scroll_end = false;

        // Setup listeners like the test
        scroller.addEventListener("scroll", function() {
            if (__scroll_end_arrived) __scroll_arrived_after_scroll_end = true;
        });
        scroller.addEventListener("scrollend", function() {
            __scroll_end_arrived = true;
        });

        __eventLog.push("=== scrollTo(0,0) ===");
        scroller.scrollTo(0, 0);
        __eventLog.push("=== after scrollTo, before rAF ===");
        __eventLog.push("scroll_end_arrived=" + __scroll_end_arrived);
        __eventLog.push("scroll_arrived_after=" + __scroll_arrived_after_scroll_end);
    "#).unwrap();

    eprintln!("After scrollTo(0,0): log={}", engine.eval_js("JSON.stringify(__eventLog)").unwrap());

    // Reset flags like the test does after waitForCompositorCommit
    engine.eval_js(r#"
        __eventLog.push("=== resetting flags ===");
        __scroll_end_arrived = false;
        __scroll_arrived_after_scroll_end = false;
    "#).unwrap();

    // Simulate the fling: scroll down 50px
    engine.eval_js(r#"
        __eventLog.push("=== fling scroll ===");
        scroller.scrollTo(0, 50);
        __eventLog.push("=== after fling ===");
        __eventLog.push("scroll_end_arrived=" + __scroll_end_arrived);
        __eventLog.push("scroll_arrived_after=" + __scroll_arrived_after_scroll_end);
    "#).unwrap();

    eprintln!("After fling: log={}", engine.eval_js("JSON.stringify(__eventLog)").unwrap());

    // Now settle (this is where snap fires)
    engine.settle();

    eprintln!("After settle: log={}", engine.eval_js("JSON.stringify(__eventLog)").unwrap());
    eprintln!("scroll_end_arrived={}", engine.eval_js("String(__scroll_end_arrived)").unwrap());
    eprintln!("scroll_arrived_after={}", engine.eval_js("String(__scroll_arrived_after_scroll_end)").unwrap());
    eprintln!("scrollTop={}", engine.eval_js("scroller.scrollTop").unwrap());
}
