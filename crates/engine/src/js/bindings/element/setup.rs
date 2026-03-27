use std::cell::RefCell;
use std::rc::Rc;

use boa_engine::{
    js_string,
    native_function::NativeFunction,
    object::ObjectInitializer,
    property::PropertyDescriptor,
    Context, JsObject, JsResult, JsSymbol, JsValue,
};

use crate::dom::{DomTree, NodeData, NodeId};

use super::cache::{ensure_iframe_content_doc, get_or_create_js_element};

/// Defines name, value, namespaceURI, prefix, localName, ownerElement, and specified
/// properties on an Attr node's JS object.
pub(crate) fn setup_attr_node_properties(
    js_obj: &JsObject,
    tree: Rc<RefCell<DomTree>>,
    node_id: NodeId,
    local_name: &str,
    namespace: &str,
    prefix: &str,
    ctx: &mut Context,
) -> JsResult<()> {
    // name = qualified name (prefix:localName or just localName)
    let qualified_name = if prefix.is_empty() {
        local_name.to_string()
    } else {
        format!("{}:{}", prefix, local_name)
    };
    js_obj.define_property_or_throw(
        js_string!("name"),
        PropertyDescriptor::builder()
            .value(JsValue::from(js_string!(qualified_name)))
            .writable(false)
            .configurable(true)
            .enumerable(true)
            .build(),
        ctx,
    )?;

    // value — read-write accessor (reads/writes from DomTree)
    let tree_for_getter = tree.clone();
    let nid_for_getter = node_id;
    let value_getter = unsafe {
        NativeFunction::from_closure(move |_this, _args, _ctx| {
            let tree = tree_for_getter.borrow();
            let node = tree.get_node(nid_for_getter);
            if let NodeData::Attr { value: ref v, .. } = node.data {
                Ok(JsValue::from(js_string!(v.clone())))
            } else {
                Ok(JsValue::from(js_string!("")))
            }
        })
    };
    let tree_for_setter = tree;
    let nid_for_setter = node_id;
    let value_setter = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx2| {
            let new_val = args
                .first()
                .map(|v| v.to_string(ctx2))
                .transpose()?
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_default();

            // 1. Update the Attr node's value in the tree
            if let NodeData::Attr { ref mut value, .. } =
                tree_for_setter.borrow_mut().get_node_mut(nid_for_setter).data
            {
                *value = new_val.clone();
            }

            // 2. Sync to owning element: scan attr_node_cache for an entry where
            //    cached NodeId matches this Attr's node_id → get (tree_ptr, element_id, qname)
            let cache = crate::js::realm_state::attr_node_cache(ctx2);
            let owner_info = {
                let c = cache.borrow();
                c.iter()
                    .find(|(_, &nid)| nid == nid_for_setter)
                    .map(|((tp, el_id, qname), _)| (*tp, *el_id, qname.clone()))
            };

            if let Some((_tree_ptr, element_id, qname)) = owner_info {
                // Update the matching DomAttribute on the element via observer wrapper
                super::super::mutation_observer::set_attribute_with_observer(
                    ctx2,
                    &tree_for_setter,
                    element_id,
                    &qname,
                    &new_val,
                );
            }

            Ok(JsValue::undefined())
        })
    };
    let realm = ctx.realm().clone();
    js_obj.define_property_or_throw(
        js_string!("value"),
        PropertyDescriptor::builder()
            .get(value_getter.to_js_function(&realm))
            .set(value_setter.to_js_function(&realm))
            .configurable(true)
            .enumerable(true)
            .build(),
        ctx,
    )?;

    // namespaceURI — null if empty, else the namespace string
    let ns_val = if namespace.is_empty() {
        JsValue::null()
    } else {
        JsValue::from(js_string!(namespace.to_string()))
    };
    js_obj.define_property_or_throw(
        js_string!("namespaceURI"),
        PropertyDescriptor::builder()
            .value(ns_val)
            .writable(false)
            .configurable(true)
            .enumerable(true)
            .build(),
        ctx,
    )?;

    // prefix — null if empty, else the prefix string
    let pfx_val = if prefix.is_empty() {
        JsValue::null()
    } else {
        JsValue::from(js_string!(prefix.to_string()))
    };
    js_obj.define_property_or_throw(
        js_string!("prefix"),
        PropertyDescriptor::builder()
            .value(pfx_val)
            .writable(false)
            .configurable(true)
            .enumerable(true)
            .build(),
        ctx,
    )?;

    // localName
    js_obj.define_property_or_throw(
        js_string!("localName"),
        PropertyDescriptor::builder()
            .value(JsValue::from(js_string!(local_name.to_string())))
            .writable(false)
            .configurable(true)
            .enumerable(true)
            .build(),
        ctx,
    )?;

    // ownerElement — null for detached Attr nodes (created via createAttribute)
    js_obj.define_property_or_throw(
        js_string!("ownerElement"),
        PropertyDescriptor::builder()
            .value(JsValue::null())
            .writable(false)
            .configurable(true)
            .enumerable(true)
            .build(),
        ctx,
    )?;

    // specified — always true per DOM4 spec
    js_obj.define_property_or_throw(
        js_string!("specified"),
        PropertyDescriptor::builder()
            .value(JsValue::from(true))
            .writable(false)
            .configurable(true)
            .enumerable(true)
            .build(),
        ctx,
    )?;

    Ok(())
}

