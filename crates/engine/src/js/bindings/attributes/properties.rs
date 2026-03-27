use boa_engine::{js_string, Context, JsResult, JsValue};

/// Native getter for element.id
pub(super) fn get_id(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "id getter");

    let tree = el.tree.borrow();
    match tree.get_attribute(el.node_id, "id") {
        Some(val) => Ok(JsValue::from(js_string!(val))),
        None => Ok(JsValue::from(js_string!(""))),
    }
}

/// Native setter for element.id
pub(super) fn set_id(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "id setter");
    let value = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    super::super::mutation_observer::set_attribute_with_observer(ctx, &el.tree, el.node_id, "id", &value);
    Ok(JsValue::undefined())
}

/// Native getter for element.className
pub(super) fn get_class_name(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "className getter");

    let tree = el.tree.borrow();
    match tree.get_attribute(el.node_id, "class") {
        Some(val) => Ok(JsValue::from(js_string!(val))),
        None => Ok(JsValue::from(js_string!(""))),
    }
}

/// Native setter for element.className
pub(super) fn set_class_name(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "className setter");
    let value = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    super::super::mutation_observer::set_attribute_with_observer(ctx, &el.tree, el.node_id, "class", &value);
    Ok(JsValue::undefined())
}
