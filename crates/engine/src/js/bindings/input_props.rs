use boa_engine::{
    class::ClassBuilder, js_string, native_function::NativeFunction, property::Attribute, Context, JsResult, JsValue,
};

use crate::dom::{DomTree, NodeData, NodeId};

// ---------------------------------------------------------------------------
// value — getter/setter
// For <textarea>: reads/writes textContent
// For all others: reads/writes the "value" attribute
// ---------------------------------------------------------------------------

fn get_value(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "value getter");

    let tree = el.tree.borrow();
    let node = tree.get_node(el.node_id);

    if let NodeData::Element { ref tag_name, .. } = node.data {
        let tag = tag_name.as_str();
        if tag.eq_ignore_ascii_case("textarea") {
            let text = tree.get_text_content(el.node_id);
            return Ok(JsValue::from(js_string!(text)));
        }
        if tag.eq_ignore_ascii_case("select") {
            let val = get_selected_option_value(&tree, el.node_id);
            return Ok(JsValue::from(js_string!(val)));
        }
    }

    match tree.get_attribute(el.node_id, "value") {
        Some(val) => Ok(JsValue::from(js_string!(val))),
        None => Ok(JsValue::from(js_string!(""))),
    }
}

/// For <select>, return the value of the selected <option> (or first option, or "").
fn get_selected_option_value(tree: &DomTree, select_id: NodeId) -> String {
    let options = get_option_children(tree, select_id);
    for &opt_id in &options {
        if tree.get_attribute(opt_id, "selected").is_some() {
            return tree.get_attribute(opt_id, "value").unwrap_or_default();
        }
    }
    if let Some(&first) = options.first() {
        return tree.get_attribute(first, "value").unwrap_or_default();
    }
    String::new()
}

fn get_option_children(tree: &DomTree, parent_id: NodeId) -> Vec<NodeId> {
    tree.get_node(parent_id)
        .children
        .iter()
        .copied()
        .filter(|&cid| {
            matches!(&tree.get_node(cid).data, NodeData::Element { tag_name, .. }
                if tag_name.eq_ignore_ascii_case("option"))
        })
        .collect()
}

fn set_value(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "value setter");

    let val = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    let node_id = el.node_id;
    let tree_rc = el.tree.clone();

    let tag = {
        let tree = tree_rc.borrow();
        match &tree.get_node(node_id).data {
            NodeData::Element { tag_name, .. } => tag_name.clone(),
            _ => String::new(),
        }
    };

    if tag.eq_ignore_ascii_case("textarea") {
        tree_rc.borrow_mut().set_text_content(node_id, &val);
    } else if tag.eq_ignore_ascii_case("select") {
        let mut tree = tree_rc.borrow_mut();
        let options = get_option_children(&tree, node_id);
        for &opt_id in &options {
            let opt_val = tree.get_attribute(opt_id, "value").unwrap_or_default();
            if opt_val == val {
                tree.set_attribute(opt_id, "selected", "");
            } else {
                tree.remove_attribute(opt_id, "selected");
            }
        }
    } else {
        super::mutation_observer::set_attribute_with_observer(ctx, &tree_rc, node_id, "value", &val);
    }

    Ok(JsValue::undefined())
}

// ---------------------------------------------------------------------------
// checked — getter/setter (boolean attribute)
// ---------------------------------------------------------------------------

fn get_checked(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "checked getter");

    let tree = el.tree.borrow();
    let has = tree.has_attribute(el.node_id, "checked");
    Ok(JsValue::from(has))
}

fn set_checked(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "checked setter");

    let val = args.first().map(|v| v.to_boolean()).unwrap_or(false);

    if val {
        super::mutation_observer::set_attribute_with_observer(ctx, &el.tree, el.node_id, "checked", "");
    } else {
        super::mutation_observer::remove_attribute_with_observer(ctx, &el.tree, el.node_id, "checked");
    }

    Ok(JsValue::undefined())
}

