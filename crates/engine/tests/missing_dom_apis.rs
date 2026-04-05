//! Isolated tests for known missing DOM APIs.
//! Each test exercises one specific API gap. Red = the feature is missing.
//! When the feature is implemented and the test goes green, that's real progress.

use braille_engine::Engine;

fn engine_with_html(html: &str) -> Engine {
    let mut e = Engine::new();
    e.load_html(html);
    e
}

// ---------------------------------------------------------------------------
// HTMLSelectElement.add()
// Found via: Python docs switchers.js calls select.add(option) to build
// a version selector dropdown. Throws "not a function".
// ---------------------------------------------------------------------------

#[test]
fn select_add_appends_option() {
    let mut e = engine_with_html("<html><body><select id='s'></select></body></html>");
    let result = e.eval_js(r#"
        var sel = document.getElementById('s');
        var opt = document.createElement('option');
        opt.value = 'py3';
        opt.text = 'Python 3';
        sel.add(opt);
        sel.options.length + ',' + sel.options[0].value + ',' + sel.options[0].text;
    "#).unwrap();
    assert_eq!(result, "1,py3,Python 3");
}

#[test]
fn select_add_multiple_options_preserves_order() {
    let mut e = engine_with_html("<html><body><select id='s'></select></body></html>");
    let result = e.eval_js(r#"
        var sel = document.getElementById('s');
        var versions = ['3.12', '3.11', '3.10'];
        for (var i = 0; i < versions.length; i++) {
            var opt = document.createElement('option');
            opt.value = versions[i];
            opt.text = 'Python ' + versions[i];
            sel.add(opt);
        }
        sel.options.length + ',' + sel.options[0].value + ',' + sel.options[2].value;
    "#).unwrap();
    assert_eq!(result, "3,3.12,3.10");
}

#[test]
fn select_add_with_before_parameter() {
    let mut e = engine_with_html("<html><body><select id='s'></select></body></html>");
    let result = e.eval_js(r#"
        var sel = document.getElementById('s');
        var opt1 = document.createElement('option');
        opt1.value = 'a';
        sel.add(opt1);
        var opt2 = document.createElement('option');
        opt2.value = 'b';
        sel.add(opt2, opt1);
        sel.options[0].value + ',' + sel.options[1].value;
    "#).unwrap();
    assert_eq!(result, "b,a");
}

// ---------------------------------------------------------------------------
// getComputedStyle returns correct defaults
// Found via: WPT test 1416 expects getComputedStyle(div).display === "block"
// ---------------------------------------------------------------------------

#[test]
fn get_computed_style_display_block_for_div() {
    let mut e = engine_with_html("<html><body><div id='d'>hello</div></body></html>");
    let result = e.eval_js(r#"
        getComputedStyle(document.getElementById('d')).display;
    "#).unwrap();
    assert_eq!(result, "block");
}

#[test]
fn get_computed_style_display_inline_for_span() {
    let mut e = engine_with_html("<html><body><span id='s'>hello</span></body></html>");
    let result = e.eval_js(r#"
        getComputedStyle(document.getElementById('s')).display;
    "#).unwrap();
    assert_eq!(result, "inline");
}
