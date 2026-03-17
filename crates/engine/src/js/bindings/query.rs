//! JS bindings for CSS selector-based element querying.
//!
//! Implements querySelector, querySelectorAll, getElementsByClassName, and
//! getElementsByTagName on Element and document objects.

use boa_engine::{
    class::ClassBuilder, js_string, native_function::NativeFunction, Context, JsError, JsResult, JsValue,
};

use crate::css::matching;
use crate::dom::node::NodeData;
use crate::dom::{DomTree, NodeId};

use super::collections;
use super::element::get_or_create_js_element;

// ---------------------------------------------------------------------------
// DocumentFragment.prototype.getElementById (NonElementParentNode mixin)
// ---------------------------------------------------------------------------

/// Search descendants of a DocumentFragment (or any JsElement node) for the
/// first Element with a matching `id` attribute. Returns null if not found.
/// Per spec, empty-string id never matches.
pub(crate) fn fragment_get_element_by_id(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "getElementById");
    let tree = el.tree.clone();
    let root_id = el.node_id;

    let id = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    // Per spec, empty-string ID never matches
    if id.is_empty() {
        return Ok(JsValue::null());
    }

    let found = find_by_id_in_subtree(&tree.borrow(), root_id, &id);
    match found {
        Some(node_id) => {
            let js_obj = get_or_create_js_element(node_id, tree, ctx)?;
            Ok(js_obj.into())
        }
        None => Ok(JsValue::null()),
    }
}

/// Depth-first search for an element with the given id in the subtree rooted at `root`.
/// Does NOT include `root` itself (only descendants).
fn find_by_id_in_subtree(tree: &DomTree, root: NodeId, id: &str) -> Option<NodeId> {
    let children: Vec<NodeId> = tree.get_node(root).children.clone();
    for child_id in children {
        if let NodeData::Element { ref attributes, .. } = tree.get_node(child_id).data {
            if attributes.iter().any(|a| a.local_name == "id" && a.value == id) {
                return Some(child_id);
            }
        }
        if let Some(found) = find_by_id_in_subtree(tree, child_id, id) {
            return Some(found);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Element methods
// ---------------------------------------------------------------------------

fn element_query_selector(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "querySelector");
    let selector = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    let tree_rc = el.tree.clone();
    let node_id = el.node_id;
    let tree = tree_rc.borrow();
    let result = matching::query_selector(&tree, node_id, &selector, Some(node_id));
    drop(tree);

    match result {
        Some(found_id) => {
            let js_obj = get_or_create_js_element(found_id, tree_rc, ctx)?;
            Ok(js_obj.into())
        }
        None => Ok(JsValue::null()),
    }
}

fn element_query_selector_all(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "querySelectorAll");
    let selector = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    let tree_rc = el.tree.clone();
    let node_id = el.node_id;
    let tree = tree_rc.borrow();
    let results = matching::query_selector_all(&tree, node_id, &selector, Some(node_id));
    drop(tree);

    let nodelist = collections::create_static_nodelist(results, tree_rc, ctx)?;
    Ok(nodelist.into())
}

fn element_get_elements_by_class_name(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "getElementsByClassName");
    let class_name = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    let tree_rc = el.tree.clone();
    let node_id = el.node_id;

    let collection = collections::create_live_htmlcollection_by_class(node_id, tree_rc, class_name, ctx)?;
    Ok(collection.into())
}

fn element_get_elements_by_tag_name(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "getElementsByTagName");
    let tag_name = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    let tree_rc = el.tree.clone();
    let node_id = el.node_id;

    let collection = collections::create_live_htmlcollection_by_tag(node_id, tree_rc, tag_name, ctx)?;
    Ok(collection.into())
}

// ---------------------------------------------------------------------------
// Document-level query functions
// ---------------------------------------------------------------------------

use super::document::JsDocument;

