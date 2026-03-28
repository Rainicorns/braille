use crate::dom::DomTree;
use crate::js::runtime::JsRuntime;
use std::cell::RefCell;
use std::rc::Rc;

fn make_runtime() -> JsRuntime {
    let tree = Rc::new(RefCell::new(DomTree::new()));
    {
        let mut t = tree.borrow_mut();
        let html = t.create_element("html");
        let body = t.create_element("body");
        let doc = t.document();
        t.append_child(doc, html);
        t.append_child(html, body);
    }
    JsRuntime::new(tree)
}

fn validity_field(rt: &mut JsRuntime, setup: &str, field: &str) -> String {
    rt.eval(setup).unwrap();
    rt.eval_to_string(&format!(
        "String(document.querySelector('input').validity.{field})"
    ))
    .unwrap()
}

// -----------------------------------------------------------------------
// number
// -----------------------------------------------------------------------

#[test]
fn number_valid_value() {
    let mut rt = make_runtime();
    let v = validity_field(
        &mut rt,
        r#"var i = document.createElement('input'); i.setAttribute('type','number'); i.setAttribute('value','42'); document.body.appendChild(i);"#,
        "valid",
    );
    assert_eq!(v, "true");
}

#[test]
fn number_bad_input() {
    let mut rt = make_runtime();
    let v = validity_field(
        &mut rt,
        r#"var i = document.createElement('input'); i.setAttribute('type','number'); i.setAttribute('value','abc'); document.body.appendChild(i);"#,
        "badInput",
    );
    assert_eq!(v, "true");
}

#[test]
fn number_range_underflow() {
    let mut rt = make_runtime();
    let v = validity_field(
        &mut rt,
        r#"var i = document.createElement('input'); i.setAttribute('type','number'); i.setAttribute('min','10'); i.setAttribute('value','5'); document.body.appendChild(i);"#,
        "rangeUnderflow",
    );
    assert_eq!(v, "true");
}

#[test]
fn number_range_overflow() {
    let mut rt = make_runtime();
    let v = validity_field(
        &mut rt,
        r#"var i = document.createElement('input'); i.setAttribute('type','number'); i.setAttribute('max','10'); i.setAttribute('value','15'); document.body.appendChild(i);"#,
        "rangeOverflow",
    );
    assert_eq!(v, "true");
}

#[test]
fn number_step_mismatch() {
    let mut rt = make_runtime();
    let v = validity_field(
        &mut rt,
        r#"var i = document.createElement('input'); i.setAttribute('type','number'); i.setAttribute('step','3'); i.setAttribute('min','0'); i.setAttribute('value','5'); document.body.appendChild(i);"#,
        "stepMismatch",
    );
    assert_eq!(v, "true");
}

#[test]
fn number_step_valid() {
    let mut rt = make_runtime();
    let v = validity_field(
        &mut rt,
        r#"var i = document.createElement('input'); i.setAttribute('type','number'); i.setAttribute('step','3'); i.setAttribute('min','0'); i.setAttribute('value','6'); document.body.appendChild(i);"#,
        "stepMismatch",
    );
    assert_eq!(v, "false");
}

#[test]
fn number_step_any_skips_check() {
    let mut rt = make_runtime();
    let v = validity_field(
        &mut rt,
        r#"var i = document.createElement('input'); i.setAttribute('type','number'); i.setAttribute('step','any'); i.setAttribute('min','0'); i.setAttribute('value','5.5'); document.body.appendChild(i);"#,
        "stepMismatch",
    );
    assert_eq!(v, "false");
}

// -----------------------------------------------------------------------
// range
// -----------------------------------------------------------------------

#[test]
fn range_valid_value() {
    let mut rt = make_runtime();
    let v = validity_field(
        &mut rt,
        r#"var i = document.createElement('input'); i.setAttribute('type','range'); i.setAttribute('value','50'); document.body.appendChild(i);"#,
        "valid",
    );
    assert_eq!(v, "true");
}

