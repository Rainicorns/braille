use boa_engine::{
    class::ClassBuilder,
    js_string,
    native_function::NativeFunction,
    property::Attribute,
    Context, JsError, JsResult, JsValue,
};

use super::element::JsElement;

/// Register all attribute methods and properties on the Element class.
pub(crate) fn register_attributes(class: &mut ClassBuilder) -> JsResult<()> {
    // Register methods
    class.method(
        js_string!("getAttribute"),
        1,
        NativeFunction::from_fn_ptr(get_attribute_fn),
    );

    class.method(
        js_string!("setAttribute"),
        2,
        NativeFunction::from_fn_ptr(set_attribute_fn),
    );

    class.method(
        js_string!("removeAttribute"),
        1,
        NativeFunction::from_fn_ptr(remove_attribute_fn),
    );

    class.method(
        js_string!("hasAttribute"),
        1,
        NativeFunction::from_fn_ptr(has_attribute_fn),
    );

    // Register properties (id and className)
    let realm = class.context().realm().clone();

    let id_getter = NativeFunction::from_fn_ptr(get_id);
    let id_setter = NativeFunction::from_fn_ptr(set_id);

    class.accessor(
        js_string!("id"),
        Some(id_getter.to_js_function(&realm)),
        Some(id_setter.to_js_function(&realm)),
        Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    );

    let class_getter = NativeFunction::from_fn_ptr(get_class_name);
    let class_setter = NativeFunction::from_fn_ptr(set_class_name);

    class.accessor(
        js_string!("className"),
        Some(class_getter.to_js_function(&realm)),
        Some(class_setter.to_js_function(&realm)),
        Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    );

    Ok(())
}

/// Native implementation of element.getAttribute(name)
fn get_attribute_fn(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("getAttribute: `this` is not an object").into()))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("getAttribute: `this` is not an Element").into()))?;
    let name = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    let tree = el.tree.borrow();
    match tree.get_attribute(el.node_id, &name) {
        Some(val) => Ok(JsValue::from(js_string!(val))),
        None => Ok(JsValue::null()),
    }
}

/// Native implementation of element.setAttribute(name, value)
fn set_attribute_fn(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("setAttribute: `this` is not an object").into()))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("setAttribute: `this` is not an Element").into()))?;
    let name = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();
    let value = args
        .get(1)
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    el.tree.borrow_mut().set_attribute(el.node_id, &name, &value);
    Ok(JsValue::undefined())
}

/// Native implementation of element.removeAttribute(name)
fn remove_attribute_fn(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("removeAttribute: `this` is not an object").into()))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("removeAttribute: `this` is not an Element").into()))?;
    let name = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    el.tree.borrow_mut().remove_attribute(el.node_id, &name);
    Ok(JsValue::undefined())
}

/// Native implementation of element.hasAttribute(name)
fn has_attribute_fn(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("hasAttribute: `this` is not an object").into()))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("hasAttribute: `this` is not an Element").into()))?;
    let name = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    let tree = el.tree.borrow();
    let has_attr = tree.has_attribute(el.node_id, &name);
    Ok(JsValue::from(has_attr))
}

/// Native getter for element.id
fn get_id(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("id getter: `this` is not an object").into()))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("id getter: `this` is not an Element").into()))?;

    let tree = el.tree.borrow();
    match tree.get_attribute(el.node_id, "id") {
        Some(val) => Ok(JsValue::from(js_string!(val))),
        None => Ok(JsValue::from(js_string!(""))),
    }
}

/// Native setter for element.id
fn set_id(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("id setter: `this` is not an object").into()))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("id setter: `this` is not an Element").into()))?;
    let value = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    el.tree.borrow_mut().set_attribute(el.node_id, "id", &value);
    Ok(JsValue::undefined())
}

/// Native getter for element.className
fn get_class_name(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("className getter: `this` is not an object").into()))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("className getter: `this` is not an Element").into()))?;

    let tree = el.tree.borrow();
    match tree.get_attribute(el.node_id, "class") {
        Some(val) => Ok(JsValue::from(js_string!(val))),
        None => Ok(JsValue::from(js_string!(""))),
    }
}

