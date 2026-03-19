use std::rc::Rc;

use boa_engine::{
    class::ClassBuilder, js_string, native_function::NativeFunction, property::Attribute, Context, JsResult, JsValue,
};

use crate::js::realm_state;

use super::collections;
use super::element::get_or_create_js_element;

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

    // getRootNode() method
    class.method(js_string!("getRootNode"), 0, NativeFunction::from_fn_ptr(get_root_node));

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
/// When the parent is the Document node of the *main* document tree, returns
/// the global `document` object so that `node.parentNode === document` holds true.
/// For foreign documents, returns the JsElement wrapper for that document node.
fn get_parent_node(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "parentNode getter");
    let tree = el.tree.borrow();
    match tree.get_parent(el.node_id) {
        Some(parent_id) => {
            // If the parent is the Document node, check if this is the main document tree.
            // Only return the global `document` object for the main tree.
            if matches!(tree.get_node(parent_id).data, crate::dom::NodeData::Document) {
                drop(tree);
                let global = ctx.global_object();
                let doc_val = global.get(js_string!("document"), ctx)?;
                if let Some(doc_obj) = doc_val.as_object() {
                    if let Some(doc) = doc_obj.downcast_ref::<super::document::JsDocument>() {
                        if Rc::ptr_eq(&el.tree, &doc.tree) {
                            return Ok(doc_val);
                        }
                    }
                }
                // Foreign document: return the JsElement wrapper for the document node
                let tree_rc = el.tree.clone();
                let js_obj = get_or_create_js_element(parent_id, tree_rc, ctx)?;
                return Ok(js_obj.into());
            }
            let tree_rc = el.tree.clone();
            drop(tree);
            let js_obj = get_or_create_js_element(parent_id, tree_rc, ctx)?;
            Ok(js_obj.into())
        }
        None => Ok(JsValue::null()),
    }
}

/// Native getter for element.parentElement
/// Returns the parent only if it's an Element (not Document)
fn get_parent_element(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "parentElement getter");
    let tree = el.tree.borrow();
    match tree.dom_parent_element(el.node_id) {
        Some(parent_id) => {
            let tree_rc = el.tree.clone();
            drop(tree);
            let js_obj = get_or_create_js_element(parent_id, tree_rc, ctx)?;
            Ok(js_obj.into())
        }
        None => Ok(JsValue::null()),
    }
}

/// Native getter for element.firstChild
fn get_first_child(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "firstChild getter");
    let tree = el.tree.borrow();
    match tree.first_child(el.node_id) {
        Some(child_id) => {
            let tree_rc = el.tree.clone();
            drop(tree);
            let js_obj = get_or_create_js_element(child_id, tree_rc, ctx)?;
            Ok(js_obj.into())
        }
        None => Ok(JsValue::null()),
    }
}

/// Native getter for element.lastChild
fn get_last_child(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "lastChild getter");
    let tree = el.tree.borrow();
    match tree.last_child(el.node_id) {
        Some(child_id) => {
            let tree_rc = el.tree.clone();
            drop(tree);
            let js_obj = get_or_create_js_element(child_id, tree_rc, ctx)?;
            Ok(js_obj.into())
        }
        None => Ok(JsValue::null()),
    }
}

/// Native getter for element.nextSibling
fn get_next_sibling(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "nextSibling getter");
    let tree = el.tree.borrow();
    match tree.next_sibling(el.node_id) {
        Some(sibling_id) => {
            let tree_rc = el.tree.clone();
            drop(tree);
            let js_obj = get_or_create_js_element(sibling_id, tree_rc, ctx)?;
            Ok(js_obj.into())
        }
        None => Ok(JsValue::null()),
    }
}

/// Native getter for element.previousSibling
fn get_previous_sibling(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "previousSibling getter");
    let tree = el.tree.borrow();
    match tree.prev_sibling(el.node_id) {
        Some(sibling_id) => {
            let tree_rc = el.tree.clone();
            drop(tree);
            let js_obj = get_or_create_js_element(sibling_id, tree_rc, ctx)?;
            Ok(js_obj.into())
        }
        None => Ok(JsValue::null()),
    }
}

