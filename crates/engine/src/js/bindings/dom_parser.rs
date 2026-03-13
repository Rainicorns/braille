use std::cell::RefCell;
use std::rc::Rc;

use boa_engine::{
    js_string,
    native_function::NativeFunction,
    object::FunctionObjectBuilder,
    property::{Attribute, PropertyDescriptor},
    Context, JsError, JsObject, JsValue,
};

use crate::dom::DomTree;
use crate::dom::node::{DomAttribute, NodeId};
use crate::html::parser::parse_html;

use super::document::add_document_properties_to_element;
use super::element::get_or_create_js_element;

/// Parse an XML string into a new DomTree using quick-xml's NsReader.
fn parse_xml_into_tree(input: &str) -> Rc<RefCell<DomTree>> {
    use quick_xml::events::Event;
    use quick_xml::name::ResolveResult;
    use quick_xml::reader::NsReader;

    let tree = Rc::new(RefCell::new(DomTree::new_xml()));
    let doc_node_id = tree.borrow().document();

    let mut reader = NsReader::from_str(input);
    reader.config_mut().trim_text(false);
    let mut stack: Vec<NodeId> = vec![doc_node_id];

    loop {
        match reader.read_resolved_event() {
            Ok((resolve, Event::Start(e))) => {
                let ns_uri = match resolve {
                    ResolveResult::Bound(ns) => {
                        std::str::from_utf8(ns.as_ref())
                            .unwrap_or("")
                            .to_string()
                    }
                    ResolveResult::Unbound => String::new(),
                    ResolveResult::Unknown(prefix_bytes) => {
                        // Unknown prefix - treat as no namespace
                        let _ = prefix_bytes;
                        String::new()
                    }
                };
                let local_name = std::str::from_utf8(e.local_name().as_ref())
                    .unwrap_or("")
                    .to_string();
                // Extract the full qualified name directly from the event bytes
                // to avoid lifetime issues with prefix()
                let full_name = std::str::from_utf8(e.name().as_ref())
                    .unwrap_or("")
                    .to_string();
                let tag_name = if full_name.contains(':') {
                    full_name
                } else {
                    local_name
                };

                // Collect attributes
                let attrs: Vec<DomAttribute> = e
                    .attributes()
                    .filter_map(|a| {
                        let a = a.ok()?;
                        let key = std::str::from_utf8(a.key.as_ref()).ok()?.to_string();
                        let val = a.unescape_value().ok()?.to_string();
                        let (prefix, local_name) = if let Some(colon_pos) = key.find(':') {
                            (key[..colon_pos].to_string(), key[colon_pos + 1..].to_string())
                        } else {
                            (String::new(), key)
                        };
                        Some(DomAttribute {
                            local_name,
                            prefix,
                            namespace: String::new(),
                            value: val,
                        })
                    })
                    .collect();

                let node_id = tree
                    .borrow_mut()
                    .create_element_ns(&tag_name, attrs, &ns_uri);
                let parent = *stack.last().unwrap();
                tree.borrow_mut().append_child(parent, node_id);
                stack.push(node_id);
            }
            Ok((_, Event::End(_))) => {
                if stack.len() > 1 {
                    stack.pop();
                }
            }
            Ok((resolve, Event::Empty(e))) => {
                let ns_uri = match resolve {
                    ResolveResult::Bound(ns) => {
                        std::str::from_utf8(ns.as_ref())
                            .unwrap_or("")
                            .to_string()
                    }
                    ResolveResult::Unbound => String::new(),
                    ResolveResult::Unknown(_) => String::new(),
                };
                let local_name = std::str::from_utf8(e.local_name().as_ref())
                    .unwrap_or("")
                    .to_string();
                let full_name = std::str::from_utf8(e.name().as_ref())
                    .unwrap_or("")
                    .to_string();
                let tag_name = if full_name.contains(':') {
                    full_name
                } else {
                    local_name
                };

                let attrs: Vec<DomAttribute> = e
                    .attributes()
                    .filter_map(|a| {
                        let a = a.ok()?;
                        let key = std::str::from_utf8(a.key.as_ref()).ok()?.to_string();
                        let val = a.unescape_value().ok()?.to_string();
                        let (prefix, local_name) = if let Some(colon_pos) = key.find(':') {
                            (key[..colon_pos].to_string(), key[colon_pos + 1..].to_string())
                        } else {
                            (String::new(), key)
                        };
                        Some(DomAttribute {
                            local_name,
                            prefix,
                            namespace: String::new(),
                            value: val,
                        })
                    })
                    .collect();

                let node_id = tree
                    .borrow_mut()
                    .create_element_ns(&tag_name, attrs, &ns_uri);
                let parent = *stack.last().unwrap();
                tree.borrow_mut().append_child(parent, node_id);
            }
            Ok((_, Event::Text(e))) => {
                let text = e.unescape().unwrap_or_default().to_string();
                if !text.is_empty() {
                    let node_id = tree.borrow_mut().create_text(&text);
                    let parent = *stack.last().unwrap();
                    tree.borrow_mut().append_child(parent, node_id);
                }
            }
            Ok((_, Event::Comment(e))) => {
                let text = e.unescape().unwrap_or_default().to_string();
                let node_id = tree.borrow_mut().create_comment(&text);
                let parent = *stack.last().unwrap();
                tree.borrow_mut().append_child(parent, node_id);
            }
            Ok((_, Event::CData(e))) => {
                let text = String::from_utf8_lossy(&e).to_string();
                if !text.is_empty() {
                    let node_id = tree.borrow_mut().create_text(&text);
                    let parent = *stack.last().unwrap();
                    tree.borrow_mut().append_child(parent, node_id);
                }
            }
            Ok((_, Event::Eof)) => break,
            Ok(_) => {} // skip Decl, DocType, PI for now
            Err(_) => break, // best-effort parsing, skip errors
        }
    }

    tree
}