#[test]
fn range_underflow_default_min() {
    let mut rt = make_runtime();
    let v = validity_field(
        &mut rt,
        r#"var i = document.createElement('input'); i.setAttribute('type','range'); i.setAttribute('value','-1'); document.body.appendChild(i);"#,
        "rangeUnderflow",
    );
    assert_eq!(v, "true");
}

#[test]
fn range_overflow_default_max() {
    let mut rt = make_runtime();
    let v = validity_field(
        &mut rt,
        r#"var i = document.createElement('input'); i.setAttribute('type','range'); i.setAttribute('value','101'); document.body.appendChild(i);"#,
        "rangeOverflow",
    );
    assert_eq!(v, "true");
}

#[test]
fn range_step_mismatch() {
    let mut rt = make_runtime();
    let v = validity_field(
        &mut rt,
        r#"var i = document.createElement('input'); i.setAttribute('type','range'); i.setAttribute('value','5.5'); document.body.appendChild(i);"#,
        "stepMismatch",
    );
    assert_eq!(v, "true");
}

// -----------------------------------------------------------------------
// date
// -----------------------------------------------------------------------

#[test]
fn date_valid() {
    let mut rt = make_runtime();
    let v = validity_field(
        &mut rt,
        r#"var i = document.createElement('input'); i.setAttribute('type','date'); i.setAttribute('value','2024-01-15'); document.body.appendChild(i);"#,
        "valid",
    );
    assert_eq!(v, "true");
}

#[test]
fn date_bad_format() {
    let mut rt = make_runtime();
    let v = validity_field(
        &mut rt,
        r#"var i = document.createElement('input'); i.setAttribute('type','date'); i.setAttribute('value','01-15-2024'); document.body.appendChild(i);"#,
        "badInput",
    );
    assert_eq!(v, "true");
}

#[test]
fn date_range_underflow() {
    let mut rt = make_runtime();
    let v = validity_field(
        &mut rt,
        r#"var i = document.createElement('input'); i.setAttribute('type','date'); i.setAttribute('min','2024-06-01'); i.setAttribute('value','2024-01-15'); document.body.appendChild(i);"#,
        "rangeUnderflow",
    );
    assert_eq!(v, "true");
}

#[test]
fn date_range_overflow() {
    let mut rt = make_runtime();
    let v = validity_field(
        &mut rt,
        r#"var i = document.createElement('input'); i.setAttribute('type','date'); i.setAttribute('max','2024-06-01'); i.setAttribute('value','2024-12-15'); document.body.appendChild(i);"#,
        "rangeOverflow",
    );
    assert_eq!(v, "true");
}

// -----------------------------------------------------------------------
// time
// -----------------------------------------------------------------------

#[test]
fn time_valid_hhmm() {
    let mut rt = make_runtime();
    let v = validity_field(
        &mut rt,
        r#"var i = document.createElement('input'); i.setAttribute('type','time'); i.setAttribute('value','14:30'); document.body.appendChild(i);"#,
        "valid",
    );
    assert_eq!(v, "true");
}

#[test]
fn time_valid_hhmmss() {
    let mut rt = make_runtime();
    let v = validity_field(
        &mut rt,
        r#"var i = document.createElement('input'); i.setAttribute('type','time'); i.setAttribute('value','14:30:45'); document.body.appendChild(i);"#,
        "valid",
    );
    assert_eq!(v, "true");
}

#[test]
fn time_bad_format() {
    let mut rt = make_runtime();
    let v = validity_field(
        &mut rt,
        r#"var i = document.createElement('input'); i.setAttribute('type','time'); i.setAttribute('value','2:30pm'); document.body.appendChild(i);"#,
        "badInput",
    );
    assert_eq!(v, "true");
}

#[test]
fn time_range_underflow() {
    let mut rt = make_runtime();
    let v = validity_field(
        &mut rt,
        r#"var i = document.createElement('input'); i.setAttribute('type','time'); i.setAttribute('min','09:00'); i.setAttribute('value','08:00'); document.body.appendChild(i);"#,
        "rangeUnderflow",
    );
    assert_eq!(v, "true");
}

