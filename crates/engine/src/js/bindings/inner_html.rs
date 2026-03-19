use boa_engine::{
    class::ClassBuilder, js_string, native_function::NativeFunction, property::Attribute, Context, JsError, JsResult,
    JsValue,
};

use crate::dom::NodeId;

pub(crate) fn get_inner_html(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "innerHTML getter");

    let tree = el.tree.borrow();
    let html = tree.serialize_children_html(el.node_id);
    Ok(JsValue::from(js_string!(html)))
}

pub(crate) fn set_inner_html(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "innerHTML setter");

    let html_string = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    let node_id = el.node_id;
    let tree_rc = el.tree.clone();

    // Capture existing children for MutationObserver
    let removed_children: Vec<NodeId> = tree_rc.borrow().get_node(node_id).children.clone();

    let wrapper = format!("<html><body>{}</body></html>", html_string);
    let temp_tree_rc = crate::html::parse_html(&wrapper);
    let temp_tree = temp_tree_rc.borrow();

    tree_rc.borrow_mut().clear_children(node_id);

    let mut added_ids: Vec<NodeId> = Vec::new();
    if let Some(temp_body) = temp_tree.body() {
        let temp_body_children: Vec<NodeId> = temp_tree.get_node(temp_body).children.clone();
        for &child_id in &temp_body_children {
            let new_id = tree_rc.borrow_mut().import_subtree(&temp_tree, child_id);
            tree_rc.borrow_mut().append_child(node_id, new_id);
            added_ids.push(new_id);
        }
    }

    // Queue MutationObserver childList record
    if !removed_children.is_empty() || !added_ids.is_empty() {
        super::mutation_observer::queue_childlist_mutation(
            ctx,
            &tree_rc,
            node_id,
            added_ids,
            removed_children,
            None,
            None,
        );
    }

    Ok(JsValue::undefined())
}

fn get_outer_html(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "outerHTML getter");

    let tree = el.tree.borrow();
    let html = tree.serialize_node_html(el.node_id);
    Ok(JsValue::from(js_string!(html)))
}

fn set_outer_html(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "outerHTML setter");

    let html_string = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    let node_id = el.node_id;
    let tree_rc = el.tree.clone();

    // Per spec: if parent is null, return. If parent is a Document, throw.
    let parent_id = match tree_rc.borrow().get_node(node_id).parent {
        Some(p) => p,
        None => return Ok(JsValue::undefined()),
    };

    // Parse the replacement HTML
    let wrapper = format!("<html><body>{}</body></html>", html_string);
    let temp_tree_rc = crate::html::parse_html(&wrapper);
    let temp_tree = temp_tree_rc.borrow();

    // Import new nodes
    let mut new_ids: Vec<NodeId> = Vec::new();
    if let Some(temp_body) = temp_tree.body() {
        let temp_body_children: Vec<NodeId> = temp_tree.get_node(temp_body).children.clone();
        for &child_id in &temp_body_children {
            let new_id = tree_rc.borrow_mut().import_subtree(&temp_tree, child_id);
            new_ids.push(new_id);
        }
    }

    // Insert new nodes before the target element, then remove the target
    for &new_id in &new_ids {
        tree_rc.borrow_mut().insert_before(node_id, new_id);
    }
    tree_rc.borrow_mut().remove_child(parent_id, node_id);

    // Queue MutationObserver childList record
    super::mutation_observer::queue_childlist_mutation(ctx, &tree_rc, parent_id, new_ids, vec![node_id], None, None);

    Ok(JsValue::undefined())
}