/// Register the DOMParser global constructor.
///
/// DOMParser has one method: parseFromString(string, mimeType)
/// which returns a new Document object.
pub(crate) fn register_dom_parser(ctx: &mut Context) {
    let realm = ctx.realm().clone();

    // DOMParser.prototype
    let proto = boa_engine::object::ObjectInitializer::new(ctx).build();

    // parseFromString method on prototype
    let parse_fn = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx2| {
            // Get the input string
            let input = args
                .first()
                .map(|v| v.to_string(ctx2))
                .transpose()?
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_default();

            // Get the mimeType
            let mime_type = args
                .get(1)
                .map(|v| v.to_string(ctx2))
                .transpose()?
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_default();

            match mime_type.as_str() {
                "text/html" => {
                    // Use html5ever to parse into a new tree
                    let new_tree = parse_html(&input);
                    let doc_id = new_tree.borrow().document();
                    let js_obj = get_or_create_js_element(doc_id, new_tree.clone(), ctx2)?;
                    add_document_properties_to_element(
                        &js_obj,
                        new_tree,
                        "text/html".to_string(),
                        ctx2,
                    )?;
                    // Set contentType
                    js_obj.define_property_or_throw(
                        js_string!("contentType"),
                        PropertyDescriptor::builder()
                            .value(JsValue::from(js_string!("text/html")))
                            .writable(false)
                            .configurable(true)
                            .enumerable(false)
                            .build(),
                        ctx2,
                    )?;
                    Ok(js_obj.into())
                }
                "text/xml" | "application/xml" | "application/xhtml+xml" | "image/svg+xml" => {
                    // Use quick-xml to parse into a new XML tree
                    let new_tree = parse_xml_into_tree(&input);
                    let content_type = mime_type.clone();
                    let doc_id = new_tree.borrow().document();
                    let js_obj = get_or_create_js_element(doc_id, new_tree.clone(), ctx2)?;
                    add_document_properties_to_element(
                        &js_obj,
                        new_tree,
                        content_type.clone(),
                        ctx2,
                    )?;
                    // Set contentType
                    js_obj.define_property_or_throw(
                        js_string!("contentType"),
                        PropertyDescriptor::builder()
                            .value(JsValue::from(js_string!(content_type)))
                            .writable(false)
                            .configurable(true)
                            .enumerable(false)
                            .build(),
                        ctx2,
                    )?;
                    Ok(js_obj.into())
                }
                _ => Err(JsError::from_opaque(
                    js_string!("TypeError: Invalid MIME type").into(),
                )),
            }
        })
    };

    proto
        .set(
            js_string!("parseFromString"),
            parse_fn.to_js_function(&realm),
            false,
            ctx,
        )
        .expect("failed to set DOMParser.prototype.parseFromString");

    // DOMParser constructor: new DOMParser() returns an instance with the prototype
    let proto_for_ctor = proto.clone();
    let ctor = unsafe {
        NativeFunction::from_closure(move |this, _args, _ctx2| {
            // When called as constructor, `this` is the new object
            if let Some(obj) = this.as_object() {
                obj.set_prototype(Some(proto_for_ctor.clone()));
            }
            Ok(JsValue::undefined())
        })
    };

    let ctor_obj: JsObject = FunctionObjectBuilder::new(ctx.realm(), ctor)
        .name(js_string!("DOMParser"))
        .length(0)
        .constructor(true)
        .build()
        .into();

    ctor_obj
        .define_property_or_throw(
            js_string!("prototype"),
            PropertyDescriptor::builder()
                .value(proto.clone())
                .writable(false)
                .configurable(false)
                .enumerable(false)
                .build(),
            ctx,
        )
        .expect("failed to define DOMParser.prototype");

    proto
        .define_property_or_throw(
            js_string!("constructor"),
            PropertyDescriptor::builder()
                .value(ctor_obj.clone())
                .writable(true)
                .configurable(true)
                .enumerable(false)
                .build(),
            ctx,
        )
        .expect("failed to set DOMParser.prototype.constructor");

    ctx.register_global_property(
        js_string!("DOMParser"),
        ctor_obj,
        Attribute::WRITABLE | Attribute::CONFIGURABLE,
    )
    .expect("failed to register DOMParser global");
}
