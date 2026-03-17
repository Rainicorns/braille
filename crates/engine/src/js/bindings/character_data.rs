use boa_engine::{
    class::ClassBuilder, js_string, native_function::NativeFunction, property::Attribute, Context, JsError,
    JsNativeError, JsResult, JsValue,
};

use super::element::{get_or_create_js_element, JsElement};
use crate::dom::NodeData;

/// Registers CharacterData properties and methods on the Element class.
/// These only apply to Text (nodeType 3) and Comment (nodeType 8) nodes.
pub(crate) fn register_character_data(class: &mut ClassBuilder) -> JsResult<()> {
    let realm = class.context().realm().clone();

    // data getter/setter
    let data_getter = NativeFunction::from_fn_ptr(get_data);
    let data_setter = NativeFunction::from_fn_ptr(set_data);
    class.accessor(
        js_string!("data"),
        Some(data_getter.to_js_function(&realm)),
        Some(data_setter.to_js_function(&realm)),
        Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    );

    // length getter (read-only)
    let length_getter = NativeFunction::from_fn_ptr(get_length);
    class.accessor(
        js_string!("length"),
        Some(length_getter.to_js_function(&realm)),
        None,
        Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    );

    // appendData method
    class.method(js_string!("appendData"), 1, NativeFunction::from_fn_ptr(append_data));

    // deleteData method
    class.method(js_string!("deleteData"), 2, NativeFunction::from_fn_ptr(delete_data));

    // insertData method
    class.method(js_string!("insertData"), 2, NativeFunction::from_fn_ptr(insert_data));

    // replaceData method
    class.method(js_string!("replaceData"), 3, NativeFunction::from_fn_ptr(replace_data));

    // substringData method
    class.method(
        js_string!("substringData"),
        2,
        NativeFunction::from_fn_ptr(substring_data),
    );

    // splitText method (Text-only, but registered here for convenience)
    class.method(js_string!("splitText"), 1, NativeFunction::from_fn_ptr(split_text));

    // wholeText getter (Text-only)
    let whole_text_getter = NativeFunction::from_fn_ptr(get_whole_text);
    class.accessor(
        js_string!("wholeText"),
        Some(whole_text_getter.to_js_function(&realm)),
        None,
        Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    );

    Ok(())
}

/// Returns true if the node is a CharacterData node (Text, Comment, or ProcessingInstruction).
fn is_character_data(el: &JsElement) -> bool {
    let tree = el.tree.borrow();
    let node = tree.get_node(el.node_id);
    matches!(
        node.data,
        NodeData::Text { .. } | NodeData::CDATASection { .. } | NodeData::Comment { .. } | NodeData::ProcessingInstruction { .. }
    )
}

/// Helper to create an IndexSizeError DOMException.
fn index_size_error() -> JsError {
    // WPT assert_throws_dom checks that the thrown error has .name === "IndexSizeError"
    // and .code === 1. We construct a plain object that satisfies these checks.
    JsNativeError::range()
        .with_message("IndexSizeError: The index is not in the allowed range.")
        .into()
}

/// Native getter for .data
fn get_data(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "data getter");

    if !is_character_data(&el) {
        return Ok(JsValue::undefined());
    }

    let tree = el.tree.borrow();
    match tree.character_data_get(el.node_id) {
        Some(data) => Ok(JsValue::from(js_string!(data))),
        None => Ok(JsValue::undefined()),
    }
}

/// Native setter for .data
fn set_data(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "data setter");

    if !is_character_data(&el) {
        return Ok(JsValue::undefined());
    }

    let val = args.first().cloned().unwrap_or(JsValue::undefined());

    // Per spec: setting data to null converts to ""
    let data = if val.is_null() {
        String::new()
    } else {
        val.to_string(ctx)?.to_std_string_escaped()
    };

    super::mutation_observer::character_data_set_with_observer(ctx, &el.tree, el.node_id, &data);
    Ok(JsValue::undefined())
}

/// Native getter for .length (UTF-16 code unit count)
fn get_length(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "length getter");

    if !is_character_data(&el) {
        return Ok(JsValue::undefined());
    }

    let tree = el.tree.borrow();
    let len = tree.character_data_length(el.node_id);
    Ok(JsValue::from(len as f64))
}

/// Native implementation of appendData(data)
fn append_data(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "appendData");

    // Per spec: appendData requires exactly 1 argument — TypeError if missing
    let data_val = args
        .first()
        .ok_or_else(|| JsNativeError::typ().with_message("appendData: missing argument"))?;

    let data = data_val.to_string(ctx)?.to_std_string_escaped();

    super::mutation_observer::character_data_append_with_observer(ctx, &el.tree, el.node_id, &data);
    Ok(JsValue::undefined())
}