// ---------------------------------------------------------------------------
// type — getter/setter (defaults to "text")
// ---------------------------------------------------------------------------

fn get_type(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "type getter");

    let tree = el.tree.borrow();
    match tree.get_attribute(el.node_id, "type") {
        Some(val) if !val.is_empty() => Ok(JsValue::from(js_string!(val))),
        _ => Ok(JsValue::from(js_string!("text"))),
    }
}

fn set_type(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "type setter");

    let val = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    super::mutation_observer::set_attribute_with_observer(ctx, &el.tree, el.node_id, "type", &val);
    Ok(JsValue::undefined())
}

// ---------------------------------------------------------------------------
// disabled — getter/setter (boolean attribute)
// ---------------------------------------------------------------------------

fn get_disabled(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "disabled getter");

    let tree = el.tree.borrow();
    let has = tree.has_attribute(el.node_id, "disabled");
    Ok(JsValue::from(has))
}

fn set_disabled(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "disabled setter");

    let val = args.first().map(|v| v.to_boolean()).unwrap_or(false);

    if val {
        super::mutation_observer::set_attribute_with_observer(ctx, &el.tree, el.node_id, "disabled", "");
    } else {
        super::mutation_observer::remove_attribute_with_observer(ctx, &el.tree, el.node_id, "disabled");
    }

    Ok(JsValue::undefined())
}

// ---------------------------------------------------------------------------
// name — getter/setter
// ---------------------------------------------------------------------------

fn get_name(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "name getter");

    let tree = el.tree.borrow();
    match tree.get_attribute(el.node_id, "name") {
        Some(val) => Ok(JsValue::from(js_string!(val))),
        None => Ok(JsValue::from(js_string!(""))),
    }
}

fn set_name(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "name setter");

    let val = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    super::mutation_observer::set_attribute_with_observer(ctx, &el.tree, el.node_id, "name", &val);
    Ok(JsValue::undefined())
}

// ---------------------------------------------------------------------------
// placeholder — getter/setter
// ---------------------------------------------------------------------------

fn get_placeholder(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "placeholder getter");

    let tree = el.tree.borrow();
    match tree.get_attribute(el.node_id, "placeholder") {
        Some(val) => Ok(JsValue::from(js_string!(val))),
        None => Ok(JsValue::from(js_string!(""))),
    }
}

fn set_placeholder(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "placeholder setter");

    let val = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    super::mutation_observer::set_attribute_with_observer(ctx, &el.tree, el.node_id, "placeholder", &val);
    Ok(JsValue::undefined())
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

