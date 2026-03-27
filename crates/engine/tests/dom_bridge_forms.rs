//! DOM bridge form tests: labels, property/attribute separation, live collections, form association.
use braille_engine::Engine;


fn engine_with_html(html: &str) -> Engine {
    let mut e = Engine::new();
    e.load_html(html);
    e
}

// Label association: htmlFor, control, labels, click-on-label
// =========================================================================

#[test]
fn label_html_for_getter_reflects_for_attribute() {
    let mut e = engine_with_html(r#"<html><body>
        <label id="lbl" for="username">Name</label>
        <input id="username" />
    </body></html>"#);
    let result = e.eval_js(r#"document.getElementById('lbl').htmlFor"#).unwrap();
    assert_eq!(result, "username");
}

#[test]
fn label_html_for_getter_returns_empty_when_no_for() {
    let mut e = engine_with_html(r#"<html><body>
        <label id="lbl">Name <input /></label>
    </body></html>"#);
    let result = e.eval_js(r#"document.getElementById('lbl').htmlFor"#).unwrap();
    assert_eq!(result, "");
}

#[test]
fn label_html_for_setter_updates_for_attribute() {
    let mut e = engine_with_html(r#"<html><body>
        <label id="lbl">Name</label>
        <input id="email" />
    </body></html>"#);
    e.eval_js(r#"document.getElementById('lbl').htmlFor = 'email'"#).unwrap();
    let result = e.eval_js(r#"document.getElementById('lbl').getAttribute('for')"#).unwrap();
    assert_eq!(result, "email");
}

#[test]
fn label_html_for_undefined_on_non_label() {
    let mut e = engine_with_html(r#"<html><body><div id="d"></div></body></html>"#);
    let result = e.eval_js(r#"typeof document.getElementById('d').htmlFor"#).unwrap();
    assert_eq!(result, "undefined");
}

#[test]
fn label_control_returns_element_by_for_attribute() {
    let mut e = engine_with_html(r#"<html><body>
        <label id="lbl" for="inp">Name</label>
        <input id="inp" />
    </body></html>"#);
    let result = e.eval_js(r#"document.getElementById('lbl').control.id"#).unwrap();
    assert_eq!(result, "inp");
}

#[test]
fn label_control_returns_first_labelable_descendant() {
    let mut e = engine_with_html(r#"<html><body>
        <label id="lbl">Name <span><input id="inner" /></span></label>
    </body></html>"#);
    let result = e.eval_js(r#"document.getElementById('lbl').control.id"#).unwrap();
    assert_eq!(result, "inner");
}

#[test]
fn label_control_returns_null_when_no_control_found() {
    let mut e = engine_with_html(r#"<html><body>
        <label id="lbl" for="nonexistent">Name</label>
    </body></html>"#);
    let result = e.eval_js(r#"document.getElementById('lbl').control === null"#).unwrap();
    assert_eq!(result, "true");
}

#[test]
fn label_control_returns_null_when_no_descendant() {
    let mut e = engine_with_html(r#"<html><body>
        <label id="lbl">Just text</label>
    </body></html>"#);
    let result = e.eval_js(r#"document.getElementById('lbl').control === null"#).unwrap();
    assert_eq!(result, "true");
}

#[test]
fn label_control_undefined_on_non_label() {
    let mut e = engine_with_html(r#"<html><body><div id="d"></div></body></html>"#);
    let result = e.eval_js(r#"typeof document.getElementById('d').control"#).unwrap();
    assert_eq!(result, "undefined");
}

#[test]
fn input_labels_returns_label_with_for_attribute() {
    let mut e = engine_with_html(r#"<html><body>
        <label id="lbl" for="inp">Name</label>
        <input id="inp" />
    </body></html>"#);
    let result = e.eval_js(r#"
        var labels = document.getElementById('inp').labels;
        labels.length + ':' + labels[0].id
    "#).unwrap();
    assert_eq!(result, "1:lbl");
}

#[test]
fn input_labels_returns_ancestor_label() {
    let mut e = engine_with_html(r#"<html><body>
        <label id="lbl">Name <input id="inp" /></label>
    </body></html>"#);
    let result = e.eval_js(r#"
        var labels = document.getElementById('inp').labels;
        labels.length + ':' + labels[0].id
    "#).unwrap();
    assert_eq!(result, "1:lbl");
}

#[test]
fn input_labels_returns_multiple_labels() {
    let mut e = engine_with_html(r#"<html><body>
        <label id="lbl1" for="inp">First</label>
        <label id="lbl2" for="inp">Second</label>
        <input id="inp" />
    </body></html>"#);
    let result = e.eval_js(r#"document.getElementById('inp').labels.length"#).unwrap();
    assert_eq!(result, "2");
}

#[test]
fn input_labels_returns_empty_for_no_labels() {
    let mut e = engine_with_html(r#"<html><body>
        <input id="inp" />
    </body></html>"#);
    let result = e.eval_js(r#"document.getElementById('inp').labels.length"#).unwrap();
    assert_eq!(result, "0");
}

#[test]
fn input_labels_undefined_on_non_labelable() {
    let mut e = engine_with_html(r#"<html><body><div id="d"></div></body></html>"#);
    let result = e.eval_js(r#"typeof document.getElementById('d').labels"#).unwrap();
    assert_eq!(result, "undefined");
}

#[test]
fn input_labels_works_for_select() {
    let mut e = engine_with_html(r#"<html><body>
        <label id="lbl" for="sel">Pick</label>
        <select id="sel"><option>A</option></select>
    </body></html>"#);
    let result = e.eval_js(r#"document.getElementById('sel').labels.length"#).unwrap();
    assert_eq!(result, "1");
}

#[test]
fn input_labels_works_for_textarea() {
    let mut e = engine_with_html(r#"<html><body>
        <label id="lbl" for="ta">Bio</label>
        <textarea id="ta"></textarea>
    </body></html>"#);
    let result = e.eval_js(r#"document.getElementById('ta').labels.length"#).unwrap();
    assert_eq!(result, "1");
}

#[test]
fn input_labels_works_for_button() {
    let mut e = engine_with_html(r#"<html><body>
        <label id="lbl" for="btn">Action</label>
        <button id="btn">Go</button>
    </body></html>"#);
    let result = e.eval_js(r#"document.getElementById('btn').labels.length"#).unwrap();
    assert_eq!(result, "1");
}

#[test]
fn click_on_label_activates_associated_control() {
    let mut e = engine_with_html(r#"<html><body>
        <label id="lbl" for="cb">Check me</label>
        <input id="cb" type="checkbox" />
    </body></html>"#);
    e.eval_js(r#"
        window.__clicked = false;
        document.getElementById('cb').addEventListener('click', function() {
            window.__clicked = true;
        });
    "#).unwrap();
    e.eval_js(r#"document.getElementById('lbl').click()"#).unwrap();
    let result = e.eval_js(r#"window.__clicked"#).unwrap();
    assert_eq!(result, "true");
}

// =========================================================================
// Property vs Attribute separation (HTML spec compliance)
// =========================================================================

#[test]
fn input_value_property_does_not_update_attribute() {
    // Per HTML spec: setting .value property should NOT change getAttribute('value')
    let mut e = engine_with_html(r#"<html><body><input id="i" /></body></html>"#);
    let result = e.eval_js(r#"
        var el = document.getElementById('i');
        el.value = 'hello';
        el.getAttribute('value')
    "#).unwrap();
    assert_eq!(result, "null", "getAttribute('value') should be null after setting .value property");
}

#[test]
fn input_value_property_preserves_existing_attribute() {
    // Setting .value should not change an existing value attribute
    let mut e = engine_with_html(r#"<html><body><input id="i" value="initial" /></body></html>"#);
    let result = e.eval_js(r#"
        var el = document.getElementById('i');
        el.value = 'changed';
        JSON.stringify([el.value, el.getAttribute('value')])
    "#).unwrap();
    assert_eq!(result, r#"["changed","initial"]"#);
}

#[test]
fn input_set_attribute_value_updates_property_when_not_dirty() {
    // setAttribute('value', ...) should update .value if the property hasn't been set directly
    let mut e = engine_with_html(r#"<html><body><input id="i" /></body></html>"#);
    let result = e.eval_js(r#"
        var el = document.getElementById('i');
        el.setAttribute('value', 'from-attr');
        el.value
    "#).unwrap();
    assert_eq!(result, "from-attr");
}

#[test]
fn input_set_attribute_value_does_not_override_dirty_property() {
    // setAttribute('value', ...) should NOT override .value if it was set directly (dirty)
    let mut e = engine_with_html(r#"<html><body><input id="i" /></body></html>"#);
    let result = e.eval_js(r#"
        var el = document.getElementById('i');
        el.value = 'dirty';
        el.setAttribute('value', 'from-attr');
        el.value
    "#).unwrap();
    assert_eq!(result, "dirty");
}

#[test]
fn input_default_value_reads_and_writes_attribute() {
    let mut e = engine_with_html(r#"<html><body><input id="i" value="initial" /></body></html>"#);
    // defaultValue reads the attribute
    let result = e.eval_js(r#"document.getElementById('i').defaultValue"#).unwrap();
    assert_eq!(result, "initial");
    // Setting defaultValue updates the attribute
    let result = e.eval_js(r#"
        var el = document.getElementById('i');
        el.defaultValue = 'new-default';
        el.getAttribute('value')
    "#).unwrap();
    assert_eq!(result, "new-default");
}

#[test]
fn input_default_checked_reads_and_writes_attribute() {
    let mut e = engine_with_html(r#"<html><body><input id="c" type="checkbox" /></body></html>"#);
    // defaultChecked initially false
    let result = e.eval_js(r#"document.getElementById('c').defaultChecked"#).unwrap();
    assert_eq!(result, "false");
    // Setting defaultChecked updates the attribute
    let result = e.eval_js(r#"
        var el = document.getElementById('c');
        el.defaultChecked = true;
        el.hasAttribute('checked')
    "#).unwrap();
    assert_eq!(result, "true");
}

#[test]
fn click_on_label_with_descendant_control_activates_it() {
    let mut e = engine_with_html(r#"<html><body>
        <label id="lbl">Check me <input id="cb" type="checkbox" /></label>
    </body></html>"#);
    e.eval_js(r#"
        window.__clicked = false;
        document.getElementById('cb').addEventListener('click', function() {
            window.__clicked = true;
        });
    "#).unwrap();
    e.eval_js(r#"document.getElementById('lbl').click()"#).unwrap();
    let result = e.eval_js(r#"window.__clicked"#).unwrap();
    assert_eq!(result, "true");
}

// =========================================================================
// select.options, select.selectedOptions, select.length
// =========================================================================

#[test]
fn select_options_returns_all_option_elements() {
    let mut e = engine_with_html(r#"<html><body>
        <select id="s">
            <option value="a">A</option>
            <option value="b">B</option>
            <option value="c">C</option>
        </select>
    </body></html>"#);
    assert_eq!(e.eval_js("document.getElementById('s').options.length").unwrap(), "3");
}

#[test]
fn select_options_indexed_access() {
    let mut e = engine_with_html(r#"<html><body>
        <select id="s">
            <option value="x">X</option>
            <option value="y">Y</option>
        </select>
    </body></html>"#);
    assert_eq!(
        e.eval_js("document.getElementById('s').options[1].getAttribute('value')").unwrap(),
        "y"
    );
}

#[test]
fn select_selected_options_returns_selected() {
    let mut e = engine_with_html(r#"<html><body>
        <select id="s">
            <option value="a">A</option>
            <option value="b" selected>B</option>
            <option value="c">C</option>
        </select>
    </body></html>"#);
    assert_eq!(e.eval_js("document.getElementById('s').selectedOptions.length").unwrap(), "1");
    assert_eq!(
        e.eval_js("document.getElementById('s').selectedOptions[0].getAttribute('value')").unwrap(),
        "b"
    );
}

#[test]
fn select_selected_options_empty_when_none_explicitly_selected() {
    let mut e = engine_with_html(r#"<html><body>
        <select id="s">
            <option value="a">A</option>
            <option value="b">B</option>
        </select>
    </body></html>"#);
    assert_eq!(e.eval_js("document.getElementById('s').selectedOptions.length").unwrap(), "0");
}

#[test]
fn select_length_returns_number_of_options() {
    let mut e = engine_with_html(r#"<html><body>
        <select id="s">
            <option value="a">A</option>
            <option value="b">B</option>
            <option value="c">C</option>
        </select>
    </body></html>"#);
    assert_eq!(e.eval_js("document.getElementById('s').length").unwrap(), "3");
}

#[test]
fn select_length_returns_zero_for_empty_select() {
    let mut e = engine_with_html(r#"<html><body>
        <select id="s"></select>
    </body></html>"#);
    assert_eq!(e.eval_js("document.getElementById('s').length").unwrap(), "0");
}

#[test]
fn select_length_undefined_for_non_select() {
    let mut e = engine_with_html(r#"<html><body><div id="d"></div></body></html>"#);
    assert_eq!(e.eval_js("String(document.getElementById('d').length)").unwrap(), "undefined");
}

// =========================================================================
// input.form attribute support
// =========================================================================

#[test]
fn input_form_getter_uses_form_attribute() {
    let mut e = engine_with_html(
        r#"<html><body><form id="myform"><input type="text" /></form><input id="ext" form="myform" /></body></html>"#,
    );
    let result = e.eval_js(r#"document.getElementById("ext").form.id"#).unwrap();
    assert_eq!(result, "myform");
}

#[test]
fn input_form_getter_returns_null_for_invalid_form_attribute() {
    let mut e = engine_with_html(
        r#"<html><body><input id="ext" form="nonexistent" /></body></html>"#,
    );
    let result = e.eval_js(r#"document.getElementById("ext").form === null"#).unwrap();
    assert_eq!(result, "true");
}

#[test]
fn input_form_getter_falls_back_to_ancestor() {
    let mut e = engine_with_html(
        r#"<html><body><form id="f"><input id="inp" type="text" /></form></body></html>"#,
    );
    let result = e.eval_js(r#"document.getElementById("inp").form.id"#).unwrap();
    assert_eq!(result, "f");
}

#[test]
fn form_elements_includes_external_inputs_with_form_attribute() {
    let mut e = engine_with_html(
        r#"<html><body><form id="myform"><input type="text" name="inside" /></form><input type="text" name="outside" form="myform" /></body></html>"#,
    );
    let result = e.eval_js(r#"document.getElementById("myform").elements.length"#).unwrap();
    assert_eq!(result, "2");
}

#[test]
fn form_elements_external_inputs_have_correct_names() {
    let mut e = engine_with_html(
        r#"<html><body><form id="myform"><input name="a" /></form><input name="b" form="myform" /><textarea name="c" form="myform"></textarea></body></html>"#,
    );
    let result = e.eval_js(r#"
        var elems = document.getElementById("myform").elements;
        var names = [];
        for (var i = 0; i < elems.length; i++) {
            names.push(elems[i].name);
        }
        names.join(",");
    "#).unwrap();
    assert_eq!(result, "a,b,c");
}

