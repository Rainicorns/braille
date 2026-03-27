use std::cell::RefCell;
use std::rc::Rc;

use boa_engine::{
    js_string,
    native_function::NativeFunction,
    object::ObjectInitializer,
    property::{Attribute, PropertyDescriptor},
    Context, JsError, JsObject, JsValue,
};

use crate::dom::DomTree;

use super::super::class_list::register_class_list_class;
use super::super::element::{get_or_create_js_element, JsElement};
use super::super::query;
use super::super::style::register_style_class;
use super::creation::*;
use super::domimpl::*;
use super::events::*;
use super::mutation::*;
use super::traversal::*;
use super::JsDocument;
use crate::js::realm_state;

/// Register the DOMImplementation global constructor (illegal — just for instanceof)
pub(crate) fn register_domimplementation(ctx: &mut Context) {
    let proto = ObjectInitializer::new(ctx).build();

    let ctor = unsafe {
        NativeFunction::from_closure(|_this, _args, _ctx| {
            Err(JsError::from_opaque(JsValue::from(js_string!("Illegal constructor"))))
        })
    };
    let ctor_obj: JsObject = boa_engine::object::FunctionObjectBuilder::new(ctx.realm(), ctor)
        .name(js_string!("DOMImplementation"))
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
        .expect("failed to define DOMImplementation.prototype");

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
        .expect("failed to set DOMImplementation.prototype.constructor");

    realm_state::set_domimpl_proto(ctx, proto);

    ctx.register_global_property(
        js_string!("DOMImplementation"),
        ctor_obj,
        Attribute::WRITABLE | Attribute::CONFIGURABLE,
    )
    .expect("failed to register DOMImplementation global");
}