pub(crate) fn document_query_selector(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("document.querySelector: `this` is not an object").into()))?;
    let doc = obj
        .downcast_ref::<JsDocument>()
        .ok_or_else(|| JsError::from_opaque(js_string!("document.querySelector: `this` is not document").into()))?;
    let selector = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    let tree_rc = doc.tree.clone();
    let tree = tree_rc.borrow();
    let root = tree.document();
    let result = matching::query_selector(&tree, root, &selector, None);
    drop(tree);

    match result {
        Some(found_id) => {
            let js_obj = get_or_create_js_element(found_id, tree_rc, ctx)?;
            Ok(js_obj.into())
        }
        None => Ok(JsValue::null()),
    }
}

pub(crate) fn document_query_selector_all(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("document.querySelectorAll: `this` is not an object").into()))?;
    let doc = obj
        .downcast_ref::<JsDocument>()
        .ok_or_else(|| JsError::from_opaque(js_string!("document.querySelectorAll: `this` is not document").into()))?;
    let selector = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    let tree_rc = doc.tree.clone();
    let tree = tree_rc.borrow();
    let root = tree.document();
    let results = matching::query_selector_all(&tree, root, &selector, None);
    drop(tree);

    let nodelist = collections::create_static_nodelist(results, tree_rc, ctx)?;
    Ok(nodelist.into())
}

pub(crate) fn document_get_elements_by_class_name(
    this: &JsValue,
    args: &[JsValue],
    ctx: &mut Context,
) -> JsResult<JsValue> {
    let obj = this.as_object().ok_or_else(|| {
        JsError::from_opaque(js_string!("document.getElementsByClassName: `this` is not an object").into())
    })?;
    let doc = obj.downcast_ref::<JsDocument>().ok_or_else(|| {
        JsError::from_opaque(js_string!("document.getElementsByClassName: `this` is not document").into())
    })?;
    let class_name = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    let tree_rc = doc.tree.clone();
    let root = tree_rc.borrow().document();

    let collection = collections::create_live_htmlcollection_by_class(root, tree_rc, class_name, ctx)?;
    Ok(collection.into())
}

pub(crate) fn document_get_elements_by_tag_name(
    this: &JsValue,
    args: &[JsValue],
    ctx: &mut Context,
) -> JsResult<JsValue> {
    let obj = this.as_object().ok_or_else(|| {
        JsError::from_opaque(js_string!("document.getElementsByTagName: `this` is not an object").into())
    })?;
    let doc = obj.downcast_ref::<JsDocument>().ok_or_else(|| {
        JsError::from_opaque(js_string!("document.getElementsByTagName: `this` is not document").into())
    })?;
    let tag_name = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    let tree_rc = doc.tree.clone();
    let root = tree_rc.borrow().document();

    let collection = collections::create_live_htmlcollection_by_tag(root, tree_rc, tag_name, ctx)?;
    Ok(collection.into())
}

pub(crate) fn document_get_elements_by_tag_name_ns(
    this: &JsValue,
    args: &[JsValue],
    ctx: &mut Context,
) -> JsResult<JsValue> {
    let obj = this.as_object().ok_or_else(|| {
        JsError::from_opaque(js_string!("document.getElementsByTagNameNS: `this` is not an object").into())
    })?;
    let doc = obj.downcast_ref::<JsDocument>().ok_or_else(|| {
        JsError::from_opaque(js_string!("document.getElementsByTagNameNS: `this` is not document").into())
    })?;

    // First arg: namespace (null/undefined -> empty string)
    let namespace = match args.first() {
        Some(v) if !v.is_null() && !v.is_undefined() => v.to_string(ctx)?.to_std_string_escaped(),
        _ => String::new(),
    };

    // Second arg: local name
    let local_name = args
        .get(1)
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    let tree_rc = doc.tree.clone();
    let root = tree_rc.borrow().document();

    let collection = collections::create_live_htmlcollection_by_tag_name_ns(root, tree_rc, namespace, local_name, ctx)?;
    Ok(collection.into())
}

fn element_get_elements_by_tag_name_ns(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "getElementsByTagNameNS");

    // First arg: namespace (null/undefined -> empty string)
    let namespace = match args.first() {
        Some(v) if !v.is_null() && !v.is_undefined() => v.to_string(ctx)?.to_std_string_escaped(),
        _ => String::new(),
    };

    // Second arg: local name
    let local_name = args
        .get(1)
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    let tree_rc = el.tree.clone();
    let node_id = el.node_id;

    let collection =
        collections::create_live_htmlcollection_by_tag_name_ns(node_id, tree_rc, namespace, local_name, ctx)?;
    Ok(collection.into())
}