/// Sets up a stub `sheet` property on `<style>` elements, returning a CSSStyleSheet-like
/// object with `insertRule()` (returns 0), `deleteRule()` (no-op), and `cssRules` (empty).
pub(crate) fn setup_style_sheet(js_obj: &JsObject, ctx: &mut Context) -> JsResult<()> {
    let sheet_getter = unsafe {
        NativeFunction::from_closure(|_this, _args, ctx2| {
            let insert_rule =
                NativeFunction::from_fn_ptr(|_this, _args, _ctx| Ok(JsValue::from(0)));
            let delete_rule =
                NativeFunction::from_fn_ptr(|_this, _args, _ctx| Ok(JsValue::undefined()));

            let css_rules = ObjectInitializer::new(ctx2).build();
            css_rules.define_property_or_throw(
                js_string!("length"),
                PropertyDescriptor::builder()
                    .value(JsValue::from(0))
                    .writable(false)
                    .configurable(true)
                    .enumerable(true)
                    .build(),
                ctx2,
            )?;

            let sheet = ObjectInitializer::new(ctx2)
                .function(insert_rule, js_string!("insertRule"), 1)
                .function(delete_rule, js_string!("deleteRule"), 1)
                .build();
            sheet.define_property_or_throw(
                js_string!("cssRules"),
                PropertyDescriptor::builder()
                    .value(JsValue::from(css_rules))
                    .writable(false)
                    .configurable(true)
                    .enumerable(true)
                    .build(),
                ctx2,
            )?;

            // Also expose Symbol.toStringTag
            sheet.define_property_or_throw(
                JsSymbol::to_string_tag(),
                PropertyDescriptor::builder()
                    .value(JsValue::from(js_string!("CSSStyleSheet")))
                    .writable(false)
                    .configurable(true)
                    .enumerable(false)
                    .build(),
                ctx2,
            )?;

            Ok(JsValue::from(sheet))
        })
    };

    let realm = ctx.realm().clone();
    js_obj.define_property_or_throw(
        js_string!("sheet"),
        PropertyDescriptor::builder()
            .get(sheet_getter.to_js_function(&realm))
            .configurable(true)
            .enumerable(true)
            .build(),
        ctx,
    )?;

    Ok(())
}

/// Defines the .content getter on `<template>` elements that returns the template's
/// DocumentFragment content node.
pub(crate) fn setup_template_content_getter(
    js_obj: &JsObject,
    tree: Rc<RefCell<DomTree>>,
    node_id: NodeId,
    ctx: &mut Context,
) -> JsResult<()> {
    let tree_for_template_content = tree;
    let node_id_for_template_content = node_id;
    let content_getter = unsafe {
        NativeFunction::from_closure(move |_this, _args, ctx2| {
            let content_id = {
                let tree_ref = tree_for_template_content.borrow();
                tree_ref.get_node(node_id_for_template_content).template_contents
            };
            match content_id {
                Some(cid) => {
                    let obj = get_or_create_js_element(cid, tree_for_template_content.clone(), ctx2)?;
                    Ok(JsValue::from(obj))
                }
                None => Ok(JsValue::null()),
            }
        })
    };
    let realm = ctx.realm().clone();
    js_obj.define_property_or_throw(
        js_string!("content"),
        PropertyDescriptor::builder()
            .get(content_getter.to_js_function(&realm))
            .configurable(true)
            .enumerable(true)
            .build(),
        ctx,
    )?;
    Ok(())
}

