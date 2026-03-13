use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use boa_engine::{
    class::{Class, ClassBuilder},
    js_string,
    native_function::NativeFunction,
    property::{Attribute, PropertyDescriptor},
    Context, JsData, JsError, JsNativeError, JsObject, JsResult, JsValue,
};
use boa_gc::{Finalize, Trace};

use crate::dom::{DomTree, NodeData, NodeId};

use super::class_list::JsClassList;
use super::document::JsDocument;
use super::event::JsEvent;
use super::event_target::{ListenerEntry, EVENT_LISTENERS};
use super::window::{WINDOW_LISTENER_ID, WINDOW_OBJECT};

/// Compare nodes from different DomTrees for equality (recursive).
fn cross_tree_is_equal_node(tree_a: &DomTree, a: NodeId, tree_b: &DomTree, b: NodeId) -> bool {
    let node_a = tree_a.get_node(a);
    let node_b = tree_b.get_node(b);

    if tree_a.node_type(a) != tree_b.node_type(b) {
        return false;
    }

    match (&node_a.data, &node_b.data) {
        (
            NodeData::Element { tag_name: t1, attributes: a1, namespace: ns1 },
            NodeData::Element { tag_name: t2, attributes: a2, namespace: ns2 },
        ) => {
            if t1 != t2 || ns1 != ns2 || a1.len() != a2.len() {
                return false;
            }
            for attr in a1 {
                if !a2.iter().any(|a| a.local_name == attr.local_name && a.namespace == attr.namespace && a.value == attr.value) {
                    return false;
                }
            }
        }
        (
            NodeData::Doctype { name: n1, public_id: p1, system_id: s1 },
            NodeData::Doctype { name: n2, public_id: p2, system_id: s2 },
        ) => {
            if n1 != n2 || p1 != p2 || s1 != s2 {
                return false;
            }
        }
        (NodeData::Text { content: c1 }, NodeData::Text { content: c2 }) => {
            if c1 != c2 { return false; }
        }
        (NodeData::Comment { content: c1 }, NodeData::Comment { content: c2 }) => {
            if c1 != c2 { return false; }
        }
        (
            NodeData::ProcessingInstruction { target: t1, data: d1 },
            NodeData::ProcessingInstruction { target: t2, data: d2 },
        ) => {
            if t1 != t2 || d1 != d2 { return false; }
        }
        (
            NodeData::Attr { local_name: ln1, namespace: ns1, prefix: p1, value: v1 },
            NodeData::Attr { local_name: ln2, namespace: ns2, prefix: p2, value: v2 },
        ) => {
            if ln1 != ln2 || ns1 != ns2 || p1 != p2 || v1 != v2 { return false; }
        }
        (NodeData::Document, NodeData::Document) => {}
        (NodeData::DocumentFragment, NodeData::DocumentFragment) => {}
        _ => return false,
    }

    if node_a.children.len() != node_b.children.len() {
        return false;
    }
    for (ca, cb) in node_a.children.iter().zip(node_b.children.iter()) {
        if !cross_tree_is_equal_node(tree_a, *ca, tree_b, *cb) {
            return false;
        }
    }
    true
}

/// Extract (NodeId, tree) from a JsValue that could be either JsElement or JsDocument.
/// Returns None if the value is not a Node-like object.
fn extract_node_id(val: &JsValue) -> Option<(NodeId, Rc<RefCell<DomTree>>)> {
    let obj = val.as_object()?;
    if let Some(el) = obj.downcast_ref::<JsElement>() {
        return Some((el.node_id, el.tree.clone()));
    }
    if let Some(doc) = obj.downcast_ref::<JsDocument>() {
        let doc_id = doc.tree.borrow().document();
        return Some((doc_id, doc.tree.clone()));
    }
    None
}

// ---------------------------------------------------------------------------
// NodeId -> JsObject cache (thread-local, same pattern as EVENT_LISTENERS)
// ---------------------------------------------------------------------------

/// Cache key is (tree_ptr, node_id) so nodes from different trees don't collide.
pub(crate) type NodeCache = HashMap<(usize, NodeId), JsObject>;

thread_local! {
    pub(crate) static NODE_CACHE: RefCell<Option<Rc<RefCell<NodeCache>>>> = const { RefCell::new(None) };
}

// ---------------------------------------------------------------------------
// DOM tree thread-local (used by Text/Comment constructors)
// ---------------------------------------------------------------------------
thread_local! {
    pub(crate) static DOM_TREE: RefCell<Option<Rc<RefCell<DomTree>>>> = const { RefCell::new(None) };
}

// ---------------------------------------------------------------------------
// Prototype objects for proper instanceof support
// (Node.prototype -> CharacterData.prototype -> Text.prototype / Comment.prototype)
// ---------------------------------------------------------------------------

/// Holds the prototype objects for DOM node types.
/// Stored in a thread-local so get_or_create_js_element can assign the right prototype.
#[derive(Clone)]
pub(crate) struct DomPrototypes {
    pub(crate) text_proto: JsObject,
    pub(crate) comment_proto: JsObject,
    /// ProcessingInstruction.prototype
    pub(crate) pi_proto: Option<JsObject>,
    /// Attr.prototype
    pub(crate) attr_proto: Option<JsObject>,
    /// Map from lowercase tag name -> prototype object (e.g. "div" -> HTMLDivElement.prototype)
    pub(crate) html_tag_protos: HashMap<String, JsObject>,
    /// Fallback HTMLElement.prototype for HTML elements without a specific type
    pub(crate) html_element_proto: Option<JsObject>,
    /// HTMLUnknownElement.prototype for unknown HTML elements
    pub(crate) html_unknown_proto: Option<JsObject>,
    /// DocumentFragment.prototype
    pub(crate) document_fragment_proto: Option<JsObject>,
    /// DocumentType.prototype
    pub(crate) document_type_proto: Option<JsObject>,
    /// Document.prototype
    pub(crate) document_proto: Option<JsObject>,
}

thread_local! {
    pub(crate) static DOM_PROTOTYPES: RefCell<Option<DomPrototypes>> = const { RefCell::new(None) };
}