fn insert_adjacent_html(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "insertAdjacentHTML");
    let pos = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();
    let hs = args
        .get(1)
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();
    let nid = el.node_id;
    let trc = el.tree.clone();
    let w = format!("<html><body>{}</body></html>", hs);
    let tmp = crate::html::parse_html(&w);
    let tt = tmp.borrow();
    let kids: Vec<NodeId> = match tt.body() {
        Some(b) => tt.get_node(b).children.clone(),
        None => vec![],
    };
    let mut imp: Vec<NodeId> = vec![];
    for &c in &kids {
        imp.push(trc.borrow_mut().import_subtree(&tt, c));
    }
    match pos.to_lowercase().as_str() {
        "beforebegin" => {
            for id in imp {
                trc.borrow_mut().insert_before(nid, id);
            }
        }
        "afterbegin" => {
            for id in imp.into_iter().rev() {
                let mut t = trc.borrow_mut();
                let fc = t.get_node(nid).children.first().copied();
                match fc {
                    Some(f) => t.insert_before(f, id),
                    None => t.append_child(nid, id),
                }
            }
        }
        "beforeend" => {
            for id in imp {
                trc.borrow_mut().append_child(nid, id);
            }
        }
        "afterend" => {
            for id in imp.into_iter().rev() {
                trc.borrow_mut().insert_after(nid, id);
            }
        }
        other => {
            return Err(JsError::from_opaque(
                js_string!(format!("invalid position '{}'", other)).into(),
            ));
        }
    }
    Ok(JsValue::undefined())
}

