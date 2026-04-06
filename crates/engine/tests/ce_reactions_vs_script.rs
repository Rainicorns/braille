use braille_engine::Engine;

#[test]
fn ce_reactions_follow_script_execution() {
    let html = r#"<!DOCTYPE html><html><head></head><body></body></html>"#;
    let mut engine = Engine::new();
    engine.load_html(html);
    engine.settle();
    let result = engine.eval_js(r#"
(function() {
    window.__test_results = [];
    var results = window.__test_results;
    class Script1 extends HTMLScriptElement {
        constructor() { super(); }
        connectedCallback() { results.push("ce connected s1"); }
    }
    class Script2 extends HTMLScriptElement {
        constructor() { super(); }
        connectedCallback() { results.push("ce connected s2"); }
    }
    customElements.define("script-1", Script1, { extends: "script" });
    customElements.define("script-2", Script2, { extends: "script" });
    var s1 = new Script1();
    s1.textContent = "window.__test_results.push('s1')";
    var s2 = new Script2();
    s2.textContent = "window.__test_results.push('s2')";
    document.body.append(s1, s2);
    return JSON.stringify(results);
})()
    "#);
    eprintln!("Result: {:?}", result);
    assert_eq!(result.unwrap(), r#"["s1","s2","ce connected s1","ce connected s2"]"#);
}