/// Native getter for element.childNodes
/// Returns a live NodeList of all child nodes (cached per element for identity)
fn get_child_nodes(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "childNodes getter");
    let tree_rc = el.tree.clone();
    let node_id = el.node_id;
    let tree_ptr = Rc::as_ptr(&tree_rc) as usize;
    let cache_key = (tree_ptr, node_id);

    // Check cache first for identity (el.childNodes === el.childNodes)
    let cache = realm_state::child_nodes_cache(ctx);
    if let Some(cached_obj) = cache.borrow().get(&cache_key).cloned() {
        return Ok(cached_obj.into());
    }

    // Create a new live NodeList
    let nodelist = collections::create_live_nodelist(node_id, tree_rc, ctx)?;

    // Cache it
    let cache = realm_state::child_nodes_cache(ctx);
    cache.borrow_mut().insert(cache_key, nodelist.clone());

    Ok(nodelist.into())
}

/// Native getter for element.children
/// Returns a live HTMLCollection of Element-only children (cached per element for identity)
fn get_children(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "children getter");
    let tree_rc = el.tree.clone();
    let node_id = el.node_id;
    let tree_ptr = Rc::as_ptr(&tree_rc) as usize;
    let cache_key = (tree_ptr, node_id);

    // Check cache first for identity
    let cache = realm_state::children_cache(ctx);
    if let Some(cached_obj) = cache.borrow().get(&cache_key).cloned() {
        return Ok(cached_obj.into());
    }

    // Create a new live HTMLCollection
    let htmlcollection = collections::create_live_htmlcollection(node_id, tree_rc, ctx)?;

    // Cache it
    let cache = realm_state::children_cache(ctx);
    cache.borrow_mut().insert(cache_key, htmlcollection.clone());

    Ok(htmlcollection.into())
}

/// Native method for element.hasChildNodes()
/// Returns a boolean indicating whether the element has children
fn has_child_nodes(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "hasChildNodes");
    let tree = el.tree.borrow();
    let has_children = !tree.children(el.node_id).is_empty();
    Ok(JsValue::from(has_children))
}

/// Native method for node.getRootNode(options?)
/// Walks parent chain to root. If options.composed is true, traverses through shadow boundaries.
/// When the root is the Document node, returns the global `document` object to preserve identity.
fn get_root_node(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "getRootNode");
    let tree_rc = el.tree.clone();

    // Parse composed option
    let composed = args
        .first()
        .and_then(|v| v.as_object())
        .map(|obj| {
            obj.get(js_string!("composed"), ctx)
                .ok()
                .and_then(|v| v.as_boolean())
                .unwrap_or(false)
        })
        .unwrap_or(false);

    let root_id = {
        let tree = tree_rc.borrow();
        if composed {
            tree.shadow_including_root_of(el.node_id)
        } else {
            tree.root_of(el.node_id)
        }
    };

    // If the root is the Document node and this is the main tree, return the
    // global `document` object so that `element.getRootNode() === document`.
    // For foreign documents, return the JsElement wrapper.
    {
        let tree = tree_rc.borrow();
        if matches!(tree.get_node(root_id).data, crate::dom::NodeData::Document) {
            drop(tree);
            let global = ctx.global_object();
            let doc_val = global.get(js_string!("document"), ctx)?;
            if let Some(doc_obj) = doc_val.as_object() {
                if let Some(doc) = doc_obj.downcast_ref::<super::document::JsDocument>() {
                    if Rc::ptr_eq(&tree_rc, &doc.tree) {
                        return Ok(doc_val);
                    }
                }
            }
            // Foreign document: return JsElement wrapper
            let js_obj = get_or_create_js_element(root_id, tree_rc, ctx)?;
            return Ok(js_obj.into());
        }
    }

    let js_obj = get_or_create_js_element(root_id, tree_rc, ctx)?;
    Ok(js_obj.into())
}

