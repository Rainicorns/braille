// alt_for: wpt:dom/nodes/moveBefore/hover-style-update.html
use braille_engine::Engine;

#[test]
fn hover_applies_css_styles() {
    let mut engine = Engine::new();
    engine.load_html(r#"<!DOCTYPE html>
<style>
  .item .controls { background-color: blue; }
  .item:hover .controls { background-color: red; }
</style>
<body>
<div class="item" id="itemA">
    <div class="controls" id="controlsA">A</div>
</div>
<script>
    window.__results = {};
    var item = document.getElementById('itemA');
    var controls = document.getElementById('controlsA');

    // Before hover
    window.__results.beforeHover = item.matches(':hover');

    // Set hover on itemA, then check after a frame
    requestAnimationFrame(function() {
        globalThis.__hoveredNode = item.__nid;
        __n_setHoveredNode(item.__nid);

        requestAnimationFrame(function() {
            window.__results.afterHover = item.matches(':hover');
            window.__results.afterBg = getComputedStyle(controls).backgroundColor;

            // Clear hover
            globalThis.__hoveredNode = -1;
            __n_setHoveredNode(-1);

            requestAnimationFrame(function() {
                window.__results.clearedHover = item.matches(':hover');
                window.__results.clearedBg = getComputedStyle(controls).backgroundColor;
            });
        });
    });
</script>
</body>"#);
    engine.settle();

    let r = engine.eval_js("JSON.stringify(window.__results)").unwrap();
    eprintln!("results: {}", r);

    let results: serde_json::Value = serde_json::from_str(&r).unwrap();
    assert_eq!(results["beforeHover"], false, "should not be hovered initially");
    assert_eq!(results["afterHover"], true, "should be hovered after setting __hoveredNode");
    assert_eq!(results["afterBg"], "rgb(255, 0, 0)", "hover CSS should apply red background");
    assert_eq!(results["clearedHover"], false, "should not be hovered after clearing");
    assert_eq!(results["clearedBg"], "rgb(0, 0, 255)", "should revert to blue after clearing hover");
}

#[test]
fn hover_matches_on_ancestors() {
    let mut engine = Engine::new();
    engine.load_html(r#"<!DOCTYPE html>
<body>
<div id="grandparent">
    <div id="parent">
        <div id="child">Hello</div>
    </div>
</div>
<script>
    var c = document.getElementById('child');
    var p = document.getElementById('parent');
    var gp = document.getElementById('grandparent');

    // Hover the child
    globalThis.__hoveredNode = c.__nid;
    __n_setHoveredNode(c.__nid);

    window.__results = {
        childHover: c.matches(':hover'),
        parentHover: p.matches(':hover'),
        grandparentHover: gp.matches(':hover'),
        bodyHover: document.body.matches(':hover'),
    };
</script>
</body>"#);
    engine.settle();

    let r = engine.eval_js("JSON.stringify(window.__results)").unwrap();
    eprintln!("results: {}", r);

    let results: serde_json::Value = serde_json::from_str(&r).unwrap();
    assert_eq!(results["childHover"], true, "hovered element should match :hover");
    assert_eq!(results["parentHover"], true, "parent of hovered element should match :hover");
    assert_eq!(results["grandparentHover"], true, "grandparent should match :hover");
    assert_eq!(results["bodyHover"], true, "body should match :hover");
}
