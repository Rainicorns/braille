use boa_engine::{
    class::ClassBuilder,
    js_string,
    native_function::NativeFunction,
    object::builtins::JsArray,
    property::Attribute,
    Context, JsError, JsResult, JsValue,
};

use crate::dom::{DomTree, NodeData, NodeId};
use super::element::{JsElement, get_or_create_js_element};

// ---------------------------------------------------------------------------
// Helper: get all <option> children of a node
// ---------------------------------------------------------------------------

fn get_option_children(tree: &DomTree, select_id: NodeId) -> Vec<NodeId> {
    let node = tree.get_node(select_id);
    node.children
        .iter()
        .copied()
        .filter(|&child_id| {
            if let NodeData::Element { ref tag_name, .. } = tree.get_node(child_id).data {
                tag_name.eq_ignore_ascii_case("option")
            } else {
                false
            }
        })
        .collect()
}

/// Returns true if the node is a <select> element.
fn is_select(tree: &DomTree, node_id: NodeId) -> bool {
    if let NodeData::Element { ref tag_name, .. } = tree.get_node(node_id).data {
        tag_name.eq_ignore_ascii_case("select")
    } else {
        false
    }
}

/// Returns true if the node is an <option> element.
fn is_option(tree: &DomTree, node_id: NodeId) -> bool {
    if let NodeData::Element { ref tag_name, .. } = tree.get_node(node_id).data {
        tag_name.eq_ignore_ascii_case("option")
    } else {
        false
    }
}

// ---------------------------------------------------------------------------
// select.selectedIndex getter/setter
// ---------------------------------------------------------------------------

fn get_selected_index(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("selectedIndex getter: `this` is not an object").into()))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("selectedIndex getter: `this` is not an Element").into()))?;

    let tree = el.tree.borrow();

    if !is_select(&tree, el.node_id) {
        return Ok(JsValue::from(-1));
    }

    let options = get_option_children(&tree, el.node_id);
    for (i, &opt_id) in options.iter().enumerate() {
        if tree.get_attribute(opt_id, "selected").is_some() {
            return Ok(JsValue::from(i as i32));
        }
    }

    Ok(JsValue::from(-1))
}

fn set_selected_index(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("selectedIndex setter: `this` is not an object").into()))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("selectedIndex setter: `this` is not an Element").into()))?;

    let index = args
        .first()
        .map(|v| v.to_i32(ctx))
        .transpose()?
        .unwrap_or(-1);

    let node_id = el.node_id;
    let tree_rc = el.tree.clone();

    if !matches!(&tree_rc.borrow().get_node(node_id).data, NodeData::Element { tag_name, .. } if tag_name.eq_ignore_ascii_case("select")) {
        return Ok(JsValue::undefined());
    }

    let mut tree = tree_rc.borrow_mut();
    let options = get_option_children(&tree, node_id);
    for (i, &opt_id) in options.iter().enumerate() {
        if i as i32 == index {
            tree.set_attribute(opt_id, "selected", "");
        } else {
            tree.remove_attribute(opt_id, "selected");
        }
    }

    Ok(JsValue::undefined())
}

// ---------------------------------------------------------------------------
// select.options getter
// ---------------------------------------------------------------------------

fn get_options(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("options getter: `this` is not an object").into()))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("options getter: `this` is not an Element").into()))?;

    let tree_rc = el.tree.clone();
    let node_id = el.node_id;

    if !matches!(&tree_rc.borrow().get_node(node_id).data, NodeData::Element { tag_name, .. } if tag_name.eq_ignore_ascii_case("select")) {
        return Ok(JsValue::undefined());
    }

    let options = get_option_children(&tree_rc.borrow(), node_id);
    let arr = JsArray::new(ctx);
    for opt_id in options {
        let js_obj = get_or_create_js_element(opt_id, tree_rc.clone(), ctx)?;
        arr.push(js_obj, ctx)?;
    }
    Ok(arr.into())
}

// ---------------------------------------------------------------------------
// option.selected getter/setter
// ---------------------------------------------------------------------------

fn get_option_selected(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("selected getter: `this` is not an object").into()))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("selected getter: `this` is not an Element").into()))?;

    let tree = el.tree.borrow();

    if !is_option(&tree, el.node_id) {
        return Ok(JsValue::from(false));
    }

    let has_selected = tree.get_attribute(el.node_id, "selected").is_some();
    Ok(JsValue::from(has_selected))
}

fn set_option_selected(this: &JsValue, args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("selected setter: `this` is not an object").into()))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("selected setter: `this` is not an Element").into()))?;

    let val = args
        .first()
        .map(|v| v.to_boolean())
        .unwrap_or(false);

    let node_id = el.node_id;
    let tree_rc = el.tree.clone();

    if !matches!(&tree_rc.borrow().get_node(node_id).data, NodeData::Element { tag_name, .. } if tag_name.eq_ignore_ascii_case("option")) {
        return Ok(JsValue::undefined());
    }

    if val {
        super::mutation_observer::set_attribute_with_observer(&tree_rc, node_id, "selected", "");
    } else {
        super::mutation_observer::remove_attribute_with_observer(&tree_rc, node_id, "selected");
    }

    Ok(JsValue::undefined())
}