/// Native getter for element.firstElementChild
fn get_first_element_child(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "firstElementChild getter");
    let tree = el.tree.borrow();
    let element_kids = tree.element_children(el.node_id);
    match element_kids.first().copied() {
        Some(child_id) => {
            let tree_rc = el.tree.clone();
            drop(tree);
            let js_obj = get_or_create_js_element(child_id, tree_rc, ctx)?;
            Ok(js_obj.into())
        }
        None => Ok(JsValue::null()),
    }
}

/// Native getter for element.lastElementChild
fn get_last_element_child(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "lastElementChild getter");
    let tree = el.tree.borrow();
    let element_kids = tree.element_children(el.node_id);
    match element_kids.last().copied() {
        Some(child_id) => {
            let tree_rc = el.tree.clone();
            drop(tree);
            let js_obj = get_or_create_js_element(child_id, tree_rc, ctx)?;
            Ok(js_obj.into())
        }
        None => Ok(JsValue::null()),
    }
}

/// Native getter for element.nextElementSibling
fn get_next_element_sibling(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "nextElementSibling getter");
    let tree = el.tree.borrow();
    match tree.next_sibling_element(el.node_id) {
        Some(sibling_id) => {
            let tree_rc = el.tree.clone();
            drop(tree);
            let js_obj = get_or_create_js_element(sibling_id, tree_rc, ctx)?;
            Ok(js_obj.into())
        }
        None => Ok(JsValue::null()),
    }
}

/// Native getter for element.previousElementSibling
fn get_previous_element_sibling(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "previousElementSibling getter");
    let tree = el.tree.borrow();
    match tree.prev_sibling_element(el.node_id) {
        Some(sibling_id) => {
            let tree_rc = el.tree.clone();
            drop(tree);
            let js_obj = get_or_create_js_element(sibling_id, tree_rc, ctx)?;
            Ok(js_obj.into())
        }
        None => Ok(JsValue::null()),
    }
}

/// Native getter for element.childElementCount
fn get_child_element_count(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "childElementCount getter");
    let tree = el.tree.borrow();
    let count = tree.element_children(el.node_id).len();
    Ok(JsValue::from(count as i32))
}

