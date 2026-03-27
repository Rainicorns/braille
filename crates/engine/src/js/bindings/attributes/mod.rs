mod attr_nodes;
mod core;
mod namespace;
mod properties;

use boa_engine::{
    class::ClassBuilder, js_string, native_function::NativeFunction, property::Attribute, JsResult,
};

use self::attr_nodes::*;
use self::core::*;
use self::namespace::*;
use self::properties::*;

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

    class.method(
        js_string!("getAttributeNode"),
        1,
        NativeFunction::from_fn_ptr(get_attribute_node_fn),
    );

    class.method(
        js_string!("getAttributeNodeNS"),
        2,
        NativeFunction::from_fn_ptr(get_attribute_node_ns_fn),
    );

    class.method(
        js_string!("setAttributeNS"),
        3,
        NativeFunction::from_fn_ptr(set_attribute_ns_fn),
    );

    class.method(
        js_string!("getAttributeNS"),
        2,
        NativeFunction::from_fn_ptr(get_attribute_ns_fn),
    );

    class.method(
        js_string!("removeAttributeNS"),
        2,
        NativeFunction::from_fn_ptr(remove_attribute_ns_fn),
    );

    class.method(
        js_string!("hasAttributeNS"),
        2,
        NativeFunction::from_fn_ptr(has_attribute_ns_fn),
    );

    class.method(
        js_string!("hasAttributes"),
        0,
        NativeFunction::from_fn_ptr(has_attributes_fn),
    );

    class.method(
        js_string!("toggleAttribute"),
        1,
        NativeFunction::from_fn_ptr(toggle_attribute_fn),
    );

    class.method(
        js_string!("setAttributeNode"),
        1,
        NativeFunction::from_fn_ptr(set_attribute_node_fn),
    );

    class.method(
        js_string!("setAttributeNodeNS"),
        1,
        NativeFunction::from_fn_ptr(set_attribute_node_ns_fn),
    );

    class.method(
        js_string!("removeAttributeNode"),
        1,
        NativeFunction::from_fn_ptr(remove_attribute_node_fn),
    );

    class.method(
        js_string!("getAttributeNames"),
        0,
        NativeFunction::from_fn_ptr(get_attribute_names_fn),
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

#[cfg(test)]
mod tests {
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
                attributes.push(crate::dom::node::DomAttribute::new("id", "app"));
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

        let result = rt
            .eval(
                r#"
            var el = document.getElementById("app");
            el.getAttribute("id");
        "#,
            )
            .unwrap();

        let value = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(value, "app");
    }

    #[test]
    fn get_attribute_returns_null_for_missing() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        let result = rt
            .eval(
                r#"
            var el = document.getElementById("app");
            el.getAttribute("nonexistent");
        "#,
            )
            .unwrap();

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
        )
        .unwrap();

        // Verify via DomTree
        let t = tree.borrow();
        let div_id = 3; // div#app
        assert_eq!(t.get_attribute(div_id, "data-x"), Some("hello".to_string()));
    }

    #[test]
    fn set_attribute_then_get_attribute() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        let result = rt
            .eval(
                r#"
            var el = document.getElementById("app");
            el.setAttribute("data-x", "hello");
            el.getAttribute("data-x");
        "#,
            )
            .unwrap();

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
        )
        .unwrap();

        // Verify via DomTree
        let t = tree.borrow();
        let div_id = 3; // div#app
        assert_eq!(t.get_attribute(div_id, "id"), None);
    }

    #[test]
    fn has_attribute_returns_true_for_existing() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        let result = rt
            .eval(
                r#"
            var el = document.getElementById("app");
            el.hasAttribute("id");
        "#,
            )
            .unwrap();

        assert_eq!(result.as_boolean(), Some(true));
    }

    #[test]
    fn has_attribute_returns_false_for_missing() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        let result = rt
            .eval(
                r#"
            var el = document.getElementById("app");
            el.hasAttribute("nonexistent");
        "#,
            )
            .unwrap();

        assert_eq!(result.as_boolean(), Some(false));
    }

    #[test]
    fn id_getter_returns_id_attribute() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        let result = rt
            .eval(
                r#"
            var el = document.getElementById("app");
            el.id;
        "#,
            )
            .unwrap();

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
        )
        .unwrap();

        // Verify via DomTree
        let t = tree.borrow();
        let div_id = 3; // div#app
        assert_eq!(t.get_attribute(div_id, "id"), Some("newId".to_string()));
    }

    #[test]
    fn id_setter_then_getter() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        let result = rt
            .eval(
                r#"
            var el = document.getElementById("app");
            el.id = "newId";
            el.id;
        "#,
            )
            .unwrap();

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
        )
        .unwrap();

        let result = rt
            .eval(
                r#"
            var el = document.getElementById("app");
            el.className;
        "#,
            )
            .unwrap();

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
        )
        .unwrap();

        // Verify via DomTree
        let t = tree.borrow();
        let div_id = 3; // div#app
        assert_eq!(t.get_attribute(div_id, "class"), Some("wrapper".to_string()));
    }

    #[test]
    fn class_name_getter_returns_empty_string_for_missing() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        let result = rt
            .eval(
                r#"
            var el = document.getElementById("app");
            el.className;
        "#,
            )
            .unwrap();

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
        )
        .unwrap();

        let result = rt
            .eval(
                r#"
            var el = document.createElement("div");
            el.id;
        "#,
            )
            .unwrap();

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
        )
        .unwrap();

        // Verify the final state via DomTree
        let t = tree.borrow();
        let div_id = 3; // div#app
        assert_eq!(t.get_attribute(div_id, "id"), Some("main".to_string()));
        assert_eq!(t.get_attribute(div_id, "class"), Some("wrapper".to_string()));
        assert_eq!(t.get_attribute(div_id, "data-value"), None);
    }

    #[test]
    fn set_attribute_ns_then_read_attributes_array() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        let result = rt
            .eval(
                r#"
            var el = document.createElement("foo");
            el.setAttributeNS("http://www.w3.org/XML/1998/namespace", "a:bb", "pass");
            var attr = el.attributes[0];
            attr ? attr.value : "NO_ATTR";
        "#,
            )
            .unwrap();

        let value = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(value, "pass");
    }
}