#[test]
fn time_range_overflow() {
    let mut rt = make_runtime();
    let v = validity_field(
        &mut rt,
        r#"var i = document.createElement('input'); i.setAttribute('type','time'); i.setAttribute('max','17:00'); i.setAttribute('value','18:00'); document.body.appendChild(i);"#,
        "rangeOverflow",
    );
    assert_eq!(v, "true");
}

// -----------------------------------------------------------------------
// datetime-local
// -----------------------------------------------------------------------

#[test]
fn datetime_local_valid() {
    let mut rt = make_runtime();
    let v = validity_field(
        &mut rt,
        r#"var i = document.createElement('input'); i.setAttribute('type','datetime-local'); i.setAttribute('value','2024-01-15T14:30'); document.body.appendChild(i);"#,
        "valid",
    );
    assert_eq!(v, "true");
}

#[test]
fn datetime_local_bad_format() {
    let mut rt = make_runtime();
    let v = validity_field(
        &mut rt,
        r#"var i = document.createElement('input'); i.setAttribute('type','datetime-local'); i.setAttribute('value','2024-01-15 14:30'); document.body.appendChild(i);"#,
        "badInput",
    );
    assert_eq!(v, "true");
}

#[test]
fn datetime_local_range_underflow() {
    let mut rt = make_runtime();
    let v = validity_field(
        &mut rt,
        r#"var i = document.createElement('input'); i.setAttribute('type','datetime-local'); i.setAttribute('min','2024-06-01T00:00'); i.setAttribute('value','2024-01-15T14:30'); document.body.appendChild(i);"#,
        "rangeUnderflow",
    );
    assert_eq!(v, "true");
}

#[test]
fn datetime_local_range_overflow() {
    let mut rt = make_runtime();
    let v = validity_field(
        &mut rt,
        r#"var i = document.createElement('input'); i.setAttribute('type','datetime-local'); i.setAttribute('max','2024-06-01T00:00'); i.setAttribute('value','2024-12-15T14:30'); document.body.appendChild(i);"#,
        "rangeOverflow",
    );
    assert_eq!(v, "true");
}

// -----------------------------------------------------------------------
// month
// -----------------------------------------------------------------------

#[test]
fn month_valid() {
    let mut rt = make_runtime();
    let v = validity_field(
        &mut rt,
        r#"var i = document.createElement('input'); i.setAttribute('type','month'); i.setAttribute('value','2024-01'); document.body.appendChild(i);"#,
        "valid",
    );
    assert_eq!(v, "true");
}

#[test]
fn month_bad_format() {
    let mut rt = make_runtime();
    let v = validity_field(
        &mut rt,
        r#"var i = document.createElement('input'); i.setAttribute('type','month'); i.setAttribute('value','Jan 2024'); document.body.appendChild(i);"#,
        "badInput",
    );
    assert_eq!(v, "true");
}

#[test]
fn month_range_underflow() {
    let mut rt = make_runtime();
    let v = validity_field(
        &mut rt,
        r#"var i = document.createElement('input'); i.setAttribute('type','month'); i.setAttribute('min','2024-06'); i.setAttribute('value','2024-01'); document.body.appendChild(i);"#,
        "rangeUnderflow",
    );
    assert_eq!(v, "true");
}

#[test]
fn month_range_overflow() {
    let mut rt = make_runtime();
    let v = validity_field(
        &mut rt,
        r#"var i = document.createElement('input'); i.setAttribute('type','month'); i.setAttribute('max','2024-06'); i.setAttribute('value','2024-12'); document.body.appendChild(i);"#,
        "rangeOverflow",
    );
    assert_eq!(v, "true");
}

// -----------------------------------------------------------------------
// week
// -----------------------------------------------------------------------