/// Look up or create the JsObject wrapper for a given NodeId.
/// Returns the same JsObject every time for the same NodeId, preserving `===` identity.
/// For Text and Comment nodes, sets the prototype to Text.prototype / Comment.prototype
/// so that `instanceof` checks work correctly.
pub(crate) fn get_or_create_js_element(
    node_id: NodeId,
    tree: Rc<RefCell<DomTree>>,
    ctx: &mut Context,
) -> JsResult<JsObject> {
    // Cache key includes tree pointer so nodes from different trees don't collide
    let tree_ptr = Rc::as_ptr(&tree) as usize;
    let cache_key = (tree_ptr, node_id);

    // Check cache first
    let cached = NODE_CACHE.with(|cell| {
        let rc = cell.borrow();
        let cache_rc = rc.as_ref().expect("NODE_CACHE not initialized");
        let cache = cache_rc.borrow();
        cache.get(&cache_key).cloned()
    });

    if let Some(obj) = cached {
        return Ok(obj);
    }

    // Determine node kind before creating the object
    enum NodeKind {
        Text,
        Comment,
        ProcessingInstruction { target: String },
        Attr { local_name: String, namespace: String, prefix: String },
        HtmlElement(String), // lowercase tag name
        NonHtmlElement,
        DocumentFragment,
        Doctype { name: String, public_id: String, system_id: String },
        Document,
    }

    let node_kind = {
        let tree_ref = tree.borrow();
        let node = tree_ref.get_node(node_id);
        match &node.data {
            NodeData::Text { .. } => NodeKind::Text,
            NodeData::Comment { .. } => NodeKind::Comment,
            NodeData::Element { tag_name, namespace, .. } => {
                if namespace == "http://www.w3.org/1999/xhtml" {
                    // Extract local name (after colon if present)
                    // Preserve original case — createElement and parser already lowercase,
                    // but createElementNS("SPAN") should NOT match "span" in html_tag_protos
                    let local = if let Some(pos) = tag_name.find(':') {
                        &tag_name[pos + 1..]
                    } else {
                        tag_name.as_str()
                    };
                    NodeKind::HtmlElement(local.to_string())
                } else {
                    NodeKind::NonHtmlElement
                }
            }
            NodeData::DocumentFragment => NodeKind::DocumentFragment,
            NodeData::Doctype { name, public_id, system_id } => NodeKind::Doctype {
                name: name.clone(),
                public_id: public_id.clone(),
                system_id: system_id.clone(),
            },
            NodeData::ProcessingInstruction { target, .. } => NodeKind::ProcessingInstruction { target: target.clone() },
            NodeData::Attr { local_name, namespace, prefix, .. } => NodeKind::Attr {
                local_name: local_name.clone(),
                namespace: namespace.clone(),
                prefix: prefix.clone(),
            },
            NodeData::Document => NodeKind::Document,
        }
    };

    // Cache miss — create and store
    let element = JsElement::new(node_id, tree.clone());
    let js_obj = JsElement::from_data(element, ctx)?;

    // Set the right prototype based on node kind (for instanceof support)
    DOM_PROTOTYPES.with(|cell| {
        let protos = cell.borrow();
        if let Some(ref p) = *protos {
            match &node_kind {
                NodeKind::Text => {
                    js_obj.set_prototype(Some(p.text_proto.clone()));
                }
                NodeKind::Comment => {
                    js_obj.set_prototype(Some(p.comment_proto.clone()));
                }
                NodeKind::ProcessingInstruction { .. } => {
                    if let Some(ref proto) = p.pi_proto {
                        js_obj.set_prototype(Some(proto.clone()));
                    }
                }
                NodeKind::Attr { .. } => {
                    if let Some(ref proto) = p.attr_proto {
                        js_obj.set_prototype(Some(proto.clone()));
                    }
                }
                NodeKind::HtmlElement(tag) => {
                    // Look up specific HTML element prototype by tag name
                    if let Some(proto) = p.html_tag_protos.get(tag.as_str()) {
                        js_obj.set_prototype(Some(proto.clone()));
                    } else if is_known_html_element(tag) {
                        // Known HTML element without a specific type -> HTMLElement
                        if let Some(ref proto) = p.html_element_proto {
                            js_obj.set_prototype(Some(proto.clone()));
                        }
                    } else {
                        // Unknown HTML element -> HTMLUnknownElement
                        if let Some(ref proto) = p.html_unknown_proto {
                            js_obj.set_prototype(Some(proto.clone()));
                        }
                    }
                }
                NodeKind::NonHtmlElement => {
                    // Non-HTML namespace elements keep Element.prototype (default)
                }
                NodeKind::DocumentFragment => {
                    if let Some(ref proto) = p.document_fragment_proto {
                        js_obj.set_prototype(Some(proto.clone()));
                    }
                }
                NodeKind::Doctype { .. } => {
                    if let Some(ref proto) = p.document_type_proto {
                        js_obj.set_prototype(Some(proto.clone()));
                    }
                }
                NodeKind::Document => {
                    if let Some(ref proto) = p.document_proto {
                        js_obj.set_prototype(Some(proto.clone()));
                    }
                }
            }
        }
    });

    // Set own property for ProcessingInstruction nodes (target)
    if let NodeKind::ProcessingInstruction { target } = &node_kind {
        js_obj.define_property_or_throw(
            js_string!("target"),
            PropertyDescriptor::builder()
                .value(JsValue::from(js_string!(target.clone())))
                .writable(false)
                .configurable(true)
                .enumerable(true)
                .build(),
            ctx,
        )?;
    }

    // Set own properties for Attr nodes (name, value, namespaceURI, prefix, localName, ownerElement, specified)
    if let NodeKind::Attr { local_name, namespace, prefix } = &node_kind {
        // name = qualified name (prefix:localName or just localName)
        let qualified_name = if prefix.is_empty() {
            local_name.clone()
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
        let tree_for_setter = tree.clone();
        let nid_for_setter = node_id;
        let value_setter = unsafe {
            NativeFunction::from_closure(move |_this, args, ctx2| {
                let new_val = args
                    .first()
                    .map(|v| v.to_string(ctx2))
                    .transpose()?
                    .map(|s| s.to_std_string_escaped())
                    .unwrap_or_default();
                if let NodeData::Attr { ref mut value, .. } = tree_for_setter.borrow_mut().get_node_mut(nid_for_setter).data {
                    *value = new_val;
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
            JsValue::from(js_string!(namespace.clone()))
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
            JsValue::from(js_string!(prefix.clone()))
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
                .value(JsValue::from(js_string!(local_name.clone())))
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
    }

    // Set own properties for Doctype nodes (name, publicId, systemId)
    if let NodeKind::Doctype { name, public_id, system_id } = &node_kind {
        js_obj.define_property_or_throw(
            js_string!("name"),
            PropertyDescriptor::builder()
                .value(JsValue::from(js_string!(name.clone())))
                .writable(false)
                .configurable(true)
                .enumerable(true)
                .build(),
            ctx,
        )?;
        js_obj.define_property_or_throw(
            js_string!("publicId"),
            PropertyDescriptor::builder()
                .value(JsValue::from(js_string!(public_id.clone())))
                .writable(false)
                .configurable(true)
                .enumerable(true)
                .build(),
            ctx,
        )?;
        js_obj.define_property_or_throw(
            js_string!("systemId"),
            PropertyDescriptor::builder()
                .value(JsValue::from(js_string!(system_id.clone())))
                .writable(false)
                .configurable(true)
                .enumerable(true)
                .build(),
            ctx,
        )?;
    }

    // Set own property for <template> elements (content -> DocumentFragment)
    if let NodeKind::HtmlElement(ref tag) = node_kind {
        if tag == "template" {
            let tree_for_content = tree.clone();
            let nid_for_content = node_id;
            let content_getter = unsafe {
                NativeFunction::from_closure(move |_this, _args, ctx2| {
                    let content_id = {
                        let tree_ref = tree_for_content.borrow();
                        tree_ref.get_node(nid_for_content).template_contents
                    };
                    match content_id {
                        Some(cid) => {
                            let obj = get_or_create_js_element(cid, tree_for_content.clone(), ctx2)?;
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
        }
    }

    NODE_CACHE.with(|cell| {
        let rc = cell.borrow();
        let cache_rc = rc.as_ref().expect("NODE_CACHE not initialized");
        let mut cache = cache_rc.borrow_mut();
        cache.insert(cache_key, js_obj.clone());
    });

    Ok(js_obj)
}

/// Returns true if the given lowercase tag name is a known HTML element
/// (i.e., it should get HTMLElement.prototype rather than HTMLUnknownElement.prototype).
fn is_known_html_element(tag: &str) -> bool {
    matches!(
        tag,
        "a" | "abbr" | "acronym" | "address" | "area" | "article" | "aside"
        | "audio" | "b" | "base" | "bdi" | "bdo" | "bgsound" | "big"
        | "blockquote" | "body" | "br" | "button" | "canvas" | "caption"
        | "center" | "cite" | "code" | "col" | "colgroup" | "data"
        | "datalist" | "dd" | "del" | "details" | "dfn" | "dialog" | "dir"
        | "div" | "dl" | "dt" | "embed" | "em" | "fieldset" | "figcaption"
        | "figure" | "font" | "footer" | "form" | "frame" | "frameset"
        | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "head" | "header"
        | "hgroup" | "hr" | "html" | "i" | "iframe" | "img" | "input"
        | "ins" | "isindex" | "kbd" | "label" | "legend" | "li" | "link"
        | "main" | "map" | "mark" | "marquee" | "meta" | "meter" | "nav"
        | "nobr" | "noframes" | "noscript" | "object" | "ol" | "optgroup"
        | "option" | "output" | "p" | "param" | "pre" | "progress" | "q"
        | "rp" | "rt" | "ruby" | "s" | "samp" | "script" | "section"
        | "select" | "small" | "source" | "spacer" | "span" | "strike"
        | "style" | "sub" | "summary" | "sup" | "table" | "tbody" | "td"
        | "template" | "textarea" | "tfoot" | "th" | "thead" | "time"
        | "title" | "tr" | "track" | "tt" | "u" | "ul" | "var" | "video"
        | "wbr"
    )
}

// ---------------------------------------------------------------------------
// JsElement — the Class-based wrapper around a DomTree node
// ---------------------------------------------------------------------------

#[derive(Debug, Trace, Finalize, JsData)]
pub(crate) struct JsElement {
    #[unsafe_ignore_trace]
    pub(crate) node_id: NodeId,
    #[unsafe_ignore_trace]
    pub(crate) tree: Rc<RefCell<DomTree>>,
}

impl JsElement {
    pub fn new(node_id: NodeId, tree: Rc<RefCell<DomTree>>) -> Self {
        Self { node_id, tree }
    }

    /// Native implementation of element.appendChild(child)
    fn append_child(this: &JsValue, args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
        let this_obj = this
            .as_object()
            .ok_or_else(|| JsNativeError::typ().with_message("appendChild: this is not an object"))?;
        let parent = this_obj
            .downcast_ref::<JsElement>()
            .ok_or_else(|| JsNativeError::typ().with_message("appendChild: this is not a Node"))?;
        let parent_id = parent.node_id;
        let tree = parent.tree.clone();

        let child_arg = args
            .first()
            .ok_or_else(|| JsNativeError::typ().with_message("appendChild: 1 argument required"))?;
        if child_arg.is_null() || child_arg.is_undefined() {
            return Err(JsNativeError::typ().with_message("appendChild: argument 1 is not a Node").into());
        }
        let child_obj = child_arg
            .as_object()
            .ok_or_else(|| JsNativeError::typ().with_message("appendChild: argument 1 is not a Node"))?;
        let child = child_obj
            .downcast_ref::<JsElement>()
            .ok_or_else(|| JsNativeError::typ().with_message("appendChild: argument 1 is not a Node"))?;

        // Check if node is a Document - must reject before adoption changes it
        {
            let node_tree_ref = child.tree.borrow();
            let node_data = &node_tree_ref.get_node(child.node_id).data;
            if matches!(node_data, crate::dom::NodeData::Document) {
                return Err(JsNativeError::typ()
                    .with_message("HierarchyRequestError: Cannot insert a Document node")
                    .into());
            }
        }

        // Cross-tree adoption: if child is from a different DomTree, adopt it first
        let child_id = if !Rc::ptr_eq(&tree, &child.tree) {
            let src_tree = child.tree.clone();
            let src_id = child.node_id;
            let adopted_id = super::mutation::adopt_node(&src_tree, src_id, &tree);
            drop(child);
            let mut child_mut = child_obj.downcast_mut::<JsElement>().unwrap();
            child_mut.node_id = adopted_id;
            child_mut.tree = tree.clone();
            drop(child_mut);
            super::mutation::update_node_cache_after_adoption(&src_tree, src_id, &tree, adopted_id, &child_obj);
            adopted_id
        } else {
            child.node_id
        };

        // Pre-insertion validation (appendChild is insertBefore with null ref child)
        super::mutation::validate_pre_insert(&tree.borrow(), parent_id, child_id, None, None)?;

        // Perform the insertion (handles DocumentFragment children)
        super::mutation::do_insert(&tree, parent_id, child_id, None);

        // appendChild returns the appended child (or fragment)
        Ok(child_arg.clone())
    }

    /// Native getter for element.textContent
    /// Per spec: Document and Doctype return null, others return text.
    fn get_text_content(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
        let obj = this
            .as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("textContent getter: `this` is not an object").into()))?;
        let el = obj
            .downcast_ref::<JsElement>()
            .ok_or_else(|| JsError::from_opaque(js_string!("textContent getter: `this` is not an Element").into()))?;
        let tree = el.tree.borrow();
        let node = tree.get_node(el.node_id);
        // Per DOM spec: Document and Doctype nodes return null for textContent
        if matches!(node.data, crate::dom::NodeData::Document | crate::dom::NodeData::Doctype { .. }) {
            return Ok(JsValue::null());
        }
        let text = tree.get_text_content(el.node_id);
        Ok(JsValue::from(js_string!(text)))
    }

    /// Native setter for element.textContent
    /// Per spec:
    /// - Document, Doctype: no-op
    /// - Element, DocumentFragment: remove all children, then if value is non-empty create Text child
    /// - Text, Comment: set data
    fn set_text_content(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
        let obj = this
            .as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("textContent setter: `this` is not an object").into()))?;
        let el = obj
            .downcast_ref::<JsElement>()
            .ok_or_else(|| JsError::from_opaque(js_string!("textContent setter: `this` is not an Element").into()))?;

        // Per spec: Document and Doctype nodes ignore textContent setter
        {
            let tree = el.tree.borrow();
            let node = tree.get_node(el.node_id);
            if matches!(node.data, crate::dom::NodeData::Document | crate::dom::NodeData::Doctype { .. }) {
                return Ok(JsValue::undefined());
            }
        }

        let val = args.first().cloned().unwrap_or(JsValue::undefined());

        // Per spec: for Text/Comment/PI/Attr nodes, setting textContent sets data/value
        {
            let tree = el.tree.borrow();
            let node = tree.get_node(el.node_id);
            if matches!(node.data, crate::dom::NodeData::Attr { .. }) {
                drop(tree);
                let data = if val.is_null() {
                    String::new()
                } else {
                    val.to_string(ctx)?.to_std_string_escaped()
                };
                if let crate::dom::NodeData::Attr { ref mut value, .. } = el.tree.borrow_mut().get_node_mut(el.node_id).data {
                    *value = data;
                }
                return Ok(JsValue::undefined());
            }
            if matches!(node.data, crate::dom::NodeData::Text { .. } | crate::dom::NodeData::Comment { .. } | crate::dom::NodeData::ProcessingInstruction { .. }) {
                drop(tree);
                // null converts to ""
                let data = if val.is_null() {
                    String::new()
                } else {
                    val.to_string(ctx)?.to_std_string_escaped()
                };
                el.tree.borrow_mut().character_data_set(el.node_id, &data);
                return Ok(JsValue::undefined());
            }
        }

        // For Element/DocumentFragment: determine string value
        // null and undefined -> treat as null (remove all children, no text child)
        let text = if val.is_null() || val.is_undefined() {
            None
        } else {
            let s = val.to_string(ctx)?.to_std_string_escaped();
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        };

        let mut tree = el.tree.borrow_mut();
        // Remove all children
        tree.clear_children(el.node_id);

        // If value is non-empty, create a single Text child
        if let Some(text_str) = text {
            let text_id = tree.create_text(&text_str);
            tree.append_child(el.node_id, text_id);
        }

        Ok(JsValue::undefined())
    }

    /// Native getter for element.classList
    fn get_class_list(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
        let obj = this
            .as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("classList getter: `this` is not an object").into()))?;

        // Extract node_id and tree first, then drop the borrow guard
        let (node_id, tree) = {
            let el = obj
                .downcast_ref::<JsElement>()
                .ok_or_else(|| JsError::from_opaque(js_string!("classList getter: `this` is not an Element").into()))?;
            (el.node_id, el.tree.clone())
        };

        // Check for cached classList object (borrow guard on el is now dropped)
        let cache_key = js_string!("__classList");
        let cached = obj.get(cache_key.clone(), ctx)?;
        if cached.is_object() {
            // Update numeric indices on the cached object
            let cached_obj = cached.as_object().unwrap();

            // Get deduplicated classes
            let classes: Vec<String> = {
                let tree_borrow = tree.borrow();
                tree_borrow
                    .get_attribute(node_id, "class")
                    .map(|class_str| {
                        let mut seen = Vec::new();
                        for token in class_str.split_whitespace() {
                            let s = token.to_string();
                            if !seen.contains(&s) {
                                seen.push(s);
                            }
                        }
                        seen
                    })
                    .unwrap_or_default()
            };

            // Update numeric indices: set current values and clear extras
            for i in 0..20 {
                let key = js_string!(i.to_string());
                if i < classes.len() {
                    cached_obj.set(
                        key,
                        JsValue::from(js_string!(classes[i].clone())),
                        false,
                        ctx,
                    )?;
                } else {
                    // Set beyond-range indices to undefined to effectively clear them
                    cached_obj.set(key, JsValue::undefined(), false, ctx)?;
                }
            }

            return Ok(cached);
        }

        // Get deduplicated classes for indexed access
        let classes: Vec<String> = {
            let tree_borrow = tree.borrow();
            tree_borrow
                .get_attribute(node_id, "class")
                .map(|class_str| {
                    let mut seen = Vec::new();
                    for token in class_str.split_whitespace() {
                        let s = token.to_string();
                        if !seen.contains(&s) {
                            seen.push(s);
                        }
                    }
                    seen
                })
                .unwrap_or_default()
        };

        let class_list = JsClassList::new(node_id, tree);
        let js_obj = JsClassList::from_data(class_list, ctx)?;

        // Populate numeric indices for classList[0], classList[1], etc.
        for (i, class_name) in classes.iter().enumerate() {
            js_obj.set(
                js_string!(i.to_string()),
                JsValue::from(js_string!(class_name.clone())),
                false,
                ctx,
            )?;
        }

        // Cache the classList object on the element
        let cached_val: JsValue = js_obj.clone().into();
        obj.set(cache_key, cached_val, false, ctx)?;

        Ok(js_obj.into())
    }

    /// Native setter for element.classList — sets the class attribute via value
    fn set_class_list(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
        let obj = this
            .as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("classList setter: `this` is not an object").into()))?;
        let el = obj
            .downcast_ref::<JsElement>()
            .ok_or_else(|| JsError::from_opaque(js_string!("classList setter: `this` is not an Element").into()))?;

        let value = args
            .first()
            .map(|v| v.to_string(ctx))
            .transpose()?
            .map(|s| s.to_std_string_escaped())
            .unwrap_or_default();

        el.tree
            .borrow_mut()
            .set_attribute(el.node_id, "class", &value);
        Ok(JsValue::undefined())
    }

    /// Native implementation of element.remove()
    fn remove(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
        let this_obj = this
            .as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("remove: `this` is not an object").into()))?;
        let el = this_obj
            .downcast_ref::<JsElement>()
            .ok_or_else(|| JsError::from_opaque(js_string!("remove: `this` is not an Element").into()))?;
        let node_id = el.node_id;
        let tree = el.tree.clone();
        tree.borrow_mut().remove_from_parent(node_id);
        Ok(JsValue::undefined())
    }

    /// Native implementation of node.isEqualNode(other)
    fn is_equal_node(this: &JsValue, args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
        let (this_id, tree) = extract_node_id(this)
            .ok_or_else(|| JsError::from_opaque(js_string!("isEqualNode: `this` is not a Node").into()))?;

        let other_val = match args.first() {
            Some(v) if !v.is_null() && !v.is_undefined() => v,
            _ => return Ok(JsValue::from(false)),
        };
        let (other_id, other_tree) = match extract_node_id(other_val) {
            Some(ids) => ids,
            None => return Ok(JsValue::from(false)),
        };

        // If both nodes are in the same tree, use the tree's built-in comparison
        if Rc::ptr_eq(&tree, &other_tree) {
            let result = tree.borrow().is_equal_node(this_id, other_id);
            return Ok(JsValue::from(result));
        }

        // Cross-tree comparison: compare nodes from different trees
        let result = cross_tree_is_equal_node(&tree.borrow(), this_id, &other_tree.borrow(), other_id);
        Ok(JsValue::from(result))
    }

    /// Native implementation of node.isSameNode(other)
    fn is_same_node(this: &JsValue, args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
        let (this_id, tree) = extract_node_id(this)
            .ok_or_else(|| JsError::from_opaque(js_string!("isSameNode: `this` is not a Node").into()))?;

        let other_val = match args.first() {
            Some(v) if !v.is_null() && !v.is_undefined() => v,
            _ => return Ok(JsValue::from(false)),
        };
        let (other_id, other_tree) = match extract_node_id(other_val) {
            Some(ids) => ids,
            None => return Ok(JsValue::from(false)),
        };

        // Same node requires same tree AND same id
        let same = Rc::ptr_eq(&tree, &other_tree) && this_id == other_id;
        Ok(JsValue::from(same))
    }

    /// Native implementation of node.compareDocumentPosition(other)
    fn compare_document_position(this: &JsValue, args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
        let (this_id, tree) = extract_node_id(this)
            .ok_or_else(|| JsError::from_opaque(js_string!("compareDocumentPosition: `this` is not a Node").into()))?;

        let other_val = args
            .first()
            .ok_or_else(|| JsError::from_opaque(js_string!("compareDocumentPosition: missing argument").into()))?;
        let (other_id, other_tree) = extract_node_id(other_val)
            .ok_or_else(|| JsError::from_opaque(js_string!("compareDocumentPosition: argument is not a Node").into()))?;

        // If nodes are in different trees, they're disconnected
        if !Rc::ptr_eq(&tree, &other_tree) {
            let dir = if (Rc::as_ptr(&other_tree) as usize) < (Rc::as_ptr(&tree) as usize) { 0x02u16 } else { 0x04u16 };
            return Ok(JsValue::from((0x01 | 0x20 | dir) as i32));
        }

        let result = tree.borrow().compare_document_position(this_id, other_id);
        Ok(JsValue::from(result as i32))
    }

    /// Native implementation of element.contains(other)
    fn contains(this: &JsValue, args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
        let (this_id, tree) = extract_node_id(this)
            .ok_or_else(|| JsError::from_opaque(js_string!("contains: `this` is not a Node").into()))?;

        let other_val = match args.first() {
            Some(v) if !v.is_null() && !v.is_undefined() => v,
            _ => return Ok(JsValue::from(false)),
        };
        let (other_id, other_tree) = match extract_node_id(other_val) {
            Some(ids) => ids,
            None => return Ok(JsValue::from(false)),
        };

        // If other is from a different tree, it can't be contained
        if !Rc::ptr_eq(&tree, &other_tree) {
            return Ok(JsValue::from(false));
        }

        // Walk up from other to see if we reach this
        let tree_ref = tree.borrow();
        let mut current = other_id;
        loop {
            if current == this_id {
                return Ok(JsValue::from(true));
            }
            match tree_ref.get_node(current).parent {
                Some(parent_id) => current = parent_id,
                None => return Ok(JsValue::from(false)),
            }
        }
    }

    /// Parse the third argument to addEventListener/removeEventListener.
    /// Returns (capture, once). `once` only matters for addEventListener.
    fn parse_listener_options(args: &[JsValue], ctx: &mut Context) -> JsResult<(bool, bool)> {
        let mut capture = false;
        let mut once = false;

        if let Some(opt_val) = args.get(2) {
            if let Some(opt_obj) = opt_val.as_object() {
                let c = opt_obj.get(js_string!("capture"), ctx)?;
                if !c.is_undefined() {
                    capture = c.to_boolean();
                }
                let o = opt_obj.get(js_string!("once"), ctx)?;
                if !o.is_undefined() {
                    once = o.to_boolean();
                }
            } else {
                // Coerce non-object values to boolean (handles numbers, strings, null, undefined, etc.)
                capture = opt_val.to_boolean();
            }
        }

        Ok((capture, once))
    }

    /// Native implementation of element.addEventListener(type, callback, options?)
    fn add_event_listener(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
        let this_obj = this
            .as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("addEventListener: `this` is not an object").into()))?;
        let el = this_obj
            .downcast_ref::<JsElement>()
            .ok_or_else(|| JsError::from_opaque(js_string!("addEventListener: `this` is not an Element").into()))?;
        let node_id = el.node_id;

        // First arg: event type string
        let event_type = args
            .first()
            .ok_or_else(|| JsError::from_opaque(js_string!("addEventListener: missing type argument").into()))?
            .to_string(ctx)?
            .to_std_string_escaped();

        // Parse options BEFORE checking for null callback (spec: options getters must be invoked)
        let (capture, once) = Self::parse_listener_options(args, ctx)?;

        // Second arg: callback (must be callable)
        let callback_val = args
            .get(1)
            .ok_or_else(|| JsError::from_opaque(js_string!("addEventListener: missing callback argument").into()))?;

        // If callback is null or undefined, silently return (per spec)
        if callback_val.is_null() || callback_val.is_undefined() {
            return Ok(JsValue::undefined());
        }

        let callback = callback_val
            .as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("addEventListener: callback is not an object").into()))?
            .clone();

        EVENT_LISTENERS.with(|el| {
            let rc = el.borrow();
            let listeners_rc = rc.as_ref().expect("EVENT_LISTENERS not initialized");
            let mut map = listeners_rc.borrow_mut();
            let entries = map.entry(node_id).or_insert_with(Vec::new);

            // Check for duplicates: same event_type + same callback object (by pointer) + same capture
            let duplicate = entries.iter().any(|entry| {
                entry.event_type == event_type
                    && entry.capture == capture
                    && entry.callback == callback
            });

            if !duplicate {
                entries.push(ListenerEntry {
                    event_type,
                    callback,
                    capture,
                    once,
                    passive: None,
                });
            }
        });

        Ok(JsValue::undefined())
    }

    /// Native implementation of element.removeEventListener(type, callback, options?)
    fn remove_event_listener(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
        let this_obj = this
            .as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("removeEventListener: `this` is not an object").into()))?;
        let el = this_obj
            .downcast_ref::<JsElement>()
            .ok_or_else(|| JsError::from_opaque(js_string!("removeEventListener: `this` is not an Element").into()))?;
        let node_id = el.node_id;

        // First arg: event type string
        let event_type = args
            .first()
            .ok_or_else(|| JsError::from_opaque(js_string!("removeEventListener: missing type argument").into()))?
            .to_string(ctx)?
            .to_std_string_escaped();

        // Parse options BEFORE checking for null callback (spec: options getters must be invoked)
        let (capture, _once) = Self::parse_listener_options(args, ctx)?;

        // Second arg: callback
        let callback_val = args
            .get(1)
            .ok_or_else(|| JsError::from_opaque(js_string!("removeEventListener: missing callback argument").into()))?;

        // If callback is null or undefined, silently return
        if callback_val.is_null() || callback_val.is_undefined() {
            return Ok(JsValue::undefined());
        }

        let callback = callback_val
            .as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("removeEventListener: callback is not an object").into()))?
            .clone();

        EVENT_LISTENERS.with(|el| {
            let rc = el.borrow();
            let listeners_rc = rc.as_ref().expect("EVENT_LISTENERS not initialized");
            let mut map = listeners_rc.borrow_mut();
            if let Some(entries) = map.get_mut(&node_id) {
                entries.retain(|entry| {
                    !(entry.event_type == event_type
                        && entry.capture == capture
                        && entry.callback == callback)
                });
                // Clean up empty vec
                if entries.is_empty() {
                    map.remove(&node_id);
                }
            }
        });

        Ok(JsValue::undefined())
    }

    /// Native implementation of element.dispatchEvent(event)
    ///
    /// Implements the W3C event dispatch algorithm:
    /// 1. Build propagation path from target up to root
    /// 2. Capture phase (root -> parent of target)
    /// 3. At-target phase (target itself)
    /// 4. Bubble phase (parent of target -> root), only if event.bubbles
    fn dispatch_event(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
        let this_obj = this
            .as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("dispatchEvent: `this` is not an object").into()))?;
        let el = this_obj
            .downcast_ref::<JsElement>()
            .ok_or_else(|| JsError::from_opaque(js_string!("dispatchEvent: `this` is not an Element").into()))?;
        let target_node_id = el.node_id;
        let tree = el.tree.clone();

        let event_val = args
            .first()
            .ok_or_else(|| {
                JsError::from_native(
                    boa_engine::JsNativeError::typ()
                        .with_message("Failed to execute 'dispatchEvent' on 'EventTarget': 1 argument required, but only 0 present.")
                )
            })?
            .clone();

        // null/undefined arg -> TypeError
        if event_val.is_null() || event_val.is_undefined() {
            return Err(JsError::from_native(
                boa_engine::JsNativeError::typ()
                    .with_message("Failed to execute 'dispatchEvent' on 'EventTarget': parameter 1 is not of type 'Event'.")
            ));
        }

        let event_obj = event_val
            .as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("dispatchEvent: argument is not an object").into()))?
            .clone();

        // Read event_type and bubbles from the event's native data (JsEvent or JsCustomEvent)
        let is_custom_event;
        let (event_type, bubbles) = if let Some(evt) = event_obj.downcast_ref::<JsEvent>() {
            is_custom_event = false;
            // Check initialized flag
            if !evt.initialized {
                return Err(JsError::from_opaque(
                    js_string!("InvalidStateError: The event is not initialized.").into(),
                ));
            }
            // Check dispatching flag
            if evt.dispatching {
                return Err(JsError::from_opaque(
                    js_string!("InvalidStateError: The event is already being dispatched.").into(),
                ));
            }
            (evt.event_type.clone(), evt.bubbles)
        } else if let Some(evt) = event_obj.downcast_ref::<super::event::JsCustomEvent>() {
            is_custom_event = true;
            if !evt.initialized {
                return Err(JsError::from_opaque(
                    js_string!("InvalidStateError: The event is not initialized.").into(),
                ));
            }
            if evt.dispatching {
                return Err(JsError::from_opaque(
                    js_string!("InvalidStateError: The event is already being dispatched.").into(),
                ));
            }
            (evt.event_type.clone(), evt.bubbles)
        } else {
            return Err(JsError::from_opaque(js_string!("dispatchEvent: argument is not an Event").into()));
        };

        // Check cancelBubble (propagation_stopped) — if already set, dispatch is a no-op
        let already_stopped = if is_custom_event {
            event_obj.downcast_ref::<super::event::JsCustomEvent>().unwrap().propagation_stopped
        } else {
            event_obj.downcast_ref::<JsEvent>().unwrap().propagation_stopped
        };
        if already_stopped {
            return Self::finish_dispatch_generic(&event_obj, is_custom_event, ctx);
        }

        // 1. Build propagation path: [root, ..., grandparent, parent, target]
        let propagation_path = {
            let tree_ref = tree.borrow();
            let mut path = vec![target_node_id];
            let mut current = target_node_id;
            while let Some(parent_id) = tree_ref.get_node(current).parent {
                path.push(parent_id);
                current = parent_id;
            }
            path.reverse();
            path
        };

        // Check if window should be in the propagation path.
        // Window is added when the root of the path is the Document node.
        let include_window = {
            let tree_ref = tree.borrow();
            if let Some(&root_id) = propagation_path.first() {
                matches!(tree_ref.get_node(root_id).data, NodeData::Document)
            } else {
                false
            }
        };

        // 2. Set event.target and dispatching flag
        if is_custom_event {
            let mut evt = event_obj.downcast_mut::<super::event::JsCustomEvent>().unwrap();
            evt.target = Some(target_node_id);
            evt.dispatching = true;
        } else {
            let mut evt = event_obj.downcast_mut::<JsEvent>().unwrap();
            evt.target = Some(target_node_id);
            evt.dispatching = true;
        }

        // Set the JS-level target and srcElement properties
        Self::set_event_prop(&event_obj, "target", this.clone(), ctx)?;
        Self::set_event_prop(&event_obj, "srcElement", this.clone(), ctx)?;

        // Helper: create a JsElement JS object for a given NodeId (uses cache)
        let make_js_element = |node_id: NodeId, ctx: &mut Context| -> JsResult<JsObject> {
            get_or_create_js_element(node_id, tree.clone(), ctx)
        };

        // Helper macro-like closure: set phase/current_target on either event type
        let set_phase = |event_obj: &JsObject, node_id: Option<NodeId>, phase: u8, is_custom: bool| {
            if is_custom {
                let mut evt = event_obj.downcast_mut::<super::event::JsCustomEvent>().unwrap();
                evt.current_target = node_id;
                evt.phase = phase;
            } else {
                let mut evt = event_obj.downcast_mut::<JsEvent>().unwrap();
                evt.current_target = node_id;
                evt.phase = phase;
            }
        };

        // 3. Capture phase (phase = 1): Walk from window (if included) -> root down to (but NOT including) the target

        // Window capture listeners first (if applicable)
        if include_window {
            // Set phase on native event data
            if is_custom_event {
                let mut evt = event_obj.downcast_mut::<super::event::JsCustomEvent>().unwrap();
                evt.current_target = Some(WINDOW_LISTENER_ID);
                evt.phase = 1;
            } else {
                let mut evt = event_obj.downcast_mut::<JsEvent>().unwrap();
                evt.current_target = Some(WINDOW_LISTENER_ID);
                evt.phase = 1;
            }
            let window_val: JsValue = WINDOW_OBJECT.with(|cell: &std::cell::RefCell<Option<JsObject>>| {
                cell.borrow().as_ref().map(|w| JsValue::from(w.clone()))
            }).unwrap_or(JsValue::undefined());
            Self::set_event_prop(&event_obj, "currentTarget", window_val, ctx)?;

            let should_stop = invoke_listeners_for_node(
                WINDOW_LISTENER_ID, &event_type, &event_obj, &event_val, true, false, ctx,
            )?;
            if should_stop {
                return Self::finish_dispatch_generic(&event_obj, is_custom_event, ctx);
            }
        }

        let target_index = propagation_path.len() - 1;
        for &node_id in &propagation_path[..target_index] {

            set_phase(&event_obj, Some(node_id), 1, is_custom_event);
            let current_target_js = make_js_element(node_id, ctx)?;
            Self::set_event_prop(&event_obj, "currentTarget", JsValue::from(current_target_js), ctx)?;

            let should_stop = Self::invoke_listeners_for_node(
                node_id, &event_type, &event_obj, &event_val, true, false, ctx,
            )?;
            if should_stop {
                return Self::finish_dispatch_generic(&event_obj, is_custom_event, ctx);
            }
        }

        // 4. At-target phase (phase = 2): capture listeners first, then non-capture
        set_phase(&event_obj, Some(target_node_id), 2, is_custom_event);
        Self::set_event_prop(&event_obj, "currentTarget", this.clone(), ctx)?;

        // First: capture listeners at target
        let should_stop = Self::invoke_listeners_for_node(
            target_node_id, &event_type, &event_obj, &event_val, true, false, ctx,
        )?;
        if should_stop {
            return Self::finish_dispatch_generic(&event_obj, is_custom_event, ctx);
        }

        // Second: non-capture listeners at target
        set_phase(&event_obj, Some(target_node_id), 2, is_custom_event);
        let should_stop = Self::invoke_listeners_for_node(
            target_node_id, &event_type, &event_obj, &event_val, false, false, ctx,
        )?;
        if should_stop {
            return Self::finish_dispatch_generic(&event_obj, is_custom_event, ctx);
        }

        // 5. Bubble phase (phase = 3): Only if event.bubbles. Walk from parent up to root, then window.
        if bubbles {
            for i in (0..target_index).rev() {
                let node_id = propagation_path[i];

                set_phase(&event_obj, Some(node_id), 3, is_custom_event);
                let current_target_js = make_js_element(node_id, ctx)?;
                Self::set_event_prop(&event_obj, "currentTarget", JsValue::from(current_target_js), ctx)?;

                let should_stop = Self::invoke_listeners_for_node(
                    node_id, &event_type, &event_obj, &event_val, false, false, ctx,
                )?;
                if should_stop {
                    return Self::finish_dispatch_generic(&event_obj, is_custom_event, ctx);
                }
            }

            // Window bubble listeners last (if applicable)
            if include_window {
                if is_custom_event {
                    let mut evt = event_obj.downcast_mut::<super::event::JsCustomEvent>().unwrap();
                    evt.current_target = Some(WINDOW_LISTENER_ID);
                    evt.phase = 3;
                } else {
                    let mut evt = event_obj.downcast_mut::<JsEvent>().unwrap();
                    evt.current_target = Some(WINDOW_LISTENER_ID);
                    evt.phase = 3;
                }
                let window_val: JsValue = WINDOW_OBJECT.with(|cell: &std::cell::RefCell<Option<JsObject>>| {
                    cell.borrow().as_ref().map(|w| JsValue::from(w.clone()))
                }).unwrap_or(JsValue::undefined());
                Self::set_event_prop(&event_obj, "currentTarget", window_val, ctx)?;

                let should_stop = invoke_listeners_for_node(
                    WINDOW_LISTENER_ID, &event_type, &event_obj, &event_val, false, false, ctx,
                )?;
                if should_stop {
                    return Self::finish_dispatch_generic(&event_obj, is_custom_event, ctx);
                }
            }
        }

        Self::finish_dispatch_generic(&event_obj, is_custom_event, ctx)
    }

    /// Delegates to the standalone `invoke_listeners_for_node` function.
    fn invoke_listeners_for_node(
        node_id: NodeId,
        event_type: &str,
        event_obj: &JsObject,
        event_val: &JsValue,
        capture_only: bool,
        at_target: bool,
        ctx: &mut Context,
    ) -> JsResult<bool> {
        invoke_listeners_for_node(node_id, event_type, event_obj, event_val, capture_only, at_target, ctx)
    }

    /// Set an own data property on the event object, overriding any prototype accessor.
    fn set_event_prop(event_obj: &JsObject, name: &str, value: JsValue, ctx: &mut Context) -> JsResult<()> {
        event_obj.define_property_or_throw(
            js_string!(name),
            PropertyDescriptor::builder()
                .value(value)
                .writable(true)
                .configurable(true)
                .enumerable(true)
                .build(),
            ctx,
        )?;
        Ok(())
    }

    /// Reset event phase, currentTarget, propagation flags, dispatching after dispatch.
    fn finish_dispatch_generic(event_obj: &JsObject, is_custom: bool, ctx: &mut Context) -> JsResult<JsValue> {
        let default_prevented = if is_custom {
            let mut evt = event_obj.downcast_mut::<super::event::JsCustomEvent>().unwrap();
            evt.phase = 0;
            evt.current_target = None;
            evt.propagation_stopped = false;
            evt.immediate_propagation_stopped = false;
            evt.dispatching = false;
            evt.default_prevented
        } else {
            let mut evt = event_obj.downcast_mut::<JsEvent>().unwrap();
            evt.phase = 0;
            evt.current_target = None;
            evt.propagation_stopped = false;
            evt.immediate_propagation_stopped = false;
            evt.dispatching = false;
            evt.default_prevented
        };
        Self::set_event_prop(event_obj, "currentTarget", JsValue::null(), ctx)?;
        Ok(JsValue::from(!default_prevented))
    }
}

