mod cache;
mod events;
mod node_ops;
mod setup;
mod shadow;

#[cfg(test)]
mod tests;

// Re-export everything that was previously public from element.rs
pub(crate) use cache::{ensure_iframe_content_doc, get_or_create_js_element, DomPrototypes, NodeCache};
pub(crate) use events::{
    invoke_listeners_for_node, is_passive_default_event, is_passive_default_target, report_listener_error,
};
pub(crate) use node_ops::{
    extract_node_id, node_compare_document_position, node_contains, node_is_default_namespace,
    node_is_equal_node, node_is_same_node, node_lookup_namespace_uri, node_lookup_prefix,
};

use std::cell::RefCell;
use std::rc::Rc;

use boa_engine::{
    class::{Class, ClassBuilder},
    js_string,
    native_function::NativeFunction,
    object::ObjectInitializer,
    property::{Attribute, PropertyDescriptor},
    Context, JsData, JsError, JsNativeError, JsObject, JsResult, JsSymbol, JsValue,
};
use boa_gc::{Finalize, Trace};

use crate::dom::{DomTree, NodeId};

use super::class_list::JsClassList;
use super::event::JsEvent;

use cache::get_or_create_js_element;
use node_ops::extract_node_id;

// ---------------------------------------------------------------------------
// JsElement -- the Class-based wrapper around a DomTree node
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
    fn append_child(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
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
            return Err(JsNativeError::typ()
                .with_message("appendChild: argument 1 is not a Node")
                .into());
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
            super::mutation::update_node_cache_after_adoption(&src_tree, src_id, &tree, adopted_id, &child_obj, ctx);
            adopted_id
        } else {
            child.node_id
        };

        // Pre-insertion validation (appendChild is insertBefore with null ref child)
        super::mutation::validate_pre_insert(&tree.borrow(), parent_id, child_id, None, None)?;

        // Capture pre-state for live range updates and MutationObserver
        let (added_ids, removal_info, prev_sib, next_sib) =
            super::mutation::capture_insert_state(&tree, parent_id, child_id, None);

        // Update live ranges for removal from old parent (before the move)
        super::mutation::fire_range_removal_for_move(ctx, &tree, &removal_info, child_id);

        // Perform the insertion (handles DocumentFragment children)
        super::mutation::do_insert(&tree, parent_id, child_id, None);

        // Update live ranges for insertion + queue MutationObserver records
        super::mutation::fire_insert_records(ctx, &tree, parent_id, &added_ids, removal_info, prev_sib, next_sib);

        // appendChild returns the appended child (or fragment)
        Ok(child_arg.clone())
    }

    /// Native getter for element.textContent
    /// Per spec: Document and Doctype return null, others return text.
    fn get_text_content(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
        extract_element!(el, this, "textContent getter");
        let tree = el.tree.borrow();
        let node = tree.get_node(el.node_id);
        // Per DOM spec: Document and Doctype nodes return null for textContent
        if matches!(
            node.data,
            crate::dom::NodeData::Document | crate::dom::NodeData::Doctype { .. }
        ) {
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
        extract_element!(el, this, "textContent setter");

        // Per spec: Document and Doctype nodes ignore textContent setter
        {
            let tree = el.tree.borrow();
            let node = tree.get_node(el.node_id);
            if matches!(
                node.data,
                crate::dom::NodeData::Document | crate::dom::NodeData::Doctype { .. }
            ) {
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
                if let crate::dom::NodeData::Attr { ref mut value, .. } =
                    el.tree.borrow_mut().get_node_mut(el.node_id).data
                {
                    *value = data;
                }
                return Ok(JsValue::undefined());
            }
            if matches!(
                node.data,
                crate::dom::NodeData::Text { .. }
                    | crate::dom::NodeData::Comment { .. }
                    | crate::dom::NodeData::ProcessingInstruction { .. }
                    | crate::dom::NodeData::CDATASection { .. }
            ) {
                drop(tree);
                // null converts to ""
                let data = if val.is_null() {
                    String::new()
                } else {
                    val.to_string(ctx)?.to_std_string_escaped()
                };
                super::mutation_observer::character_data_set_with_observer(ctx, &el.tree, el.node_id, &data);
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

        // Capture existing children for MutationObserver
        let removed_children: Vec<crate::dom::NodeId> = el.tree.borrow().get_node(el.node_id).children.clone();

        let mut tree = el.tree.borrow_mut();
        // Remove all children
        tree.clear_children(el.node_id);

        // If value is non-empty, create a single Text child
        let added_id = if let Some(text_str) = text {
            let text_id = tree.create_text(&text_str);
            tree.append_child(el.node_id, text_id);
            Some(text_id)
        } else {
            None
        };

        drop(tree);

        // Queue MutationObserver childList record
        let added_ids = added_id.map(|id| vec![id]).unwrap_or_default();
        if !removed_children.is_empty() || !added_ids.is_empty() {
            super::mutation_observer::queue_childlist_mutation(
                ctx,
                &el.tree,
                el.node_id,
                added_ids,
                removed_children,
                None,
                None,
            );
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
                    cached_obj.set(key, JsValue::from(js_string!(classes[i].clone())), false, ctx)?;
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

    /// Native setter for element.classList -- sets the class attribute via value
    fn set_class_list(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
        extract_element!(el, this, "classList setter");

        let value = args
            .first()
            .map(|v| v.to_string(ctx))
            .transpose()?
            .map(|s| s.to_std_string_escaped())
            .unwrap_or_default();

        el.tree.borrow_mut().set_attribute(el.node_id, "class", &value);
        Ok(JsValue::undefined())
    }

    /// Native implementation of element.remove()
    fn remove(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
        extract_element!(el, this, "remove");
        let node_id = el.node_id;
        let tree = el.tree.clone();

        // Capture parent and siblings for MutationObserver
        let parent_info = {
            let t = tree.borrow();
            if let Some(parent_id) = t.get_node(node_id).parent {
                let parent_children = &t.get_node(parent_id).children;
                let pos = parent_children.iter().position(|&c| c == node_id);
                let prev = pos.and_then(|p| if p > 0 { Some(parent_children[p - 1]) } else { None });
                let next = pos.and_then(|p| parent_children.get(p + 1).copied());
                Some((parent_id, prev, next))
            } else {
                None
            }
        };

        tree.borrow_mut().remove_from_parent(node_id);

        // Queue MutationObserver record
        if let Some((parent_id, prev_sib, next_sib)) = parent_info {
            super::mutation_observer::queue_childlist_mutation(
                ctx,
                &tree,
                parent_id,
                vec![],
                vec![node_id],
                prev_sib,
                next_sib,
            );
        }

        Ok(JsValue::undefined())
    }
}

impl Class for JsElement {
    const NAME: &'static str = "Element";
    const LENGTH: usize = 0;

    fn data_constructor(_new_target: &JsValue, _args: &[JsValue], _context: &mut Context) -> JsResult<Self> {
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
        class.method(js_string!("remove"), 0, NativeFunction::from_fn_ptr(Self::remove));

        // contains() method
        class.method(js_string!("contains"), 1, NativeFunction::from_fn_ptr(Self::contains));

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

        // lookupNamespaceURI / lookupPrefix / isDefaultNamespace
        class.method(
            js_string!("lookupNamespaceURI"),
            1,
            NativeFunction::from_fn_ptr(Self::lookup_namespace_uri),
        );
        class.method(
            js_string!("lookupPrefix"),
            1,
            NativeFunction::from_fn_ptr(Self::lookup_prefix),
        );
        class.method(
            js_string!("isDefaultNamespace"),
            1,
            NativeFunction::from_fn_ptr(Self::is_default_namespace),
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

        // attachShadow / shadowRoot
        class.method(
            js_string!("attachShadow"),
            1,
            NativeFunction::from_fn_ptr(Self::attach_shadow),
        );
        let shadow_root_getter = NativeFunction::from_fn_ptr(Self::get_shadow_root);
        class.accessor(
            js_string!("shadowRoot"),
            Some(shadow_root_getter.to_js_function(&realm)),
            None,
            Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
        );

        // Symbol.unscopables -- spec requires null-prototype object with these ChildNode/ParentNode methods
        let unscopables = ObjectInitializer::new(class.context())
            .property(
                js_string!("before"),
                JsValue::from(true),
                Attribute::WRITABLE | Attribute::ENUMERABLE | Attribute::CONFIGURABLE,
            )
            .property(
                js_string!("after"),
                JsValue::from(true),
                Attribute::WRITABLE | Attribute::ENUMERABLE | Attribute::CONFIGURABLE,
            )
            .property(
                js_string!("replaceWith"),
                JsValue::from(true),
                Attribute::WRITABLE | Attribute::ENUMERABLE | Attribute::CONFIGURABLE,
            )
            .property(
                js_string!("remove"),
                JsValue::from(true),
                Attribute::WRITABLE | Attribute::ENUMERABLE | Attribute::CONFIGURABLE,
            )
            .property(
                js_string!("prepend"),
                JsValue::from(true),
                Attribute::WRITABLE | Attribute::ENUMERABLE | Attribute::CONFIGURABLE,
            )
            .property(
                js_string!("append"),
                JsValue::from(true),
                Attribute::WRITABLE | Attribute::ENUMERABLE | Attribute::CONFIGURABLE,
            )
            .build();
        unscopables.set_prototype(None);
        class.property(
            JsSymbol::unscopables(),
            unscopables,
            Attribute::READONLY | Attribute::NON_ENUMERABLE | Attribute::CONFIGURABLE,
        );

        Ok(())
    }
}
