use boa_engine::{
    class::ClassBuilder,
    js_string,
    native_function::NativeFunction,
    object::{builtins::JsArray, ObjectInitializer},
    property::Attribute,
    Context, JsError, JsResult, JsValue,
};

use crate::dom::{NodeData, NodeId};

use super::element::{JsElement, get_or_create_js_element};

// ---------------------------------------------------------------------------
// Helper: kebab-case to camelCase
// e.g. "user-id" -> "userId", "foo-bar-baz" -> "fooBarBaz"
// ---------------------------------------------------------------------------

fn kebab_to_camel(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut capitalize_next = false;
    for ch in s.chars() {
        if ch == '-' {
            capitalize_next = true;
        } else if capitalize_next {
            for upper in ch.to_uppercase() {
                result.push(upper);
            }
            capitalize_next = false;
        } else {
            result.push(ch);
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Helper: collect descendants recursively
// ---------------------------------------------------------------------------

fn collect_descendants(tree: &crate::dom::DomTree, node_id: NodeId, results: &mut Vec<NodeId>) {
    let children: Vec<NodeId> = tree.get_node(node_id).children.clone();
    for child_id in children {
        results.push(child_id);
        collect_descendants(tree, child_id, results);
    }
}

// ---------------------------------------------------------------------------
// Anchor: a.href getter/setter
// ---------------------------------------------------------------------------

fn get_href(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("href getter: `this` is not an object").into()))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("href getter: `this` is not an Element").into()))?;

    let tree = el.tree.borrow();
    match tree.get_attribute(el.node_id, "href") {
        Some(val) => {
            // Per WHATWG URL spec: parse through url::Url to get percent-encoded form
            match url::Url::parse(&val) {
                Ok(parsed) => Ok(JsValue::from(js_string!(parsed.to_string()))),
                Err(_) => Ok(JsValue::from(js_string!(val))),
            }
        }
        None => Ok(JsValue::from(js_string!(""))),
    }
}

fn set_href(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("href setter: `this` is not an object").into()))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("href setter: `this` is not an Element").into()))?;
    let value = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    super::mutation_observer::set_attribute_with_observer(&el.tree, el.node_id, "href", &value);
    Ok(JsValue::undefined())
}

// ---------------------------------------------------------------------------
// Form: form.action getter/setter
// ---------------------------------------------------------------------------

fn get_action(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("action getter: `this` is not an object").into()))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("action getter: `this` is not an Element").into()))?;

    let tree = el.tree.borrow();
    match tree.get_attribute(el.node_id, "action") {
        Some(val) => Ok(JsValue::from(js_string!(val))),
        None => Ok(JsValue::from(js_string!(""))),
    }
}

fn set_action(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("action setter: `this` is not an object").into()))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("action setter: `this` is not an Element").into()))?;
    let value = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    super::mutation_observer::set_attribute_with_observer(&el.tree, el.node_id, "action", &value);
    Ok(JsValue::undefined())
}

// ---------------------------------------------------------------------------
// Form: form.method getter/setter (defaults to "get")
// ---------------------------------------------------------------------------

fn get_method(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("method getter: `this` is not an object").into()))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("method getter: `this` is not an Element").into()))?;

    let tree = el.tree.borrow();
    match tree.get_attribute(el.node_id, "method") {
        Some(val) => Ok(JsValue::from(js_string!(val))),
        // Per spec, default method is "get"
        None => Ok(JsValue::from(js_string!("get"))),
    }
}

fn set_method(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("method setter: `this` is not an object").into()))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("method setter: `this` is not an Element").into()))?;
    let value = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    super::mutation_observer::set_attribute_with_observer(&el.tree, el.node_id, "method", &value);
    Ok(JsValue::undefined())
}

// ---------------------------------------------------------------------------
// Form: form.elements getter
// Returns an array of interactive child elements (input, select, textarea, button)
// ---------------------------------------------------------------------------