/// Defines contentDocument, contentWindow, src, and onload getters/setters on `<iframe>` elements.
pub(crate) fn setup_iframe_properties(
    js_obj: &JsObject,
    tree: Rc<RefCell<DomTree>>,
    node_id: NodeId,
    ctx: &mut Context,
) -> JsResult<()> {
    // contentDocument getter
    let tree_for_content_doc = tree.clone();
    let node_id_for_content_doc = node_id;
    let content_doc_getter = unsafe {
        NativeFunction::from_closure(move |_this, _args, ctx2| {
            let tp = Rc::as_ptr(&tree_for_content_doc) as usize;
            let doc_obj = ensure_iframe_content_doc(tp, node_id_for_content_doc, ctx2)?;
            Ok(JsValue::from(doc_obj))
        })
    };
    let realm = ctx.realm().clone();
    js_obj.define_property_or_throw(
        js_string!("contentDocument"),
        PropertyDescriptor::builder()
            .get(content_doc_getter.to_js_function(&realm))
            .configurable(true)
            .enumerable(true)
            .build(),
        ctx,
    )?;

    // contentWindow getter -- returns a plain object with a document property
    let tree_for_content_window = tree.clone();
    let node_id_for_content_window = node_id;
    let content_window_getter = unsafe {
        NativeFunction::from_closure(move |_this, _args, ctx2| {
            let tp = Rc::as_ptr(&tree_for_content_window) as usize;
            let doc_obj = ensure_iframe_content_doc(tp, node_id_for_content_window, ctx2)?;
            let win = ObjectInitializer::new(ctx2).build();
            win.define_property_or_throw(
                js_string!("document"),
                PropertyDescriptor::builder()
                    .value(JsValue::from(doc_obj))
                    .writable(true)
                    .configurable(true)
                    .enumerable(true)
                    .build(),
                ctx2,
            )?;
            Ok(JsValue::from(win))
        })
    };
    let realm2 = ctx.realm().clone();
    js_obj.define_property_or_throw(
        js_string!("contentWindow"),
        PropertyDescriptor::builder()
            .get(content_window_getter.to_js_function(&realm2))
            .configurable(true)
            .enumerable(true)
            .build(),
        ctx,
    )?;

    // src getter/setter — reflects the `src` DOM attribute
    let tree_for_src_getter = tree.clone();
    let node_id_for_src_getter = node_id;
    let src_getter = unsafe {
        NativeFunction::from_closure(move |_this, _args, _ctx2| {
            let t = tree_for_src_getter.borrow();
            match t.get_attribute(node_id_for_src_getter, "src") {
                Some(val) => Ok(JsValue::from(js_string!(val))),
                None => Ok(JsValue::from(js_string!(""))),
            }
        })
    };
    let tree_for_src_setter = tree.clone();
    let node_id_for_src_setter = node_id;
    let src_setter = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx2| {
            let value = args
                .first()
                .map(|v| v.to_string(ctx2))
                .transpose()?
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_default();
            super::super::mutation_observer::set_attribute_with_observer(
                ctx2,
                &tree_for_src_setter,
                node_id_for_src_setter,
                "src",
                &value,
            );
            Ok(JsValue::undefined())
        })
    };
    let realm3 = ctx.realm().clone();
    js_obj.define_property_or_throw(
        js_string!("src"),
        PropertyDescriptor::builder()
            .get(src_getter.to_js_function(&realm3))
            .set(src_setter.to_js_function(&realm3))
            .configurable(true)
            .enumerable(true)
            .build(),
        ctx,
    )?;

    // onload getter/setter — uses unified on_event system
    let tree_for_onload_getter = tree.clone();
    let node_id_for_onload_getter = node_id;
    let onload_getter = unsafe {
        NativeFunction::from_closure(move |_this, _args, ctx2| {
            let tp = Rc::as_ptr(&tree_for_onload_getter) as usize;
            match super::super::on_event::get_on_event_handler(tp, node_id_for_onload_getter, "load", ctx2) {
                Some(func) => Ok(JsValue::from(func)),
                None => Ok(JsValue::null()),
            }
        })
    };
    let tree_for_onload_setter = tree;
    let node_id_for_onload_setter = node_id;
    let onload_setter = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx2| {
            let tp = Rc::as_ptr(&tree_for_onload_setter) as usize;
            let val = args.first().cloned().unwrap_or(JsValue::null());
            if let Some(obj) = val.as_object().filter(|o| o.is_callable()) {
                super::super::on_event::set_on_event_handler(tp, node_id_for_onload_setter, "load", Some(obj.clone()), ctx2);
            } else {
                super::super::on_event::set_on_event_handler(tp, node_id_for_onload_setter, "load", None, ctx2);
            }
            Ok(JsValue::undefined())
        })
    };
    let realm4 = ctx.realm().clone();
    js_obj.define_property_or_throw(
        js_string!("onload"),
        PropertyDescriptor::builder()
            .get(onload_getter.to_js_function(&realm4))
            .set(onload_setter.to_js_function(&realm4))
            .configurable(true)
            .enumerable(true)
            .build(),
        ctx,
    )?;

    Ok(())
}