// ---------------------------------------------------------------------------
// option.text getter/setter
// ---------------------------------------------------------------------------

fn get_option_text(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("text getter: `this` is not an object").into()))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("text getter: `this` is not an Element").into()))?;

    let tree = el.tree.borrow();

    if !is_option(&tree, el.node_id) {
        return Ok(JsValue::undefined());
    }

    let text = tree.get_text_content(el.node_id);
    Ok(JsValue::from(js_string!(text)))
}

fn set_option_text(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("text setter: `this` is not an object").into()))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("text setter: `this` is not an Element").into()))?;

    let new_text = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    let node_id = el.node_id;
    let tree_rc = el.tree.clone();

    if !matches!(&tree_rc.borrow().get_node(node_id).data, NodeData::Element { tag_name, .. } if tag_name.eq_ignore_ascii_case("option")) {
        return Ok(JsValue::undefined());
    }

    tree_rc.borrow_mut().set_text_content(node_id, &new_text);
    Ok(JsValue::undefined())
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

pub(crate) fn register_select_props(class: &mut ClassBuilder) -> JsResult<()> {
    let realm = class.context().realm().clone();

    // NOTE: "value" accessor is registered in input_props.rs (handles input, textarea,
    // select, and option elements in one unified getter/setter).

    // selectedIndex getter/setter (meaningful only on <select>)
    let si_getter = NativeFunction::from_fn_ptr(get_selected_index);
    let si_setter = NativeFunction::from_fn_ptr(set_selected_index);
    class.accessor(
        js_string!("selectedIndex"),
        Some(si_getter.to_js_function(&realm)),
        Some(si_setter.to_js_function(&realm)),
        Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    );

    // options getter (meaningful only on <select>)
    let options_getter = NativeFunction::from_fn_ptr(get_options);
    class.accessor(
        js_string!("options"),
        Some(options_getter.to_js_function(&realm)),
        None,
        Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    );

    // selected getter/setter (meaningful only on <option>)
    let selected_getter = NativeFunction::from_fn_ptr(get_option_selected);
    let selected_setter = NativeFunction::from_fn_ptr(set_option_selected);
    class.accessor(
        js_string!("selected"),
        Some(selected_getter.to_js_function(&realm)),
        Some(selected_setter.to_js_function(&realm)),
        Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    );

    // text getter/setter (meaningful only on <option>)
    let text_getter = NativeFunction::from_fn_ptr(get_option_text);
    let text_setter = NativeFunction::from_fn_ptr(set_option_text);
    class.accessor(
        js_string!("text"),
        Some(text_getter.to_js_function(&realm)),
        Some(text_setter.to_js_function(&realm)),
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
    fn select_value_returns_selected_option_value() {
        let mut engine = Engine::new();
        engine.load_html(r##"<html><body>
            <select id="s">
                <option value="a">A</option>
                <option value="b" selected>B</option>
                <option value="c">C</option>
            </select>
        </body></html>"##);
        let runtime = engine.runtime.as_mut().unwrap();
        let result = runtime.eval(r#"document.getElementById('s').value"#).unwrap();
        let s = result.to_string(&mut runtime.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "b");
    }

    #[test]
    fn select_value_returns_first_option_when_none_selected() {
        let mut engine = Engine::new();
        engine.load_html(r##"<html><body>
            <select id="s">
                <option value="x">X</option>
                <option value="y">Y</option>
            </select>
        </body></html>"##);
        let runtime = engine.runtime.as_mut().unwrap();
        let result = runtime.eval(r#"document.getElementById('s').value"#).unwrap();
        let s = result.to_string(&mut runtime.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "x");
    }

    #[test]
    fn select_value_setter_changes_selection() {
        let mut engine = Engine::new();
        engine.load_html(r##"<html><body>
            <select id="s">
                <option value="a">A</option>
                <option value="b" selected>B</option>
                <option value="c">C</option>
            </select>
        </body></html>"##);
        let runtime = engine.runtime.as_mut().unwrap();
        runtime.eval(r#"document.getElementById('s').value = 'c'"#).unwrap();
        let result = runtime.eval(r#"document.getElementById('s').value"#).unwrap();
        let s = result.to_string(&mut runtime.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "c");
    }

    #[test]
    fn select_selected_index_getter() {
        let mut engine = Engine::new();
        engine.load_html(r##"<html><body>
            <select id="s">
                <option value="a">A</option>
                <option value="b" selected>B</option>
                <option value="c">C</option>
            </select>
        </body></html>"##);
        let runtime = engine.runtime.as_mut().unwrap();
        let result = runtime.eval(r#"document.getElementById('s').selectedIndex"#).unwrap();
        let n = result.to_number(&mut runtime.context).unwrap();
        assert_eq!(n, 1.0);
    }

    #[test]
    fn select_selected_index_setter() {
        let mut engine = Engine::new();
        engine.load_html(r##"<html><body>
            <select id="s">
                <option value="a">A</option>
                <option value="b">B</option>
                <option value="c">C</option>
            </select>
        </body></html>"##);
        let runtime = engine.runtime.as_mut().unwrap();
        runtime.eval(r#"document.getElementById('s').selectedIndex = 2"#).unwrap();
        let result = runtime.eval(r#"document.getElementById('s').value"#).unwrap();
        let s = result.to_string(&mut runtime.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "c");
    }

    #[test]
    fn select_options_returns_array_with_correct_length() {
        let mut engine = Engine::new();
        engine.load_html(r##"<html><body>
            <select id="s">
                <option value="a">A</option>
                <option value="b">B</option>
                <option value="c">C</option>
            </select>
        </body></html>"##);
        let runtime = engine.runtime.as_mut().unwrap();
        let result = runtime.eval(r#"document.getElementById('s').options.length"#).unwrap();
        let n = result.to_number(&mut runtime.context).unwrap();
        assert_eq!(n, 3.0);
    }

    #[test]
    fn option_value_getter_setter() {
        let mut engine = Engine::new();
        engine.load_html(r##"<html><body>
            <select id="s">
                <option id="opt" value="original">Original</option>
            </select>
        </body></html>"##);
        let runtime = engine.runtime.as_mut().unwrap();
        // getter
        let result = runtime.eval(r#"document.getElementById('opt').value"#).unwrap();
        let s = result.to_string(&mut runtime.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "original");
        // setter
        runtime.eval(r#"document.getElementById('opt').value = 'changed'"#).unwrap();
        let result2 = runtime.eval(r#"document.getElementById('opt').value"#).unwrap();
        let s2 = result2.to_string(&mut runtime.context).unwrap().to_std_string_escaped();
        assert_eq!(s2, "changed");
    }

    #[test]
    fn option_selected_getter_setter() {
        let mut engine = Engine::new();
        engine.load_html(r##"<html><body>
            <select id="s">
                <option id="opt1" value="a">A</option>
                <option id="opt2" value="b">B</option>
            </select>
        </body></html>"##);
        let runtime = engine.runtime.as_mut().unwrap();
        // Initially neither is explicitly selected
        let result = runtime.eval(r#"document.getElementById('opt1').selected"#).unwrap();
        assert_eq!(result.to_boolean(), false);
        // Set selected
        runtime.eval(r#"document.getElementById('opt2').selected = true"#).unwrap();
        let result2 = runtime.eval(r#"document.getElementById('opt2').selected"#).unwrap();
        assert_eq!(result2.to_boolean(), true);
        // Unset
        runtime.eval(r#"document.getElementById('opt2').selected = false"#).unwrap();
        let result3 = runtime.eval(r#"document.getElementById('opt2').selected"#).unwrap();
        assert_eq!(result3.to_boolean(), false);
    }

    #[test]
    fn option_text_getter_setter() {
        let mut engine = Engine::new();
        engine.load_html(r##"<html><body>
            <select id="s">
                <option id="opt" value="a">Alpha</option>
            </select>
        </body></html>"##);
        let runtime = engine.runtime.as_mut().unwrap();
        // getter
        let result = runtime.eval(r#"document.getElementById('opt').text"#).unwrap();
        let s = result.to_string(&mut runtime.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "Alpha");
        // setter
        runtime.eval(r#"document.getElementById('opt').text = 'Beta'"#).unwrap();
        let result2 = runtime.eval(r#"document.getElementById('opt').text"#).unwrap();
        let s2 = result2.to_string(&mut runtime.context).unwrap().to_std_string_escaped();
        assert_eq!(s2, "Beta");
    }

    #[test]
    fn selecting_by_value_deselects_others() {
        let mut engine = Engine::new();
        engine.load_html(r##"<html><body>
            <select id="s">
                <option id="opt1" value="a" selected>A</option>
                <option id="opt2" value="b">B</option>
                <option id="opt3" value="c">C</option>
            </select>
        </body></html>"##);
        let runtime = engine.runtime.as_mut().unwrap();
        // Initially opt1 is selected
        let r1 = runtime.eval(r#"document.getElementById('opt1').selected"#).unwrap();
        assert_eq!(r1.to_boolean(), true);
        // Change via select.value
        runtime.eval(r#"document.getElementById('s').value = 'c'"#).unwrap();
        // opt1 should no longer be selected
        let r2 = runtime.eval(r#"document.getElementById('opt1').selected"#).unwrap();
        assert_eq!(r2.to_boolean(), false);
        // opt3 should now be selected
        let r3 = runtime.eval(r#"document.getElementById('opt3').selected"#).unwrap();
        assert_eq!(r3.to_boolean(), true);
        // selectedIndex should be 2
        let r4 = runtime.eval(r#"document.getElementById('s').selectedIndex"#).unwrap();
        let n = r4.to_number(&mut runtime.context).unwrap();
        assert_eq!(n, 2.0);
    }
}
