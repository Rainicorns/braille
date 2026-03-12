use boa_engine::{
    class::{Class, ClassBuilder},
    js_string,
    native_function::NativeFunction,
    Context, JsError, JsResult, JsValue,
};

use crate::dom::{NodeData, NodeId};
use super::element::JsElement;

pub(crate) fn register_mutation(class: &mut ClassBuilder) -> JsResult<()> {
    class.method(js_string!("insertBefore"), 2, NativeFunction::from_fn_ptr(insert_before));
    class.method(js_string!("replaceChild"), 2, NativeFunction::from_fn_ptr(replace_child));
    class.method(js_string!("removeChild"), 1, NativeFunction::from_fn_ptr(remove_child));
    class.method(js_string!("cloneNode"), 1, NativeFunction::from_fn_ptr(clone_node));
    Ok(())
}

fn insert_before(this: &JsValue, args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    let this_obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("insertBefore: this is not an object").into()))?;
    let parent = this_obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("insertBefore: this is not an Element").into()))?;
    let parent_id = parent.node_id;
    let tree = parent.tree.clone();

    let new_node_arg = args
        .first()
        .ok_or_else(|| JsError::from_opaque(js_string!("insertBefore: missing first argument").into()))?;
    let new_node_obj = new_node_arg
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("insertBefore: first argument is not an object").into()))?;
    let new_node = new_node_obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("insertBefore: first argument is not an Element").into()))?;
    let new_node_id = new_node.node_id;

    let ref_arg = args.get(1).cloned().unwrap_or(JsValue::null());

    // Per spec: if new_node is a DocumentFragment, insert its children instead
    let is_fragment = matches!(tree.borrow().get_node(new_node_id).data, NodeData::DocumentFragment);

    if ref_arg.is_null() || ref_arg.is_undefined() {
        if is_fragment {
            let children: Vec<NodeId> = tree.borrow().get_node(new_node_id).children.clone();
            for frag_child in children {
                tree.borrow_mut().append_child(parent_id, frag_child);
            }
        } else {
            tree.borrow_mut().append_child(parent_id, new_node_id);
        }
    } else {
        let ref_obj = ref_arg
            .as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("insertBefore: second argument is not an object or null").into()))?;
        let ref_el = ref_obj
            .downcast_ref::<JsElement>()
            .ok_or_else(|| JsError::from_opaque(js_string!("insertBefore: second argument is not an Element").into()))?;
        let ref_id = ref_el.node_id;
        if is_fragment {
            let children: Vec<NodeId> = tree.borrow().get_node(new_node_id).children.clone();
            for frag_child in children {
                tree.borrow_mut().insert_child_before(parent_id, frag_child, ref_id);
            }
        } else {
            tree.borrow_mut().insert_child_before(parent_id, new_node_id, ref_id);
        }
    }

    Ok(new_node_arg.clone())
}

fn replace_child(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let this_obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("replaceChild: this is not an object").into()))?;
    let parent = this_obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("replaceChild: this is not an Element").into()))?;
    let parent_id = parent.node_id;
    let tree = parent.tree.clone();

    let new_child_arg = args
        .first()
        .ok_or_else(|| JsError::from_opaque(js_string!("replaceChild: missing first argument").into()))?;
    let new_child_obj = new_child_arg
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("replaceChild: first argument is not an object").into()))?;
    let new_child = new_child_obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("replaceChild: first argument is not an Element").into()))?;
    let new_child_id = new_child.node_id;

    let old_child_arg = args
        .get(1)
        .ok_or_else(|| JsError::from_opaque(js_string!("replaceChild: missing second argument").into()))?;
    let old_child_obj = old_child_arg
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("replaceChild: second argument is not an object").into()))?;
    let old_child = old_child_obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("replaceChild: second argument is not an Element").into()))?;
    let old_child_id = old_child.node_id;

    tree.borrow_mut().replace_child(parent_id, new_child_id, old_child_id);

    let old_el = JsElement::new(old_child_id, tree);
    let js_obj = JsElement::from_data(old_el, ctx)?;
    Ok(js_obj.into())
}

fn remove_child(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let this_obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("removeChild: this is not an object").into()))?;
    let parent = this_obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("removeChild: this is not an Element").into()))?;
    let parent_id = parent.node_id;
    let tree = parent.tree.clone();

    let child_arg = args
        .first()
        .ok_or_else(|| JsError::from_opaque(js_string!("removeChild: missing argument").into()))?;
    let child_obj = child_arg
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("removeChild: argument is not an object").into()))?;
    let child = child_obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("removeChild: argument is not an Element").into()))?;
    let child_id = child.node_id;

    {
        let t = tree.borrow();
        let parent_node = t.get_node(parent_id);
        if !parent_node.children.contains(&child_id) {
            return Err(JsError::from_opaque(
                js_string!("removeChild: the node to be removed is not a child of this node").into(),
            ));
        }
    }

    tree.borrow_mut().remove_child(parent_id, child_id);

    let removed_el = JsElement::new(child_id, tree);
    let js_obj = JsElement::from_data(removed_el, ctx)?;
    Ok(js_obj.into())
}

fn clone_node(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let this_obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("cloneNode: this is not an object").into()))?;
    let el = this_obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("cloneNode: this is not an Element").into()))?;
    let node_id = el.node_id;
    let tree = el.tree.clone();

    let deep = args
        .first()
        .map(|v| v.to_boolean())
        .unwrap_or(false);

    let cloned_id = tree.borrow_mut().clone_node(node_id, deep);

    let cloned_el = JsElement::new(cloned_id, tree);
    let js_obj = JsElement::from_data(cloned_el, ctx)?;
    Ok(js_obj.into())
}


