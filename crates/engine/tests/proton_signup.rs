//! Replay tests for Proton Mail signup page.
//! Recorded from live sessions with --record.

use braille_engine::transcript::ReplayFetcher;
use braille_engine::Engine;
use braille_wire::SnapMode;

#[test]
fn proton_signup_page_loads() {
    let mut fetcher = ReplayFetcher::load("tests/fixtures/proton_signup.json").unwrap();
    let mut engine = Engine::new();
    let snapshot = engine
        .navigate("https://account.proton.me/signup", &mut fetcher, SnapMode::Compact)
        .unwrap();

    assert!(snapshot.contains("Select your plan"), "missing plan selection:\n{snapshot}");
    assert!(snapshot.contains("Proton account"), "missing account form:\n{snapshot}");
    assert!(snapshot.contains("@proton.me"), "missing username field:\n{snapshot}");
    assert!(snapshot.contains("assword"), "missing password field:\n{snapshot}");
}

#[test]
fn proton_signup_select_free_plan() {
    let mut fetcher = ReplayFetcher::load("tests/fixtures/proton_signup.json").unwrap();
    let mut engine = Engine::new();
    let snapshot = engine
        .navigate("https://account.proton.me/signup", &mut fetcher, SnapMode::Accessibility)
        .unwrap();

    assert!(snapshot.contains("Free"), "no Free plan option");

    engine.handle_click("@e6");
    engine.settle();
    let snap2 = engine.snapshot(SnapMode::Accessibility);

    assert!(
        !snap2.contains("Credit/debit card"),
        "checkout section should be gone after selecting Free plan"
    );
}

/// Type username without fetch resolution — value sticks because React
/// doesn't re-render (no API responses to trigger state updates).
#[test]
fn proton_signup_type_username() {
    let mut fetcher = ReplayFetcher::load("tests/fixtures/proton_signup.json").unwrap();
    let mut engine = Engine::new();
    let _snapshot = engine
        .navigate("https://account.proton.me/signup", &mut fetcher, SnapMode::Accessibility)
        .unwrap();

    engine.handle_click("@e6");
    engine.settle();

    let result = engine.handle_type("@e11", "braille-test-bot");
    assert!(result.is_ok(), "type failed: {:?}", result);
    engine.settle();

    let forms = engine.snapshot(SnapMode::Forms);
    eprintln!("[proton] forms after type: {forms}");

    assert!(
        forms.contains("braille-test-bot"),
        "value didn't stick after type+settle.\nForms view:\n{forms}"
    );
}

/// Full daemon flow: navigate, click free plan with fetch resolution, then type.
/// This reproduces the live daemon behavior where React re-renders between interactions
/// due to API responses. Currently fails because our event dispatch doesn't bubble
/// to React's root container, so React's onChange never fires and React overwrites
/// our value with its stale internal state on re-render.
///
/// Known remaining issue: event delegation (React 18 listens on root container,
/// our dispatchEvent bubbles through the DOM tree but React's listener doesn't fire).
#[test]
#[ignore] // TODO: fix event delegation so React's root listener catches our events
fn proton_signup_type_with_full_fetch_resolution() {
    let mut fetcher = ReplayFetcher::load("tests/fixtures/proton_signup_full.json").unwrap();
    let mut engine = Engine::new();
    let _snapshot = engine
        .navigate("https://account.proton.me/signup", &mut fetcher, SnapMode::Compact)
        .unwrap();

    // Click @e6 twice (first reveals prices, second selects free plan)
    for _ in 1..=2 {
        engine.snapshot(SnapMode::Compact);
        engine.handle_click("@e6");
        engine.settle();
        engine.settle_with_fetches(&mut fetcher);
    }

    engine.snapshot(SnapMode::Compact);
    let result = engine.handle_type("@e11", "braille-test-bot");
    assert!(result.is_ok(), "type failed: {:?}", result);
    engine.settle();

    let forms = engine.snapshot(SnapMode::Forms);
    eprintln!("[proton] forms after type+settle: {forms}");

    assert!(
        forms.contains("braille-test-bot"),
        "React controlled input: value lost during settle.\nForms view:\n{forms}"
    );
}

/// Basic sanity: JS-created elements should be findable via querySelectorAll
#[test]
fn js_created_elements_queryable() {
    let mut engine = Engine::new();
    engine.load_html("<html><body><div id='root'></div></body></html>");

    let result = engine.eval_js(r#"(function() {
        var root = document.getElementById('root');
        root.innerHTML = '';
        var input = document.createElement('input');
        input.setAttribute('type', 'text');
        input.setAttribute('id', 'username');
        root.appendChild(input);
        var found = document.querySelectorAll('input');
        return 'found=' + found.length + ' byId=' + !!document.getElementById('username');
    })()"#).unwrap();

    assert!(result.contains("found=1"), "querySelectorAll didn't find JS-created input: {result}");
}

/// Simulate React-style re-render: removeChild old subtree, create new one
#[test]
fn react_style_rerender_queryable() {
    let mut engine = Engine::new();
    engine.load_html(r#"<html><body><div id='root'><form><input type='text' id='old-input' value='old'></form></div></body></html>"#);

    let before = engine.eval_js("document.querySelectorAll('input').length").unwrap();
    assert_eq!(before, "1");

    let result = engine.eval_js(r#"(function() {
        var root = document.getElementById('root');
        while (root.firstChild) root.removeChild(root.firstChild);
        var form = document.createElement('form');
        var input = document.createElement('input');
        input.setAttribute('type', 'text');
        input.setAttribute('id', 'new-input');
        input.setAttribute('value', 'new-value');
        form.appendChild(input);
        root.appendChild(form);
        var inputs = document.querySelectorAll('input');
        var byId = document.getElementById('new-input');
        return 'inputs=' + inputs.length + ' byId=' + !!byId + ' value=' + (byId ? byId.getAttribute('value') : 'null');
    })()"#).unwrap();

    assert!(result.contains("inputs=1"), "querySelectorAll didn't find new input: {result}");
    assert!(result.contains("value=new-value"), "new input value wrong: {result}");

    let snap = engine.snapshot(SnapMode::Forms);
    assert!(snap.contains("new-value"), "snapshot doesn't see new input value: {snap}");
}