/// Invoke matching listeners for a specific node during event dispatch.
///
/// - `capture_only`: if true, only invoke listeners with capture=true (capture phase)
/// - `at_target`: if true, invoke ALL matching listeners regardless of capture flag
///
/// For the bubble phase, call with capture_only=false, at_target=false,
/// which invokes only listeners with capture=false.
///
/// Returns true if propagation was stopped and dispatch should halt.
pub(crate) fn invoke_listeners_for_node(
    node_id: NodeId,
    event_type: &str,
    event_obj: &JsObject,
    event_val: &JsValue,
    capture_only: bool,
    at_target: bool,
    ctx: &mut Context,
) -> JsResult<bool> {
    // Collect matching listeners (snapshot to avoid borrow issues during callback invocation)
    let matching: Vec<(JsObject, bool)> = EVENT_LISTENERS.with(|el| {
        let rc = el.borrow();
        let listeners_rc = rc.as_ref().expect("EVENT_LISTENERS not initialized");
        let map = listeners_rc.borrow();
        match map.get(&node_id) {
            Some(entries) => entries
                .iter()
                .filter(|entry| {
                    if entry.event_type != event_type {
                        return false;
                    }
                    if at_target {
                        true
                    } else if capture_only {
                        entry.capture
                    } else {
                        !entry.capture
                    }
                })
                .map(|entry| (entry.callback.clone(), entry.once))
                .collect(),
            None => Vec::new(),
        }
    });

    for (callback, once) in &matching {
        if *once {
            EVENT_LISTENERS.with(|el| {
                let rc = el.borrow();
                let listeners_rc = rc.as_ref().expect("EVENT_LISTENERS not initialized");
                let mut map = listeners_rc.borrow_mut();
                if let Some(entries) = map.get_mut(&node_id) {
                    entries.retain(|entry| {
                        !(entry.event_type == event_type && entry.callback == *callback && entry.once)
                    });
                    if entries.is_empty() {
                        map.remove(&node_id);
                    }
                }
            });
        }

        // Per spec: if callback is callable, call with this=currentTarget.
        // If callback is an object with handleEvent method, look it up fresh and call with this=object.
        if callback.is_callable() {
            // Get currentTarget from the event to use as `this`
            let current_target = event_obj
                .get(js_string!("currentTarget"), ctx)
                .unwrap_or(JsValue::undefined());
            let _ = callback.call(&current_target, std::slice::from_ref(event_val), ctx);
        } else {
            // handleEvent protocol: look up handleEvent on the object each time
            let handle = callback.get(js_string!("handleEvent"), ctx)?;
            if let Some(handle_fn) = handle.as_object().filter(|o| o.is_callable()) {
                let _ = handle_fn.call(&JsValue::from(callback.clone()), std::slice::from_ref(event_val), ctx);
            }
        }

        let (imm_stopped, prop_stopped) = if let Some(evt) = event_obj.downcast_ref::<JsEvent>() {
            (evt.immediate_propagation_stopped, evt.propagation_stopped)
        } else if let Some(evt) = event_obj.downcast_ref::<super::event::JsCustomEvent>() {
            (evt.immediate_propagation_stopped, evt.propagation_stopped)
        } else {
            (false, false)
        };

        if imm_stopped {
            return Ok(true);
        }
        // prop_stopped: don't return yet -- continue processing listeners on this node
        let _ = prop_stopped;
    }

    let propagation_stopped = if let Some(evt) = event_obj.downcast_ref::<JsEvent>() {
        evt.propagation_stopped
    } else if let Some(evt) = event_obj.downcast_ref::<super::event::JsCustomEvent>() {
        evt.propagation_stopped
    } else {
        false
    };
    Ok(propagation_stopped)
}