// ---------------------------------------------------------------------------
// Element.matches() and Element.closest()
// ---------------------------------------------------------------------------

fn element_matches(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "matches");

    // Per spec, matches() requires 1 argument — missing arg throws TypeError
    if args.is_empty() {
        return Err(boa_engine::JsNativeError::typ()
            .with_message("Failed to execute 'matches' on 'Element': 1 argument required, but only 0 present.")
            .into());
    }
    let selector = args[0].to_string(ctx)?.to_std_string_escaped();

    // Parse the selector — if invalid, throw a DOMException (SyntaxError)
    let tree_rc = el.tree.clone();
    let node_id = el.node_id;
    let tree = tree_rc.borrow();

    // Try parsing first to detect invalid selectors
    let mut parser_input = cssparser::ParserInput::new(&selector);
    let mut parser = cssparser::Parser::new(&mut parser_input);
    if selectors::parser::SelectorList::parse(
        &crate::css::selector_impl::BrailleSelectorParser,
        &mut parser,
        selectors::parser::ParseRelative::No,
    )
    .is_err()
    {
        drop(tree);
        let exc = super::create_dom_exception(ctx, "SyntaxError", &format!("'{}' is not a valid selector", selector), 12)?;
        return Err(JsError::from_opaque(exc.into()));
    }

    let result = matching::matches_selector_str(&tree, node_id, &selector, Some(node_id));
    Ok(JsValue::from(result))
}