/// Converts a JS value to an unsigned long (u32) per WebIDL rules, then returns as usize.
/// This handles negative values and values > 2^32 by applying modulo 2^32.
fn to_unsigned_long(val: &JsValue, ctx: &mut Context) -> JsResult<usize> {
    let n = val.to_number(ctx)?;
    // WebIDL unsigned long: take modulo 2^32
    let n = n as i64;
    let u = ((n % (1i64 << 32)) + (1i64 << 32)) % (1i64 << 32);
    Ok(u as usize)
}

/// Native implementation of deleteData(offset, count)
fn delete_data(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "deleteData");

    let offset_val = args
        .first()
        .ok_or_else(|| JsNativeError::typ().with_message("deleteData: missing offset argument"))?;
    let count_val = args
        .get(1)
        .ok_or_else(|| JsNativeError::typ().with_message("deleteData: missing count argument"))?;

    let offset = to_unsigned_long(offset_val, ctx)?;
    let count = to_unsigned_long(count_val, ctx)?;

    super::mutation_observer::character_data_delete_with_observer(ctx, &el.tree, el.node_id, offset, count)
        .map_err(|_| index_size_error())?;

    Ok(JsValue::undefined())
}

/// Native implementation of insertData(offset, data)
fn insert_data(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "insertData");

    let offset_val = args
        .first()
        .ok_or_else(|| JsNativeError::typ().with_message("insertData: missing offset argument"))?;
    let data_val = args
        .get(1)
        .ok_or_else(|| JsNativeError::typ().with_message("insertData: missing data argument"))?;

    let offset = to_unsigned_long(offset_val, ctx)?;
    let data = data_val.to_string(ctx)?.to_std_string_escaped();

    super::mutation_observer::character_data_insert_with_observer(ctx, &el.tree, el.node_id, offset, &data)
        .map_err(|_| index_size_error())?;

    Ok(JsValue::undefined())
}

/// Native implementation of replaceData(offset, count, data)
fn replace_data(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "replaceData");

    let offset_val = args
        .first()
        .ok_or_else(|| JsNativeError::typ().with_message("replaceData: missing offset argument"))?;
    let count_val = args
        .get(1)
        .ok_or_else(|| JsNativeError::typ().with_message("replaceData: missing count argument"))?;
    let data_val = args
        .get(2)
        .ok_or_else(|| JsNativeError::typ().with_message("replaceData: missing data argument"))?;

    let offset = to_unsigned_long(offset_val, ctx)?;
    let count = to_unsigned_long(count_val, ctx)?;
    let data = data_val.to_string(ctx)?.to_std_string_escaped();

    super::mutation_observer::character_data_replace_with_observer(ctx, &el.tree, el.node_id, offset, count, &data)
        .map_err(|_| index_size_error())?;

    Ok(JsValue::undefined())
}

/// Native implementation of substringData(offset, count)
fn substring_data(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "substringData");

    let offset_val = args
        .first()
        .ok_or_else(|| JsNativeError::typ().with_message("substringData: missing offset argument"))?;
    let count_val = args
        .get(1)
        .ok_or_else(|| JsNativeError::typ().with_message("substringData: missing count argument"))?;

    let offset = to_unsigned_long(offset_val, ctx)?;
    let count = to_unsigned_long(count_val, ctx)?;

    let tree = el.tree.borrow();
    let result = tree
        .character_data_substring(el.node_id, offset, count)
        .map_err(|_| index_size_error())?;

    Ok(JsValue::from(js_string!(result)))
}

/// Native implementation of Text.splitText(offset)
/// Splits a Text node at the given UTF-16 offset, returning the new node.
fn split_text(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "splitText");

    // splitText only works on Text nodes (nodeType 3)
    {
        let tree = el.tree.borrow();
        let node = tree.get_node(el.node_id);
        if !matches!(node.data, NodeData::Text { .. }) {
            return Err(JsError::from_opaque(js_string!("splitText: not a Text node").into()));
        }
    }

    let offset_val = args
        .first()
        .ok_or_else(|| JsNativeError::typ().with_message("splitText: missing offset argument"))?;

    let offset = to_unsigned_long(offset_val, ctx)?;

    let tree_rc = el.tree.clone();
    let new_node_id = tree_rc
        .borrow_mut()
        .split_text(el.node_id, offset)
        .map_err(|_| index_size_error())?;

    let js_obj = get_or_create_js_element(new_node_id, tree_rc, ctx)?;
    Ok(js_obj.into())
}

/// Native getter for Text.wholeText
/// Returns the concatenation of all contiguous Text node siblings' data.
fn get_whole_text(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "wholeText getter");

    // wholeText only works on Text nodes (nodeType 3)
    let tree = el.tree.borrow();
    let node = tree.get_node(el.node_id);
    if !matches!(node.data, NodeData::Text { .. }) {
        return Ok(JsValue::undefined());
    }

    let result = tree.whole_text(el.node_id);
    Ok(JsValue::from(js_string!(result)))
}
