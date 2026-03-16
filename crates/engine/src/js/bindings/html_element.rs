use boa_engine::{
    class::{Class, ClassBuilder},
    js_string,
    native_function::NativeFunction,
    object::ObjectInitializer,
    property::Attribute,
    Context, JsError, JsResult, JsValue,
};

use crate::dom::NodeData;
use super::element::JsElement;
use super::event::JsEvent;

// ---------------------------------------------------------------------------
// HTMLElement properties — tabIndex, title, lang, dir, getBoundingClientRect,
// focus, blur, click
// ---------------------------------------------------------------------------

/// Interactive elements whose default tabIndex is 0 (not -1).
const INTERACTIVE_ELEMENTS: &[&str] = &["input", "select", "textarea", "button", "a"];

/// Getter for element.tabIndex
///
/// Returns the numeric tabindex attribute value, or a default:
/// -1 for non-interactive elements, 0 for interactive elements
/// (input, select, textarea, button, a).
fn get_tab_index(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("tabIndex getter: `this` is not an object").into()))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("tabIndex getter: `this` is not an Element").into()))?;

    let tree = el.tree.borrow();
    let node = tree.get_node(el.node_id);

    // Determine the tag name (lowercased) and the tabindex attribute
    let (tag_name, tabindex_attr) = match &node.data {
        NodeData::Element { tag_name, attributes, .. } => {
            let attr_val = attributes.iter().find(|a| a.local_name == "tabindex").map(|a| a.value.clone());
            (tag_name.to_lowercase(), attr_val)
        }
        _ => return Ok(JsValue::from(-1)),
    };

    // If tabindex attribute exists, parse it as i32
    if let Some(val) = tabindex_attr {
        let parsed = val.parse::<i32>().unwrap_or(-1);
        return Ok(JsValue::from(parsed));
    }

    // Default: 0 for interactive elements, -1 otherwise
    let default = if INTERACTIVE_ELEMENTS.contains(&tag_name.as_str()) {
        0
    } else {
        -1
    };
    Ok(JsValue::from(default))
}

/// Setter for element.tabIndex
///
/// Writes the numeric value as a string tabindex attribute.
fn set_tab_index(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("tabIndex setter: `this` is not an object").into()))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("tabIndex setter: `this` is not an Element").into()))?;

    let value = args
        .first()
        .map(|v| v.to_number(ctx))
        .transpose()?
        .unwrap_or(0.0) as i32;

    super::mutation_observer::set_attribute_with_observer(ctx, &el.tree, el.node_id, "tabindex", &value.to_string());
    Ok(JsValue::undefined())
}

/// Getter for element.title — reads the `title` attribute.
fn get_title(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("title getter: `this` is not an object").into()))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("title getter: `this` is not an Element").into()))?;

    let tree = el.tree.borrow();
    match tree.get_attribute(el.node_id, "title") {
        Some(val) => Ok(JsValue::from(js_string!(val))),
        None => Ok(JsValue::from(js_string!(""))),
    }
}

/// Setter for element.title — writes the `title` attribute.
fn set_title(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("title setter: `this` is not an object").into()))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("title setter: `this` is not an Element").into()))?;
    let value = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    super::mutation_observer::set_attribute_with_observer(ctx, &el.tree, el.node_id, "title", &value);
    Ok(JsValue::undefined())
}

/// Getter for element.lang — reads the `lang` attribute.
fn get_lang(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("lang getter: `this` is not an object").into()))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("lang getter: `this` is not an Element").into()))?;

    let tree = el.tree.borrow();
    match tree.get_attribute(el.node_id, "lang") {
        Some(val) => Ok(JsValue::from(js_string!(val))),
        None => Ok(JsValue::from(js_string!(""))),
    }
}

/// Setter for element.lang — writes the `lang` attribute.
fn set_lang(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("lang setter: `this` is not an object").into()))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("lang setter: `this` is not an Element").into()))?;
    let value = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    super::mutation_observer::set_attribute_with_observer(ctx, &el.tree, el.node_id, "lang", &value);
    Ok(JsValue::undefined())
}

