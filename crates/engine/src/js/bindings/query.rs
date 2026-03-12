//! JS bindings for CSS selector-based element querying.
//!
//! Implements querySelector, querySelectorAll, getElementsByClassName, and
//! getElementsByTagName on Element and document objects.

use boa_engine::{
    class::{Class, ClassBuilder},
    js_string,
    native_function::NativeFunction,
    object::builtins::JsArray,
    Context, JsError, JsResult, JsValue,
};

use crate::css::matching;
use crate::dom::node::NodeData;
use crate::dom::{DomTree, NodeId};

use super::element::JsElement;

// ---------------------------------------------------------------------------
// Helper functions for collecting elements by class name and tag name
// ---------------------------------------------------------------------------

fn collect_by_class(tree: &DomTree, root: NodeId, class_name: &str) -> Vec<NodeId> {
    let mut results = Vec::new();
    let children: Vec<NodeId> = tree.get_node(root).children.clone();
    for child_id in children {
        collect_by_class_recursive(tree, child_id, class_name, &mut results);
    }
    results
}

fn collect_by_class_recursive(
    tree: &DomTree,
    node_id: NodeId,
    class_name: &str,
    results: &mut Vec<NodeId>,
) {
    let node = tree.get_node(node_id);
    if let NodeData::Element { ref attributes, .. } = node.data {
        if let Some(class_attr) = attributes
            .iter()
            .find(|(k, _)| k == "class")
            .map(|(_, v)| v.as_str())
        {
            if class_attr.split_whitespace().any(|c| c == class_name) {
                results.push(node_id);
            }
        }
    }
    let children: Vec<NodeId> = node.children.clone();
    for child_id in children {
        collect_by_class_recursive(tree, child_id, class_name, results);
    }
}

fn collect_by_tag(tree: &DomTree, root: NodeId, tag_name: &str) -> Vec<NodeId> {
    let mut results = Vec::new();
    let tag_lower = tag_name.to_ascii_lowercase();
    let children: Vec<NodeId> = tree.get_node(root).children.clone();
    for child_id in children {
        collect_by_tag_recursive(tree, child_id, &tag_lower, &mut results);
    }
    results
}

fn collect_by_tag_recursive(
    tree: &DomTree,
    node_id: NodeId,
    tag_lower: &str,
    results: &mut Vec<NodeId>,
) {
    let node = tree.get_node(node_id);
    if let NodeData::Element { ref tag_name, .. } = node.data {
        if tag_lower == "*" || tag_name.to_ascii_lowercase() == tag_lower {
            results.push(node_id);
        }
    }
    let children: Vec<NodeId> = node.children.clone();
    for child_id in children {
        collect_by_tag_recursive(tree, child_id, tag_lower, results);
    }
}

// ---------------------------------------------------------------------------
// Element methods
// ---------------------------------------------------------------------------

fn element_query_selector(
    this: &JsValue,
    args: &[JsValue],
    ctx: &mut Context,
) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| {
            JsError::from_opaque(js_string!("querySelector: `this` is not an object").into())
        })?;
    let el = obj.downcast_ref::<JsElement>().ok_or_else(|| {
        JsError::from_opaque(js_string!("querySelector: `this` is not an Element").into())
    })?;
    let selector = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    let tree_rc = el.tree.clone();
    let node_id = el.node_id;
    let tree = tree_rc.borrow();
    let result = matching::query_selector(&tree, node_id, &selector);
    drop(tree);

    match result {
        Some(found_id) => {
            let element = JsElement::new(found_id, tree_rc);
            let js_obj = JsElement::from_data(element, ctx)?;
            Ok(js_obj.into())
        }
        None => Ok(JsValue::null()),
    }
}

fn element_query_selector_all(
    this: &JsValue,
    args: &[JsValue],
    ctx: &mut Context,
) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| {
            JsError::from_opaque(
                js_string!("querySelectorAll: `this` is not an object").into(),
            )
        })?;
    let el = obj.downcast_ref::<JsElement>().ok_or_else(|| {
        JsError::from_opaque(
            js_string!("querySelectorAll: `this` is not an Element").into(),
        )
    })?;
    let selector = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    let tree_rc = el.tree.clone();
    let node_id = el.node_id;
    let tree = tree_rc.borrow();
    let results = matching::query_selector_all(&tree, node_id, &selector);
    drop(tree);

    let arr = JsArray::new(ctx);
    for found_id in results {
        let element = JsElement::new(found_id, tree_rc.clone());
        let js_obj = JsElement::from_data(element, ctx)?;
        arr.push(js_obj, ctx)?;
    }
    Ok(arr.into())
}

