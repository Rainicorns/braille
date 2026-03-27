use crate::dom::{DomTree, NodeData};
use crate::js::runtime::JsRuntime;
use std::cell::RefCell;
use std::rc::Rc;

    fn make_mutation_test_tree() -> Rc<RefCell<DomTree>> {
        let tree = Rc::new(RefCell::new(DomTree::new()));
        {
            let mut t = tree.borrow_mut();
            let html = t.create_element("html");
            let body = t.create_element("body");
            let div = t.create_element("div");
            let span_a = t.create_element("span");
            let span_b = t.create_element("span");
            let span_c = t.create_element("span");
            if let NodeData::Element { ref mut attributes, .. } = t.get_node_mut(div).data {
                attributes.push(crate::dom::node::DomAttribute::new("id", "parent"));
            }
            if let NodeData::Element { ref mut attributes, .. } = t.get_node_mut(span_a).data {
                attributes.push(crate::dom::node::DomAttribute::new("id", "a"));
            }
            if let NodeData::Element { ref mut attributes, .. } = t.get_node_mut(span_b).data {
                attributes.push(crate::dom::node::DomAttribute::new("id", "b"));
            }
            if let NodeData::Element { ref mut attributes, .. } = t.get_node_mut(span_c).data {
                attributes.push(crate::dom::node::DomAttribute::new("id", "c"));
            }
            let doc = t.document();
            t.append_child(doc, html);
            t.append_child(html, body);
            t.append_child(body, div);
            t.append_child(div, span_a);
            t.append_child(div, span_b);
            t.append_child(div, span_c);
        }
        tree
    }

    #[test]
    fn insert_before_inserts_before_reference_node() {
        let tree = make_mutation_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        rt.eval(
            r#"
            var parent = document.getElementById("parent");
            var b = document.getElementById("b");
            var newNode = document.createElement("p");
            newNode.setAttribute("id", "new");
            parent.insertBefore(newNode, b);
        "#,
        )
        .unwrap();
        let t = tree.borrow();
        let parent_id = t.get_element_by_id("parent").unwrap();
        let children = &t.get_node(parent_id).children;
        assert_eq!(children.len(), 4);
        let new_id = t.get_element_by_id("new").unwrap();
        let a_id = t.get_element_by_id("a").unwrap();
        let b_id = t.get_element_by_id("b").unwrap();
        let c_id = t.get_element_by_id("c").unwrap();
        assert_eq!(children, &vec![a_id, new_id, b_id, c_id]);
    }

    #[test]
    fn insert_before_with_null_reference_appends() {
        let tree = make_mutation_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        rt.eval(
            r#"
            var parent = document.getElementById("parent");
            var newNode = document.createElement("p");
            newNode.setAttribute("id", "new");
            parent.insertBefore(newNode, null);
        "#,
        )
        .unwrap();
        let t = tree.borrow();
        let parent_id = t.get_element_by_id("parent").unwrap();
        let children = &t.get_node(parent_id).children;
        assert_eq!(children.len(), 4);
        let new_id = t.get_element_by_id("new").unwrap();
        assert_eq!(*children.last().unwrap(), new_id);
    }

    #[test]
    fn insert_before_detaches_from_old_parent() {
        let tree = make_mutation_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        rt.eval(
            r#"
            var parent = document.getElementById("parent");
            var a = document.getElementById("a");
            var c = document.getElementById("c");
            parent.insertBefore(a, c);
        "#,
        )
        .unwrap();
        let t = tree.borrow();
        let parent_id = t.get_element_by_id("parent").unwrap();
        let children = &t.get_node(parent_id).children;
        assert_eq!(children.len(), 3);
        let a_id = t.get_element_by_id("a").unwrap();
        let b_id = t.get_element_by_id("b").unwrap();
        let c_id = t.get_element_by_id("c").unwrap();
        assert_eq!(children, &vec![b_id, a_id, c_id]);
    }

    #[test]
    fn replace_child_swaps_nodes_correctly() {
        let tree = make_mutation_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        rt.eval(
            r#"
            var parent = document.getElementById("parent");
            var b = document.getElementById("b");
            var newNode = document.createElement("p");
            newNode.setAttribute("id", "new");
            parent.replaceChild(newNode, b);
        "#,
        )
        .unwrap();
        let t = tree.borrow();
        let parent_id = t.get_element_by_id("parent").unwrap();
        let children = &t.get_node(parent_id).children;
        assert_eq!(children.len(), 3);
        let new_id = t.get_element_by_id("new").unwrap();
        let a_id = t.get_element_by_id("a").unwrap();
        let c_id = t.get_element_by_id("c").unwrap();
        assert_eq!(children, &vec![a_id, new_id, c_id]);
        // After replacement, "b" is disconnected and should not be found via getElementById
        assert!(t.get_element_by_id("b").is_none());
    }

    #[test]
    fn replace_child_detaches_new_child() {
        let tree = make_mutation_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        rt.eval(
            r#"
            var parent = document.getElementById("parent");
            var a = document.getElementById("a");
            var b = document.getElementById("b");
            parent.replaceChild(a, b);
        "#,
        )
        .unwrap();
        let t = tree.borrow();
        let parent_id = t.get_element_by_id("parent").unwrap();
        let children = &t.get_node(parent_id).children;
        assert_eq!(children.len(), 2);
        let a_id = t.get_element_by_id("a").unwrap();
        let c_id = t.get_element_by_id("c").unwrap();
        assert_eq!(children, &vec![a_id, c_id]);
    }

    #[test]
    fn remove_child_removes_and_returns() {
        let tree = make_mutation_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt
            .eval(
                r#"
            var parent = document.getElementById("parent");
            var b = document.getElementById("b");
            var removed = parent.removeChild(b);
            removed.getAttribute("id");
        "#,
            )
            .unwrap();
        let id_str = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(id_str, "b");
        let t = tree.borrow();
        let parent_id = t.get_element_by_id("parent").unwrap();
        assert_eq!(t.get_node(parent_id).children.len(), 2);
        // After removal, "b" is disconnected and should not be found via getElementById
        assert!(t.get_element_by_id("b").is_none());
    }

    #[test]
    fn remove_child_on_non_child_returns_error() {
        let tree = make_mutation_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt
            .eval(
                r#"
            var body = document.body;
            var a = document.getElementById("a");
            try { body.removeChild(a); "no error"; } catch(e) { "error"; }
        "#,
            )
            .unwrap();
        let s = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "error");
    }

    #[test]
    fn clone_node_shallow_copy() {
        let tree = make_mutation_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt
            .eval(
                r#"
            var parent = document.getElementById("parent");
            var clone = parent.cloneNode(false);
            clone.hasChildNodes();
        "#,
            )
            .unwrap();
        assert!(!result.to_boolean());
    }

    #[test]
    fn clone_node_deep_copy_with_children() {
        let tree = make_mutation_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt
            .eval(
                r#"
            var parent = document.getElementById("parent");
            var clone = parent.cloneNode(true);
            clone.childNodes.length;
        "#,
            )
            .unwrap();
        let length = result.to_i32(&mut rt.context).unwrap();
        assert_eq!(length, 3);
    }

    #[test]
    fn clone_node_preserves_attributes() {
        let tree = make_mutation_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt
            .eval(
                r#"
            var parent = document.getElementById("parent");
            var clone = parent.cloneNode(false);
            clone.getAttribute("id");
        "#,
            )
            .unwrap();
        let s = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "parent");
    }

    #[test]
    fn clone_node_has_no_parent() {
        let tree = make_mutation_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt
            .eval(
                r#"
            var parent = document.getElementById("parent");
            var clone = parent.cloneNode(true);
            clone.parentNode === null;
        "#,
            )
            .unwrap();
        assert!(result.to_boolean());
    }

    #[test]
    fn insert_before_returns_new_node() {
        let tree = make_mutation_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt
            .eval(
                r#"
            var parent = document.getElementById("parent");
            var b = document.getElementById("b");
            var newNode = document.createElement("p");
            newNode.setAttribute("id", "new");
            var returned = parent.insertBefore(newNode, b);
            returned.getAttribute("id");
        "#,
            )
            .unwrap();
        let s = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "new");
    }

    #[test]
    fn replace_child_returns_old_child() {
        let tree = make_mutation_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt
            .eval(
                r#"
            var parent = document.getElementById("parent");
            var b = document.getElementById("b");
            var newNode = document.createElement("p");
            var returned = parent.replaceChild(newNode, b);
            returned.getAttribute("id");
        "#,
            )
            .unwrap();
        let s = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "b");
    }

    #[test]
    fn doctype_into_element_throws() {
        let tree = make_mutation_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt
            .eval(
                r#"
            var doc = document.implementation.createHTMLDocument("title");
            var doctype = doc.childNodes[0];
            var el = doc.createElement("a");
            var results = [];
            results.push("doctype.nodeType=" + doctype.nodeType);
            results.push("el has insertBefore=" + (typeof el.insertBefore));
            results.push("el.insertBefore.length=" + (el.insertBefore ? el.insertBefore.length : "N/A"));
            try { el.insertBefore(doctype, null); results.push("no error"); } catch(e) {
                results.push("error: " + e.message);
            }
            results.join(" | ");
        "#,
            )
            .unwrap();
        let s = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert!(s.contains("error:"), "Expected error but got: {}", s);
    }

    #[test]
    fn text_node_append_child_throws_hierarchy_request_error() {
        let tree = make_mutation_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt
            .eval(
                r#"
            var text = document.createTextNode("foo");
            var result = 'no error';
            try { text.appendChild(document.createElement("div")); } catch(e) {
                result = 'error: ' + e.message;
            }
            result;
        "#,
            )
            .unwrap();
        let s = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert!(
            s.contains("HierarchyRequestError") || s.contains("error:"),
            "Expected error but got: {}",
            s
        );
    }

    #[test]
    fn element_remove_with_siblings_via_engine_harness() {
        use crate::Engine;
        let mut engine = Engine::new();
        // Mimic the WPT pattern: test() wraps each check in try/catch
        let html = r#"<!DOCTYPE html>
<html><body>
<script>
var debug_log = [];
function test(fn, name) {
    try {
        fn();
        debug_log.push("PASS: " + name);
    } catch(e) {
        debug_log.push("FAIL: " + name + ": " + e.message);
    }
}
function assert_equals(a, b, msg) {
    if (a !== b) throw new Error(msg || "assert_equals: " + a + " !== " + b);
}
function assert_array_equals(a, b, msg) {
    var aLen = a ? a.length : undefined;
    var bLen = b ? b.length : undefined;
    if (aLen === undefined || bLen === undefined || aLen !== bLen) {
        throw new Error(msg || "assert_array_equals: length mismatch (" + aLen + " vs " + bLen + ")");
    }
    for (var i = 0; i < aLen; i++) {
        if (a[i] !== b[i]) throw new Error(msg || "assert_array_equals: index " + i);
    }
}
function assert_true(val, msg) {
    if (val !== true) throw new Error(msg || "assert_true: got " + val);
}

var node = document.createElement("div");
var parent = document.createElement("div");

test(function() {
    assert_true("remove" in node);
    assert_equals(typeof node.remove, "function");
    assert_equals(node.remove.length, 0);
}, "element should support remove()");

test(function() {
    assert_equals(node.parentNode, null, "Node should not have a parent");
    assert_equals(node.remove(), undefined);
    assert_equals(node.parentNode, null, "Removed new node should not have a parent");
}, "remove() should work if element doesn't have a parent");

test(function() {
    assert_equals(node.parentNode, null, "Node should not have a parent");
    parent.appendChild(node);
    assert_equals(node.parentNode, parent, "Appended node should have a parent");
    assert_equals(node.remove(), undefined);
    assert_equals(node.parentNode, null, "Removed node should not have a parent");
    assert_array_equals(parent.childNodes, [], "Parent should not have children");
}, "remove() should work if element does have a parent");

test(function() {
    assert_equals(node.parentNode, null, "Node should not have a parent");
    var before = parent.appendChild(document.createComment("before"));
    parent.appendChild(node);
    var after = parent.appendChild(document.createComment("after"));
    assert_equals(node.parentNode, parent, "Appended node should have a parent");
    assert_equals(node.remove(), undefined);
    assert_equals(node.parentNode, null, "Removed node should not have a parent");
    assert_array_equals(parent.childNodes, [before, after], "Parent should have two children left");
}, "remove() should work if element does have a parent and siblings");

window.__debug = debug_log.join("\n");
</script>
</body></html>"#;
        let _errors = engine.load_html_with_scripts_lossy(html, &std::collections::HashMap::new());
        let debug = engine.eval_js("window.__debug").unwrap_or_default();
        eprintln!("{}", debug);
        assert!(!debug.contains("FAIL"), "Element-remove harness test:\n{}", debug);
    }
