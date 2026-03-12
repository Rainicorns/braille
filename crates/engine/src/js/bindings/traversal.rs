use boa_engine::{
    class::{Class, ClassBuilder},
    js_string,
    native_function::NativeFunction,
    object::builtins::JsArray,
    property::Attribute,
    Context, JsError, JsResult, JsValue,
};

use super::element::JsElement;

/// Registers all traversal properties on the Element class.
pub(crate) fn register_traversal(class: &mut ClassBuilder) -> JsResult<()> {
    let realm = class.context().realm().clone();

    // parentNode getter
    let parent_node_getter = NativeFunction::from_fn_ptr(get_parent_node);
    class.accessor(
        js_string!("parentNode"),
        Some(parent_node_getter.to_js_function(&realm)),
        None,
        Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    );

    // parentElement getter
    let parent_element_getter = NativeFunction::from_fn_ptr(get_parent_element);
    class.accessor(
        js_string!("parentElement"),
        Some(parent_element_getter.to_js_function(&realm)),
        None,
        Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    );

    // firstChild getter
    let first_child_getter = NativeFunction::from_fn_ptr(get_first_child);
    class.accessor(
        js_string!("firstChild"),
        Some(first_child_getter.to_js_function(&realm)),
        None,
        Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    );

    // lastChild getter
    let last_child_getter = NativeFunction::from_fn_ptr(get_last_child);
    class.accessor(
        js_string!("lastChild"),
        Some(last_child_getter.to_js_function(&realm)),
        None,
        Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    );

    // nextSibling getter
    let next_sibling_getter = NativeFunction::from_fn_ptr(get_next_sibling);
    class.accessor(
        js_string!("nextSibling"),
        Some(next_sibling_getter.to_js_function(&realm)),
        None,
        Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    );

    // previousSibling getter
    let previous_sibling_getter = NativeFunction::from_fn_ptr(get_previous_sibling);
    class.accessor(
        js_string!("previousSibling"),
        Some(previous_sibling_getter.to_js_function(&realm)),
        None,
        Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    );

    // childNodes getter
    let child_nodes_getter = NativeFunction::from_fn_ptr(get_child_nodes);
    class.accessor(
        js_string!("childNodes"),
        Some(child_nodes_getter.to_js_function(&realm)),
        None,
        Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    );

    // children getter
    let children_getter = NativeFunction::from_fn_ptr(get_children);
    class.accessor(
        js_string!("children"),
        Some(children_getter.to_js_function(&realm)),
        None,
        Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    );

    // hasChildNodes() method
    class.method(
        js_string!("hasChildNodes"),
        0,
        NativeFunction::from_fn_ptr(has_child_nodes),
    );

    // firstElementChild getter
    let first_element_child_getter = NativeFunction::from_fn_ptr(get_first_element_child);
    class.accessor(
        js_string!("firstElementChild"),
        Some(first_element_child_getter.to_js_function(&realm)),
        None,
        Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    );

    // lastElementChild getter
    let last_element_child_getter = NativeFunction::from_fn_ptr(get_last_element_child);
    class.accessor(
        js_string!("lastElementChild"),
        Some(last_element_child_getter.to_js_function(&realm)),
        None,
        Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    );

    // nextElementSibling getter
    let next_element_sibling_getter = NativeFunction::from_fn_ptr(get_next_element_sibling);
    class.accessor(
        js_string!("nextElementSibling"),
        Some(next_element_sibling_getter.to_js_function(&realm)),
        None,
        Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    );

    // previousElementSibling getter
    let previous_element_sibling_getter = NativeFunction::from_fn_ptr(get_previous_element_sibling);
    class.accessor(
        js_string!("previousElementSibling"),
        Some(previous_element_sibling_getter.to_js_function(&realm)),
        None,
        Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    );

    // childElementCount getter
    let child_element_count_getter = NativeFunction::from_fn_ptr(get_child_element_count);
    class.accessor(
        js_string!("childElementCount"),
        Some(child_element_count_getter.to_js_function(&realm)),
        None,
        Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    );

    Ok(())
}

/// Native getter for element.parentNode
fn get_parent_node(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("parentNode getter: `this` is not an object").into()))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("parentNode getter: `this` is not an Element").into()))?;
    let tree = el.tree.borrow();
    match tree.get_parent(el.node_id) {
        Some(parent_id) => {
            drop(tree);
            let parent = JsElement::new(parent_id, el.tree.clone());
            let js_obj = JsElement::from_data(parent, ctx)?;
            Ok(js_obj.into())
        }
        None => Ok(JsValue::null()),
    }
}