pub(crate) fn register_inner_html(c: &mut ClassBuilder) -> JsResult<()> {
    let r = c.context().realm().clone();
    let g = NativeFunction::from_fn_ptr(get_inner_html);
    let s = NativeFunction::from_fn_ptr(set_inner_html);
    let o = NativeFunction::from_fn_ptr(get_outer_html);
    let os = NativeFunction::from_fn_ptr(set_outer_html);
    let i = NativeFunction::from_fn_ptr(insert_adjacent_html);
    c.accessor(
        js_string!("innerHTML"),
        Some(g.to_js_function(&r)),
        Some(s.to_js_function(&r)),
        Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    );
    c.accessor(
        js_string!("outerHTML"),
        Some(o.to_js_function(&r)),
        Some(os.to_js_function(&r)),
        Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    );
    c.method(js_string!("insertAdjacentHTML"), 2, i);
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::dom::{DomTree, NodeData};
    use crate::js::JsRuntime;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn make_test_tree() -> Rc<RefCell<DomTree>> {
        let tree = Rc::new(RefCell::new(DomTree::new()));
        {
            let mut t = tree.borrow_mut();
            let html = t.create_element("html");
            let body = t.create_element("body");
            let div = t.create_element("div");
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

    fn make_tree_with_children() -> Rc<RefCell<DomTree>> {
        let tree = Rc::new(RefCell::new(DomTree::new()));
        {
            let mut t = tree.borrow_mut();
            let html = t.create_element("html");
            let body = t.create_element("body");
            let div = t.create_element("div");
            if let NodeData::Element { ref mut attributes, .. } = t.get_node_mut(div).data {
                attributes.push(crate::dom::node::DomAttribute::new("id", "app"));
            }
            let span = t.create_element("span");
            let span_text = t.create_text("hello");
            t.append_child(span, span_text);
            let em = t.create_element("em");
            let em_text = t.create_text("world");
            t.append_child(em, em_text);
            t.append_child(div, span);
            t.append_child(div, em);
            let doc = t.document();
            t.append_child(doc, html);
            t.append_child(html, body);
            t.append_child(body, div);
        }
        tree
    }

    #[test]
    fn inner_html_getter_returns_children_html() {
        let tree = make_tree_with_children();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let r = rt.eval(r#"document.getElementById("app").innerHTML"#).unwrap();
        let h = r.as_string().unwrap().to_std_string_escaped();
        assert_eq!(h, "<span>hello</span><em>world</em>");
    }

    #[test]
    fn inner_html_getter_escapes_entities() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        rt.eval(r#"document.getElementById("app").textContent = "1 < 2 & 3 > 1""#)
            .unwrap();
        let r = rt.eval(r#"document.getElementById("app").innerHTML"#).unwrap();
        let h = r.as_string().unwrap().to_std_string_escaped();
        assert_eq!(h, "1 &lt; 2 &amp; 3 &gt; 1");
    }

    #[test]
    fn inner_html_setter_replaces_children() {
        let tree = make_tree_with_children();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        rt.eval(r#"document.getElementById("app").innerHTML = "<p>replaced</p>""#)
            .unwrap();
        let r = rt.eval(r#"document.getElementById("app").innerHTML"#).unwrap();
        assert_eq!(r.as_string().unwrap().to_std_string_escaped(), "<p>replaced</p>");
    }

    #[test]
    fn inner_html_setter_nested() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        rt.eval(r#"document.getElementById("app").innerHTML = "<div><span>nested</span></div>""#)
            .unwrap();
        let r = rt.eval(r#"document.getElementById("app").innerHTML"#).unwrap();
        assert_eq!(
            r.as_string().unwrap().to_std_string_escaped(),
            "<div><span>nested</span></div>"
        );
    }

    #[test]
    fn inner_html_setter_empty_clears() {
        let tree = make_tree_with_children();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        rt.eval(r#"document.getElementById("app").innerHTML = """#).unwrap();
        let r = rt.eval(r#"document.getElementById("app").innerHTML"#).unwrap();
        assert_eq!(r.as_string().unwrap().to_std_string_escaped(), "");
        let t = tree.borrow();
        let did = t.get_element_by_id("app").unwrap();
        assert!(t.get_node(did).children.is_empty());
    }

    #[test]
    fn outer_html_includes_element() {
        let tree = make_tree_with_children();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let r = rt.eval(r#"document.getElementById("app").outerHTML"#).unwrap();
        let h = r.as_string().unwrap().to_std_string_escaped();
        assert!(h.starts_with("<div"), "got: {}", h);
        assert!(h.contains("<span>hello</span>"), "got: {}", h);
        assert!(h.ends_with("</div>"), "got: {}", h);
    }

    #[test]
    fn void_elements_no_closing_tag() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        rt.eval(r#"document.getElementById("app").innerHTML = "<br><input><hr>""#)
            .unwrap();
        let r = rt.eval(r#"document.getElementById("app").innerHTML"#).unwrap();
        assert_eq!(r.as_string().unwrap().to_std_string_escaped(), "<br><input><hr>");
    }

    #[test]
    fn insert_adjacent_beforeend() {
        let tree = make_tree_with_children();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        rt.eval(r#"document.getElementById("app").insertAdjacentHTML("beforeend","<b>x</b>")"#)
            .unwrap();
        let r = rt.eval(r#"document.getElementById("app").innerHTML"#).unwrap();
        assert_eq!(
            r.as_string().unwrap().to_std_string_escaped(),
            "<span>hello</span><em>world</em><b>x</b>"
        );
    }

    #[test]
    fn insert_adjacent_afterbegin() {
        let tree = make_tree_with_children();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        rt.eval(r#"document.getElementById("app").insertAdjacentHTML("afterbegin","<b>x</b>")"#)
            .unwrap();
        let r = rt.eval(r#"document.getElementById("app").innerHTML"#).unwrap();
        assert_eq!(
            r.as_string().unwrap().to_std_string_escaped(),
            "<b>x</b><span>hello</span><em>world</em>"
        );
    }

    #[test]
    fn insert_adjacent_beforebegin() {
        let tree = make_tree_with_children();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        rt.eval(r#"document.getElementById("app").insertAdjacentHTML("beforebegin","<p>b</p>")"#)
            .unwrap();
        let r = rt.eval(r#"document.body.innerHTML"#).unwrap();
        let h = r.as_string().unwrap().to_std_string_escaped();
        assert!(h.contains("<p>b</p>"), "got: {}", h);
        let pp = h.find("<p>b</p>").unwrap();
        let dp = h.find("<div").unwrap();
        assert!(pp < dp, "p should be before div");
    }

    #[test]
    fn round_trip_inner_html() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        rt.eval(r#"document.getElementById("app").innerHTML = "<ul><li>a</li><li>b</li></ul>""#)
            .unwrap();
        let r = rt.eval(r#"document.getElementById("app").innerHTML"#).unwrap();
        assert_eq!(
            r.as_string().unwrap().to_std_string_escaped(),
            "<ul><li>a</li><li>b</li></ul>"
        );
    }
}
