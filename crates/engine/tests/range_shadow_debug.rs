use braille_engine::Engine;

#[test]
fn range_shadow_basic() {
    let mut engine = Engine::new();
    engine.load_html("<!doctype html><html><body></body></html>");

    let result = engine.eval_js(r#"
        var results = [];

        // Test attachShadow exists
        var host = document.createElement("div");
        results.push("attachShadow exists: " + (typeof host.attachShadow));

        // Test with open mode
        try {
            var root = host.attachShadow({mode: "open"});
            results.push("attachShadow open: OK, root=" + root);
            results.push("root.innerHTML setter exists: " + ("innerHTML" in root));
            root.innerHTML = '<div id="in-shadow">ABC</div>';
            results.push("root.firstChild: " + root.firstChild);
            results.push("root.firstChild.tagName: " + (root.firstChild && root.firstChild.tagName));
            document.body.appendChild(host);

            var range = document.createRange();
            try {
                range.setStart(root.firstChild, 1);
                results.push("setStart on shadow child: OK");
                results.push("range.startContainer: " + range.startContainer);
                results.push("range.startOffset: " + range.startOffset);
            } catch(e2) {
                results.push("setStart threw: " + e2.name + " - " + e2.message);
            }

            host.remove();
            results.push("After remove - startContainer: " + range.startContainer);
            results.push("After remove - startOffset: " + range.startOffset);
            results.push("startContainer === root.firstChild: " + (range.startContainer === root.firstChild));
        } catch(e) {
            results.push("attachShadow threw: " + e.name + " - " + e.message);
        }

        // Test load event registration
        results.push("typeof addEventListener: " + typeof addEventListener);
        results.push("addEventListener === window.addEventListener: " + (addEventListener === window.addEventListener));

        results.join("\n");
    "#).unwrap();
    eprintln!("{}", result);
}

#[test]
fn range_shadow_document_location() {
    let mut engine = Engine::new();
    engine.load_html("<!doctype html><html><body></body></html>");

    let result = engine.eval_js(r#"
        var results = [];
        results.push("typeof document.location: " + typeof document.location);
        results.push("document.location: " + document.location);
        try {
            results.push("document.location.search: " + document.location.search);
        } catch(e) {
            results.push("document.location.search threw: " + e.message);
        }
        results.join("\n");
    "#).unwrap();
    eprintln!("{}", result);
}