fn element_get_elements_by_class_name(
    this: &JsValue,
    args: &[JsValue],
    ctx: &mut Context,
) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| {
            JsError::from_opaque(
                js_string!("getElementsByClassName: `this` is not an object").into(),
            )
        })?;
    let el = obj.downcast_ref::<JsElement>().ok_or_else(|| {
        JsError::from_opaque(
            js_string!("getElementsByClassName: `this` is not an Element").into(),
        )
    })?;
    let class_name = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    let tree_rc = el.tree.clone();
    let node_id = el.node_id;
    let tree = tree_rc.borrow();
    let results = collect_by_class(&tree, node_id, &class_name);
    drop(tree);

    let arr = JsArray::new(ctx);
    for found_id in results {
        let element = JsElement::new(found_id, tree_rc.clone());
        let js_obj = JsElement::from_data(element, ctx)?;
        arr.push(js_obj, ctx)?;
    }
    Ok(arr.into())
}

fn element_get_elements_by_tag_name(
    this: &JsValue,
    args: &[JsValue],
    ctx: &mut Context,
) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| {
            JsError::from_opaque(
                js_string!("getElementsByTagName: `this` is not an object").into(),
            )
        })?;
    let el = obj.downcast_ref::<JsElement>().ok_or_else(|| {
        JsError::from_opaque(
            js_string!("getElementsByTagName: `this` is not an Element").into(),
        )
    })?;
    let tag_name = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    let tree_rc = el.tree.clone();
    let node_id = el.node_id;
    let tree = tree_rc.borrow();
    let results = collect_by_tag(&tree, node_id, &tag_name);
    drop(tree);

    let arr = JsArray::new(ctx);
    for found_id in results {
        let element = JsElement::new(found_id, tree_rc.clone());
        let js_obj = JsElement::from_data(element, ctx)?;
        arr.push(js_obj, ctx)?;
    }
    Ok(arr.into())
}

// ---------------------------------------------------------------------------
// Document-level query functions
// ---------------------------------------------------------------------------

use super::document::JsDocument;

pub(crate) fn document_query_selector(
    this: &JsValue,
    args: &[JsValue],
    ctx: &mut Context,
) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| {
            JsError::from_opaque(
                js_string!("document.querySelector: `this` is not an object").into(),
            )
        })?;
    let doc = obj.downcast_ref::<JsDocument>().ok_or_else(|| {
        JsError::from_opaque(
            js_string!("document.querySelector: `this` is not document").into(),
        )
    })?;
    let selector = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    let tree_rc = doc.tree.clone();
    let tree = tree_rc.borrow();
    let root = tree.document();
    let result = matching::query_selector(&tree, root, &selector);
    drop(tree);

    match result {
        Some(found_id) => {
            let element = JsElement::new(found_id, tree_rc);
            let js_obj = JsElement::from_data(element, ctx)?;
            Ok(js_obj.into())
        }
        None => Ok(JsValue::null()),
    }
}

pub(crate) fn document_query_selector_all(
    this: &JsValue,
    args: &[JsValue],
    ctx: &mut Context,
) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| {
            JsError::from_opaque(
                js_string!("document.querySelectorAll: `this` is not an object").into(),
            )
        })?;
    let doc = obj.downcast_ref::<JsDocument>().ok_or_else(|| {
        JsError::from_opaque(
            js_string!("document.querySelectorAll: `this` is not document").into(),
        )
    })?;
    let selector = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    let tree_rc = doc.tree.clone();
    let tree = tree_rc.borrow();
    let root = tree.document();
    let results = matching::query_selector_all(&tree, root, &selector);
    drop(tree);

    let arr = JsArray::new(ctx);
    for found_id in results {
        let element = JsElement::new(found_id, tree_rc.clone());
        let js_obj = JsElement::from_data(element, ctx)?;
        arr.push(js_obj, ctx)?;
    }
    Ok(arr.into())
}

pub(crate) fn document_get_elements_by_class_name(
    this: &JsValue,
    args: &[JsValue],
    ctx: &mut Context,
) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| {
            JsError::from_opaque(
                js_string!("document.getElementsByClassName: `this` is not an object").into(),
            )
        })?;
    let doc = obj.downcast_ref::<JsDocument>().ok_or_else(|| {
        JsError::from_opaque(
            js_string!("document.getElementsByClassName: `this` is not document").into(),
        )
    })?;
    let class_name = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    let tree_rc = doc.tree.clone();
    let tree = tree_rc.borrow();
    let root = tree.document();
    let results = collect_by_class(&tree, root, &class_name);
    drop(tree);

    let arr = JsArray::new(ctx);
    for found_id in results {
        let element = JsElement::new(found_id, tree_rc.clone());
        let js_obj = JsElement::from_data(element, ctx)?;
        arr.push(js_obj, ctx)?;
    }
    Ok(arr.into())
}

