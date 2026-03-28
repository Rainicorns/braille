use braille_engine::Engine;

#[test]
fn create_attribute_exists() {
    let mut engine = Engine::new();
    engine.load_html("<html><body></body></html>");
    let result = engine.eval_js("typeof document.createAttribute").unwrap();
    assert_eq!(result, "function", "document.createAttribute should exist");
}

#[test]
fn create_attribute_returns_attr_node() {
    let mut engine = Engine::new();
    engine.load_html("<html><body></body></html>");
    let result = engine.eval_js("var a = document.createAttribute('test'); a.nodeType").unwrap();
    assert_eq!(result, "2", "Attr nodeType should be 2");
}

#[test]
fn attr_instanceof_node() {
    let mut engine = Engine::new();
    engine.load_html("<html><body></body></html>");
    let result = engine.eval_js("document.createAttribute('x') instanceof Node").unwrap();
    assert_eq!(result, "true", "Attr should be instanceof Node");
}

#[test]
fn attr_prototype_instanceof_node() {
    let mut engine = Engine::new();
    engine.load_html("<html><body></body></html>");
    let result = engine.eval_js("Attr.prototype instanceof Node").unwrap();
    assert_eq!(result, "true", "Attr.prototype should be instanceof Node");
}

#[test]
fn append_attr_throws() {
    let mut engine = Engine::new();
    engine.load_html("<html><body></body></html>");
    let result = engine.eval_js(r#"
        var p = document.createElement('p');
        var a = document.createAttribute('foo');
        try { p.appendChild(a); 'no error'; } catch(e) { e.constructor.name + ':' + e.name; }
    "#).unwrap();
    assert!(result.contains("HierarchyRequestError"), "appendChild(attr) should throw HierarchyRequestError, got: {result}");
}
