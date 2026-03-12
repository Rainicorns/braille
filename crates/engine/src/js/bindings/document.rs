use std::cell::RefCell;
use std::rc::Rc;

use boa_engine::{
    class::Class,
    js_string,
    native_function::NativeFunction,
    object::ObjectInitializer,
    property::{Attribute, PropertyDescriptor},
    Context, JsData, JsError, JsObject, JsResult, JsValue,
};
use boa_gc::{Finalize, Trace};

use crate::dom::DomTree;

use super::element::JsElement;
use super::class_list::register_class_list_class;
use super::style::register_style_class;
use super::query;

// ---------------------------------------------------------------------------
// JsDocument — singleton global `document` object backed by DomTree
// ---------------------------------------------------------------------------

#[derive(Debug, Trace, Finalize, JsData)]
pub(crate) struct JsDocument {
    #[unsafe_ignore_trace]
    pub(crate) tree: Rc<RefCell<DomTree>>,
}

/// Native implementation of document.createElement(tagName)
fn document_create_element(
    this: &JsValue,
    args: &[JsValue],
    ctx: &mut Context,
) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("createElement: `this` is not an object").into()))?;
    let doc = obj
        .downcast_ref::<JsDocument>()
        .ok_or_else(|| JsError::from_opaque(js_string!("createElement: `this` is not document").into()))?;
    let tree = doc.tree.clone();

    let tag = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_else(|| "undefined".to_string());

    let node_id = tree.borrow_mut().create_element(&tag);

    let element = JsElement::new(node_id, tree);
    let js_obj = JsElement::from_data(element, ctx)?;
    Ok(js_obj.into())
}

/// Native implementation of document.getElementById(id)
fn document_get_element_by_id(
    this: &JsValue,
    args: &[JsValue],
    ctx: &mut Context,
) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("getElementById: `this` is not an object").into()))?;
    let doc = obj
        .downcast_ref::<JsDocument>()
        .ok_or_else(|| JsError::from_opaque(js_string!("getElementById: `this` is not document").into()))?;
    let tree = doc.tree.clone();

    let id = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    let found = tree.borrow().get_element_by_id(&id);
    match found {
        Some(node_id) => {
            let element = JsElement::new(node_id, tree);
            let js_obj = JsElement::from_data(element, ctx)?;
            Ok(js_obj.into())
        }
        None => Ok(JsValue::null()),
    }
}

/// Native getter for document.body
fn document_get_body(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("body getter: `this` is not an object").into()))?;
    let doc = obj
        .downcast_ref::<JsDocument>()
        .ok_or_else(|| JsError::from_opaque(js_string!("body getter: `this` is not document").into()))?;
    let tree = doc.tree.borrow();
    match tree.body() {
        Some(body_id) => {
            drop(tree);
            let element = JsElement::new(body_id, doc.tree.clone());
            let js_obj = JsElement::from_data(element, ctx)?;
            Ok(js_obj.into())
        }
        None => Ok(JsValue::null()),
    }
}

/// Native getter for document.head
fn document_get_head(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("head getter: `this` is not an object").into()))?;
    let doc = obj
        .downcast_ref::<JsDocument>()
        .ok_or_else(|| JsError::from_opaque(js_string!("head getter: `this` is not document").into()))?;
    let tree = doc.tree.borrow();
    match tree.head() {
        Some(head_id) => {
            drop(tree);
            let element = JsElement::new(head_id, doc.tree.clone());
            let js_obj = JsElement::from_data(element, ctx)?;
            Ok(js_obj.into())
        }
        None => Ok(JsValue::null()),
    }
}

/// Native getter for document.title
fn document_get_title(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("title getter: `this` is not an object").into()))?;
    let doc = obj
        .downcast_ref::<JsDocument>()
        .ok_or_else(|| JsError::from_opaque(js_string!("title getter: `this` is not document").into()))?;
    let tree = doc.tree.borrow();
    let titles = tree.get_elements_by_tag_name("title");
    if let Some(&title_id) = titles.first() {
        let text = tree.get_text_content(title_id);
        Ok(JsValue::from(js_string!(text)))
    } else {
        Ok(JsValue::from(js_string!("")))
    }
}

/// Native setter for document.title
fn document_set_title(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("title setter: `this` is not an object").into()))?;
    let doc = obj
        .downcast_ref::<JsDocument>()
        .ok_or_else(|| JsError::from_opaque(js_string!("title setter: `this` is not document").into()))?;
    let text = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    let mut tree = doc.tree.borrow_mut();
    let titles = tree.get_elements_by_tag_name("title");
    if let Some(&title_id) = titles.first() {
        tree.set_text_content(title_id, &text);
    } else {
        // Create <title> element if it doesn't exist
        let title_id = tree.create_element("title");
        tree.set_text_content(title_id, &text);
        // Try to append to <head> if it exists, otherwise to document
        if let Some(head_id) = tree.head() {
            tree.append_child(head_id, title_id);
        } else {
            let doc_id = tree.document();
            tree.append_child(doc_id, title_id);
        }
    }
    Ok(JsValue::undefined())
}

/// Native implementation of document.createTextNode(text)
fn document_create_text_node(
    this: &JsValue,
    args: &[JsValue],
    ctx: &mut Context,
) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("createTextNode: `this` is not an object").into()))?;
    let doc = obj
        .downcast_ref::<JsDocument>()
        .ok_or_else(|| JsError::from_opaque(js_string!("createTextNode: `this` is not document").into()))?;
    let tree = doc.tree.clone();

    let text = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    let node_id = tree.borrow_mut().create_text(&text);

    let element = JsElement::new(node_id, tree);
    let js_obj = JsElement::from_data(element, ctx)?;
    Ok(js_obj.into())
}

/// Builds the `document` global object and registers it on the context.
pub(crate) fn register_document(tree: Rc<RefCell<DomTree>>, context: &mut Context) {
    // Register the Element class first so from_data works
    context.register_global_class::<JsElement>().unwrap();

    // Register the ClassList class so from_data works for classList getter
    register_class_list_class(context);

    // Register the CSSStyleDeclaration class so from_data works for style getter
    register_style_class(context);

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
        .build();

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

    context
        .register_global_property(js_string!("document"), document, Attribute::all())
        .expect("failed to register document global");
}