/// Compiles inline event handler attributes (e.g., onclick="...") found on the DOM node
/// into JS functions via the on_event system.
pub(crate) fn compile_parsed_inline_handlers(
    tree: Rc<RefCell<DomTree>>,
    node_id: NodeId,
    ctx: &mut Context,
) {
    let inline_handlers: Vec<(String, String)> = {
        let t = tree.borrow();
        let node = t.get_node(node_id);
        match &node.data {
            NodeData::Element { attributes, .. } => attributes
                .iter()
                .filter(|a| a.local_name.starts_with("on") && a.local_name.len() > 2)
                .map(|a| (a.local_name[2..].to_string(), a.value.clone()))
                .collect(),
            _ => Vec::new(),
        }
    };
    if !inline_handlers.is_empty() {
        let tp = Rc::as_ptr(&tree) as usize;
        for (event_name, attr_value) in inline_handlers {
            super::super::on_event::compile_inline_event_handler(tp, node_id, &event_name, &attr_value, ctx);
        }
    }
}

/// Returns true if the given lowercase tag name is a known HTML element
/// (i.e., it should get HTMLElement.prototype rather than HTMLUnknownElement.prototype).
pub(crate) fn is_known_html_element(tag: &str) -> bool {
    matches!(
        tag,
        "a" | "abbr"
            | "acronym"
            | "address"
            | "area"
            | "article"
            | "aside"
            | "audio"
            | "b"
            | "base"
            | "bdi"
            | "bdo"
            | "bgsound"
            | "big"
            | "blockquote"
            | "body"
            | "br"
            | "button"
            | "canvas"
            | "caption"
            | "center"
            | "cite"
            | "code"
            | "col"
            | "colgroup"
            | "data"
            | "datalist"
            | "dd"
            | "del"
            | "details"
            | "dfn"
            | "dialog"
            | "dir"
            | "div"
            | "dl"
            | "dt"
            | "embed"
            | "em"
            | "fieldset"
            | "figcaption"
            | "figure"
            | "font"
            | "footer"
            | "form"
            | "frame"
            | "frameset"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "head"
            | "header"
            | "hgroup"
            | "hr"
            | "html"
            | "i"
            | "iframe"
            | "img"
            | "input"
            | "ins"
            | "isindex"
            | "kbd"
            | "label"
            | "legend"
            | "li"
            | "link"
            | "main"
            | "map"
            | "mark"
            | "marquee"
            | "meta"
            | "meter"
            | "nav"
            | "nobr"
            | "noframes"
            | "noscript"
            | "object"
            | "ol"
            | "optgroup"
            | "option"
            | "output"
            | "p"
            | "param"
            | "pre"
            | "progress"
            | "q"
            | "rp"
            | "rt"
            | "ruby"
            | "s"
            | "samp"
            | "script"
            | "section"
            | "select"
            | "small"
            | "source"
            | "spacer"
            | "span"
            | "strike"
            | "style"
            | "sub"
            | "summary"
            | "sup"
            | "table"
            | "tbody"
            | "td"
            | "template"
            | "textarea"
            | "tfoot"
            | "th"
            | "thead"
            | "time"
            | "title"
            | "tr"
            | "track"
            | "tt"
            | "u"
            | "ul"
            | "var"
            | "video"
            | "wbr"
    )
}