/// Native setter for element.className
fn set_class_name(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("className setter: `this` is not an object").into()))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("className setter: `this` is not an Element").into()))?;
    let value = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    el.tree.borrow_mut().set_attribute(el.node_id, "class", &value);
    Ok(JsValue::undefined())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dom::{DomTree, NodeData};
    use crate::js::JsRuntime;
    use std::cell::RefCell;
    use std::rc::Rc;

    // NOTE: These tests require register_attributes() to be called from element.rs
    // in the Element::init() method. Until that integration is complete, these tests
    // will fail because the attribute methods/properties won't be registered on
    // the Element class.

    /// Helper: build a DomTree with document > html > body > div#app
    fn make_test_tree() -> Rc<RefCell<DomTree>> {
        let tree = Rc::new(RefCell::new(DomTree::new()));
        {
            let mut t = tree.borrow_mut();
            let html = t.create_element("html");
            let body = t.create_element("body");
            let div = t.create_element("div");

            // Set id="app" on the div
            if let NodeData::Element { ref mut attributes, .. } = t.get_node_mut(div).data {
                attributes.push(("id".to_string(), "app".to_string()));
            }

            let doc = t.document();
            t.append_child(doc, html);
            t.append_child(html, body);
            t.append_child(body, div);
        }
        tree
    }

    #[test]
    fn get_attribute_returns_value() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        let result = rt.eval(
            r#"
            var el = document.getElementById("app");
            el.getAttribute("id");
        "#,
        ).unwrap();

        let value = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(value, "app");
    }

    #[test]
    fn get_attribute_returns_null_for_missing() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        let result = rt.eval(
            r#"
            var el = document.getElementById("app");
            el.getAttribute("nonexistent");
        "#,
        ).unwrap();

        assert!(result.is_null());
    }

    #[test]
    fn set_attribute_creates_new_attribute() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        rt.eval(
            r#"
            var el = document.getElementById("app");
            el.setAttribute("data-x", "hello");
        "#,
        ).unwrap();

        // Verify via DomTree
        let t = tree.borrow();
        let div_id = 3; // div#app
        assert_eq!(t.get_attribute(div_id, "data-x"), Some("hello".to_string()));
    }

    #[test]
    fn set_attribute_then_get_attribute() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        let result = rt.eval(
            r#"
            var el = document.getElementById("app");
            el.setAttribute("data-x", "hello");
            el.getAttribute("data-x");
        "#,
        ).unwrap();

        let value = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(value, "hello");
    }

    #[test]
    fn remove_attribute_removes_attribute() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        rt.eval(
            r#"
            var el = document.getElementById("app");
            el.removeAttribute("id");
        "#,
        ).unwrap();

        // Verify via DomTree
        let t = tree.borrow();
        let div_id = 3; // div#app
        assert_eq!(t.get_attribute(div_id, "id"), None);
    }

    #[test]
    fn has_attribute_returns_true_for_existing() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        let result = rt.eval(
            r#"
            var el = document.getElementById("app");
            el.hasAttribute("id");
        "#,
        ).unwrap();

        assert_eq!(result.as_boolean(), Some(true));
    }

    #[test]
    fn has_attribute_returns_false_for_missing() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        let result = rt.eval(
            r#"
            var el = document.getElementById("app");
            el.hasAttribute("nonexistent");
        "#,
        ).unwrap();

        assert_eq!(result.as_boolean(), Some(false));
    }

    #[test]
    fn id_getter_returns_id_attribute() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        let result = rt.eval(
            r#"
            var el = document.getElementById("app");
            el.id;
        "#,
        ).unwrap();

        let value = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(value, "app");
    }

    #[test]
    fn id_setter_updates_id_attribute() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        rt.eval(
            r#"
            var el = document.getElementById("app");
            el.id = "newId";
        "#,
        ).unwrap();

        // Verify via DomTree
        let t = tree.borrow();
        let div_id = 3; // div#app
        assert_eq!(t.get_attribute(div_id, "id"), Some("newId".to_string()));
    }

    #[test]
    fn id_setter_then_getter() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        let result = rt.eval(
            r#"
            var el = document.getElementById("app");
            el.id = "newId";
            el.id;
        "#,
        ).unwrap();

        let value = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(value, "newId");
    }

    #[test]
    fn class_name_getter_returns_class_attribute() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        rt.eval(
            r#"
            var el = document.getElementById("app");
            el.setAttribute("class", "container");
        "#,
        ).unwrap();

        let result = rt.eval(
            r#"
            var el = document.getElementById("app");
            el.className;
        "#,
        ).unwrap();

        let value = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(value, "container");
    }

    #[test]
    fn class_name_setter_updates_class_attribute() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        rt.eval(
            r#"
            var el = document.getElementById("app");
            el.className = "wrapper";
        "#,
        ).unwrap();

        // Verify via DomTree
        let t = tree.borrow();
        let div_id = 3; // div#app
        assert_eq!(t.get_attribute(div_id, "class"), Some("wrapper".to_string()));
    }

    #[test]
    fn class_name_getter_returns_empty_string_for_missing() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        let result = rt.eval(
            r#"
            var el = document.getElementById("app");
            el.className;
        "#,
        ).unwrap();

        let value = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(value, "");
    }

    #[test]
    fn id_getter_returns_empty_string_for_missing() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        rt.eval(
            r#"
            var el = document.getElementById("app");
            el.removeAttribute("id");
        "#,
        ).unwrap();

        let result = rt.eval(
            r#"
            var el = document.createElement("div");
            el.id;
        "#,
        ).unwrap();

        let value = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(value, "");
    }

    #[test]
    fn attribute_workflow_integration() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        rt.eval(
            r#"
            var el = document.getElementById("app");

            // Initially has id, no class
            var hasId = el.hasAttribute("id");
            var hasClass = el.hasAttribute("class");

            // Set class via setAttribute
            el.setAttribute("class", "container");

            // Set data-value via setAttribute
            el.setAttribute("data-value", "123");

            // Update id via property
            el.id = "main";

            // Update class via property
            el.className = "wrapper";

            // Remove data-value
            el.removeAttribute("data-value");
        "#,
        ).unwrap();

        // Verify the final state via DomTree
        let t = tree.borrow();
        let div_id = 3; // div#app
        assert_eq!(t.get_attribute(div_id, "id"), Some("main".to_string()));
        assert_eq!(t.get_attribute(div_id, "class"), Some("wrapper".to_string()));
        assert_eq!(t.get_attribute(div_id, "data-value"), None);
    }
}
