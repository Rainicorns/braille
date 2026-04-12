use braille_engine::Engine;

fn eval(engine: &mut Engine, code: &str) -> String {
    engine.eval_js(code).unwrap()
}

#[test]
fn window_instanceof_window() {
    let mut engine = Engine::new();
    engine.load_html("<!DOCTYPE html><html><body></body></html>");
    engine.settle();
    assert_eq!(eval(&mut engine, "window instanceof Window").trim(), "true");
}

#[test]
fn window_prototype_is_window_prototype() {
    let mut engine = Engine::new();
    engine.load_html("<!DOCTYPE html><html><body></body></html>");
    engine.settle();
    assert_eq!(
        eval(&mut engine, "Object.getPrototypeOf(window) === Window.prototype").trim(),
        "true"
    );
}

#[test]
fn window_instanceof_eventtarget() {
    let mut engine = Engine::new();
    engine.load_html("<!DOCTYPE html><html><body></body></html>");
    engine.settle();
    assert_eq!(
        eval(&mut engine, "window instanceof EventTarget").trim(),
        "true"
    );
}

#[test]
fn window_constructor_inherits_eventtarget() {
    let mut engine = Engine::new();
    engine.load_html("<!DOCTYPE html><html><body></body></html>");
    engine.settle();
    assert_eq!(
        eval(&mut engine, "Object.getPrototypeOf(Window) === EventTarget").trim(),
        "true"
    );
}

#[test]
fn window_prototype_chain_includes_window_properties() {
    let mut engine = Engine::new();
    engine.load_html("<!DOCTYPE html><html><body></body></html>");
    engine.settle();
    // Window.prototype → WindowProperties → EventTarget.prototype
    assert_eq!(
        eval(
            &mut engine,
            "Object.prototype.toString.call(Object.getPrototypeOf(Window.prototype))"
        )
        .trim(),
        "[object WindowProperties]"
    );
    assert_eq!(
        eval(
            &mut engine,
            "Object.getPrototypeOf(Object.getPrototypeOf(Window.prototype)) === EventTarget.prototype"
        )
        .trim(),
        "true"
    );
}

#[test]
fn immutable_prototype_window_setprototypeof_throws() {
    let mut engine = Engine::new();
    engine.load_html("<!DOCTYPE html><html><body></body></html>");
    engine.settle();
    assert_eq!(
        eval(
            &mut engine,
            r#"
            var threw = false;
            try { Object.setPrototypeOf(window, {}); } catch(e) { threw = e instanceof TypeError; }
            threw
        "#
        )
        .trim(),
        "true"
    );
}

#[test]
fn immutable_prototype_window_reflect_returns_false() {
    let mut engine = Engine::new();
    engine.load_html("<!DOCTYPE html><html><body></body></html>");
    engine.settle();
    assert_eq!(
        eval(&mut engine, "Reflect.setPrototypeOf(window, {})").trim(),
        "false"
    );
}

#[test]
fn immutable_prototype_window_same_value_ok() {
    let mut engine = Engine::new();
    engine.load_html("<!DOCTYPE html><html><body></body></html>");
    engine.settle();
    assert_eq!(
        eval(
            &mut engine,
            r#"
            var ok = true;
            try { Object.setPrototypeOf(window, Window.prototype); } catch(e) { ok = false; }
            ok
        "#
        )
        .trim(),
        "true"
    );
}

#[test]
fn immutable_prototype_window_prototype_setprototypeof_throws() {
    let mut engine = Engine::new();
    engine.load_html("<!DOCTYPE html><html><body></body></html>");
    engine.settle();
    assert_eq!(
        eval(
            &mut engine,
            r#"
            var threw = false;
            try { Object.setPrototypeOf(Window.prototype, {}); } catch(e) { threw = e instanceof TypeError; }
            threw
        "#
        )
        .trim(),
        "true"
    );
}

#[test]
fn window_stringification() {
    let mut engine = Engine::new();
    engine.load_html("<!DOCTYPE html><html><body></body></html>");
    engine.settle();
    let result = eval(&mut engine, "Object.prototype.toString.call(window)");
    eprintln!("window toString: {:?}", result);
    assert_eq!(result.trim(), "[object Window]");
}

#[test]
fn immutable_prototype_window_dunder_proto_throws() {
    let mut engine = Engine::new();
    engine.load_html("<!DOCTYPE html><html><body></body></html>");
    engine.settle();
    assert_eq!(
        eval(
            &mut engine,
            r#"
            var threw = false;
            try { window.__proto__ = {}; } catch(e) { threw = e instanceof TypeError; }
            threw
        "#
        )
        .trim(),
        "true"
    );
}