/// Native getter for element.parentElement
/// Returns the parent only if it's an Element (not Document)
fn get_parent_element(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("parentElement getter: `this` is not an object").into()))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("parentElement getter: `this` is not an Element").into()))?;
    let tree = el.tree.borrow();
    match tree.parent_element(el.node_id) {
        Some(parent_id) => {
            drop(tree);
            let parent = JsElement::new(parent_id, el.tree.clone());
            let js_obj = JsElement::from_data(parent, ctx)?;
            Ok(js_obj.into())
        }
        None => Ok(JsValue::null()),
    }
}

/// Native getter for element.firstChild
fn get_first_child(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("firstChild getter: `this` is not an object").into()))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("firstChild getter: `this` is not an Element").into()))?;
    let tree = el.tree.borrow();
    match tree.first_child(el.node_id) {
        Some(child_id) => {
            drop(tree);
            let child = JsElement::new(child_id, el.tree.clone());
            let js_obj = JsElement::from_data(child, ctx)?;
            Ok(js_obj.into())
        }
        None => Ok(JsValue::null()),
    }
}

/// Native getter for element.lastChild
fn get_last_child(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("lastChild getter: `this` is not an object").into()))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("lastChild getter: `this` is not an Element").into()))?;
    let tree = el.tree.borrow();
    match tree.last_child(el.node_id) {
        Some(child_id) => {
            drop(tree);
            let child = JsElement::new(child_id, el.tree.clone());
            let js_obj = JsElement::from_data(child, ctx)?;
            Ok(js_obj.into())
        }
        None => Ok(JsValue::null()),
    }
}

/// Native getter for element.nextSibling
fn get_next_sibling(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("nextSibling getter: `this` is not an object").into()))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("nextSibling getter: `this` is not an Element").into()))?;
    let tree = el.tree.borrow();
    match tree.next_sibling(el.node_id) {
        Some(sibling_id) => {
            drop(tree);
            let sibling = JsElement::new(sibling_id, el.tree.clone());
            let js_obj = JsElement::from_data(sibling, ctx)?;
            Ok(js_obj.into())
        }
        None => Ok(JsValue::null()),
    }
}

/// Native getter for element.previousSibling
fn get_previous_sibling(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("previousSibling getter: `this` is not an object").into()))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("previousSibling getter: `this` is not an Element").into()))?;
    let tree = el.tree.borrow();
    match tree.prev_sibling(el.node_id) {
        Some(sibling_id) => {
            drop(tree);
            let sibling = JsElement::new(sibling_id, el.tree.clone());
            let js_obj = JsElement::from_data(sibling, ctx)?;
            Ok(js_obj.into())
        }
        None => Ok(JsValue::null()),
    }
}

/// Native getter for element.childNodes
/// Returns an Array of all child nodes
fn get_child_nodes(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("childNodes getter: `this` is not an object").into()))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("childNodes getter: `this` is not an Element").into()))?;
    let tree_rc = el.tree.clone();
    let tree = tree_rc.borrow();
    let children = tree.children(el.node_id);
    drop(tree);

    let arr = JsArray::new(ctx);
    for child_id in children {
        let child = JsElement::new(child_id, tree_rc.clone());
        let js_obj = JsElement::from_data(child, ctx)?;
        arr.push(js_obj, ctx)?;
    }
    Ok(arr.into())
}

/// Native getter for element.children
/// Returns an Array of Element-only children (filters out Text, Comment nodes)
fn get_children(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("children getter: `this` is not an object").into()))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("children getter: `this` is not an Element").into()))?;
    let tree_rc = el.tree.clone();
    let tree = tree_rc.borrow();
    let element_children = tree.element_children(el.node_id);
    drop(tree);

    let arr = JsArray::new(ctx);
    for child_id in element_children {
        let child = JsElement::new(child_id, tree_rc.clone());
        let js_obj = JsElement::from_data(child, ctx)?;
        arr.push(js_obj, ctx)?;
    }
    Ok(arr.into())
}

/// Native method for element.hasChildNodes()
/// Returns a boolean indicating whether the element has children
fn has_child_nodes(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("hasChildNodes: `this` is not an object").into()))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("hasChildNodes: `this` is not an Element").into()))?;
    let tree = el.tree.borrow();
    let has_children = !tree.children(el.node_id).is_empty();
    Ok(JsValue::from(has_children))
}

/// Native getter for element.firstElementChild
fn get_first_element_child(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("firstElementChild getter: `this` is not an object").into()))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("firstElementChild getter: `this` is not an Element").into()))?;
    let tree = el.tree.borrow();
    let element_kids = tree.element_children(el.node_id);
    match element_kids.first().copied() {
        Some(child_id) => {
            drop(tree);
            let child = JsElement::new(child_id, el.tree.clone());
            let js_obj = JsElement::from_data(child, ctx)?;
            Ok(js_obj.into())
        }
        None => Ok(JsValue::null()),
    }
}