pub(crate) fn register_input_props(class: &mut ClassBuilder) -> JsResult<()> {
    let realm = class.context().realm().clone();

    // value
    class.accessor(
        js_string!("value"),
        Some(NativeFunction::from_fn_ptr(get_value).to_js_function(&realm)),
        Some(NativeFunction::from_fn_ptr(set_value).to_js_function(&realm)),
        Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    );

    // checked
    class.accessor(
        js_string!("checked"),
        Some(NativeFunction::from_fn_ptr(get_checked).to_js_function(&realm)),
        Some(NativeFunction::from_fn_ptr(set_checked).to_js_function(&realm)),
        Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    );

    // type
    class.accessor(
        js_string!("type"),
        Some(NativeFunction::from_fn_ptr(get_type).to_js_function(&realm)),
        Some(NativeFunction::from_fn_ptr(set_type).to_js_function(&realm)),
        Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    );

    // disabled
    class.accessor(
        js_string!("disabled"),
        Some(NativeFunction::from_fn_ptr(get_disabled).to_js_function(&realm)),
        Some(NativeFunction::from_fn_ptr(set_disabled).to_js_function(&realm)),
        Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    );

    // name
    class.accessor(
        js_string!("name"),
        Some(NativeFunction::from_fn_ptr(get_name).to_js_function(&realm)),
        Some(NativeFunction::from_fn_ptr(set_name).to_js_function(&realm)),
        Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    );

    // placeholder
    class.accessor(
        js_string!("placeholder"),
        Some(NativeFunction::from_fn_ptr(get_placeholder).to_js_function(&realm)),
        Some(NativeFunction::from_fn_ptr(set_placeholder).to_js_function(&realm)),
        Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use crate::Engine;

    #[test]
    fn input_value_getter_returns_attribute_value() {
        let mut engine = Engine::new();
        engine.load_html(r#"<html><body><input id="i" value="hello" /></body></html>"#);
        let runtime = engine.runtime.as_mut().unwrap();
        let result = runtime.eval(r#"document.getElementById("i").value"#).unwrap();
        let s = result.to_string(&mut runtime.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "hello");
    }

    #[test]
    fn input_value_getter_returns_empty_when_no_attribute() {
        let mut engine = Engine::new();
        engine.load_html(r#"<html><body><input id="i" /></body></html>"#);
        let runtime = engine.runtime.as_mut().unwrap();
        let result = runtime.eval(r#"document.getElementById("i").value"#).unwrap();
        let s = result.to_string(&mut runtime.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "");
    }

    #[test]
    fn input_value_setter_updates_value() {
        let mut engine = Engine::new();
        engine.load_html(r#"<html><body><input id="i" value="old" /></body></html>"#);
        let runtime = engine.runtime.as_mut().unwrap();
        runtime.eval(r#"document.getElementById("i").value = "new""#).unwrap();
        let result = runtime.eval(r#"document.getElementById("i").value"#).unwrap();
        let s = result.to_string(&mut runtime.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "new");
    }

    #[test]
    fn textarea_value_reads_text_content() {
        let mut engine = Engine::new();
        engine.load_html(r#"<html><body><textarea id="t"></textarea></body></html>"#);
        let runtime = engine.runtime.as_mut().unwrap();
        // Set textContent via JS so we know the content is there
        runtime
            .eval(r#"document.getElementById("t").textContent = "initial text""#)
            .unwrap();
        let result = runtime.eval(r#"document.getElementById("t").value"#).unwrap();
        let s = result.to_string(&mut runtime.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "initial text");
    }

    #[test]
    fn textarea_value_setter_writes_text_content() {
        let mut engine = Engine::new();
        engine.load_html(r#"<html><body><textarea id="t">old</textarea></body></html>"#);
        let runtime = engine.runtime.as_mut().unwrap();
        runtime
            .eval(r#"document.getElementById("t").value = "updated""#)
            .unwrap();
        let result = runtime.eval(r#"document.getElementById("t").value"#).unwrap();
        let s = result.to_string(&mut runtime.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "updated");
    }

    #[test]
    fn checked_getter_returns_false_when_absent() {
        let mut engine = Engine::new();
        engine.load_html(r#"<html><body><input id="c" type="checkbox" /></body></html>"#);
        let runtime = engine.runtime.as_mut().unwrap();
        let result = runtime.eval(r#"document.getElementById("c").checked"#).unwrap();
        assert_eq!(result.to_boolean(), false);
    }

    #[test]
    fn checked_getter_returns_true_when_present() {
        let mut engine = Engine::new();
        engine.load_html(r#"<html><body><input id="c" type="checkbox" checked /></body></html>"#);
        let runtime = engine.runtime.as_mut().unwrap();
        let result = runtime.eval(r#"document.getElementById("c").checked"#).unwrap();
        assert_eq!(result.to_boolean(), true);
    }

    #[test]
    fn checked_setter_adds_and_removes_attribute() {
        let mut engine = Engine::new();
        engine.load_html(r#"<html><body><input id="c" type="checkbox" /></body></html>"#);
        let runtime = engine.runtime.as_mut().unwrap();

        // Set checked to true
        runtime.eval(r#"document.getElementById("c").checked = true"#).unwrap();
        let result = runtime.eval(r#"document.getElementById("c").checked"#).unwrap();
        assert_eq!(result.to_boolean(), true);

        // Set checked to false
        runtime.eval(r#"document.getElementById("c").checked = false"#).unwrap();
        let result = runtime.eval(r#"document.getElementById("c").checked"#).unwrap();
        assert_eq!(result.to_boolean(), false);
    }

    #[test]
    fn type_defaults_to_text_when_absent() {
        let mut engine = Engine::new();
        engine.load_html(r#"<html><body><input id="i" /></body></html>"#);
        let runtime = engine.runtime.as_mut().unwrap();
        let result = runtime.eval(r#"document.getElementById("i").type"#).unwrap();
        let s = result.to_string(&mut runtime.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "text");
    }

    #[test]
    fn type_getter_returns_attribute_value() {
        let mut engine = Engine::new();
        engine.load_html(r#"<html><body><input id="i" type="password" /></body></html>"#);
        let runtime = engine.runtime.as_mut().unwrap();
        let result = runtime.eval(r#"document.getElementById("i").type"#).unwrap();
        let s = result.to_string(&mut runtime.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "password");
    }

    #[test]
    fn type_setter_updates_attribute() {
        let mut engine = Engine::new();
        engine.load_html(r#"<html><body><input id="i" /></body></html>"#);
        let runtime = engine.runtime.as_mut().unwrap();
        runtime.eval(r#"document.getElementById("i").type = "email""#).unwrap();
        let result = runtime.eval(r#"document.getElementById("i").type"#).unwrap();
        let s = result.to_string(&mut runtime.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "email");
    }

    #[test]
    fn disabled_getter_returns_false_when_absent() {
        let mut engine = Engine::new();
        engine.load_html(r#"<html><body><input id="i" /></body></html>"#);
        let runtime = engine.runtime.as_mut().unwrap();
        let result = runtime.eval(r#"document.getElementById("i").disabled"#).unwrap();
        assert_eq!(result.to_boolean(), false);
    }

    #[test]
    fn disabled_getter_returns_true_when_present() {
        let mut engine = Engine::new();
        engine.load_html(r#"<html><body><input id="i" disabled /></body></html>"#);
        let runtime = engine.runtime.as_mut().unwrap();
        let result = runtime.eval(r#"document.getElementById("i").disabled"#).unwrap();
        assert_eq!(result.to_boolean(), true);
    }

    #[test]
    fn disabled_setter_adds_and_removes_attribute() {
        let mut engine = Engine::new();
        engine.load_html(r#"<html><body><input id="i" /></body></html>"#);
        let runtime = engine.runtime.as_mut().unwrap();

        // Set disabled to true
        runtime.eval(r#"document.getElementById("i").disabled = true"#).unwrap();
        let result = runtime.eval(r#"document.getElementById("i").disabled"#).unwrap();
        assert_eq!(result.to_boolean(), true);

        // Verify attribute exists via hasAttribute
        let result = runtime
            .eval(r#"document.getElementById("i").hasAttribute("disabled")"#)
            .unwrap();
        assert_eq!(result.to_boolean(), true);

        // Set disabled to false
        runtime
            .eval(r#"document.getElementById("i").disabled = false"#)
            .unwrap();
        let result = runtime.eval(r#"document.getElementById("i").disabled"#).unwrap();
        assert_eq!(result.to_boolean(), false);

        // Verify attribute removed
        let result = runtime
            .eval(r#"document.getElementById("i").hasAttribute("disabled")"#)
            .unwrap();
        assert_eq!(result.to_boolean(), false);
    }

    #[test]
    fn name_getter_returns_attribute_value() {
        let mut engine = Engine::new();
        engine.load_html(r#"<html><body><input id="i" name="username" /></body></html>"#);
        let runtime = engine.runtime.as_mut().unwrap();
        let result = runtime.eval(r#"document.getElementById("i").name"#).unwrap();
        let s = result.to_string(&mut runtime.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "username");
    }

    #[test]
    fn name_getter_returns_empty_when_absent() {
        let mut engine = Engine::new();
        engine.load_html(r#"<html><body><input id="i" /></body></html>"#);
        let runtime = engine.runtime.as_mut().unwrap();
        let result = runtime.eval(r#"document.getElementById("i").name"#).unwrap();
        let s = result.to_string(&mut runtime.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "");
    }

    #[test]
    fn name_setter_updates_attribute() {
        let mut engine = Engine::new();
        engine.load_html(r#"<html><body><input id="i" /></body></html>"#);
        let runtime = engine.runtime.as_mut().unwrap();
        runtime.eval(r#"document.getElementById("i").name = "email""#).unwrap();
        let result = runtime.eval(r#"document.getElementById("i").name"#).unwrap();
        let s = result.to_string(&mut runtime.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "email");
    }

    #[test]
    fn placeholder_getter_returns_attribute_value() {
        let mut engine = Engine::new();
        engine.load_html(r#"<html><body><input id="i" placeholder="Enter name" /></body></html>"#);
        let runtime = engine.runtime.as_mut().unwrap();
        let result = runtime.eval(r#"document.getElementById("i").placeholder"#).unwrap();
        let s = result.to_string(&mut runtime.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "Enter name");
    }

    #[test]
    fn placeholder_setter_updates_attribute() {
        let mut engine = Engine::new();
        engine.load_html(r#"<html><body><input id="i" /></body></html>"#);
        let runtime = engine.runtime.as_mut().unwrap();
        runtime
            .eval(r#"document.getElementById("i").placeholder = "Type here""#)
            .unwrap();
        let result = runtime.eval(r#"document.getElementById("i").placeholder"#).unwrap();
        let s = result.to_string(&mut runtime.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "Type here");
    }

    #[test]
    fn properties_work_on_non_input_elements() {
        // These properties should work on ANY element, matching browser behavior
        let mut engine = Engine::new();
        engine.load_html(r#"<html><body><div id="d"></div></body></html>"#);
        let runtime = engine.runtime.as_mut().unwrap();

        // value on a div
        runtime.eval(r#"document.getElementById("d").value = "test""#).unwrap();
        let result = runtime.eval(r#"document.getElementById("d").value"#).unwrap();
        let s = result.to_string(&mut runtime.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "test");

        // name on a div
        runtime.eval(r#"document.getElementById("d").name = "myDiv""#).unwrap();
        let result = runtime.eval(r#"document.getElementById("d").name"#).unwrap();
        let s = result.to_string(&mut runtime.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "myDiv");
    }

    #[test]
    fn value_set_then_read_via_get_attribute() {
        // Setting .value should update the attribute, and getAttribute should see it
        let mut engine = Engine::new();
        engine.load_html(r#"<html><body><input id="i" /></body></html>"#);
        let runtime = engine.runtime.as_mut().unwrap();
        runtime.eval(r#"document.getElementById("i").value = "hello""#).unwrap();
        let result = runtime
            .eval(r#"document.getElementById("i").getAttribute("value")"#)
            .unwrap();
        let s = result.to_string(&mut runtime.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "hello");
    }

    #[test]
    fn set_value_then_snapshot_shows_updated_value() {
        let mut engine = Engine::new();
        engine.load_html(r#"<html><body><input id="i" value="old" /></body></html>"#);
        let runtime = engine.runtime.as_mut().unwrap();
        runtime.eval(r#"document.getElementById("i").value = "new""#).unwrap();

        // Take an accessibility snapshot and verify it contains the updated value
        let snapshot = engine.snapshot(braille_wire::SnapMode::Accessibility);
        assert!(
            snapshot.contains("new"),
            "Accessibility snapshot should contain the updated value 'new', got: {}",
            snapshot
        );
    }
}
