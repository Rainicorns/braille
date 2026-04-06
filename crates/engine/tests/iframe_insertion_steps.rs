use braille_engine::Engine;

#[test]
fn iframe_load_fires_synchronously_on_append() {
    let html = r#"<!DOCTYPE html><html><head></head><body></body></html>"#;
    let mut engine = Engine::new();
    engine.load_html(html);
    engine.settle();
    let result = engine.eval_js(r#"
(function() {
    var fragment = document.createDocumentFragment();
    var iframe1 = document.createElement('iframe');
    var iframe2 = document.createElement('iframe');
    fragment.appendChild(iframe1);
    fragment.appendChild(iframe2);

    var iframe1Loaded = false, iframe2Loaded = false;
    iframe1.onload = function() {
        iframe1Loaded = true;
    };
    iframe2.onload = function() {
        iframe2Loaded = true;
    };

    document.body.append(fragment);

    var results = [];
    results.push('iframe1Loaded=' + iframe1Loaded);
    results.push('iframe2Loaded=' + iframe2Loaded);
    results.push('iframe1.contentWindow=' + (iframe1.contentWindow ? 'exists' : 'null'));
    results.push('iframe2.contentWindow=' + (iframe2.contentWindow ? 'exists' : 'null'));
    return results.join('; ');
})()
    "#);
    eprintln!("Result: {:?}", result);
    let r = result.unwrap();
    assert!(r.contains("iframe1Loaded=true"), "iframe1 should have loaded: {r}");
    assert!(r.contains("iframe2Loaded=true"), "iframe2 should have loaded: {r}");
}