impl Class for JsElement {
    const NAME: &'static str = "Element";
    const LENGTH: usize = 0;

    fn data_constructor(
        _new_target: &JsValue,
        _args: &[JsValue],
        _context: &mut Context,
    ) -> JsResult<Self> {
        Err(JsError::from_opaque(
            js_string!("Element cannot be constructed directly from JS").into(),
        ))
    }

    fn init(class: &mut ClassBuilder) -> JsResult<()> {
        // appendChild method
        class.method(
            js_string!("appendChild"),
            1,
            NativeFunction::from_fn_ptr(Self::append_child),
        );

        // textContent getter/setter
        let realm = class.context().realm().clone();

        let getter = NativeFunction::from_fn_ptr(Self::get_text_content);
        let setter = NativeFunction::from_fn_ptr(Self::set_text_content);

        class.accessor(
            js_string!("textContent"),
            Some(getter.to_js_function(&realm)),
            Some(setter.to_js_function(&realm)),
            Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
        );

        // classList getter/setter
        let class_list_getter = NativeFunction::from_fn_ptr(Self::get_class_list);
        let class_list_setter = NativeFunction::from_fn_ptr(Self::set_class_list);
        class.accessor(
            js_string!("classList"),
            Some(class_list_getter.to_js_function(&realm)),
            Some(class_list_setter.to_js_function(&realm)),
            Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
        );

        // Register traversal properties (parentNode, firstChild, etc.)
        super::traversal::register_traversal(class)?;

        // Register attribute methods (getAttribute, setAttribute, etc.)
        super::attributes::register_attributes(class)?;

        // Register node info properties (nodeType, nodeName, tagName, etc.)
        super::node_info::register_node_info(class)?;

        // Register innerHTML, outerHTML, insertAdjacentHTML
        super::inner_html::register_inner_html(class)?;

        // Register mutation methods (insertBefore, replaceChild, removeChild, cloneNode)
        super::mutation::register_mutation(class)?;

        // Register style accessor
        super::style::register_style(class)?;

        // Register query methods (querySelector, querySelectorAll, etc.)
        super::query::register_query(class)?;

        // Register input properties (value, checked, type, disabled, name, placeholder)
        super::input_props::register_input_props(class)?;

        // Register select/option properties (select.value, selectedIndex, options, option.selected/text)
        super::select_props::register_select_props(class)?;

        // Register anchor/form/dataset properties (href, action, method, elements, hidden, dataset)
        super::anchor_form::register_anchor_form(class)?;

        // Register common HTMLElement properties (tabIndex, title, lang, dir, getBoundingClientRect, focus, blur, click)
        super::html_element::register_html_element(class)?;

        // Register CharacterData properties and methods (data, length, appendData, etc.)
        super::character_data::register_character_data(class)?;

        // remove() method
        class.method(
            js_string!("remove"),
            0,
            NativeFunction::from_fn_ptr(Self::remove),
        );

        // contains() method
        class.method(
            js_string!("contains"),
            1,
            NativeFunction::from_fn_ptr(Self::contains),
        );

        // isEqualNode / isSameNode / compareDocumentPosition
        class.method(
            js_string!("isEqualNode"),
            1,
            NativeFunction::from_fn_ptr(Self::is_equal_node),
        );
        class.method(
            js_string!("isSameNode"),
            1,
            NativeFunction::from_fn_ptr(Self::is_same_node),
        );
        class.method(
            js_string!("compareDocumentPosition"),
            1,
            NativeFunction::from_fn_ptr(Self::compare_document_position),
        );

        // addEventListener / removeEventListener / dispatchEvent
        class.method(
            js_string!("addEventListener"),
            2,
            NativeFunction::from_fn_ptr(Self::add_event_listener),
        );
        class.method(
            js_string!("removeEventListener"),
            2,
            NativeFunction::from_fn_ptr(Self::remove_event_listener),
        );
        class.method(
            js_string!("dispatchEvent"),
            1,
            NativeFunction::from_fn_ptr(Self::dispatch_event),
        );

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Standalone node comparison functions (for use on document object too)
// ---------------------------------------------------------------------------

pub(crate) fn node_is_equal_node(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    JsElement::is_equal_node(this, args, ctx)
}

pub(crate) fn node_is_same_node(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    JsElement::is_same_node(this, args, ctx)
}

pub(crate) fn node_compare_document_position(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    JsElement::compare_document_position(this, args, ctx)
}

pub(crate) fn node_contains(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    JsElement::contains(this, args, ctx)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use crate::Engine;
    use crate::js::bindings::event_target::EVENT_LISTENERS;

    /// Helper to count total listeners across all elements.
    fn listener_count() -> usize {
        EVENT_LISTENERS.with(|el| {
            let rc = el.borrow();
            let listeners_rc = rc.as_ref().expect("EVENT_LISTENERS not initialized");
            let map = listeners_rc.borrow();
            map.values().map(|v| v.len()).sum::<usize>()
        })
    }

    #[test]
    fn add_event_listener_basic() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><button id='btn'>Click</button></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        // Should not throw
        runtime
            .eval("document.getElementById('btn').addEventListener('click', function() {})")
            .unwrap();
    }

    #[test]
    fn remove_event_listener_basic() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><button id='btn'>Click</button></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        runtime
            .eval(
                r#"
                var handler = function() {};
                var btn = document.getElementById('btn');
                btn.addEventListener('click', handler);
                btn.removeEventListener('click', handler);
            "#,
            )
            .unwrap();

        // Listener map should be empty after removal
        assert_eq!(listener_count(), 0);
    }