/// Getter for element.dir — reads the `dir` attribute.
fn get_dir(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("dir getter: `this` is not an object").into()))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("dir getter: `this` is not an Element").into()))?;

    let tree = el.tree.borrow();
    match tree.get_attribute(el.node_id, "dir") {
        Some(val) => Ok(JsValue::from(js_string!(val))),
        None => Ok(JsValue::from(js_string!(""))),
    }
}

/// Setter for element.dir — writes the `dir` attribute.
fn set_dir(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("dir setter: `this` is not an object").into()))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("dir setter: `this` is not an Element").into()))?;
    let value = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    super::mutation_observer::set_attribute_with_observer(ctx, &el.tree, el.node_id, "dir", &value);
    Ok(JsValue::undefined())
}

/// element.getBoundingClientRect() — STUB returning all zeros.
///
/// Returns a plain JS object with {x, y, width, height, top, right, bottom, left}
/// all set to 0. This is needed because many frameworks call it.
///
/// TODO: Integrate with layout engine when available.
fn get_bounding_client_rect(_this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    // Stub: no layout engine, return all zeros
    let rect = ObjectInitializer::new(ctx)
        .property(js_string!("x"), JsValue::from(0.0_f64), Attribute::all())
        .property(js_string!("y"), JsValue::from(0.0_f64), Attribute::all())
        .property(js_string!("width"), JsValue::from(0.0_f64), Attribute::all())
        .property(js_string!("height"), JsValue::from(0.0_f64), Attribute::all())
        .property(js_string!("top"), JsValue::from(0.0_f64), Attribute::all())
        .property(js_string!("right"), JsValue::from(0.0_f64), Attribute::all())
        .property(js_string!("bottom"), JsValue::from(0.0_f64), Attribute::all())
        .property(js_string!("left"), JsValue::from(0.0_f64), Attribute::all())
        .build();

    Ok(rect.into())
}

/// element.focus() — STUB, no-op.
///
/// TODO: Set the engine's focused_element when Engine is accessible from bindings.
fn focus(_this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    // Stub: no-op. Cannot access Engine from JsElement binding yet.
    Ok(JsValue::undefined())
}

/// element.blur() — STUB, no-op.
///
/// TODO: Clear the engine's focused_element when Engine is accessible from bindings.
fn blur(_this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    // Stub: no-op.
    Ok(JsValue::undefined())
}

/// element.click() — dispatches a synthetic 'click' MouseEvent with bubbles=true, cancelable=true.
/// Per spec: if the element is disabled, the click() method does nothing.
fn click(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    // Check if element is disabled — if so, skip the click per spec
    if let Some(obj) = this.as_object() {
        if let Some(el) = obj.downcast_ref::<JsElement>() {
            let tree = el.tree.borrow();
            if super::activation::is_disabled(&tree, el.node_id) {
                return Ok(JsValue::undefined());
            }
        }
    }

    let event = JsEvent {
        event_type: "click".to_string(),
        bubbles: true,
        cancelable: true,
        default_prevented: false,
        propagation_stopped: false,
        immediate_propagation_stopped: false,
        target: None,
        current_target: None,
        phase: 0,
        dispatching: false,
        time_stamp: super::event::dom_high_res_time_stamp(ctx),
        initialized: true,
        kind: super::event::EventKind::mouse_default(),
    };

    let event_obj = JsEvent::from_data(event, ctx)?;
    super::event::attach_is_trusted_own_property(&event_obj, ctx)?;
    let event_val = JsValue::from(event_obj);

    // Dispatch by calling this.dispatchEvent(event) through the JS method
    let this_obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("click: `this` is not an object").into()))?;
    let dispatch_fn = this_obj.get(js_string!("dispatchEvent"), ctx)?;
    let dispatch_obj = dispatch_fn
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("click: dispatchEvent not found").into()))?;
    dispatch_obj.call(this, &[event_val], ctx)?;

    Ok(JsValue::undefined())
}