/// Builds the `document` global object and registers it on the context.
pub(crate) fn register_document(tree: Rc<RefCell<DomTree>>, context: &mut Context) {
    // Register the Element class first so from_data works
    context.register_global_class::<JsElement>().unwrap();

    // Register the ClassList class so from_data works for classList getter
    register_class_list_class(context);

    // Register the CSSStyleDeclaration class so from_data works for style getter
    register_style_class(context);

    // Save tree pointer and doc_id for NODE_CACHE registration below
    let tree_ptr = Rc::as_ptr(&tree) as usize;
    let doc_id = tree.borrow().document();

    let tree_for_tw = tree.clone();
    let doc_data = JsDocument { tree };

    let document: JsObject = ObjectInitializer::with_native_data(doc_data, context)
        .function(
            NativeFunction::from_fn_ptr(document_create_element),
            js_string!("createElement"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(document_get_element_by_id),
            js_string!("getElementById"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(document_create_text_node),
            js_string!("createTextNode"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(query::document_query_selector),
            js_string!("querySelector"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(query::document_query_selector_all),
            js_string!("querySelectorAll"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(query::document_get_elements_by_class_name),
            js_string!("getElementsByClassName"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(query::document_get_elements_by_tag_name),
            js_string!("getElementsByTagName"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(query::document_get_elements_by_tag_name_ns),
            js_string!("getElementsByTagNameNS"),
            2,
        )
        .function(
            NativeFunction::from_fn_ptr(document_create_element_ns),
            js_string!("createElementNS"),
            2,
        )
        .function(
            NativeFunction::from_fn_ptr(document_create_comment),
            js_string!("createComment"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(document_create_processing_instruction),
            js_string!("createProcessingInstruction"),
            2,
        )
        .function(
            NativeFunction::from_fn_ptr(document_create_document_fragment),
            js_string!("createDocumentFragment"),
            0,
        )
        .function(
            NativeFunction::from_fn_ptr(document_create_range),
            js_string!("createRange"),
            0,
        )
        .function(
            NativeFunction::from_fn_ptr(document_create_attribute),
            js_string!("createAttribute"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(document_create_attribute_ns),
            js_string!("createAttributeNS"),
            2,
        )
        .function(
            NativeFunction::from_fn_ptr(document_create_event),
            js_string!("createEvent"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(document_add_event_listener),
            js_string!("addEventListener"),
            2,
        )
        .function(
            NativeFunction::from_fn_ptr(document_remove_event_listener),
            js_string!("removeEventListener"),
            2,
        )
        .function(
            NativeFunction::from_fn_ptr(document_dispatch_event),
            js_string!("dispatchEvent"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(super::super::mutation::document_append),
            js_string!("append"),
            0,
        )
        .function(
            NativeFunction::from_fn_ptr(super::super::mutation::document_prepend),
            js_string!("prepend"),
            0,
        )
        .function(
            NativeFunction::from_fn_ptr(super::super::mutation::document_replace_children),
            js_string!("replaceChildren"),
            0,
        )
        .function(
            NativeFunction::from_fn_ptr(super::super::mutation::document_normalize),
            js_string!("normalize"),
            0,
        )
        .function(
            NativeFunction::from_fn_ptr(document_get_root_node),
            js_string!("getRootNode"),
            0,
        )
        .function(
            NativeFunction::from_fn_ptr(document_create_element_ns),
            js_string!("createElementNS"),
            2,
        )
        .function(
            NativeFunction::from_fn_ptr(document_contains),
            js_string!("contains"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(document_append_child),
            js_string!("appendChild"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(document_remove_child),
            js_string!("removeChild"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(document_import_node),
            js_string!("importNode"),
            2,
        )
        .function(
            NativeFunction::from_fn_ptr(document_adopt_node),
            js_string!("adoptNode"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(super::super::element::node_compare_document_position),
            js_string!("compareDocumentPosition"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(document_clone_node),
            js_string!("cloneNode"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(super::super::element::node_is_equal_node),
            js_string!("isEqualNode"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(super::super::element::node_is_same_node),
            js_string!("isSameNode"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(super::super::element::node_contains),
            js_string!("contains"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(super::super::element::node_lookup_namespace_uri),
            js_string!("lookupNamespaceURI"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(super::super::element::node_lookup_prefix),
            js_string!("lookupPrefix"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(super::super::element::node_is_default_namespace),
            js_string!("isDefaultNamespace"),
            1,
        )
        .build();

    // createTreeWalker
    super::super::tree_walker::register_create_tree_walker(&document, tree_for_tw.clone(), context);

    // createNodeIterator
    super::super::node_iterator::register_create_node_iterator(&document, tree_for_tw, context);

    // Add accessor properties (body, head, title)
    let realm = context.realm().clone();

    // document.body (getter only)
    let body_getter = NativeFunction::from_fn_ptr(document_get_body);
    document
        .define_property_or_throw(
            js_string!("body"),
            PropertyDescriptor::builder()
                .get(body_getter.to_js_function(&realm))
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to define document.body");

    // document.head (getter only)
    let head_getter = NativeFunction::from_fn_ptr(document_get_head);
    document
        .define_property_or_throw(
            js_string!("head"),
            PropertyDescriptor::builder()
                .get(head_getter.to_js_function(&realm))
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to define document.head");

    // document.documentElement (getter only)
    let document_element_getter = NativeFunction::from_fn_ptr(document_get_document_element);
    document
        .define_property_or_throw(
            js_string!("documentElement"),
            PropertyDescriptor::builder()
                .get(document_element_getter.to_js_function(&realm))
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to define document.documentElement");

    // document.defaultView (getter only) — returns window
    let dv_getter = unsafe {
        NativeFunction::from_closure(move |_this, _args, ctx2| {
            let global = ctx2.global_object();
            let window = global.get(js_string!("window"), ctx2)?;
            Ok(window)
        })
    };
    document
        .define_property_or_throw(
            js_string!("defaultView"),
            PropertyDescriptor::builder()
                .get(dv_getter.to_js_function(&realm))
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to define document.defaultView");

    // document.title (getter and setter)
    let title_getter = NativeFunction::from_fn_ptr(document_get_title);
    let title_setter = NativeFunction::from_fn_ptr(document_set_title);
    document
        .define_property_or_throw(
            js_string!("title"),
            PropertyDescriptor::builder()
                .get(title_getter.to_js_function(&realm))
                .set(title_setter.to_js_function(&realm))
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to define document.title");

    // document.implementation — object with createDocumentType, createHTMLDocument, createDocument, hasFeature
    let implementation = ObjectInitializer::new(context)
        .function(
            NativeFunction::from_fn_ptr(domimpl_create_document_type),
            js_string!("createDocumentType"),
            3,
        )
        .function(
            NativeFunction::from_fn_ptr(domimpl_create_html_document),
            js_string!("createHTMLDocument"),
            0,
        )
        .function(
            NativeFunction::from_fn_ptr(domimpl_create_document),
            js_string!("createDocument"),
            2,
        )
        .function(
            NativeFunction::from_fn_ptr(domimpl_has_feature),
            js_string!("hasFeature"),
            0,
        )
        .build();
    // Set DOMImplementation prototype for instanceof checks
    if let Some(p) = realm_state::domimpl_proto(context) {
        implementation.set_prototype(Some(p));
    }
    document
        .define_property_or_throw(
            js_string!("implementation"),
            PropertyDescriptor::builder()
                .value(JsValue::from(implementation))
                .writable(false)
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to define document.implementation");

    // document.doctype (getter only)
    let doctype_getter = NativeFunction::from_fn_ptr(document_get_doctype);
    document
        .define_property_or_throw(
            js_string!("doctype"),
            PropertyDescriptor::builder()
                .get(doctype_getter.to_js_function(&realm))
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to define document.doctype");

    // document.nodeName (always "#document")
    document
        .define_property_or_throw(
            js_string!("nodeName"),
            PropertyDescriptor::builder()
                .value(JsValue::from(js_string!("#document")))
                .writable(false)
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to define document.nodeName");

    // document.nodeType (always 9)
    document
        .define_property_or_throw(
            js_string!("nodeType"),
            PropertyDescriptor::builder()
                .value(JsValue::from(9))
                .writable(false)
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to define document.nodeType");

    // document.textContent (getter returns null, setter is no-op)
    let text_content_getter = NativeFunction::from_fn_ptr(|_this, _args, _ctx| Ok(JsValue::null()));
    let text_content_setter = NativeFunction::from_fn_ptr(|_this, _args, _ctx| Ok(JsValue::undefined()));
    document
        .define_property_or_throw(
            js_string!("textContent"),
            PropertyDescriptor::builder()
                .get(text_content_getter.to_js_function(&realm))
                .set(text_content_setter.to_js_function(&realm))
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to define document.textContent");

    // document.nodeValue (getter returns null, setter is no-op)
    let node_value_getter = NativeFunction::from_fn_ptr(|_this, _args, _ctx| Ok(JsValue::null()));
    let node_value_setter = NativeFunction::from_fn_ptr(|_this, _args, _ctx| Ok(JsValue::undefined()));
    document
        .define_property_or_throw(
            js_string!("nodeValue"),
            PropertyDescriptor::builder()
                .get(node_value_getter.to_js_function(&realm))
                .set(node_value_setter.to_js_function(&realm))
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to define document.nodeValue");

    // document.parentNode (getter returns null)
    let parent_node_getter = NativeFunction::from_fn_ptr(document_get_parent_node);
    document
        .define_property_or_throw(
            js_string!("parentNode"),
            PropertyDescriptor::builder()
                .get(parent_node_getter.to_js_function(&realm))
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to define document.parentNode");

    // document.parentElement (getter returns null)
    let parent_element_getter = NativeFunction::from_fn_ptr(document_get_parent_element);
    document
        .define_property_or_throw(
            js_string!("parentElement"),
            PropertyDescriptor::builder()
                .get(parent_element_getter.to_js_function(&realm))
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to define document.parentElement");

    // document.childNodes (getter)
    let child_nodes_getter = NativeFunction::from_fn_ptr(document_get_child_nodes);
    document
        .define_property_or_throw(
            js_string!("childNodes"),
            PropertyDescriptor::builder()
                .get(child_nodes_getter.to_js_function(&realm))
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to define document.childNodes");

    // document.firstChild (getter)
    let first_child_getter = NativeFunction::from_fn_ptr(document_get_first_child);
    document
        .define_property_or_throw(
            js_string!("firstChild"),
            PropertyDescriptor::builder()
                .get(first_child_getter.to_js_function(&realm))
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to define document.firstChild");

    // document.lastChild (getter)
    let last_child_getter = NativeFunction::from_fn_ptr(document_get_last_child);
    document
        .define_property_or_throw(
            js_string!("lastChild"),
            PropertyDescriptor::builder()
                .get(last_child_getter.to_js_function(&realm))
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to define document.lastChild");

    // document.ownerDocument (getter returns null per spec)
    let owner_doc_getter = NativeFunction::from_fn_ptr(document_get_parent_node); // reuse null-returning fn
    document
        .define_property_or_throw(
            js_string!("ownerDocument"),
            PropertyDescriptor::builder()
                .get(owner_doc_getter.to_js_function(&realm))
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to define document.ownerDocument");

    // document.nextSibling (getter returns null — document has no parent)
    let next_sib_getter = NativeFunction::from_fn_ptr(document_get_parent_node);
    document
        .define_property_or_throw(
            js_string!("nextSibling"),
            PropertyDescriptor::builder()
                .get(next_sib_getter.to_js_function(&realm))
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to define document.nextSibling");

    // document.previousSibling (getter returns null — document has no parent)
    let prev_sib_getter = NativeFunction::from_fn_ptr(document_get_parent_node);
    document
        .define_property_or_throw(
            js_string!("previousSibling"),
            PropertyDescriptor::builder()
                .get(prev_sib_getter.to_js_function(&realm))
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to define document.previousSibling");

    // document.hasChildNodes()
    let has_child_nodes_fn = NativeFunction::from_fn_ptr(document_has_child_nodes);
    document
        .define_property_or_throw(
            js_string!("hasChildNodes"),
            PropertyDescriptor::builder()
                .value(has_child_nodes_fn.to_js_function(&realm))
                .writable(true)
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to define document.hasChildNodes");

    // document.URL (always "about:blank" — no real URL context in the engine)
    document
        .define_property_or_throw(
            js_string!("URL"),
            PropertyDescriptor::builder()
                .value(JsValue::from(js_string!("about:blank")))
                .writable(false)
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to define document.URL");

    // document.documentURI (alias for URL per spec)
    document
        .define_property_or_throw(
            js_string!("documentURI"),
            PropertyDescriptor::builder()
                .value(JsValue::from(js_string!("about:blank")))
                .writable(false)
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to define document.documentURI");

    // document.compatMode (always "CSS1Compat" — no-quirks mode)
    document
        .define_property_or_throw(
            js_string!("compatMode"),
            PropertyDescriptor::builder()
                .value(JsValue::from(js_string!("CSS1Compat")))
                .writable(false)
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to define document.compatMode");

    // document.characterSet (always "UTF-8")
    document
        .define_property_or_throw(
            js_string!("characterSet"),
            PropertyDescriptor::builder()
                .value(JsValue::from(js_string!("UTF-8")))
                .writable(false)
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to define document.characterSet");

    // document.charset (legacy alias for characterSet)
    document
        .define_property_or_throw(
            js_string!("charset"),
            PropertyDescriptor::builder()
                .value(JsValue::from(js_string!("UTF-8")))
                .writable(false)
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to define document.charset");

    // document.inputEncoding (legacy alias for characterSet)
    document
        .define_property_or_throw(
            js_string!("inputEncoding"),
            PropertyDescriptor::builder()
                .value(JsValue::from(js_string!("UTF-8")))
                .writable(false)
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to define document.inputEncoding");

    // document.contentType (always "text/html" for the global parsed document)
    document
        .define_property_or_throw(
            js_string!("contentType"),
            PropertyDescriptor::builder()
                .value(JsValue::from(js_string!("text/html")))
                .writable(false)
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to define document.contentType");

    // Store the document JsObject in NODE_CACHE so that get_or_create_js_element
    // returns this same object when looking up the Document node. This ensures
    // evt.currentTarget === document during event propagation.
    {
        let cache = realm_state::node_cache(context);
        cache.borrow_mut().insert((tree_ptr, doc_id), document.clone());
    }

    context
        .register_global_property(js_string!("document"), document, Attribute::all())
        .expect("failed to register document global");
}