    #[test]
    fn add_event_listener_with_capture_bool() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><div id='d'></div></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        runtime
            .eval("document.getElementById('d').addEventListener('click', function() {}, true)")
            .unwrap();

        assert_eq!(listener_count(), 1);

        // Verify the capture flag is true
        EVENT_LISTENERS.with(|el| {
            let rc = el.borrow();
            let listeners_rc = rc.as_ref().unwrap();
            let map = listeners_rc.borrow();
            let entries = map.values().next().unwrap();
            assert!(entries[0].capture);
            assert!(!entries[0].once);
        });
    }

    #[test]
    fn add_event_listener_with_options_object() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><div id='d'></div></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        runtime
            .eval("document.getElementById('d').addEventListener('click', function() {}, { capture: true, once: true })")
            .unwrap();

        EVENT_LISTENERS.with(|el| {
            let rc = el.borrow();
            let listeners_rc = rc.as_ref().unwrap();
            let map = listeners_rc.borrow();
            let entries = map.values().next().unwrap();
            assert!(entries[0].capture);
            assert!(entries[0].once);
        });
    }

    #[test]
    fn listener_count_increases() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><div id='d'></div></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        runtime
            .eval(
                r#"
                var d = document.getElementById('d');
                d.addEventListener('click', function() { console.log('click') });
                d.addEventListener('mouseover', function() { console.log('hover') });
            "#,
            )
            .unwrap();

        assert_eq!(listener_count(), 2);
    }

    #[test]
    fn no_duplicate_listeners() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><div id='d'></div></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        runtime
            .eval(
                r#"
                var d = document.getElementById('d');
                var handler = function() {};
                d.addEventListener('click', handler);
                d.addEventListener('click', handler);
                d.addEventListener('click', handler);
            "#,
            )
            .unwrap();

        // Same callback + same type + same capture should only be stored once
        assert_eq!(listener_count(), 1);
    }

    #[test]
    fn same_callback_different_capture_not_duplicate() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><div id='d'></div></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        runtime
            .eval(
                r#"
                var d = document.getElementById('d');
                var handler = function() {};
                d.addEventListener('click', handler, false);
                d.addEventListener('click', handler, true);
            "#,
            )
            .unwrap();

        // Different capture flag means they are distinct listeners
        assert_eq!(listener_count(), 2);
    }

    #[test]
    fn remove_only_matching_listener() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><div id='d'></div></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        runtime
            .eval(
                r#"
                var d = document.getElementById('d');
                var h1 = function() {};
                var h2 = function() {};
                d.addEventListener('click', h1);
                d.addEventListener('click', h2);
                d.removeEventListener('click', h1);
            "#,
            )
            .unwrap();

        // Only h2 should remain
        assert_eq!(listener_count(), 1);
    }

    #[test]
    fn remove_nonexistent_listener_is_noop() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><div id='d'></div></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        runtime
            .eval(
                r#"
                var d = document.getElementById('d');
                var h1 = function() {};
                var h2 = function() {};
                d.addEventListener('click', h1);
                d.removeEventListener('click', h2);
            "#,
            )
            .unwrap();

        // h1 should still be there, h2 was never added
        assert_eq!(listener_count(), 1);
    }

    #[test]
    fn remove_with_capture_must_match() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><div id='d'></div></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        runtime
            .eval(
                r#"
                var d = document.getElementById('d');
                var handler = function() {};
                d.addEventListener('click', handler, true);
                d.removeEventListener('click', handler, false);
            "#,
            )
            .unwrap();

        // Capture flag doesn't match, so the listener should NOT be removed
        assert_eq!(listener_count(), 1);

        // Now remove with matching capture
        runtime
            .eval(
                r#"
                var d = document.getElementById('d');
                d.removeEventListener('click', handler, true);
            "#,
            )
            .unwrap();

        assert_eq!(listener_count(), 0);
    }

    #[test]
    fn listeners_on_multiple_elements() {
        let mut engine = Engine::new();
        engine.load_html(
            "<html><body><div id='a'></div><div id='b'></div></body></html>",
        );
        let runtime = engine.runtime.as_mut().unwrap();
        runtime
            .eval(
                r#"
                document.getElementById('a').addEventListener('click', function() {});
                document.getElementById('b').addEventListener('click', function() {});
            "#,
            )
            .unwrap();

        // Two different elements, each with one listener
        EVENT_LISTENERS.with(|el| {
            let rc = el.borrow();
            let listeners_rc = rc.as_ref().unwrap();
            let map = listeners_rc.borrow();
            assert_eq!(map.len(), 2);
            let total: usize = map.values().map(|v| v.len()).sum();
            assert_eq!(total, 2);
        });
    }

    #[test]
    fn add_event_listener_null_callback_is_noop() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><div id='d'></div></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        // null callback should not throw
        runtime
            .eval("document.getElementById('d').addEventListener('click', null)")
            .unwrap();

        assert_eq!(listener_count(), 0);
    }

    #[test]
    fn remove_event_listener_null_callback_is_noop() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><div id='d'></div></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        runtime
            .eval(
                r#"
                var d = document.getElementById('d');
                d.addEventListener('click', function() {});
                d.removeEventListener('click', null);
            "#,
            )
            .unwrap();

        // The listener should still be there
        assert_eq!(listener_count(), 1);
    }

    #[test]
    fn add_event_listener_default_options() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><div id='d'></div></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        runtime
            .eval("document.getElementById('d').addEventListener('click', function() {})")
            .unwrap();

        EVENT_LISTENERS.with(|el| {
            let rc = el.borrow();
            let listeners_rc = rc.as_ref().unwrap();
            let map = listeners_rc.borrow();
            let entries = map.values().next().unwrap();
            assert!(!entries[0].capture);
            assert!(!entries[0].once);
        });
    }

    #[test]
    fn event_type_stored_correctly() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><div id='d'></div></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        runtime
            .eval(
                r#"
                var d = document.getElementById('d');
                d.addEventListener('mousedown', function() {});
                d.addEventListener('mouseup', function() {});
                d.addEventListener('keypress', function() {});
            "#,
            )
            .unwrap();

        EVENT_LISTENERS.with(|el| {
            let rc = el.borrow();
            let listeners_rc = rc.as_ref().unwrap();
            let map = listeners_rc.borrow();
            let entries = map.values().next().unwrap();
            let types: Vec<&str> = entries.iter().map(|e| e.event_type.as_str()).collect();
            assert!(types.contains(&"mousedown"));
            assert!(types.contains(&"mouseup"));
            assert!(types.contains(&"keypress"));
        });
    }

    // ---- dispatchEvent tests ----

    #[test]
    fn dispatch_event_fires_listener() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><button id='btn'>Click</button></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        runtime.eval(r#"
            var result = '';
            var btn = document.getElementById('btn');
            btn.addEventListener('click', function(e) { result = 'fired:' + e.type; });
            btn.dispatchEvent(new Event('click'));
        "#).unwrap();
        let result = runtime.eval("result").unwrap();
        let s = result.to_string(&mut runtime.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "fired:click");
    }

    #[test]
    fn dispatch_event_bubbles_to_parent() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><div id='parent'><button id='btn'>Click</button></div></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        runtime.eval(r#"
            var log = [];
            document.getElementById('parent').addEventListener('click', function() { log.push('parent'); });
            document.getElementById('btn').addEventListener('click', function() { log.push('btn'); });
            document.getElementById('btn').dispatchEvent(new Event('click', { bubbles: true }));
        "#).unwrap();
        let result = runtime.eval("log.join(',')").unwrap();
        let s = result.to_string(&mut runtime.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "btn,parent");
    }

    #[test]
    fn dispatch_event_no_bubbles_stays_at_target() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><div id='parent'><button id='btn'>Click</button></div></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        runtime.eval(r#"
            var log = [];
            document.getElementById('parent').addEventListener('click', function() { log.push('parent'); });
            document.getElementById('btn').addEventListener('click', function() { log.push('btn'); });
            document.getElementById('btn').dispatchEvent(new Event('click', { bubbles: false }));
        "#).unwrap();
        let result = runtime.eval("log.join(',')").unwrap();
        let s = result.to_string(&mut runtime.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "btn");
    }

    #[test]
    fn dispatch_event_capture_phase() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><div id='outer'><div id='inner'><button id='btn'>Click</button></div></div></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        runtime.eval(r#"
            var log = [];
            document.getElementById('outer').addEventListener('click', function() { log.push('outer-capture'); }, true);
            document.getElementById('inner').addEventListener('click', function() { log.push('inner-capture'); }, true);
            document.getElementById('btn').addEventListener('click', function() { log.push('btn-target'); });
            document.getElementById('outer').addEventListener('click', function() { log.push('outer-bubble'); });
            document.getElementById('btn').dispatchEvent(new Event('click', { bubbles: true }));
        "#).unwrap();
        let result = runtime.eval("log.join(',')").unwrap();
        let s = result.to_string(&mut runtime.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "outer-capture,inner-capture,btn-target,outer-bubble");
    }

    #[test]
    fn dispatch_event_stop_propagation() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><div id='parent'><button id='btn'>Click</button></div></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        runtime.eval(r#"
            var log = [];
            document.getElementById('btn').addEventListener('click', function(e) { log.push('btn'); e.stopPropagation(); });
            document.getElementById('parent').addEventListener('click', function() { log.push('parent'); });
            document.getElementById('btn').dispatchEvent(new Event('click', { bubbles: true }));
        "#).unwrap();
        let result = runtime.eval("log.join(',')").unwrap();
        let s = result.to_string(&mut runtime.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "btn");
    }

    #[test]
    fn dispatch_event_stop_immediate_propagation() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><button id='btn'>Click</button></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        runtime.eval(r#"
            var log = [];
            var btn = document.getElementById('btn');
            btn.addEventListener('click', function(e) { log.push('first'); e.stopImmediatePropagation(); });
            btn.addEventListener('click', function() { log.push('second'); });
            btn.dispatchEvent(new Event('click'));
        "#).unwrap();
        let result = runtime.eval("log.join(',')").unwrap();
        let s = result.to_string(&mut runtime.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "first");
    }

    #[test]
    fn dispatch_event_once_removes_listener() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><button id='btn'>Click</button></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        runtime.eval(r#"
            var count = 0;
            var btn = document.getElementById('btn');
            btn.addEventListener('click', function() { count++; }, { once: true });
            btn.dispatchEvent(new Event('click'));
            btn.dispatchEvent(new Event('click'));
        "#).unwrap();
        let result = runtime.eval("count").unwrap();
        let n = result.to_number(&mut runtime.context).unwrap();
        assert_eq!(n, 1.0);
    }

    #[test]
    fn dispatch_event_returns_true_if_not_prevented() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><button id='btn'>Click</button></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        runtime.eval(r#"
            var btn = document.getElementById('btn');
            btn.addEventListener('click', function() {});
            var result = btn.dispatchEvent(new Event('click'));
        "#).unwrap();
        let result = runtime.eval("result").unwrap();
        assert_eq!(result.to_boolean(), true);
    }

    #[test]
    fn dispatch_event_returns_false_if_prevented() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><button id='btn'>Click</button></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        runtime.eval(r#"
            var btn = document.getElementById('btn');
            btn.addEventListener('click', function(e) { e.preventDefault(); });
            var result = btn.dispatchEvent(new Event('click', { cancelable: true }));
        "#).unwrap();
        let result = runtime.eval("result").unwrap();
        assert_eq!(result.to_boolean(), false);
    }

    #[test]
    fn dispatch_event_target_has_correct_tag() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><div id='parent'><button id='btn'>Click</button></div></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        runtime.eval(r#"
            var info = [];
            document.getElementById('parent').addEventListener('click', function(e) {
                info.push('target-tag:' + e.target.tagName);
                info.push('currentTarget-tag:' + e.currentTarget.tagName);
            });
            document.getElementById('btn').dispatchEvent(new Event('click', { bubbles: true }));
        "#).unwrap();
        let result = runtime.eval("info.join(',')").unwrap();
        let s = result.to_string(&mut runtime.context).unwrap().to_std_string_escaped();
        // tagName returns uppercase for HTML elements (per spec), but our impl may
        // return lowercase depending on the parser. Check case-insensitively.
        let s_lower = s.to_ascii_lowercase();
        assert!(s_lower.contains("target-tag:button"), "target should be button: {}", s);
        assert!(s_lower.contains("currenttarget-tag:div"), "currentTarget should be div: {}", s);
    }

    #[test]
    fn dispatch_event_stop_propagation_in_capture_phase() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><div id='outer'><div id='inner'><button id='btn'>Click</button></div></div></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        runtime.eval(r#"
            var log = [];
            document.getElementById('outer').addEventListener('click', function(e) {
                log.push('outer-capture');
                e.stopPropagation();
            }, true);
            document.getElementById('inner').addEventListener('click', function() { log.push('inner-capture'); }, true);
            document.getElementById('btn').addEventListener('click', function() { log.push('btn-target'); });
            document.getElementById('btn').dispatchEvent(new Event('click', { bubbles: true }));
        "#).unwrap();
        let result = runtime.eval("log.join(',')").unwrap();
        let s = result.to_string(&mut runtime.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "outer-capture");
    }

    #[test]
    fn dispatch_event_no_listeners_returns_true() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><button id='btn'>Click</button></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        runtime.eval(r#"
            var btn = document.getElementById('btn');
            var result = btn.dispatchEvent(new Event('click'));
        "#).unwrap();
        let result = runtime.eval("result").unwrap();
        assert_eq!(result.to_boolean(), true);
    }

    #[test]
    fn dispatch_event_at_target_fires_both_capture_and_bubble_listeners() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><button id='btn'>Click</button></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        runtime.eval(r#"
            var log = [];
            var btn = document.getElementById('btn');
            btn.addEventListener('click', function() { log.push('capture'); }, true);
            btn.addEventListener('click', function() { log.push('bubble'); }, false);
            btn.dispatchEvent(new Event('click'));
        "#).unwrap();
        let result = runtime.eval("log.join(',')").unwrap();
        let s = result.to_string(&mut runtime.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "capture,bubble");
    }

    #[test]
    fn dispatch_event_stop_propagation_still_fires_remaining_listeners_on_same_node() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><button id='btn'>Click</button></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        runtime.eval(r#"
            var log = [];
            var btn = document.getElementById('btn');
            btn.addEventListener('click', function(e) { log.push('first'); e.stopPropagation(); });
            btn.addEventListener('click', function() { log.push('second'); });
            btn.dispatchEvent(new Event('click'));
        "#).unwrap();
        let result = runtime.eval("log.join(',')").unwrap();
        let s = result.to_string(&mut runtime.context).unwrap().to_std_string_escaped();
        // stopPropagation stops at the next node, but remaining listeners on this node still fire
        assert_eq!(s, "first,second");
    }
}