/// Register all HTMLElement properties and methods on the Element class.
pub(crate) fn register_html_element(class: &mut ClassBuilder) -> JsResult<()> {
    let realm = class.context().realm().clone();

    // tabIndex getter/setter
    let tab_index_getter = NativeFunction::from_fn_ptr(get_tab_index);
    let tab_index_setter = NativeFunction::from_fn_ptr(set_tab_index);
    class.accessor(
        js_string!("tabIndex"),
        Some(tab_index_getter.to_js_function(&realm)),
        Some(tab_index_setter.to_js_function(&realm)),
        Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    );

    // title getter/setter
    let title_getter = NativeFunction::from_fn_ptr(get_title);
    let title_setter = NativeFunction::from_fn_ptr(set_title);
    class.accessor(
        js_string!("title"),
        Some(title_getter.to_js_function(&realm)),
        Some(title_setter.to_js_function(&realm)),
        Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    );

    // lang getter/setter
    let lang_getter = NativeFunction::from_fn_ptr(get_lang);
    let lang_setter = NativeFunction::from_fn_ptr(set_lang);
    class.accessor(
        js_string!("lang"),
        Some(lang_getter.to_js_function(&realm)),
        Some(lang_setter.to_js_function(&realm)),
        Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    );

    // dir getter/setter
    let dir_getter = NativeFunction::from_fn_ptr(get_dir);
    let dir_setter = NativeFunction::from_fn_ptr(set_dir);
    class.accessor(
        js_string!("dir"),
        Some(dir_getter.to_js_function(&realm)),
        Some(dir_setter.to_js_function(&realm)),
        Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    );

    // getBoundingClientRect method (stub)
    class.method(
        js_string!("getBoundingClientRect"),
        0,
        NativeFunction::from_fn_ptr(get_bounding_client_rect),
    );

    // focus method (stub)
    class.method(
        js_string!("focus"),
        0,
        NativeFunction::from_fn_ptr(focus),
    );

    // blur method (stub)
    class.method(
        js_string!("blur"),
        0,
        NativeFunction::from_fn_ptr(blur),
    );

    // click method
    class.method(
        js_string!("click"),
        0,
        NativeFunction::from_fn_ptr(click),
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
    fn tab_index_returns_negative_one_for_div() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><div id='d'></div></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        let result = runtime.eval("document.getElementById('d').tabIndex").unwrap();
        let n = result.to_number(&mut runtime.context).unwrap();
        assert_eq!(n, -1.0);
    }

    #[test]
    fn tab_index_returns_zero_for_input() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><input id='i' /></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        let result = runtime.eval("document.getElementById('i').tabIndex").unwrap();
        let n = result.to_number(&mut runtime.context).unwrap();
        assert_eq!(n, 0.0);
    }

    #[test]
    fn tab_index_returns_zero_for_button() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><button id='b'>Click</button></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        let result = runtime.eval("document.getElementById('b').tabIndex").unwrap();
        let n = result.to_number(&mut runtime.context).unwrap();
        assert_eq!(n, 0.0);
    }

    #[test]
    fn tab_index_returns_zero_for_anchor() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><a id='a' href='#'>Link</a></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        let result = runtime.eval("document.getElementById('a').tabIndex").unwrap();
        let n = result.to_number(&mut runtime.context).unwrap();
        assert_eq!(n, 0.0);
    }

    #[test]
    fn tab_index_setter_updates_attribute() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><div id='d'></div></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        runtime.eval(r#"
            var d = document.getElementById('d');
            d.tabIndex = 5;
        "#).unwrap();
        let result = runtime.eval("document.getElementById('d').tabIndex").unwrap();
        let n = result.to_number(&mut runtime.context).unwrap();
        assert_eq!(n, 5.0);

        // Verify the underlying attribute
        let attr = runtime.eval("document.getElementById('d').getAttribute('tabindex')").unwrap();
        let s = attr.to_string(&mut runtime.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "5");
    }

    #[test]
    fn tab_index_reads_custom_tabindex_attribute() {
        let mut engine = Engine::new();
        engine.load_html(r#"<html><body><div id='d' tabindex="3"></div></body></html>"#);
        let runtime = engine.runtime.as_mut().unwrap();
        let result = runtime.eval("document.getElementById('d').tabIndex").unwrap();
        let n = result.to_number(&mut runtime.context).unwrap();
        assert_eq!(n, 3.0);
    }

    #[test]
    fn title_getter_setter() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><div id='d'></div></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();

        // Default is empty string
        let result = runtime.eval("document.getElementById('d').title").unwrap();
        let s = result.to_string(&mut runtime.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "");

        // Set title
        runtime.eval("document.getElementById('d').title = 'hello'").unwrap();
        let result = runtime.eval("document.getElementById('d').title").unwrap();
        let s = result.to_string(&mut runtime.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "hello");

        // Verify underlying attribute
        let attr = runtime.eval("document.getElementById('d').getAttribute('title')").unwrap();
        let s = attr.to_string(&mut runtime.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "hello");
    }

    #[test]
    fn lang_getter_setter() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><div id='d'></div></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();

        // Default is empty string
        let result = runtime.eval("document.getElementById('d').lang").unwrap();
        let s = result.to_string(&mut runtime.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "");

        // Set lang
        runtime.eval("document.getElementById('d').lang = 'en'").unwrap();
        let result = runtime.eval("document.getElementById('d').lang").unwrap();
        let s = result.to_string(&mut runtime.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "en");

        // Verify underlying attribute
        let attr = runtime.eval("document.getElementById('d').getAttribute('lang')").unwrap();
        let s = attr.to_string(&mut runtime.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "en");
    }

    #[test]
    fn dir_getter_setter() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><div id='d'></div></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();

        // Default is empty string
        let result = runtime.eval("document.getElementById('d').dir").unwrap();
        let s = result.to_string(&mut runtime.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "");

        // Set dir
        runtime.eval("document.getElementById('d').dir = 'rtl'").unwrap();
        let result = runtime.eval("document.getElementById('d').dir").unwrap();
        let s = result.to_string(&mut runtime.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "rtl");

        // Verify underlying attribute
        let attr = runtime.eval("document.getElementById('d').getAttribute('dir')").unwrap();
        let s = attr.to_string(&mut runtime.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "rtl");
    }

    #[test]
    fn get_bounding_client_rect_returns_zeros() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><div id='d'></div></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        runtime.eval(r#"
            var rect = document.getElementById('d').getBoundingClientRect();
        "#).unwrap();

        for prop in &["x", "y", "width", "height", "top", "right", "bottom", "left"] {
            let result = runtime.eval(&format!("rect.{}", prop)).unwrap();
            let n = result.to_number(&mut runtime.context).unwrap();
            assert_eq!(n, 0.0, "getBoundingClientRect().{} should be 0", prop);
        }
    }

    #[test]
    fn focus_and_blur_do_not_throw() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><input id='i' /></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();

        // focus() should not throw
        runtime.eval("document.getElementById('i').focus()").unwrap();

        // blur() should not throw
        runtime.eval("document.getElementById('i').blur()").unwrap();
    }

    #[test]
    fn click_dispatches_click_event() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><button id='btn'>Click</button></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        runtime.eval(r#"
            var clicked = false;
            var eventType = '';
            var btn = document.getElementById('btn');
            btn.addEventListener('click', function(e) {
                clicked = true;
                eventType = e.type;
            });
            btn.click();
        "#).unwrap();

        let clicked = runtime.eval("clicked").unwrap();
        assert_eq!(clicked.to_boolean(), true);

        let event_type = runtime.eval("eventType").unwrap();
        let s = event_type.to_string(&mut runtime.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "click");
    }
}