#[cfg(test)]
mod tests {
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
                attributes.push(("id".to_string(), "parent".to_string()));
            }
            if let NodeData::Element { ref mut attributes, .. } = t.get_node_mut(span_a).data {
                attributes.push(("id".to_string(), "a".to_string()));
            }
            if let NodeData::Element { ref mut attributes, .. } = t.get_node_mut(span_b).data {
                attributes.push(("id".to_string(), "b".to_string()));
            }
            if let NodeData::Element { ref mut attributes, .. } = t.get_node_mut(span_c).data {
                attributes.push(("id".to_string(), "c".to_string()));
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
        rt.eval(r#"
            var parent = document.getElementById("parent");
            var b = document.getElementById("b");
            var newNode = document.createElement("p");
            newNode.setAttribute("id", "new");
            parent.insertBefore(newNode, b);
        "#).unwrap();
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
        rt.eval(r#"
            var parent = document.getElementById("parent");
            var newNode = document.createElement("p");
            newNode.setAttribute("id", "new");
            parent.insertBefore(newNode, null);
        "#).unwrap();
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
        rt.eval(r#"
            var parent = document.getElementById("parent");
            var a = document.getElementById("a");
            var c = document.getElementById("c");
            parent.insertBefore(a, c);
        "#).unwrap();
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
        rt.eval(r#"
            var parent = document.getElementById("parent");
            var b = document.getElementById("b");
            var newNode = document.createElement("p");
            newNode.setAttribute("id", "new");
            parent.replaceChild(newNode, b);
        "#).unwrap();
        let t = tree.borrow();
        let parent_id = t.get_element_by_id("parent").unwrap();
        let children = &t.get_node(parent_id).children;
        assert_eq!(children.len(), 3);
        let new_id = t.get_element_by_id("new").unwrap();
        let a_id = t.get_element_by_id("a").unwrap();
        let c_id = t.get_element_by_id("c").unwrap();
        assert_eq!(children, &vec![a_id, new_id, c_id]);
        let b_id = t.get_element_by_id("b").unwrap();
        assert!(t.get_node(b_id).parent.is_none());
    }

    #[test]
    fn replace_child_detaches_new_child() {
        let tree = make_mutation_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        rt.eval(r#"
            var parent = document.getElementById("parent");
            var a = document.getElementById("a");
            var b = document.getElementById("b");
            parent.replaceChild(a, b);
        "#).unwrap();
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
        let result = rt.eval(r#"
            var parent = document.getElementById("parent");
            var b = document.getElementById("b");
            var removed = parent.removeChild(b);
            removed.getAttribute("id");
        "#).unwrap();
        let id_str = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(id_str, "b");
        let t = tree.borrow();
        let parent_id = t.get_element_by_id("parent").unwrap();
        assert_eq!(t.get_node(parent_id).children.len(), 2);
        let b_id = t.get_element_by_id("b").unwrap();
        assert!(t.get_node(b_id).parent.is_none());
    }

    #[test]
    fn remove_child_on_non_child_returns_error() {
        let tree = make_mutation_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt.eval(r#"
            var body = document.body;
            var a = document.getElementById("a");
            try { body.removeChild(a); "no error"; } catch(e) { "error"; }
        "#).unwrap();
        let s = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "error");
    }


    #[test]
    fn clone_node_shallow_copy() {
        let tree = make_mutation_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt.eval(r#"
            var parent = document.getElementById("parent");
            var clone = parent.cloneNode(false);
            clone.hasChildNodes();
        "#).unwrap();
        assert!(!result.to_boolean());
    }

    #[test]
    fn clone_node_deep_copy_with_children() {
        let tree = make_mutation_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt.eval(r#"
            var parent = document.getElementById("parent");
            var clone = parent.cloneNode(true);
            clone.childNodes.length;
        "#).unwrap();
        let length = result.to_i32(&mut rt.context).unwrap();
        assert_eq!(length, 3);
    }

    #[test]
    fn clone_node_preserves_attributes() {
        let tree = make_mutation_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt.eval(r#"
            var parent = document.getElementById("parent");
            var clone = parent.cloneNode(false);
            clone.getAttribute("id");
        "#).unwrap();
        let s = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "parent");
    }

    #[test]
    fn clone_node_has_no_parent() {
        let tree = make_mutation_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt.eval(r#"
            var parent = document.getElementById("parent");
            var clone = parent.cloneNode(true);
            clone.parentNode === null;
        "#).unwrap();
        assert!(result.to_boolean());
    }

    #[test]
    fn insert_before_returns_new_node() {
        let tree = make_mutation_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt.eval(r#"
            var parent = document.getElementById("parent");
            var b = document.getElementById("b");
            var newNode = document.createElement("p");
            newNode.setAttribute("id", "new");
            var returned = parent.insertBefore(newNode, b);
            returned.getAttribute("id");
        "#).unwrap();
        let s = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "new");
    }

    #[test]
    fn replace_child_returns_old_child() {
        let tree = make_mutation_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt.eval(r#"
            var parent = document.getElementById("parent");
            var b = document.getElementById("b");
            var newNode = document.createElement("p");
            var returned = parent.replaceChild(newNode, b);
            returned.getAttribute("id");
        "#).unwrap();
        let s = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "b");
    }
}

