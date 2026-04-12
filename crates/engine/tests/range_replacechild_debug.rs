use braille_engine::Engine;

#[test]
fn range_splittext_detached() {
    let mut engine = Engine::new();
    engine.load_html("<!doctype html><html><body></body></html>");

    let result = engine.eval_js(r#"
        var t = document.createTextNode("abcdefgh");
        var r = document.createRange();
        r.setStart(t, 1);
        r.setEnd(t, 3);
        var results = [];
        results.push("before: sc=" + (r.startContainer === t) + " so=" + r.startOffset + " ec=" + (r.endContainer === t) + " eo=" + r.endOffset);
        var newNode = t.splitText(1);
        results.push("after: sc=" + (r.startContainer === t) + " so=" + r.startOffset);
        results.push("after: ec=" + (r.endContainer === newNode) + " eo=" + r.endOffset);
        results.push("t.data=" + JSON.stringify(t.data) + " newNode.data=" + JSON.stringify(newNode.data));
        results.join("\n");
    "#).unwrap();
    eprintln!("{}", result);
}

#[test]
fn range_replacechild_self() {
    let mut engine = Engine::new();
    engine.load_html("<!doctype html><html><body><div id='testDiv'><p id='p0'>A</p><p id='p1'>B</p></div></body></html>");

    let result = engine.eval_js(r#"
        var testDiv = document.getElementById('testDiv');
        var p0 = document.getElementById('p0');
        var results = [];

        var r = document.createRange();
        r.setStart(p0, 0);
        r.setEnd(p0, 0);
        results.push("before: sc=" + (r.startContainer === p0) + " so=" + r.startOffset);

        testDiv.replaceChild(p0, p0);
        results.push("after: sc=" + (r.startContainer === p0) + " so=" + r.startOffset);
        results.push("sc.id=" + (r.startContainer.id || r.startContainer.nodeName));

        results.join("\n");
    "#).unwrap();
    eprintln!("{}", result);
}
