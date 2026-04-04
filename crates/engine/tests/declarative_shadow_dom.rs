use braille_engine::Engine;

#[test]
fn domparser_clonenode_with_doctype() {
    let mut engine = Engine::new();
    engine.load_html("<html><body></body></html>");

    let result = engine.eval_js(r#"
        var parser = new DOMParser();
        var doc = parser.parseFromString("<!DOCTYPE html><html></html>", "text/html");
        var clone = doc.cloneNode(true);
        'childNodes=' + clone.childNodes.length +
        ' doctype=' + (clone.doctype ? clone.doctype.name : 'null');
    "#).unwrap();
    eprintln!("DOMParser cloneNode result: {}", result);
}

#[test]
fn declarative_shadow_dom_via_innerhtml() {
    let mut engine = Engine::new();
    engine.load_html("<html><body></body></html>");

    let result = engine.eval_js(r#"
        var div = document.createElement('div');
        div.innerHTML = '<div><template shadowrootmode="open">test</template></div>';
        var inner = div.firstChild;
        inner.shadowRoot ? inner.shadowRoot.textContent : 'no shadowRoot';
    "#).unwrap();
    eprintln!("innerHTML result: {}", result);
    assert_eq!(result, "test");
}

#[test]
fn declarative_shadow_dom_via_document_write() {
    let mut engine = Engine::new();
    engine.load_html("<html><body></body></html>");

    let result = engine.eval_js(r#"
        var doc = document.cloneNode(document);
        doc.write('<div><template shadowrootmode=open>test</template></div>');
        var fc = doc.body.firstChild;
        fc.shadowRoot ? fc.shadowRoot.textContent : 'no shadowRoot';
    "#).unwrap();
    eprintln!("doc.write result: {}", result);
    assert_eq!(result, "test");
}

#[test]
fn declarative_shadow_dom_closed_mode() {
    let mut engine = Engine::new();
    engine.load_html("<html><body></body></html>");

    let result = engine.eval_js(r#"
        var div = document.createElement('div');
        div.innerHTML = '<div><template shadowrootmode="closed">secret</template></div>';
        var inner = div.firstChild;
        // Closed shadow roots should NOT be accessible via .shadowRoot
        inner.shadowRoot === null ? 'correctly null' : 'incorrectly exposed';
    "#).unwrap();
    eprintln!("closed mode result: {}", result);
    assert_eq!(result, "correctly null");
}
