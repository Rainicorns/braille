use braille_engine::Engine;

#[test]
fn scrollend_fires_on_mandatory_snap_after_unhide() {
    let mut engine = Engine::new();
    engine.load_html(r#"<!DOCTYPE html>
<html>
<head>
<style>
    #root {
        width: 400px;
        height: 400px;
        overflow: auto;
        scroll-snap-type: y mandatory;
    }
    .page {
        height: 400px;
        scroll-snap-align: start;
    }
    .hidden {
        display: none;
    }
</style>
</head>
<body>
<div id="root" class="hidden">
    <div class="page">Page 1</div>
    <div class="page">Page 2</div>
</div>
<script>
    window.__scrollendFired = false;
    var root = document.getElementById('root');
    root.addEventListener('scrollend', function(e) {
        window.__scrollendFired = true;
    });
    requestAnimationFrame(function() {
        root.classList.remove('hidden');
    });
</script>
</body>
</html>"#);

    engine.settle();

    let fired = engine.eval_js("String(window.__scrollendFired)").unwrap();
    assert_eq!(fired, "true", "scrollend should fire after unhiding mandatory snap container");
}
