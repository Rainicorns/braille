use braille_engine::Engine;

#[test]
fn crash_with_manual_flush() {
    let mut engine = Engine::new();
    engine.load_html(r#"<!DOCTYPE html>
<progress id="p"><iframe id="i"></iframe>
<script>
var p = document.getElementById("p");
i.contentDocument.adoptNode(p);
// Force a microtask flush between adopt and append
Promise.resolve().then(function() {
    document.body.appendChild(p);
});
</script>"#);
    assert_eq!(engine.eval_js("'ok'").unwrap(), "ok");
}

#[test]
fn crash_minimal() {
    // Minimal: just adopt then immediately append in same script
    let mut engine = Engine::new();
    engine.load_html(r#"<!DOCTYPE html>
<progress id="p"><iframe id="i"></iframe>
<script>
i.contentDocument.adoptNode(p);
document.body.appendChild(p);
</script>"#);
    assert_eq!(engine.eval_js("'ok'").unwrap(), "ok");
}
