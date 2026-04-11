use braille_engine::Engine;

#[test]
fn range_comparepoint_offset_check() {
    let mut engine = Engine::new();
    engine.load_html("<!doctype html><html><body><p>Hello</p></body></html>");

    let result = engine.eval_js(r#"
        var div = document.createElement('div');
        div.innerHTML = '<p>Short</p>';
        document.body.appendChild(div);
        var p = div.querySelector('p');
        var text = p.firstChild;

        var results = [];
        results.push("text.data: " + JSON.stringify(text.data));
        results.push("text.nodeType: " + text.nodeType);
        results.push("text.childNodes: " + text.childNodes);
        results.push("text.childNodes.length: " + text.childNodes.length);

        var r = document.createRange();
        r.selectNodeContents(div);
        try {
            var cp = r.comparePoint(text, 20);
            results.push("comparePoint(text, 20) = " + cp);
        } catch(e) {
            results.push("comparePoint threw: " + e.name + " - " + e.message);
        }

        // Test negative offset (WebIDL unsigned long)
        try {
            var cp2 = r.comparePoint(text, -1);
            results.push("comparePoint(text, -1) = " + cp2);
        } catch(e) {
            results.push("comparePoint(-1) threw: " + e.name + " - " + e.message);
        }

        results.join("\n");
    "#).unwrap();
    eprintln!("{}", result);
}

#[test]
fn range_comparepoint_cdata() {
    let mut engine = Engine::new();
    engine.load_html("<!doctype html><html><body><div id='testDiv'></div></body></html>");

    let result = engine.eval_js(r#"
        var testDiv = document.getElementById('testDiv');
        var p = document.createElement("p");
        var xmlDocument = new Document();
        var cdata1 = xmlDocument.createCDATASection("1234");
        p.appendChild(cdata1);
        p.appendChild(xmlDocument.createCDATASection("5678"));
        p.append("9012");
        testDiv.appendChild(p);

        var results = [];
        results.push("p.firstChild: " + p.firstChild);
        results.push("p.firstChild.nodeType: " + p.firstChild.nodeType);
        results.push("p.firstChild.data: " + JSON.stringify(p.firstChild.data));
        results.push("typeof p.firstChild.data: " + typeof p.firstChild.data);
        results.push("p.firstChild.childNodes: " + p.firstChild.childNodes);

        var r = document.createRange();
        r.selectNodeContents(testDiv);

        // Test comparePoint with CDATASection firstChild, offset 20
        try {
            var cp = r.comparePoint(p.firstChild, 20);
            results.push("comparePoint(cdata, 20) = " + cp);
        } catch(e) {
            results.push("comparePoint(cdata, 20) threw: " + e.name + " - " + e.message);
        }

        results.join("\n");
    "#).unwrap();
    eprintln!("{}", result);
}