#[test]
fn week_valid() {
    let mut rt = make_runtime();
    let v = validity_field(
        &mut rt,
        r#"var i = document.createElement('input'); i.setAttribute('type','week'); i.setAttribute('value','2024-W03'); document.body.appendChild(i);"#,
        "valid",
    );
    assert_eq!(v, "true");
}

#[test]
fn week_bad_format() {
    let mut rt = make_runtime();
    let v = validity_field(
        &mut rt,
        r#"var i = document.createElement('input'); i.setAttribute('type','week'); i.setAttribute('value','2024-3'); document.body.appendChild(i);"#,
        "badInput",
    );
    assert_eq!(v, "true");
}

#[test]
fn week_range_underflow() {
    let mut rt = make_runtime();
    let v = validity_field(
        &mut rt,
        r#"var i = document.createElement('input'); i.setAttribute('type','week'); i.setAttribute('min','2024-W20'); i.setAttribute('value','2024-W03'); document.body.appendChild(i);"#,
        "rangeUnderflow",
    );
    assert_eq!(v, "true");
}

#[test]
fn week_range_overflow() {
    let mut rt = make_runtime();
    let v = validity_field(
        &mut rt,
        r#"var i = document.createElement('input'); i.setAttribute('type','week'); i.setAttribute('max','2024-W20'); i.setAttribute('value','2024-W40'); document.body.appendChild(i);"#,
        "rangeOverflow",
    );
    assert_eq!(v, "true");
}

// -----------------------------------------------------------------------
// color
// -----------------------------------------------------------------------

#[test]
fn color_valid() {
    let mut rt = make_runtime();
    let v = validity_field(
        &mut rt,
        r#"var i = document.createElement('input'); i.setAttribute('type','color'); i.setAttribute('value','#ff0000'); document.body.appendChild(i);"#,
        "valid",
    );
    assert_eq!(v, "true");
}

#[test]
fn color_bad_input_short_hex() {
    let mut rt = make_runtime();
    let v = validity_field(
        &mut rt,
        r#"var i = document.createElement('input'); i.setAttribute('type','color'); i.setAttribute('value','#fff'); document.body.appendChild(i);"#,
        "badInput",
    );
    assert_eq!(v, "true");
}

#[test]
fn color_bad_input_no_hash() {
    let mut rt = make_runtime();
    let v = validity_field(
        &mut rt,
        r#"var i = document.createElement('input'); i.setAttribute('type','color'); i.setAttribute('value','ff0000'); document.body.appendChild(i);"#,
        "badInput",
    );
    assert_eq!(v, "true");
}

#[test]
fn color_bad_input_invalid_chars() {
    let mut rt = make_runtime();
    let v = validity_field(
        &mut rt,
        r#"var i = document.createElement('input'); i.setAttribute('type','color'); i.setAttribute('value','#gggggg'); document.body.appendChild(i);"#,
        "badInput",
    );
    assert_eq!(v, "true");
}

// -----------------------------------------------------------------------
// empty values should not trigger type-specific validation
// -----------------------------------------------------------------------

#[test]
fn number_empty_is_valid() {
    let mut rt = make_runtime();
    let v = validity_field(
        &mut rt,
        r#"var i = document.createElement('input'); i.setAttribute('type','number'); document.body.appendChild(i);"#,
        "valid",
    );
    assert_eq!(v, "true");
}

#[test]
fn date_empty_is_valid() {
    let mut rt = make_runtime();
    let v = validity_field(
        &mut rt,
        r#"var i = document.createElement('input'); i.setAttribute('type','date'); document.body.appendChild(i);"#,
        "valid",
    );
    assert_eq!(v, "true");
}

use crate::Engine;

// NOTE: Per HTML spec, programmatically setting .value does NOT fire an input event.
// Only user interaction (typing) should fire input events. The property/attribute
// separation branch (worktree-agent-a778440a) correctly removed this behavior.