fn get_form_elements(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("elements getter: `this` is not an object").into()))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("elements getter: `this` is not an Element").into()))?;

    let tree_rc = el.tree.clone();
    let node_id = el.node_id;

    // Collect all descendant NodeIds
    let descendants = {
        let tree = tree_rc.borrow();
        let mut descs = Vec::new();
        collect_descendants(&tree, node_id, &mut descs);
        descs
    };

    // Filter to interactive elements, then wrap as JsElement
    let interactive_tags: &[&str] = &["input", "select", "textarea", "button"];
    let arr = JsArray::new(ctx);

    let tree = tree_rc.borrow();
    let mut interactive_ids = Vec::new();
    for desc_id in descendants {
        let node = tree.get_node(desc_id);
        if let NodeData::Element { ref tag_name, .. } = node.data {
            if interactive_tags.contains(&tag_name.to_ascii_lowercase().as_str()) {
                interactive_ids.push(desc_id);
            }
        }
    }
    drop(tree);

    for id in interactive_ids {
        let js_obj = get_or_create_js_element(id, tree_rc.clone(), ctx)?;
        arr.push(js_obj, ctx)?;
    }

    Ok(arr.into())
}

// ---------------------------------------------------------------------------
// All elements: element.hidden getter/setter (boolean attribute)
// ---------------------------------------------------------------------------

fn get_hidden(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("hidden getter: `this` is not an object").into()))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("hidden getter: `this` is not an Element").into()))?;

    let tree = el.tree.borrow();
    let has = tree.has_attribute(el.node_id, "hidden");
    Ok(JsValue::from(has))
}

fn set_hidden(this: &JsValue, args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("hidden setter: `this` is not an object").into()))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("hidden setter: `this` is not an Element").into()))?;

    let value = args
        .first()
        .map(|v| v.to_boolean())
        .unwrap_or(false);

    if value {
        super::mutation_observer::set_attribute_with_observer(&el.tree, el.node_id, "hidden", "");
    } else {
        super::mutation_observer::remove_attribute_with_observer(&el.tree, el.node_id, "hidden");
    }
    Ok(JsValue::undefined())
}

// ---------------------------------------------------------------------------
// All elements: element.dataset getter
// Returns a plain JS object with data-* attributes converted to camelCase.
// NOTE: This is a snapshot — mutations to the returned object do NOT write
// back to the DOM. This is a known limitation. A full Proxy-based dataset
// would require Boa Proxy support.
// ---------------------------------------------------------------------------

fn get_dataset(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("dataset getter: `this` is not an object").into()))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("dataset getter: `this` is not an Element").into()))?;

    let tree = el.tree.borrow();
    let node = tree.get_node(el.node_id);

    let mut builder = ObjectInitializer::new(ctx);

    if let NodeData::Element { ref attributes, .. } = node.data {
        for attr in attributes {
            if let Some(suffix) = attr.local_name.strip_prefix("data-") {
                let camel = kebab_to_camel(suffix);
                builder.property(js_string!(camel), js_string!(attr.value.clone()), Attribute::all());
            }
        }
    }

    let dataset_obj = builder.build();
    Ok(dataset_obj.into())
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