/// Native getter for element.lastElementChild
fn get_last_element_child(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("lastElementChild getter: `this` is not an object").into()))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("lastElementChild getter: `this` is not an Element").into()))?;
    let tree = el.tree.borrow();
    let element_kids = tree.element_children(el.node_id);
    match element_kids.last().copied() {
        Some(child_id) => {
            drop(tree);
            let child = JsElement::new(child_id, el.tree.clone());
            let js_obj = JsElement::from_data(child, ctx)?;
            Ok(js_obj.into())
        }
        None => Ok(JsValue::null()),
    }
}

/// Native getter for element.nextElementSibling
fn get_next_element_sibling(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("nextElementSibling getter: `this` is not an object").into()))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("nextElementSibling getter: `this` is not an Element").into()))?;
    let tree = el.tree.borrow();
    match tree.next_sibling_element(el.node_id) {
        Some(sibling_id) => {
            drop(tree);
            let sibling = JsElement::new(sibling_id, el.tree.clone());
            let js_obj = JsElement::from_data(sibling, ctx)?;
            Ok(js_obj.into())
        }
        None => Ok(JsValue::null()),
    }
}

/// Native getter for element.previousElementSibling
fn get_previous_element_sibling(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("previousElementSibling getter: `this` is not an object").into()))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("previousElementSibling getter: `this` is not an Element").into()))?;
    let tree = el.tree.borrow();
    match tree.prev_sibling_element(el.node_id) {
        Some(sibling_id) => {
            drop(tree);
            let sibling = JsElement::new(sibling_id, el.tree.clone());
            let js_obj = JsElement::from_data(sibling, ctx)?;
            Ok(js_obj.into())
        }
        None => Ok(JsValue::null()),
    }
}