#[test]
fn invalid_event_fires_on_check_validity() {
    let mut engine = Engine::new();
    engine.load_html(r#"<html><body>
        <input id="i" type="text" required>
        <script>
            window.__invalidFired = false;
            window.__invalidBubbles = null;
            window.__invalidCancelable = null;
            document.getElementById('i').addEventListener('invalid', function(e) {
                window.__invalidFired = true;
                window.__invalidBubbles = e.bubbles;
                window.__invalidCancelable = e.cancelable;
            });
        </script>
    </body></html>"#);

    // checkValidity on a required empty input should fire invalid
    let result = engine.eval_js("document.getElementById('i').checkValidity()").unwrap();
    assert_eq!(result, "false", "checkValidity should return false for empty required input");

    let fired = engine.eval_js("window.__invalidFired").unwrap();
    assert_eq!(fired, "true", "invalid event should fire when checkValidity fails");
    let bubbles = engine.eval_js("window.__invalidBubbles").unwrap();
    assert_eq!(bubbles, "false", "invalid event should NOT bubble");
    let cancelable = engine.eval_js("window.__invalidCancelable").unwrap();
    assert_eq!(cancelable, "true", "invalid event should be cancelable");
}

#[test]
fn invalid_event_does_not_fire_on_valid_input() {
    let mut engine = Engine::new();
    engine.load_html(r#"<html><body>
        <input id="i" type="text" required value="filled">
        <script>
            window.__invalidFired = false;
            document.getElementById('i').addEventListener('invalid', function() {
                window.__invalidFired = true;
            });
        </script>
    </body></html>"#);

    let result = engine.eval_js("document.getElementById('i').checkValidity()").unwrap();
    assert_eq!(result, "true", "checkValidity should return true for filled required input");

    let fired = engine.eval_js("window.__invalidFired").unwrap();
    assert_eq!(fired, "false", "invalid event should NOT fire when input is valid");
}

#[test]
fn invalid_event_fires_with_custom_validity() {
    let mut engine = Engine::new();
    engine.load_html(r#"<html><body>
        <input id="i" type="text">
        <script>
            window.__invalidFired = false;
            var el = document.getElementById('i');
            el.setCustomValidity('custom error');
            el.addEventListener('invalid', function() {
                window.__invalidFired = true;
            });
        </script>
    </body></html>"#);

    let result = engine.eval_js("document.getElementById('i').checkValidity()").unwrap();
    assert_eq!(result, "false", "checkValidity should return false with custom validity");

    let fired = engine.eval_js("window.__invalidFired").unwrap();
    assert_eq!(fired, "true", "invalid event should fire with setCustomValidity");
}

// -- form element properties --

// -- form.enctype --

#[test]
fn form_enctype_defaults_to_urlencoded() {
    let mut engine = Engine::new();
    engine.load_html(r#"<html><body><form id="f"></form></body></html>"#);
    let runtime = engine.runtime.as_mut().unwrap();
    let result = runtime.eval_to_string(r#"document.getElementById("f").enctype"#).unwrap();
    assert_eq!(result, "application/x-www-form-urlencoded");
}

#[test]
fn form_enctype_returns_valid_value() {
    let mut engine = Engine::new();
    engine.load_html(r#"<html><body><form id="f" enctype="multipart/form-data"></form></body></html>"#);
    let runtime = engine.runtime.as_mut().unwrap();
    let result = runtime.eval_to_string(r#"document.getElementById("f").enctype"#).unwrap();
    assert_eq!(result, "multipart/form-data");
}

#[test]
fn form_enctype_invalid_falls_back_to_default() {
    let mut engine = Engine::new();
    engine.load_html(r#"<html><body><form id="f" enctype="bogus"></form></body></html>"#);
    let runtime = engine.runtime.as_mut().unwrap();
    let result = runtime.eval_to_string(r#"document.getElementById("f").enctype"#).unwrap();
    assert_eq!(result, "application/x-www-form-urlencoded");
}

#[test]
fn form_enctype_setter_updates_attribute() {
    let mut engine = Engine::new();
    engine.load_html(r#"<html><body><form id="f"></form></body></html>"#);
    let runtime = engine.runtime.as_mut().unwrap();
    runtime.eval(r#"document.getElementById("f").enctype = "text/plain""#).unwrap();
    let result = runtime.eval_to_string(r#"document.getElementById("f").enctype"#).unwrap();
    assert_eq!(result, "text/plain");
}

// -- form.encoding (alias for enctype) --

#[test]
fn form_encoding_is_alias_for_enctype() {
    let mut engine = Engine::new();
    engine.load_html(r#"<html><body><form id="f" enctype="multipart/form-data"></form></body></html>"#);
    let runtime = engine.runtime.as_mut().unwrap();
    let result = runtime.eval_to_string(r#"document.getElementById("f").encoding"#).unwrap();
    assert_eq!(result, "multipart/form-data");
}

#[test]
fn form_encoding_setter_updates_enctype() {
    let mut engine = Engine::new();
    engine.load_html(r#"<html><body><form id="f"></form></body></html>"#);
    let runtime = engine.runtime.as_mut().unwrap();
    runtime.eval(r#"document.getElementById("f").encoding = "text/plain""#).unwrap();
    let result = runtime.eval_to_string(r#"document.getElementById("f").enctype"#).unwrap();
    assert_eq!(result, "text/plain");
}

// -- form.noValidate --

#[test]
fn form_no_validate_false_when_absent() {
    let mut engine = Engine::new();
    engine.load_html(r#"<html><body><form id="f"></form></body></html>"#);
    let runtime = engine.runtime.as_mut().unwrap();
    let result = runtime.eval_to_string(r#"document.getElementById("f").noValidate"#).unwrap();
    assert_eq!(result, "false");
}

#[test]
fn form_no_validate_true_when_present() {
    let mut engine = Engine::new();
    engine.load_html(r#"<html><body><form id="f" novalidate></form></body></html>"#);
    let runtime = engine.runtime.as_mut().unwrap();
    let result = runtime.eval_to_string(r#"document.getElementById("f").noValidate"#).unwrap();
    assert_eq!(result, "true");
}

#[test]
fn form_no_validate_setter_adds_attribute() {
    let mut engine = Engine::new();
    engine.load_html(r#"<html><body><form id="f"></form></body></html>"#);
    let runtime = engine.runtime.as_mut().unwrap();
    runtime.eval(r#"document.getElementById("f").noValidate = true"#).unwrap();
    let result = runtime.eval_to_string(r#"document.getElementById("f").noValidate"#).unwrap();
    assert_eq!(result, "true");
}

#[test]
fn form_no_validate_setter_removes_attribute() {
    let mut engine = Engine::new();
    engine.load_html(r#"<html><body><form id="f" novalidate></form></body></html>"#);
    let runtime = engine.runtime.as_mut().unwrap();
    runtime.eval(r#"document.getElementById("f").noValidate = false"#).unwrap();
    let result = runtime.eval_to_string(r#"document.getElementById("f").noValidate"#).unwrap();
    assert_eq!(result, "false");
}

// -- form.target --

#[test]
fn form_target_defaults_to_empty() {
    let mut engine = Engine::new();
    engine.load_html(r#"<html><body><form id="f"></form></body></html>"#);
    let runtime = engine.runtime.as_mut().unwrap();
    let result = runtime.eval_to_string(r#"document.getElementById("f").target"#).unwrap();
    assert_eq!(result, "");
}

#[test]
fn form_target_getter_returns_attribute() {
    let mut engine = Engine::new();
    engine.load_html(r#"<html><body><form id="f" target="_blank"></form></body></html>"#);
    let runtime = engine.runtime.as_mut().unwrap();
    let result = runtime.eval_to_string(r#"document.getElementById("f").target"#).unwrap();
    assert_eq!(result, "_blank");
}

#[test]
fn form_target_setter_updates_attribute() {
    let mut engine = Engine::new();
    engine.load_html(r#"<html><body><form id="f"></form></body></html>"#);
    let runtime = engine.runtime.as_mut().unwrap();
    runtime.eval(r#"document.getElementById("f").target = "_self""#).unwrap();
    let result = runtime.eval_to_string(r#"document.getElementById("f").target"#).unwrap();
    assert_eq!(result, "_self");
}

// -- form.acceptCharset --

#[test]
fn form_accept_charset_defaults_to_empty() {
    let mut engine = Engine::new();
    engine.load_html(r#"<html><body><form id="f"></form></body></html>"#);
    let runtime = engine.runtime.as_mut().unwrap();
    let result = runtime.eval_to_string(r#"document.getElementById("f").acceptCharset"#).unwrap();
    assert_eq!(result, "");
}

#[test]
fn form_accept_charset_getter_returns_attribute() {
    let mut engine = Engine::new();
    engine.load_html(r#"<html><body><form id="f" accept-charset="UTF-8"></form></body></html>"#);
    let runtime = engine.runtime.as_mut().unwrap();
    let result = runtime.eval_to_string(r#"document.getElementById("f").acceptCharset"#).unwrap();
    assert_eq!(result, "UTF-8");
}

#[test]
fn form_accept_charset_setter_updates_attribute() {
    let mut engine = Engine::new();
    engine.load_html(r#"<html><body><form id="f"></form></body></html>"#);
    let runtime = engine.runtime.as_mut().unwrap();
    runtime.eval(r#"document.getElementById("f").acceptCharset = "ISO-8859-1""#).unwrap();
    let result = runtime.eval_to_string(r#"document.getElementById("f").acceptCharset"#).unwrap();
    assert_eq!(result, "ISO-8859-1");
}

// -- form.autocomplete --

#[test]
fn form_autocomplete_defaults_to_on() {
    let mut engine = Engine::new();
    engine.load_html(r#"<html><body><form id="f"></form></body></html>"#);
    let runtime = engine.runtime.as_mut().unwrap();
    let result = runtime.eval_to_string(r#"document.getElementById("f").autocomplete"#).unwrap();
    assert_eq!(result, "on");
}

#[test]
fn form_autocomplete_getter_returns_attribute() {
    let mut engine = Engine::new();
    engine.load_html(r#"<html><body><form id="f" autocomplete="off"></form></body></html>"#);
    let runtime = engine.runtime.as_mut().unwrap();
    let result = runtime.eval_to_string(r#"document.getElementById("f").autocomplete"#).unwrap();
    assert_eq!(result, "off");
}

#[test]
fn form_autocomplete_setter_updates_attribute() {
    let mut engine = Engine::new();
    engine.load_html(r#"<html><body><form id="f"></form></body></html>"#);
    let runtime = engine.runtime.as_mut().unwrap();
    runtime.eval(r#"document.getElementById("f").autocomplete = "off""#).unwrap();
    let result = runtime.eval_to_string(r#"document.getElementById("f").autocomplete"#).unwrap();
    assert_eq!(result, "off");
}

// -- form.length --

#[test]
fn form_length_returns_number_of_controls() {
    let mut engine = Engine::new();
    engine.load_html(r#"<html><body><form id="f"><input type="text"><select><option>A</option></select><textarea></textarea><button>Go</button><div>Not interactive</div></form></body></html>"#);
    let runtime = engine.runtime.as_mut().unwrap();
    let result = runtime.eval_to_string(r#"document.getElementById("f").length"#).unwrap();
    assert_eq!(result, "4");
}

#[test]
fn form_length_returns_zero_for_empty_form() {
    let mut engine = Engine::new();
    engine.load_html(r#"<html><body><form id="f"><p>No controls here</p></form></body></html>"#);
    let runtime = engine.runtime.as_mut().unwrap();
    let result = runtime.eval_to_string(r#"document.getElementById("f").length"#).unwrap();
    assert_eq!(result, "0");
}