fn element_closest(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "closest");
    let selector = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    let tree_rc = el.tree.clone();
    let tree = tree_rc.borrow();
    let scope_id = el.node_id;
    let mut current = el.node_id;
    loop {
        if matches!(tree.get_node(current).data, NodeData::Element { .. })
            && matching::matches_selector_str(&tree, current, &selector, Some(scope_id))
        {
            drop(tree);
            let js_obj = get_or_create_js_element(current, tree_rc, ctx)?;
            return Ok(js_obj.into());
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
        js_string!("getElementsByTagNameNS"),
        2,
        NativeFunction::from_fn_ptr(element_get_elements_by_tag_name_ns),
    );
    class.method(js_string!("matches"), 1, NativeFunction::from_fn_ptr(element_matches));
    class.method(js_string!("closest"), 1, NativeFunction::from_fn_ptr(element_closest));
    class.method(
        js_string!("webkitMatchesSelector"),
        1,
        NativeFunction::from_fn_ptr(element_matches),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::dom::node::DomAttribute;
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
                attributes.push(DomAttribute::new("class", "container"));
            }

            let p1 = t.create_element("p");
            if let NodeData::Element { ref mut attributes, .. } = t.get_node_mut(p1).data {
                attributes.push(DomAttribute::new("id", "first"));
            }
            t.set_text_content(p1, "First paragraph");

            let p2 = t.create_element("p");
            if let NodeData::Element { ref mut attributes, .. } = t.get_node_mut(p2).data {
                attributes.push(DomAttribute::new("class", "highlight"));
            }
            t.set_text_content(p2, "Second paragraph");

            let nested = t.create_element("div");
            if let NodeData::Element { ref mut attributes, .. } = t.get_node_mut(nested).data {
                attributes.push(DomAttribute::new("class", "nested"));
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
        let result = rt
            .eval(
                r##"
            var body = document.body;
            var p = body.querySelector("p");
            p.textContent
        "##,
            )
            .unwrap();
        let text = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(text, "First paragraph");
    }

    #[test]
    fn query_selector_by_class() {
        let tree = make_query_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt
            .eval(
                r##"
            var body = document.body;
            var el = body.querySelector(".highlight");
            el.textContent
        "##,
            )
            .unwrap();
        let text = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(text, "Second paragraph");
    }

    #[test]
    fn query_selector_by_id() {
        let tree = make_query_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt
            .eval(
                r##"
            var body = document.body;
            var el = body.querySelector("#first");
            el.textContent
        "##,
            )
            .unwrap();
        let text = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(text, "First paragraph");
    }

    #[test]
    fn query_selector_returns_null_for_no_match() {
        let tree = make_query_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt
            .eval(
                r##"
            var body = document.body;
            body.querySelector(".nonexistent")
        "##,
            )
            .unwrap();
        assert!(result.is_null());
    }

    #[test]
    fn query_selector_all_returns_array() {
        let tree = make_query_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt
            .eval(
                r##"
            var body = document.body;
            var elems = body.querySelectorAll("p");
            elems.length
        "##,
            )
            .unwrap();
        let length = result.to_i32(&mut rt.context).unwrap();
        assert_eq!(length, 3);
    }

    #[test]
    fn query_selector_all_empty_for_no_match() {
        let tree = make_query_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt
            .eval(
                r##"
            var body = document.body;
            var elems = body.querySelectorAll(".nonexistent");
            elems.length
        "##,
            )
            .unwrap();
        let length = result.to_i32(&mut rt.context).unwrap();
        assert_eq!(length, 0);
    }

    #[test]
    fn get_elements_by_class_name() {
        let tree = make_query_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt
            .eval(
                r##"
            var body = document.body;
            var elems = body.getElementsByClassName("highlight");
            elems.length
        "##,
            )
            .unwrap();
        let length = result.to_i32(&mut rt.context).unwrap();
        assert_eq!(length, 1);

        let result2 = rt
            .eval(
                r##"
            var body = document.body;
            var elems = body.getElementsByClassName("highlight");
            elems[0].textContent
        "##,
            )
            .unwrap();
        let text = result2.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(text, "Second paragraph");
    }

    #[test]
    fn get_elements_by_tag_name() {
        let tree = make_query_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt
            .eval(
                r##"
            var body = document.body;
            var elems = body.getElementsByTagName("div");
            elems.length
        "##,
            )
            .unwrap();
        let length = result.to_i32(&mut rt.context).unwrap();
        assert_eq!(length, 2);
    }

    #[test]
    fn document_query_selector_test() {
        let tree = make_query_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt
            .eval(
                r##"
            var el = document.querySelector("#first");
            el.textContent
        "##,
            )
            .unwrap();
        let text = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(text, "First paragraph");
    }

    #[test]
    fn document_query_selector_all_test() {
        let tree = make_query_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt
            .eval(
                r##"
            var elems = document.querySelectorAll("p");
            elems.length
        "##,
            )
            .unwrap();
        let length = result.to_i32(&mut rt.context).unwrap();
        assert_eq!(length, 3);
    }

    #[test]
    fn complex_selector_descendant() {
        let tree = make_query_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt
            .eval(
                r##"
            var el = document.querySelector("div.container p");
            el.textContent
        "##,
            )
            .unwrap();
        let text = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(text, "First paragraph");
    }

    #[test]
    fn complex_selector_nested_span() {
        let tree = make_query_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt
            .eval(
                r##"
            var el = document.querySelector(".nested span");
            el.textContent
        "##,
            )
            .unwrap();
        let text = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(text, "Nested span");
    }

    #[test]
    fn document_get_elements_by_class_name_test() {
        let tree = make_query_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt
            .eval(
                r##"
            var elems = document.getElementsByClassName("container");
            elems.length
        "##,
            )
            .unwrap();
        let length = result.to_i32(&mut rt.context).unwrap();
        assert_eq!(length, 1);
    }

    #[test]
    fn document_get_elements_by_tag_name_test() {
        let tree = make_query_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt
            .eval(
                r##"
            var elems = document.getElementsByTagName("p");
            elems.length
        "##,
            )
            .unwrap();
        let length = result.to_i32(&mut rt.context).unwrap();
        assert_eq!(length, 3);
    }

    #[test]
    fn get_elements_by_tag_name_case_insensitive() {
        let tree = make_query_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt
            .eval(
                r##"
            var elems = document.getElementsByTagName("P");
            elems.length
        "##,
            )
            .unwrap();
        let length = result.to_i32(&mut rt.context).unwrap();
        assert_eq!(length, 3);
    }
}