/// Native getter for element.childElementCount
fn get_child_element_count(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("childElementCount getter: `this` is not an object").into()))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("childElementCount getter: `this` is not an Element").into()))?;
    let tree = el.tree.borrow();
    let count = tree.element_children(el.node_id).len();
    Ok(JsValue::from(count as i32))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dom::{DomTree, NodeData};
    use crate::js::runtime::JsRuntime;
    use std::cell::RefCell;
    use std::rc::Rc;

    /// Helper: build a DomTree with document > html > body > div#parent > [span#child1, span#child2]
    fn make_traversal_test_tree() -> Rc<RefCell<DomTree>> {
        let tree = Rc::new(RefCell::new(DomTree::new()));
        {
            let mut t = tree.borrow_mut();
            let html = t.create_element("html");
            let body = t.create_element("body");
            let div = t.create_element("div");
            let span1 = t.create_element("span");
            let span2 = t.create_element("span");

            // Set id="parent" on the div
            if let NodeData::Element { ref mut attributes, .. } = t.get_node_mut(div).data {
                attributes.push(("id".to_string(), "parent".to_string()));
            }
            // Set id="child1" on first span
            if let NodeData::Element { ref mut attributes, .. } = t.get_node_mut(span1).data {
                attributes.push(("id".to_string(), "child1".to_string()));
            }
            // Set id="child2" on second span
            if let NodeData::Element { ref mut attributes, .. } = t.get_node_mut(span2).data {
                attributes.push(("id".to_string(), "child2".to_string()));
            }

            let doc = t.document();
            t.append_child(doc, html);
            t.append_child(html, body);
            t.append_child(body, div);
            t.append_child(div, span1);
            t.append_child(div, span2);
        }
        tree
    }

    #[test]
    fn parent_node_returns_parent() {
        let tree = make_traversal_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        let result = rt
            .eval(
                r#"
            var child = document.getElementById("child1");
            child.parentNode !== null
        "#,
            )
            .unwrap();

        assert!(result.to_boolean());
    }

    #[test]
    fn parent_element_returns_parent_element() {
        let tree = make_traversal_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        let result = rt
            .eval(
                r#"
            var child = document.getElementById("child1");
            child.parentElement !== null
        "#,
            )
            .unwrap();

        assert!(result.to_boolean());
    }

    #[test]
    fn parent_element_returns_null_for_root() {
        let tree = make_traversal_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        // html's parent is Document, so parentElement should be null
        let result = rt
            .eval(
                r#"
            var parent = document.getElementById("parent");
            var body = parent.parentNode; // body
            var html = body.parentNode;   // html
            html.parentElement === null
        "#,
            )
            .unwrap();

        assert!(result.to_boolean());
    }

    #[test]
    fn first_child_returns_first_child() {
        let tree = make_traversal_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        let result = rt
            .eval(
                r#"
            var parent = document.getElementById("parent");
            parent.firstChild !== null
        "#,
            )
            .unwrap();

        assert!(result.to_boolean());
    }

    #[test]
    fn last_child_returns_last_child() {
        let tree = make_traversal_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        let result = rt
            .eval(
                r#"
            var parent = document.getElementById("parent");
            parent.lastChild !== null
        "#,
            )
            .unwrap();

        assert!(result.to_boolean());
    }

    #[test]
    fn next_sibling_returns_next_sibling() {
        let tree = make_traversal_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        let result = rt
            .eval(
                r#"
            var child1 = document.getElementById("child1");
            child1.nextSibling !== null
        "#,
            )
            .unwrap();

        assert!(result.to_boolean());
    }

    #[test]
    fn previous_sibling_returns_previous_sibling() {
        let tree = make_traversal_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        let result = rt
            .eval(
                r#"
            var child2 = document.getElementById("child2");
            child2.previousSibling !== null
        "#,
            )
            .unwrap();

        assert!(result.to_boolean());
    }

    #[test]
    fn child_nodes_returns_array_with_correct_length() {
        let tree = make_traversal_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        let result = rt
            .eval(
                r#"
            var parent = document.getElementById("parent");
            parent.childNodes.length
        "#,
            )
            .unwrap();

        let length = result.to_i32(&mut rt.context).unwrap();
        assert_eq!(length, 2); // two span children
    }

    #[test]
    fn children_returns_array_with_correct_length() {
        let tree = make_traversal_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        let result = rt
            .eval(
                r#"
            var parent = document.getElementById("parent");
            parent.children.length
        "#,
            )
            .unwrap();

        let length = result.to_i32(&mut rt.context).unwrap();
        assert_eq!(length, 2); // two span children
    }

    #[test]
    fn children_filters_text_nodes() {
        let tree = Rc::new(RefCell::new(DomTree::new()));
        {
            let mut t = tree.borrow_mut();
            let html = t.create_element("html");
            let body = t.create_element("body");
            let div = t.create_element("div");
            let span = t.create_element("span");
            let text = t.create_text("some text");

            // Set id="parent" on the div
            if let NodeData::Element { ref mut attributes, .. } = t.get_node_mut(div).data {
                attributes.push(("id".to_string(), "parent".to_string()));
            }

            let doc = t.document();
            t.append_child(doc, html);
            t.append_child(html, body);
            t.append_child(body, div);
            t.append_child(div, span);
            t.append_child(div, text);
        }

        let mut rt = JsRuntime::new(Rc::clone(&tree));

        let result = rt
            .eval(
                r#"
            var parent = document.getElementById("parent");
            parent.children.length
        "#,
            )
            .unwrap();

        let children_length = result.to_i32(&mut rt.context).unwrap();
        assert_eq!(children_length, 1); // only the span, text node filtered out

        let result2 = rt
            .eval(
                r#"
            var parent = document.getElementById("parent");
            parent.childNodes.length
        "#,
            )
            .unwrap();

        let child_nodes_length = result2.to_i32(&mut rt.context).unwrap();
        assert_eq!(child_nodes_length, 2); // both span and text
    }

    #[test]
    fn has_child_nodes_returns_true_when_has_children() {
        let tree = make_traversal_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        let result = rt
            .eval(
                r#"
            var parent = document.getElementById("parent");
            parent.hasChildNodes()
        "#,
            )
            .unwrap();

        assert!(result.to_boolean());
    }

    #[test]
    fn has_child_nodes_returns_false_when_no_children() {
        let tree = make_traversal_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        let result = rt
            .eval(
                r#"
            var child = document.getElementById("child1");
            child.hasChildNodes()
        "#,
            )
            .unwrap();

        assert!(!result.to_boolean());
    }

    #[test]
    fn traversal_integration_test() {
        let tree = make_traversal_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        // Test complete traversal scenario
        let result = rt
            .eval(
                r#"
            var child1 = document.getElementById("child1");
            var parent = child1.parentNode;
            var child2 = child1.nextSibling;
            var firstChild = parent.firstChild;
            var lastChild = parent.lastChild;

            // Verify relationships
            firstChild !== null &&
            lastChild !== null &&
            child2 !== null &&
            parent.childNodes.length === 2 &&
            parent.children.length === 2 &&
            parent.hasChildNodes() === true &&
            child1.previousSibling === null &&
            child2.nextSibling === null
        "#,
            )
            .unwrap();

        assert!(result.to_boolean());
    }
}