#[cfg(test)]
mod tests {
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
                attributes.push(crate::dom::node::DomAttribute::new("id", "parent"));
            }
            // Set id="child1" on first span
            if let NodeData::Element { ref mut attributes, .. } = t.get_node_mut(span1).data {
                attributes.push(crate::dom::node::DomAttribute::new("id", "child1"));
            }
            // Set id="child2" on second span
            if let NodeData::Element { ref mut attributes, .. } = t.get_node_mut(span2).data {
                attributes.push(crate::dom::node::DomAttribute::new("id", "child2"));
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
    fn nodelist_wpt_iterator_behavior() {
        let tree = make_traversal_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        // Reproduce the exact WPT test "Iterator behavior of Node.childNodes"
        let result = rt
            .eval(
                r#"
            var node = document.createElement("div");
            var kid1 = document.createElement("p");
            var kid2 = document.createTextNode("hey");
            var kid3 = document.createElement("span");
            node.appendChild(kid1);
            node.appendChild(kid2);
            node.appendChild(kid3);

            var list = node.childNodes;

            var keys = list.keys();
            var keysIsArray = keys instanceof Array;

            keys = [...keys];
            var keysCorrect = (keys.length === 3 && keys[0] === 0 && keys[1] === 1 && keys[2] === 2);

            var values = list.values();
            var valuesIsArray = values instanceof Array;

            values = [...values];
            var valuesCorrect = (values.length === 3 && values[0] === kid1 && values[1] === kid2 && values[2] === kid3);

            var entries = list.entries();
            var entriesIsArray = entries instanceof Array;

            entries = [...entries];
            var entriesCorrect = (entries.length === 3);

            var cur = 0;
            var thisObj = {};
            list.forEach(function(value, key, listObj) {
                if (listObj !== list) throw new Error("listObj !== list");
                if (this !== thisObj) throw new Error("this !== thisObj");
                cur++;
            }, thisObj);

            var forEachOk = (cur === 3);

            var symbolIterEq = (list[Symbol.iterator] === Array.prototype[Symbol.iterator]);
            var keysEq = (list.keys === Array.prototype.keys);
            var forEachEq = (list.forEach === Array.prototype.forEach);

            JSON.stringify({
                keysIsArray: keysIsArray,
                keysCorrect: keysCorrect,
                valuesIsArray: valuesIsArray,
                valuesCorrect: valuesCorrect,
                entriesIsArray: entriesIsArray,
                entriesCorrect: entriesCorrect,
                forEachOk: forEachOk,
                symbolIterEq: symbolIterEq,
                keysEq: keysEq,
                forEachEq: forEachEq
            })
        "#,
            )
            .unwrap();

        let s = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        eprintln!("WPT Iterator test: {}", s);

        assert!(
            s.contains("\"keysIsArray\":false"),
            "keys() should not be Array, got: {}",
            s
        );
        assert!(
            s.contains("\"keysCorrect\":true"),
            "keys() values should be correct, got: {}",
            s
        );
        assert!(s.contains("\"forEachOk\":true"), "forEach should work, got: {}", s);
        assert!(
            s.contains("\"symbolIterEq\":true"),
            "Symbol.iterator identity, got: {}",
            s
        );
        assert!(s.contains("\"keysEq\":true"), "keys identity, got: {}", s);
    }

    #[test]
    fn nodelist_has_iterator_methods() {
        let tree = make_traversal_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        let result = rt
            .eval(
                r#"
            var node = document.createElement("div");
            var kid1 = document.createElement("p");
            var kid2 = document.createTextNode("hey");
            var kid3 = document.createElement("span");
            node.appendChild(kid1);
            node.appendChild(kid2);
            node.appendChild(kid3);

            var list = node.childNodes;
            var results = [];
            results.push("length=" + list.length);
            results.push("typeof_keys=" + typeof list.keys);
            results.push("typeof_forEach=" + typeof list.forEach);
            results.push("typeof_values=" + typeof list.values);
            results.push("typeof_entries=" + typeof list.entries);
            results.push("typeof_item=" + typeof list.item);
            results.push("has_symbol_iter=" + (Symbol.iterator in list));

            // Try calling keys
            try {
                var k = list.keys();
                results.push("keys_callable=true");
                results.push("keys_instanceof_array=" + (k instanceof Array));
            } catch(e) {
                results.push("keys_error=" + e.message);
            }

            // Try spread
            try {
                var spread = [...list];
                results.push("spread_length=" + spread.length);
            } catch(e) {
                results.push("spread_error=" + e.message);
            }

            // Try forEach
            try {
                var count = 0;
                list.forEach(function() { count++; });
                results.push("forEach_count=" + count);
            } catch(e) {
                results.push("forEach_error=" + e.message);
            }

            // Check identity with Array.prototype methods
            results.push("keys_eq_array=" + (list.keys === Array.prototype.keys));
            results.push("forEach_eq_array=" + (list.forEach === Array.prototype.forEach));
            results.push("iter_eq_array=" + (list[Symbol.iterator] === Array.prototype[Symbol.iterator]));

            results.join("|")
        "#,
            )
            .unwrap();

        let s = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        eprintln!("NodeList debug: {}", s);

        assert!(
            s.contains("typeof_keys=function"),
            "keys should be function, got: {}",
            s
        );
        assert!(
            s.contains("typeof_forEach=function"),
            "forEach should be function, got: {}",
            s
        );
        assert!(s.contains("keys_callable=true"), "keys should be callable, got: {}", s);
        assert!(s.contains("spread_length=3"), "spread should have 3 items, got: {}", s);
        assert!(s.contains("forEach_count=3"), "forEach should run 3 times, got: {}", s);
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
                attributes.push(crate::dom::node::DomAttribute::new("id", "parent"));
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