pub(crate) fn register_anchor_form(class: &mut ClassBuilder) -> JsResult<()> {
    let realm = class.context().realm().clone();

    // a.href
    let href_getter = NativeFunction::from_fn_ptr(get_href);
    let href_setter = NativeFunction::from_fn_ptr(set_href);
    class.accessor(
        js_string!("href"),
        Some(href_getter.to_js_function(&realm)),
        Some(href_setter.to_js_function(&realm)),
        Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    );

    // form.action
    let action_getter = NativeFunction::from_fn_ptr(get_action);
    let action_setter = NativeFunction::from_fn_ptr(set_action);
    class.accessor(
        js_string!("action"),
        Some(action_getter.to_js_function(&realm)),
        Some(action_setter.to_js_function(&realm)),
        Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    );

    // form.method
    let method_getter = NativeFunction::from_fn_ptr(get_method);
    let method_setter = NativeFunction::from_fn_ptr(set_method);
    class.accessor(
        js_string!("method"),
        Some(method_getter.to_js_function(&realm)),
        Some(method_setter.to_js_function(&realm)),
        Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    );

    // form.elements
    let elements_getter = NativeFunction::from_fn_ptr(get_form_elements);
    class.accessor(
        js_string!("elements"),
        Some(elements_getter.to_js_function(&realm)),
        None,
        Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    );

    // element.hidden
    let hidden_getter = NativeFunction::from_fn_ptr(get_hidden);
    let hidden_setter = NativeFunction::from_fn_ptr(set_hidden);
    class.accessor(
        js_string!("hidden"),
        Some(hidden_getter.to_js_function(&realm)),
        Some(hidden_setter.to_js_function(&realm)),
        Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    );

    // element.dataset
    let dataset_getter = NativeFunction::from_fn_ptr(get_dataset);
    class.accessor(
        js_string!("dataset"),
        Some(dataset_getter.to_js_function(&realm)),
        None,
        Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::kebab_to_camel;
    use crate::Engine;

    // -- kebab_to_camel unit tests --

    #[test]
    fn kebab_to_camel_simple() {
        assert_eq!(kebab_to_camel("user-id"), "userId");
    }

    #[test]
    fn kebab_to_camel_multiple_segments() {
        assert_eq!(kebab_to_camel("foo-bar-baz"), "fooBarBaz");
    }

    #[test]
    fn kebab_to_camel_no_dashes() {
        assert_eq!(kebab_to_camel("name"), "name");
    }

    #[test]
    fn kebab_to_camel_single_char_segments() {
        assert_eq!(kebab_to_camel("a-b-c"), "aBC");
    }

    // -- a.href --

    #[test]
    fn a_href_getter_returns_attribute() {
        let mut engine = Engine::new();
        engine.load_html(r#"<html><body><a id="link" href="https://example.com">Link</a></body></html>"#);
        let runtime = engine.runtime.as_mut().unwrap();
        let result = runtime
            .eval(r#"document.getElementById("link").href"#)
            .unwrap();
        let s = result
            .to_string(&mut runtime.context)
            .unwrap()
            .to_std_string_escaped();
        assert_eq!(s, "https://example.com/");
    }

    #[test]
    fn a_href_setter_updates_attribute() {
        let mut engine = Engine::new();
        engine.load_html(r#"<html><body><a id="link" href="https://old.com">Link</a></body></html>"#);
        let runtime = engine.runtime.as_mut().unwrap();
        runtime
            .eval(r#"document.getElementById("link").href = "https://new.com""#)
            .unwrap();
        let result = runtime
            .eval(r#"document.getElementById("link").href"#)
            .unwrap();
        let s = result
            .to_string(&mut runtime.context)
            .unwrap()
            .to_std_string_escaped();
        assert_eq!(s, "https://new.com/");
    }

    // -- form.action --

    #[test]
    fn form_action_getter_returns_attribute() {
        let mut engine = Engine::new();
        engine.load_html(r#"<html><body><form id="f" action="/submit"></form></body></html>"#);
        let runtime = engine.runtime.as_mut().unwrap();
        let result = runtime
            .eval(r#"document.getElementById("f").action"#)
            .unwrap();
        let s = result
            .to_string(&mut runtime.context)
            .unwrap()
            .to_std_string_escaped();
        assert_eq!(s, "/submit");
    }

    #[test]
    fn form_action_setter_updates_attribute() {
        let mut engine = Engine::new();
        engine.load_html(r#"<html><body><form id="f" action="/old"></form></body></html>"#);
        let runtime = engine.runtime.as_mut().unwrap();
        runtime
            .eval(r#"document.getElementById("f").action = "/new""#)
            .unwrap();
        let result = runtime
            .eval(r#"document.getElementById("f").action"#)
            .unwrap();
        let s = result
            .to_string(&mut runtime.context)
            .unwrap()
            .to_std_string_escaped();
        assert_eq!(s, "/new");
    }

    // -- form.method --

    #[test]
    fn form_method_defaults_to_get() {
        let mut engine = Engine::new();
        engine.load_html(r#"<html><body><form id="f"></form></body></html>"#);
        let runtime = engine.runtime.as_mut().unwrap();
        let result = runtime
            .eval(r#"document.getElementById("f").method"#)
            .unwrap();
        let s = result
            .to_string(&mut runtime.context)
            .unwrap()
            .to_std_string_escaped();
        assert_eq!(s, "get");
    }

    #[test]
    fn form_method_setter_updates_attribute() {
        let mut engine = Engine::new();
        engine.load_html(r#"<html><body><form id="f"></form></body></html>"#);
        let runtime = engine.runtime.as_mut().unwrap();
        runtime
            .eval(r#"document.getElementById("f").method = "post""#)
            .unwrap();
        let result = runtime
            .eval(r#"document.getElementById("f").method"#)
            .unwrap();
        let s = result
            .to_string(&mut runtime.context)
            .unwrap()
            .to_std_string_escaped();
        assert_eq!(s, "post");
    }

    // -- form.elements --

    #[test]
    fn form_elements_returns_interactive_children() {
        let mut engine = Engine::new();
        engine.load_html(r#"<html><body><form id="f"><input type="text"><select><option>A</option></select><textarea></textarea><button>Go</button><div>Not interactive</div></form></body></html>"#);
        let runtime = engine.runtime.as_mut().unwrap();
        let result = runtime
            .eval(r#"document.getElementById("f").elements.length"#)
            .unwrap();
        let n = result.to_number(&mut runtime.context).unwrap();
        assert_eq!(n, 4.0);
    }

    #[test]
    fn form_elements_returns_correct_tag_names() {
        let mut engine = Engine::new();
        engine.load_html(r#"<html><body><form id="f"><input type="text"><button>Go</button></form></body></html>"#);
        let runtime = engine.runtime.as_mut().unwrap();
        let result = runtime
            .eval(r#"
                var elems = document.getElementById("f").elements;
                var tags = [];
                for (var i = 0; i < elems.length; i++) {
                    tags.push(elems[i].tagName);
                }
                tags.join(",");
            "#)
            .unwrap();
        let s = result
            .to_string(&mut runtime.context)
            .unwrap()
            .to_std_string_escaped();
        // tagName returns uppercase per spec
        assert_eq!(s, "INPUT,BUTTON");
    }

    // -- element.hidden --

    #[test]
    fn hidden_getter_true_when_attribute_present() {
        let mut engine = Engine::new();
        engine.load_html(r#"<html><body><div id="d" hidden></div></body></html>"#);
        let runtime = engine.runtime.as_mut().unwrap();
        let result = runtime
            .eval(r#"document.getElementById("d").hidden"#)
            .unwrap();
        assert_eq!(result.to_boolean(), true);
    }

    #[test]
    fn hidden_getter_false_when_attribute_absent() {
        let mut engine = Engine::new();
        engine.load_html(r#"<html><body><div id="d"></div></body></html>"#);
        let runtime = engine.runtime.as_mut().unwrap();
        let result = runtime
            .eval(r#"document.getElementById("d").hidden"#)
            .unwrap();
        assert_eq!(result.to_boolean(), false);
    }

    #[test]
    fn hidden_setter_adds_attribute() {
        let mut engine = Engine::new();
        engine.load_html(r#"<html><body><div id="d"></div></body></html>"#);
        let runtime = engine.runtime.as_mut().unwrap();
        runtime
            .eval(r#"document.getElementById("d").hidden = true"#)
            .unwrap();
        let result = runtime
            .eval(r#"document.getElementById("d").hidden"#)
            .unwrap();
        assert_eq!(result.to_boolean(), true);
    }

    #[test]
    fn hidden_setter_removes_attribute() {
        let mut engine = Engine::new();
        engine.load_html(r#"<html><body><div id="d" hidden></div></body></html>"#);
        let runtime = engine.runtime.as_mut().unwrap();
        runtime
            .eval(r#"document.getElementById("d").hidden = false"#)
            .unwrap();
        let result = runtime
            .eval(r#"document.getElementById("d").hidden"#)
            .unwrap();
        assert_eq!(result.to_boolean(), false);
    }

    // -- element.dataset --

    #[test]
    fn dataset_reads_data_attributes() {
        let mut engine = Engine::new();
        engine.load_html(r#"<html><body><div id="d" data-name="Alice" data-age="30"></div></body></html>"#);
        let runtime = engine.runtime.as_mut().unwrap();
        let result = runtime
            .eval(r#"
                var ds = document.getElementById("d").dataset;
                ds.name + "," + ds.age;
            "#)
            .unwrap();
        let s = result
            .to_string(&mut runtime.context)
            .unwrap()
            .to_std_string_escaped();
        assert_eq!(s, "Alice,30");
    }

    #[test]
    fn dataset_converts_kebab_to_camel() {
        let mut engine = Engine::new();
        engine.load_html(r#"<html><body><div id="d" data-user-id="5" data-first-name="Bob"></div></body></html>"#);
        let runtime = engine.runtime.as_mut().unwrap();
        let result = runtime
            .eval(r#"
                var ds = document.getElementById("d").dataset;
                ds.userId + "," + ds.firstName;
            "#)
            .unwrap();
        let s = result
            .to_string(&mut runtime.context)
            .unwrap()
            .to_std_string_escaped();
        assert_eq!(s, "5,Bob");
    }
}