pub(crate) fn document_get_elements_by_tag_name(
    this: &JsValue,
    args: &[JsValue],
    ctx: &mut Context,
) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| {
            JsError::from_opaque(
                js_string!("document.getElementsByTagName: `this` is not an object").into(),
            )
        })?;
    let doc = obj.downcast_ref::<JsDocument>().ok_or_else(|| {
        JsError::from_opaque(
            js_string!("document.getElementsByTagName: `this` is not document").into(),
        )
    })?;
    let tag_name = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    let tree_rc = doc.tree.clone();
    let tree = tree_rc.borrow();
    let root = tree.document();
    let results = collect_by_tag(&tree, root, &tag_name);
    drop(tree);

    let arr = JsArray::new(ctx);
    for found_id in results {
        let element = JsElement::new(found_id, tree_rc.clone());
        let js_obj = JsElement::from_data(element, ctx)?;
        arr.push(js_obj, ctx)?;
    }
    Ok(arr.into())
}

// ---------------------------------------------------------------------------
// Element.matches() and Element.closest()
// ---------------------------------------------------------------------------

fn element_matches(
    this: &JsValue,
    args: &[JsValue],
    ctx: &mut Context,
) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("matches: `this` is not an object").into()))?;
    let el = obj.downcast_ref::<JsElement>().ok_or_else(|| {
        JsError::from_opaque(js_string!("matches: `this` is not an Element").into())
    })?;
    let selector = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    let tree_rc = el.tree.clone();
    let node_id = el.node_id;
    let tree = tree_rc.borrow();
    let result = matching::matches_selector_str(&tree, node_id, &selector);
    Ok(JsValue::from(result))
}

fn element_closest(
    this: &JsValue,
    args: &[JsValue],
    ctx: &mut Context,
) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("closest: `this` is not an object").into()))?;
    let el = obj.downcast_ref::<JsElement>().ok_or_else(|| {
        JsError::from_opaque(js_string!("closest: `this` is not an Element").into())
    })?;
    let selector = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    let tree_rc = el.tree.clone();
    let tree = tree_rc.borrow();
    let mut current = el.node_id;
    loop {
        if matches!(tree.get_node(current).data, NodeData::Element { .. }) {
            if matching::matches_selector_str(&tree, current, &selector) {
                drop(tree);
                let element = JsElement::new(current, tree_rc);
                let js_obj = JsElement::from_data(element, ctx)?;
                return Ok(js_obj.into());
            }
        }
        match tree.get_node(current).parent {
            Some(parent_id) => current = parent_id,
            None => return Ok(JsValue::null()),
        }
    }
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

