use braille_engine::Engine;

#[test]
fn domexception_not_error_subtype() {
    let mut e = Engine::new();
    e.load_html("<html><body></body></html>");
    let r = e.eval_js("DOMException.prototype instanceof Error").unwrap();
    eprintln!("DOMException.prototype instanceof Error = {}", r);
    assert_eq!(r, "false", "DOMException should NOT extend Error per spec");
}