pub(crate) fn register_query(class: &mut ClassBuilder) -> JsResult<()> {
    class.method(
        js_string!("querySelector"),
        1,
        NativeFunction::from_fn_ptr(element_query_selector),
    );
    class.method(
        js_string!("querySelectorAll"),
        1,
        NativeFunction::from_fn_ptr(element_query_selector_all),
    );
    class.method(
        js_string!("getElementsByClassName"),
        1,
        NativeFunction::from_fn_ptr(element_get_elements_by_class_name),
    );
    class.method(
        js_string!("getElementsByTagName"),
        1,
        NativeFunction::from_fn_ptr(element_get_elements_by_tag_name),
    );
    class.method(
        js_string!("matches"),
        1,
        NativeFunction::from_fn_ptr(element_matches),
    );
    class.method(
        js_string!("closest"),
        1,
        NativeFunction::from_fn_ptr(element_closest),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::dom::{DomTree, NodeData};
    use crate::js::runtime::JsRuntime;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn make_query_test_tree() -> Rc<RefCell<DomTree>> {
        let tree = Rc::new(RefCell::new(DomTree::new()));
        {
            let mut t = tree.borrow_mut();
            let html = t.create_element("html");
            let body = t.create_element("body");

            let container = t.create_element("div");
            if let NodeData::Element { ref mut attributes, .. } = t.get_node_mut(container).data {
                attributes.push(("class".to_string(), "container".to_string()));
            }

            let p1 = t.create_element("p");
            if let NodeData::Element { ref mut attributes, .. } = t.get_node_mut(p1).data {
                attributes.push(("id".to_string(), "first".to_string()));
            }
            t.set_text_content(p1, "First paragraph");

            let p2 = t.create_element("p");
            if let NodeData::Element { ref mut attributes, .. } = t.get_node_mut(p2).data {
                attributes.push(("class".to_string(), "highlight".to_string()));
            }
            t.set_text_content(p2, "Second paragraph");

            let nested = t.create_element("div");
            if let NodeData::Element { ref mut attributes, .. } = t.get_node_mut(nested).data {
                attributes.push(("class".to_string(), "nested".to_string()));
            }

            let span = t.create_element("span");
            t.set_text_content(span, "Nested span");

            let p3 = t.create_element("p");
            t.set_text_content(p3, "Third paragraph");

            let doc = t.document();
            t.append_child(doc, html);
            t.append_child(html, body);
            t.append_child(body, container);
            t.append_child(container, p1);
            t.append_child(container, p2);
            t.append_child(container, nested);
            t.append_child(nested, span);
            t.append_child(body, p3);
        }
        tree
    }

    #[test]
    fn query_selector_by_tag() {
        let tree = make_query_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt.eval(r##"
            var body = document.body;
            var p = body.querySelector("p");
            p.textContent
        "##).unwrap();
        let text = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(text, "First paragraph");
    }

    #[test]
    fn query_selector_by_class() {
        let tree = make_query_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt.eval(r##"
            var body = document.body;
            var el = body.querySelector(".highlight");
            el.textContent
        "##).unwrap();
        let text = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(text, "Second paragraph");
    }

    #[test]
    fn query_selector_by_id() {
        let tree = make_query_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt.eval(r##"
            var body = document.body;
            var el = body.querySelector("#first");
            el.textContent
        "##).unwrap();
        let text = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(text, "First paragraph");
    }

    #[test]
    fn query_selector_returns_null_for_no_match() {
        let tree = make_query_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt.eval(r##"
            var body = document.body;
            body.querySelector(".nonexistent")
        "##).unwrap();
        assert!(result.is_null());
    }

    #[test]
    fn query_selector_all_returns_array() {
        let tree = make_query_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt.eval(r##"
            var body = document.body;
            var elems = body.querySelectorAll("p");
            elems.length
        "##).unwrap();
        let length = result.to_i32(&mut rt.context).unwrap();
        assert_eq!(length, 3);
    }

    #[test]
    fn query_selector_all_empty_for_no_match() {
        let tree = make_query_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt.eval(r##"
            var body = document.body;
            var elems = body.querySelectorAll(".nonexistent");
            elems.length
        "##).unwrap();
        let length = result.to_i32(&mut rt.context).unwrap();
        assert_eq!(length, 0);
    }

    #[test]
    fn get_elements_by_class_name() {
        let tree = make_query_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt.eval(r##"
            var body = document.body;
            var elems = body.getElementsByClassName("highlight");
            elems.length
        "##).unwrap();
        let length = result.to_i32(&mut rt.context).unwrap();
        assert_eq!(length, 1);

        let result2 = rt.eval(r##"
            var body = document.body;
            var elems = body.getElementsByClassName("highlight");
            elems[0].textContent
        "##).unwrap();
        let text = result2.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(text, "Second paragraph");
    }

    #[test]
    fn get_elements_by_tag_name() {
        let tree = make_query_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt.eval(r##"
            var body = document.body;
            var elems = body.getElementsByTagName("div");
            elems.length
        "##).unwrap();
        let length = result.to_i32(&mut rt.context).unwrap();
        assert_eq!(length, 2);
    }

    #[test]
    fn document_query_selector_test() {
        let tree = make_query_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt.eval(r##"
            var el = document.querySelector("#first");
            el.textContent
        "##).unwrap();
        let text = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(text, "First paragraph");
    }

    #[test]
    fn document_query_selector_all_test() {
        let tree = make_query_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt.eval(r##"
            var elems = document.querySelectorAll("p");
            elems.length
        "##).unwrap();
        let length = result.to_i32(&mut rt.context).unwrap();
        assert_eq!(length, 3);
    }

    #[test]
    fn complex_selector_descendant() {
        let tree = make_query_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt.eval(r##"
            var el = document.querySelector("div.container p");
            el.textContent
        "##).unwrap();
        let text = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(text, "First paragraph");
    }

    #[test]
    fn complex_selector_nested_span() {
        let tree = make_query_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt.eval(r##"
            var el = document.querySelector(".nested span");
            el.textContent
        "##).unwrap();
        let text = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(text, "Nested span");
    }

    #[test]
    fn document_get_elements_by_class_name_test() {
        let tree = make_query_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt.eval(r##"
            var elems = document.getElementsByClassName("container");
            elems.length
        "##).unwrap();
        let length = result.to_i32(&mut rt.context).unwrap();
        assert_eq!(length, 1);
    }

    #[test]
    fn document_get_elements_by_tag_name_test() {
        let tree = make_query_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt.eval(r##"
            var elems = document.getElementsByTagName("p");
            elems.length
        "##).unwrap();
        let length = result.to_i32(&mut rt.context).unwrap();
        assert_eq!(length, 3);
    }

    #[test]
    fn get_elements_by_tag_name_case_insensitive() {
        let tree = make_query_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt.eval(r##"
            var elems = document.getElementsByTagName("P");
            elems.length
        "##).unwrap();
        let length = result.to_i32(&mut rt.context).unwrap();
        assert_eq!(length, 3);
    }
}
