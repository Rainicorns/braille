//! Native DOM bindings connecting JS objects to the Rust DomTree.
//!
//! Architecture: native Rust functions accept simple types (u32 nodeIds, Strings).
//! JS wrapper code on prototypes calls these native functions.
//! A node cache (JS-side Map) ensures identity: same NodeId → same JS object.

use std::cell::RefCell;
use std::rc::Rc;

use rquickjs::{Ctx, Function};

use crate::dom::node::NodeData;
use crate::dom::tree::DomTree;
use crate::dom::NodeId;

use super::state::EngineState;

// Thread-local for DomTree access from native functions.
thread_local! {
    static TREE: RefCell<Option<Rc<RefCell<DomTree>>>> = const { RefCell::new(None) };
}

fn with_tree<F, R>(f: F) -> R
where
    F: FnOnce(&DomTree) -> R,
{
    TREE.with(|t| {
        let borrow = t.borrow();
        let tree_rc = borrow.as_ref().expect("DOM bridge tree not set");
        let tree = tree_rc.borrow();
        f(&tree)
    })
}

fn with_tree_mut<F, R>(f: F) -> R
where
    F: FnOnce(&mut DomTree) -> R,
{
    TREE.with(|t| {
        let borrow = t.borrow();
        let tree_rc = borrow.as_ref().expect("DOM bridge tree not set");
        let mut tree = tree_rc.borrow_mut();
        f(&mut tree)
    })
}

/// Install the DOM bridge. Must be called once during runtime initialization.
pub fn install(ctx: &Ctx<'_>, tree: Rc<RefCell<DomTree>>, _state: Rc<RefCell<EngineState>>) {
    TREE.with(|t| {
        *t.borrow_mut() = Some(tree);
    });

    register_native_functions(ctx);
    register_js_wrappers(ctx);
}

fn register_native_functions(ctx: &Ctx<'_>) {
    let g = ctx.globals();

    // getAttribute(nodeId, name) -> string | null (empty string = null)
    g.set("__n_getAttribute", Function::new(ctx.clone(), |node_id: u32, name: String| -> String {
        with_tree(|tree| {
            tree.get_attribute(node_id as NodeId, &name).unwrap_or_default()
        })
    }).unwrap()).unwrap();

    // hasAttribute(nodeId, name) -> bool
    g.set("__n_hasAttribute", Function::new(ctx.clone(), |node_id: u32, name: String| -> bool {
        with_tree(|tree| tree.has_attribute(node_id as NodeId, &name))
    }).unwrap()).unwrap();

    // setAttribute(nodeId, name, value)
    g.set("__n_setAttribute", Function::new(ctx.clone(), |node_id: u32, name: String, value: String| {
        with_tree_mut(|tree| tree.set_attribute(node_id as NodeId, &name, &value));
    }).unwrap()).unwrap();

    // removeAttribute(nodeId, name)
    g.set("__n_removeAttribute", Function::new(ctx.clone(), |node_id: u32, name: String| {
        with_tree_mut(|tree| { tree.remove_attribute(node_id as NodeId, &name); });
    }).unwrap()).unwrap();

    // getTextContent(nodeId) -> string
    g.set("__n_getTextContent", Function::new(ctx.clone(), |node_id: u32| -> String {
        with_tree(|tree| tree.get_text_content(node_id as NodeId))
    }).unwrap()).unwrap();

    // getTagName(nodeId) -> string (uppercase)
    g.set("__n_getTagName", Function::new(ctx.clone(), |node_id: u32| -> String {
        with_tree(|tree| {
            let node = tree.get_node(node_id as NodeId);
            match &node.data {
                NodeData::Element { tag_name, .. } => tag_name.to_uppercase(),
                _ => String::new(),
            }
        })
    }).unwrap()).unwrap();

    // getNodeType(nodeId) -> u32
    g.set("__n_getNodeType", Function::new(ctx.clone(), |node_id: u32| -> u32 {
        with_tree(|tree| {
            let node = tree.get_node(node_id as NodeId);
            match &node.data {
                NodeData::Element { .. } => 1,
                NodeData::Text { .. } => 3,
                NodeData::Comment { .. } => 8,
                NodeData::Document => 9,
                NodeData::DocumentFragment => 11,
                NodeData::Doctype { .. } => 10,
                _ => 1,
            }
        })
    }).unwrap()).unwrap();

    // getParent(nodeId) -> nodeId or -1
    g.set("__n_getParent", Function::new(ctx.clone(), |node_id: u32| -> i32 {
        with_tree(|tree| {
            tree.get_node(node_id as NodeId).parent.map(|p| p as i32).unwrap_or(-1)
        })
    }).unwrap()).unwrap();

    // getChildElementIds(nodeId) -> array of nodeIds (element children only)
    g.set("__n_getChildElementIds", Function::new(ctx.clone(), |node_id: u32| -> Vec<u32> {
        with_tree(|tree| {
            let node = tree.get_node(node_id as NodeId);
            node.children.iter()
                .filter(|&&cid| matches!(tree.get_node(cid).data, NodeData::Element { .. }))
                .map(|&cid| cid as u32)
                .collect()
        })
    }).unwrap()).unwrap();

    // getElementById(id) -> nodeId or -1
    g.set("__n_getElementById", Function::new(ctx.clone(), |id: String| -> i32 {
        with_tree(|tree| {
            tree.get_element_by_id(&id).map(|nid| nid as i32).unwrap_or(-1)
        })
    }).unwrap()).unwrap();

    // querySelector(rootNodeId, selector) -> nodeId or -1
    g.set("__n_querySelector", Function::new(ctx.clone(), |root_id: u32, selector: String| -> i32 {
        with_tree(|tree| {
            crate::css::matching::query_selector(tree, root_id as NodeId, &selector, None)
                .map(|nid| nid as i32)
                .unwrap_or(-1)
        })
    }).unwrap()).unwrap();

    // querySelectorAll(rootNodeId, selector) -> array of nodeIds
    g.set("__n_querySelectorAll", Function::new(ctx.clone(), |root_id: u32, selector: String| -> Vec<u32> {
        with_tree(|tree| {
            crate::css::matching::query_selector_all(tree, root_id as NodeId, &selector, None)
                .into_iter()
                .map(|nid| nid as u32)
                .collect()
        })
    }).unwrap()).unwrap();

    // hasAttrValue(nodeId, name) -> bool (has the attribute at all?)
    g.set("__n_hasAttrValue", Function::new(ctx.clone(), |node_id: u32, name: String| -> bool {
        with_tree(|tree| tree.get_attribute(node_id as NodeId, &name).is_some())
    }).unwrap()).unwrap();

    // createElement(tagName) -> nodeId
    g.set("__n_createElement", Function::new(ctx.clone(), |tag: String| -> u32 {
        with_tree_mut(|tree| {
            tree.create_element(&tag.to_lowercase()) as u32
        })
    }).unwrap()).unwrap();

    // createTextNode(text) -> nodeId
    g.set("__n_createTextNode", Function::new(ctx.clone(), |text: String| -> u32 {
        with_tree_mut(|tree| {
            tree.create_text(&text) as u32
        })
    }).unwrap()).unwrap();

    // appendChild(parentId, childId)
    g.set("__n_appendChild", Function::new(ctx.clone(), |parent_id: u32, child_id: u32| {
        with_tree_mut(|tree| {
            tree.append_child(parent_id as NodeId, child_id as NodeId);
        });
    }).unwrap()).unwrap();

    // removeChild(parentId, childId)
    g.set("__n_removeChild", Function::new(ctx.clone(), |parent_id: u32, child_id: u32| {
        with_tree_mut(|tree| {
            tree.remove_child(parent_id as NodeId, child_id as NodeId);
        });
    }).unwrap()).unwrap();

    // insertBefore(parentId, newChildId, refChildId) — refChildId -1 means append
    g.set("__n_insertBefore", Function::new(ctx.clone(), |parent_id: u32, new_child_id: u32, ref_child_id: i32| {
        with_tree_mut(|tree| {
            if ref_child_id < 0 {
                tree.append_child(parent_id as NodeId, new_child_id as NodeId);
            } else {
                // tree.insert_before(sibling, child) puts child before sibling
                tree.insert_before(ref_child_id as NodeId, new_child_id as NodeId);
            }
        });
    }).unwrap()).unwrap();

    // setTextContent(nodeId, text) — removes all children and sets text
    g.set("__n_setTextContent", Function::new(ctx.clone(), |node_id: u32, text: String| {
        with_tree_mut(|tree| {
            tree.set_text_content(node_id as NodeId, &text);
        });
    }).unwrap()).unwrap();

    // getBodyId() -> nodeId or -1
    g.set("__n_getBodyId", Function::new(ctx.clone(), || -> i32 {
        with_tree(|tree| {
            tree.body().map(|id| id as i32).unwrap_or(-1)
        })
    }).unwrap()).unwrap();

    // contains(ancestorId, descendantId) -> bool
    g.set("__n_contains", Function::new(ctx.clone(), |ancestor_id: u32, descendant_id: u32| -> bool {
        if ancestor_id == descendant_id {
            return true;
        }
        with_tree(|tree| {
            let mut current = Some(descendant_id as NodeId);
            while let Some(id) = current {
                if id == ancestor_id as NodeId {
                    return true;
                }
                current = tree.get_node(id).parent;
            }
            false
        })
    }).unwrap()).unwrap();

    // closest(nodeId, selector) -> nodeId or -1
    g.set("__n_closest", Function::new(ctx.clone(), |node_id: u32, selector: String| -> i32 {
        with_tree(|tree| {
            let mut current = Some(node_id as NodeId);
            while let Some(id) = current {
                if matches!(tree.get_node(id).data, NodeData::Element { .. })
                    && crate::css::matching::matches_selector_str(tree, id, &selector, None)
                {
                    return id as i32;
                }
                current = tree.get_node(id).parent;
            }
            -1
        })
    }).unwrap()).unwrap();

    // getDataAttribute(nodeId, camelCaseName) -> string or empty
    g.set("__n_getDataAttr", Function::new(ctx.clone(), |node_id: u32, name: String| -> String {
        // Convert camelCase to kebab-case: fooBar -> foo-bar
        let mut kebab = String::from("data-");
        for ch in name.chars() {
            if ch.is_uppercase() {
                kebab.push('-');
                kebab.push(ch.to_lowercase().next().unwrap_or(ch));
            } else {
                kebab.push(ch);
            }
        }
        with_tree(|tree| {
            tree.get_attribute(node_id as NodeId, &kebab).unwrap_or_default()
        })
    }).unwrap()).unwrap();

    // innerHTML setter: parse HTML fragment and replace children
    g.set("__n_setInnerHTML", Function::new(ctx.clone(), |parent_id: u32, html: String| {
        // Parse the fragment into a temporary tree, then move children over
        let fragment_tree = crate::html::parser::parse_html_fragment(&html, "div", "");
        with_tree_mut(|tree| {
            // Remove existing children
            let old_children: Vec<NodeId> = tree.get_node(parent_id as NodeId).children.clone();
            for child_id in old_children {
                tree.remove_child(parent_id as NodeId, child_id);
            }
            // Import nodes from fragment tree into our tree
            let frag = fragment_tree.borrow();
            let frag_doc = frag.document();
            let frag_children: Vec<NodeId> = frag.get_node(frag_doc).children.clone();
            for &frag_child_id in &frag_children {
                import_node_recursive(tree, &frag, frag_child_id, parent_id as NodeId);
            }
        });
    }).unwrap()).unwrap();

    // createComment(text) -> nodeId
    g.set("__n_createComment", Function::new(ctx.clone(), |text: String| -> u32 {
        with_tree_mut(|tree| {
            tree.create_comment(&text) as u32
        })
    }).unwrap()).unwrap();

    // getAllChildIds(nodeId) -> array of ALL child nodeIds (elements, text, comments)
    g.set("__n_getAllChildIds", Function::new(ctx.clone(), |node_id: u32| -> Vec<u32> {
        with_tree(|tree| {
            tree.get_node(node_id as NodeId).children.iter().map(|&c| c as u32).collect()
        })
    }).unwrap()).unwrap();

    // getFirstChild(nodeId) -> nodeId or -1
    g.set("__n_getFirstChild", Function::new(ctx.clone(), |node_id: u32| -> i32 {
        with_tree(|tree| {
            tree.get_node(node_id as NodeId).children.first().map(|&c| c as i32).unwrap_or(-1)
        })
    }).unwrap()).unwrap();

    // getLastChild(nodeId) -> nodeId or -1
    g.set("__n_getLastChild", Function::new(ctx.clone(), |node_id: u32| -> i32 {
        with_tree(|tree| {
            tree.get_node(node_id as NodeId).children.last().map(|&c| c as i32).unwrap_or(-1)
        })
    }).unwrap()).unwrap();

    // getNextSibling(nodeId) -> nodeId or -1
    g.set("__n_getNextSibling", Function::new(ctx.clone(), |node_id: u32| -> i32 {
        with_tree(|tree| {
            let node = tree.get_node(node_id as NodeId);
            if let Some(parent_id) = node.parent {
                let siblings = &tree.get_node(parent_id).children;
                if let Some(pos) = siblings.iter().position(|&c| c == node_id as NodeId) {
                    if pos + 1 < siblings.len() {
                        return siblings[pos + 1] as i32;
                    }
                }
            }
            -1
        })
    }).unwrap()).unwrap();

    // getPrevSibling(nodeId) -> nodeId or -1
    g.set("__n_getPrevSibling", Function::new(ctx.clone(), |node_id: u32| -> i32 {
        with_tree(|tree| {
            let node = tree.get_node(node_id as NodeId);
            if let Some(parent_id) = node.parent {
                let siblings = &tree.get_node(parent_id).children;
                if let Some(pos) = siblings.iter().position(|&c| c == node_id as NodeId) {
                    if pos > 0 {
                        return siblings[pos - 1] as i32;
                    }
                }
            }
            -1
        })
    }).unwrap()).unwrap();

    // getCharData(nodeId) -> string (text/comment node data)
    g.set("__n_getCharData", Function::new(ctx.clone(), |node_id: u32| -> String {
        with_tree(|tree| {
            tree.character_data_get(node_id as NodeId).unwrap_or_default()
        })
    }).unwrap()).unwrap();

    // setCharData(nodeId, data) — set text/comment node data
    g.set("__n_setCharData", Function::new(ctx.clone(), |node_id: u32, data: String| {
        with_tree_mut(|tree| {
            tree.character_data_set(node_id as NodeId, &data);
        });
    }).unwrap()).unwrap();

    // cloneNode(nodeId, deep) -> new nodeId
    g.set("__n_cloneNode", Function::new(ctx.clone(), |node_id: u32, deep: bool| -> u32 {
        with_tree_mut(|tree| {
            tree.clone_node(node_id as NodeId, deep) as u32
        })
    }).unwrap()).unwrap();

    // replaceChild(parentId, newChildId, oldChildId)
    g.set("__n_replaceChild", Function::new(ctx.clone(), |parent_id: u32, new_id: u32, old_id: u32| {
        with_tree_mut(|tree| {
            tree.replace_child(parent_id as NodeId, new_id as NodeId, old_id as NodeId);
        });
    }).unwrap()).unwrap();

    // createDocFragment() -> nodeId
    g.set("__n_createDocFragment", Function::new(ctx.clone(), || -> u32 {
        with_tree_mut(|tree| {
            tree.create_document_fragment() as u32
        })
    }).unwrap()).unwrap();

    // getDoctypeInfo() -> JSON with name, publicId, systemId, nodeId or empty
    g.set("__n_getDoctypeInfo", Function::new(ctx.clone(), || -> String {
        with_tree(|tree| {
            let doc = tree.document();
            for &child_id in &tree.get_node(doc).children {
                if let NodeData::Doctype { name, public_id, system_id } = &tree.get_node(child_id).data {
                    return serde_json::json!({
                        "name": name,
                        "publicId": public_id,
                        "systemId": system_id,
                        "nodeId": child_id
                    }).to_string();
                }
            }
            String::new()
        })
    }).unwrap()).unwrap();

    // getInnerHTML(nodeId) -> string
    g.set("__n_getInnerHTML", Function::new(ctx.clone(), |node_id: u32| -> String {
        with_tree(|tree| {
            tree.serialize_children_html(node_id as NodeId)
        })
    }).unwrap()).unwrap();

    // matchesSelector(nodeId, selector) -> bool
    g.set("__n_matchesSelector", Function::new(ctx.clone(), |node_id: u32, selector: String| -> bool {
        with_tree(|tree| {
            crate::css::matching::matches_selector_str(tree, node_id as NodeId, &selector, None)
        })
    }).unwrap()).unwrap();

    // getNodeValue(nodeId) -> string (for text/comment) or empty string (for elements)
    // JS side will convert empty to null for elements
    g.set("__n_getNodeValue", Function::new(ctx.clone(), |node_id: u32| -> String {
        with_tree(|tree| {
            let node = tree.get_node(node_id as NodeId);
            match &node.data {
                NodeData::Text { content } | NodeData::Comment { content } | NodeData::CDATASection { content } => content.clone(),
                NodeData::ProcessingInstruction { data, .. } => data.clone(),
                _ => String::new(),
            }
        })
    }).unwrap()).unwrap();

    // __n_getAttributeNames(nodeId) -> JSON array of attribute names
    g.set("__n_getAttributeNames", Function::new(ctx.clone(), |node_id: u32| -> String {
        with_tree(|tree| {
            let names = tree.attribute_names(node_id as NodeId);
            serde_json::to_string(&names).unwrap_or_else(|_| "[]".to_string())
        })
    }).unwrap()).unwrap();

    // __n_cssSupports(declaration) -> bool — check if a CSS declaration parses
    g.set("__n_cssSupports", Function::new(ctx.clone(), |decl: String| -> bool {
        !crate::css::parser::parse_inline_style(&decl).is_empty()
    }).unwrap()).unwrap();

    // __n_getComputedStyle(nodeId, prop) -> string value or empty
    g.set("__n_getComputedStyle", Function::new(ctx.clone(), |node_id: u32, prop: String| -> String {
        with_tree(|tree| {
            let node = tree.get_node(node_id as NodeId);
            node.computed_style.as_ref()
                .and_then(|cs| cs.get(&prop))
                .cloned()
                .unwrap_or_default()
        })
    }).unwrap()).unwrap();

    // __n_getComputedStyleAll(nodeId) -> JSON string of all computed styles
    g.set("__n_getComputedStyleAll", Function::new(ctx.clone(), |node_id: u32| -> String {
        with_tree(|tree| {
            let node = tree.get_node(node_id as NodeId);
            match &node.computed_style {
                Some(cs) => serde_json::to_string(cs).unwrap_or_else(|_| "{}".to_string()),
                None => "{}".to_string(),
            }
        })
    }).unwrap()).unwrap();

    // __n_findLabelControl(labelNodeId) -> nodeId or -1
    // For <label>: if `for` attribute is set, find element by ID; otherwise find first labelable descendant.
    g.set("__n_findLabelControl", Function::new(ctx.clone(), |label_id: u32| -> i32 {
        with_tree(|tree| {
            let node = tree.get_node(label_id as NodeId);
            // Only meaningful for <label> elements
            if let NodeData::Element { tag_name, .. } = &node.data {
                if !tag_name.eq_ignore_ascii_case("label") {
                    return -1;
                }
            } else {
                return -1;
            }

            // Check `for` attribute
            if let Some(for_id) = tree.get_attribute(label_id as NodeId, "for") {
                if !for_id.is_empty() {
                    return tree.get_element_by_id(&for_id).map(|nid| nid as i32).unwrap_or(-1);
                }
            }

            // Find first labelable descendant
            fn find_first_labelable(tree: &DomTree, node_id: NodeId) -> Option<NodeId> {
                let node = tree.get_node(node_id);
                for &child_id in &node.children {
                    let child = tree.get_node(child_id);
                    if let NodeData::Element { tag_name, .. } = &child.data {
                        let tag = tag_name.to_lowercase();
                        if matches!(tag.as_str(), "input" | "select" | "textarea" | "button") {
                            return Some(child_id);
                        }
                    }
                    if let Some(found) = find_first_labelable(tree, child_id) {
                        return Some(found);
                    }
                }
                None
            }

            find_first_labelable(tree, label_id as NodeId).map(|nid| nid as i32).unwrap_or(-1)
        })
    }).unwrap()).unwrap();

    // __n_findLabelsForControl(controlNodeId) -> array of label nodeIds
    // Returns labels whose `for` matches this element's id, or labels that are ancestors of this element.
    g.set("__n_findLabelsForControl", Function::new(ctx.clone(), |control_id: u32| -> Vec<u32> {
        with_tree(|tree| {
            let node = tree.get_node(control_id as NodeId);
            // Only meaningful for labelable elements
            let is_labelable = if let NodeData::Element { tag_name, .. } = &node.data {
                let tag = tag_name.to_lowercase();
                matches!(tag.as_str(), "input" | "select" | "textarea" | "button")
            } else {
                false
            };
            if !is_labelable {
                return Vec::new();
            }

            let mut labels = Vec::new();

            // Get the control's id attribute
            let control_id_attr = tree.get_attribute(control_id as NodeId, "id");

            // Walk all nodes to find <label> elements
            fn collect_labels(
                tree: &DomTree,
                node_id: NodeId,
                control_id: NodeId,
                control_id_attr: &Option<String>,
                labels: &mut Vec<u32>,
            ) {
                let node = tree.get_node(node_id);
                if let NodeData::Element { tag_name, .. } = &node.data {
                    if tag_name.eq_ignore_ascii_case("label") {
                        // Check `for` attribute matches control's id
                        if let Some(for_id) = tree.get_attribute(node_id, "for") {
                            if let Some(ref cid) = control_id_attr {
                                if !for_id.is_empty() && &for_id == cid {
                                    labels.push(node_id as u32);
                                    // Don't return early — still need to search children for other labels
                                }
                            }
                        } else {
                            // No `for` attribute: check if control is a descendant of this label
                            fn is_descendant(tree: &DomTree, ancestor: NodeId, target: NodeId) -> bool {
                                let node = tree.get_node(ancestor);
                                for &child_id in &node.children {
                                    if child_id == target {
                                        return true;
                                    }
                                    if is_descendant(tree, child_id, target) {
                                        return true;
                                    }
                                }
                                false
                            }
                            if is_descendant(tree, node_id, control_id) {
                                labels.push(node_id as u32);
                            }
                        }
                    }
                }
                let children: Vec<NodeId> = tree.get_node(node_id).children.clone();
                for child_id in children {
                    collect_labels(tree, child_id, control_id, control_id_attr, labels);
                }
            }

            collect_labels(tree, tree.document(), control_id as NodeId, &control_id_attr, &mut labels);
            labels
        })
    }).unwrap()).unwrap();
}

fn register_js_wrappers(ctx: &Ctx<'_>) {
    // This JS code sets up:
    // 1. __braille_node_cache: Map<nodeId, elementWrapper> for identity
    // 2. __braille_get_element_wrapper(nodeId): creates/retrieves element wrapper
    // 3. Element prototype with methods calling native helpers
    // 4. document.getElementById etc. overrides
    // 5. Event listener storage and dispatch
    // 6. __braille_click(nodeId) for Rust click_element calls
    ctx.eval::<(), _>(r#"
    (function() {
        var _cache = {};
        var _listeners = {};      // key: nodeId + ":" + eventType -> array of {cb, capture}
        var _captureKeys = {};    // key: nodeId + ":" + eventType -> array of capture callbacks
        var _bubbleKeys = {};     // key: nodeId + ":" + eventType -> array of bubble callbacks
        var _winListeners = {};   // window bubble listeners
        var _winCapture = {};     // window capture listeners
        var _docCapture = {};     // document capture listeners

        // Element prototype
        var EP = {};
        EP.getAttribute = function(name) {
            var v = __n_getAttribute(this.__nid, name);
            return __n_hasAttrValue(this.__nid, name) ? v : null;
        };
        EP.setAttribute = function(name, value) {
            var old = __n_hasAttrValue(this.__nid, name) ? __n_getAttribute(this.__nid, name) : null;
            __n_setAttribute(this.__nid, name, String(value));
            if (typeof __mo_notify === 'function') __mo_notify('attributes', this, {attributeName: name, oldValue: old});
        };
        EP.removeAttribute = function(name) {
            var old = __n_hasAttrValue(this.__nid, name) ? __n_getAttribute(this.__nid, name) : null;
            __n_removeAttribute(this.__nid, name);
            if (typeof __mo_notify === 'function') __mo_notify('attributes', this, {attributeName: name, oldValue: old});
        };
        EP.hasAttribute = function(name) { return __n_hasAttribute(this.__nid, name); };

        EP.addEventListener = function(type, cb, opts) {
            if (typeof cb !== 'function') return;
            var capture = !!(opts === true || (opts && opts.capture));
            var once = !!(opts && typeof opts === 'object' && opts.once);
            var key = this.__nid + ':' + type;
            var store = capture ? _captureKeys : _bubbleKeys;
            if (!store[key]) store[key] = [];
            if (once) {
                var el = this;
                var wrapper = function(e) { cb.call(el, e); el.removeEventListener(type, wrapper, capture); };
                wrapper._origCb = cb;
                store[key].push(wrapper);
            } else {
                store[key].push(cb);
            }
        };
        EP.removeEventListener = function(type, cb, opts) {
            var capture = !!(opts === true || (opts && opts.capture));
            var key = this.__nid + ':' + type;
            var store = capture ? _captureKeys : _bubbleKeys;
            if (store[key]) {
                store[key] = store[key].filter(function(f) { return f !== cb && f._origCb !== cb; });
            }
        };
        EP.dispatchEvent = function(event) {
            __dispatch(this.__nid, event);
            return !event.defaultPrevented;
        };
        // Pointer capture
        var __pointerCaptures = {};
        EP.setPointerCapture = function(pointerId) { __pointerCaptures[pointerId] = this.__nid; };
        EP.releasePointerCapture = function(pointerId) { if (__pointerCaptures[pointerId] === this.__nid) delete __pointerCaptures[pointerId]; };
        EP.hasPointerCapture = function(pointerId) { return __pointerCaptures[pointerId] === this.__nid; };

        EP.click = function() {
            var event = new MouseEvent('click', {bubbles: true, cancelable: true});
            event.target = this;
            event.currentTarget = this;
            __dispatch(this.__nid, event);

            // <details>/<summary> toggle
            if (this.tagName === 'SUMMARY') {
                var details = this.parentNode;
                if (details && details.tagName === 'DETAILS') {
                    if (details.hasAttribute('open')) details.removeAttribute('open');
                    else details.setAttribute('open', '');
                    details.dispatchEvent(new Event('toggle', {bubbles: false}));
                }
            }

            // Deliver React onClick/onSubmit via __reactProps$.
            // Our capture/bubble dispatch fires React's native listeners but
            // React's internal event processing may not complete in headless mode.
            var node = this;
            while (node && node.__nid !== undefined) {
                var pk = Object.keys(node).find(function(k) { return k.indexOf('__reactProps$') === 0; });
                if (pk && node[pk]) {
                    var synth = {
                        type: 'click', target: this, currentTarget: node,
                        bubbles: true, cancelable: true, defaultPrevented: false,
                        preventDefault: function() { this.defaultPrevented = true; },
                        stopPropagation: function() { this._stopped = true; },
                        nativeEvent: event, persist: function() {},
                    };
                    if (typeof node[pk].onClick === 'function') {
                        node[pk].onClick(synth);
                        if (synth._stopped) break;
                    }
                    if (node.tagName === 'FORM' && typeof node[pk].onSubmit === 'function') {
                        var s2 = {
                            type: 'submit', target: this, currentTarget: node,
                            bubbles: true, cancelable: true, defaultPrevented: false,
                            preventDefault: function() { this.defaultPrevented = true; },
                            stopPropagation: function() {}, persist: function() {},
                        };
                        node[pk].onSubmit(s2);
                    }
                }
                node = node.parentNode;
            }

            // Implicit form submission: <button type="submit"> or <input type="submit"> inside a <form>
            if (!event.defaultPrevented) {
                var tag = this.tagName;
                var btype = (this.getAttribute('type') || '').toLowerCase();
                if ((tag === 'BUTTON' && (btype === 'submit' || btype === '')) || (tag === 'INPUT' && btype === 'submit')) {
                    var form = this.form;
                    if (form) {
                        var submitEvt = new Event('submit', {bubbles: true, cancelable: true});
                        submitEvt.submitter = this;
                        form.dispatchEvent(submitEvt);
                    }
                }
            }

            // Label activation: clicking a label focuses/clicks its associated control
            if (!event.defaultPrevented && this.tagName === 'LABEL') {
                var controlId = __n_findLabelControl(this.__nid);
                if (controlId >= 0) {
                    var ctrl = __w(controlId);
                    if (ctrl && ctrl.__nid !== this.__nid) {
                        if (typeof ctrl.focus === 'function') ctrl.focus();
                        ctrl.click();
                    }
                }
            }
        };
        // <dialog> element APIs
        EP.showModal = function() {
            if (this.tagName === 'DIALOG') { this.setAttribute('open', ''); if (!this.__props) this.__props = {}; this.__props._dialogModal = true; }
        };
        EP.show = function() {
            if (this.tagName === 'DIALOG') this.setAttribute('open', '');
        };
        EP.close = function(returnValue) {
            if (this.tagName === 'DIALOG') {
                this.removeAttribute('open');
                if (!this.__props) this.__props = {};
                if (returnValue !== undefined) this.__props._returnValue = String(returnValue);
                this.dispatchEvent(new Event('close', {bubbles: false}));
            }
        };

        EP.querySelector = function(sel) {
            var id = __n_querySelector(this.__nid, sel);
            return id >= 0 ? __w(id) : null;
        };
        EP.querySelectorAll = function(sel) {
            return __n_querySelectorAll(this.__nid, sel).map(__w);
        };
        EP.getElementsByTagName = function(tag) {
            var self = this;
            return new Proxy([], {
                get: function(t, p) {
                    var live = self.querySelectorAll(tag);
                    if (p === 'length') return live.length;
                    if (p === 'item') return function(i) { return live[i] || null; };
                    if (p === 'namedItem') return function(name) {
                        for (var i = 0; i < live.length; i++) {
                            if (live[i].getAttribute('name') === name || live[i].getAttribute('id') === name) return live[i];
                        }
                        return null;
                    };
                    if (p === Symbol.iterator) return function() { return live[Symbol.iterator](); };
                    if (typeof p === 'string' && !isNaN(p)) return live[parseInt(p)];
                    if (p === 'forEach') return function(cb) { for (var i = 0; i < live.length; i++) cb(live[i], i); };
                    return live[p];
                }
            });
        };
        EP.getElementsByClassName = function(cls) {
            var self = this;
            return new Proxy([], {
                get: function(t, p) {
                    var live = self.querySelectorAll('.' + cls);
                    if (p === 'length') return live.length;
                    if (p === 'item') return function(i) { return live[i] || null; };
                    if (p === Symbol.iterator) return function() { return live[Symbol.iterator](); };
                    if (typeof p === 'string' && !isNaN(p)) return live[parseInt(p)];
                    if (p === 'forEach') return function(cb) { for (var i = 0; i < live.length; i++) cb(live[i], i); };
                    return live[p];
                }
            });
        };
        EP.contains = function(other) {
            if (!other || other.__nid === undefined) return false;
            return __n_contains(this.__nid, other.__nid);
        };
        EP.insertBefore = function(n, ref_) { return n; };
        EP.appendChild = function(child) { return child; };
        EP.removeChild = function(child) { return child; };
        EP.cloneNode = function(deep) {
            var nid = __n_cloneNode(this.__nid, !!deep);
            return __w(nid);
        };
        EP.replaceChild = function(newChild, oldChild) {
            if (newChild && newChild.__nid !== undefined && oldChild && oldChild.__nid !== undefined) {
                if (newChild.nodeType === 11) {
                    // DocumentFragment: insert all fragment children before oldChild, then remove oldChild
                    var kids = __n_getAllChildIds(newChild.__nid);
                    for (var i = 0; i < kids.length; i++) {
                        __n_insertBefore(this.__nid, kids[i], oldChild.__nid);
                    }
                    __n_removeChild(this.__nid, oldChild.__nid);
                } else {
                    __n_replaceChild(this.__nid, newChild.__nid, oldChild.__nid);
                }
            }
            return oldChild;
        };
        EP.hasChildNodes = function() { return __n_getFirstChild(this.__nid) >= 0; };
        EP.getBoundingClientRect = function() {
            // Return plausible non-zero defaults instead of all zeros
            var s = __n_getAttribute(this.__nid, 'style') || '';
            // display:none → all zeros
            if (/display\s*:\s*none/i.test(s)) return {top:0,left:0,width:0,height:0,right:0,bottom:0,x:0,y:0};
            // Also check computed style for display:none
            var compDisplay = __n_getComputedStyle(this.__nid, 'display');
            if (compDisplay === 'none') return {top:0,left:0,width:0,height:0,right:0,bottom:0,x:0,y:0};
            var w = 0, h = 0, found = false;
            // Try inline style first
            var wm = s.match(/(?:^|;)\s*width\s*:\s*(\d+)/);
            var hm = s.match(/(?:^|;)\s*height\s*:\s*(\d+)/);
            if (wm) { w = parseInt(wm[1]); found = true; }
            if (hm) { h = parseInt(hm[1]); found = true; }
            // Fall back to computed style if inline didn't have dimensions
            if (!wm) {
                var cw = __n_getComputedStyle(this.__nid, 'width');
                if (cw) { var pw = parseInt(cw); if (!isNaN(pw)) { w = pw; found = true; } }
            }
            if (!hm) {
                var ch = __n_getComputedStyle(this.__nid, 'height');
                if (ch) { var ph = parseInt(ch); if (!isNaN(ph)) { h = ph; found = true; } }
            }
            // If no explicit dimensions, use content-based defaults for visible elements
            if (!found) {
                var tag = this.tagName;
                if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT' || tag === 'BUTTON' || tag === 'IMG') { w = 100; h = 20; }
                else if (__n_getTextContent(this.__nid).trim()) { w = 100; h = 20; }
            }
            return {top:0,left:0,width:w,height:h,right:w,bottom:h,x:0,y:0};
        };
        EP.getClientRects = function() { return [this.getBoundingClientRect()]; };
        // focus/blur defined later after defineProperties to track activeElement
        EP.scrollIntoView = function() {};
        EP.matches = function(sel) { return __n_matchesSelector(this.__nid, sel); };
        EP.closest = function(sel) {
            var id = __n_closest(this.__nid, sel);
            return id >= 0 ? __w(id) : null;
        };
        EP.getAttributeNames = function() {
            return JSON.parse(__n_getAttributeNames(this.__nid));
        };
        EP.append = function() {
            for (var i = 0; i < arguments.length; i++) {
                var arg = arguments[i];
                if (typeof arg === 'string') arg = document.createTextNode(arg);
                this.appendChild(arg);
            }
        };
        EP.prepend = function() {
            var first = this.firstChild;
            for (var i = 0; i < arguments.length; i++) {
                var arg = arguments[i];
                if (typeof arg === 'string') arg = document.createTextNode(arg);
                if (first) this.insertBefore(arg, first);
                else this.appendChild(arg);
            }
        };
        EP.replaceChildren = function() {
            while (this.firstChild) this.removeChild(this.firstChild);
            for (var i = 0; i < arguments.length; i++) {
                var arg = arguments[i];
                if (typeof arg === 'string') arg = document.createTextNode(arg);
                this.appendChild(arg);
            }
        };
        EP.after = function() {
            var parent = this.parentNode;
            var next = this.nextSibling;
            if (!parent) return;
            for (var i = 0; i < arguments.length; i++) {
                var arg = arguments[i];
                if (typeof arg === 'string') arg = document.createTextNode(arg);
                if (next) parent.insertBefore(arg, next);
                else parent.appendChild(arg);
            }
        };
        EP.before = function() {
            var parent = this.parentNode;
            if (!parent) return;
            for (var i = 0; i < arguments.length; i++) {
                var arg = arguments[i];
                if (typeof arg === 'string') arg = document.createTextNode(arg);
                parent.insertBefore(arg, this);
            }
        };
        EP.replaceWith = function() {
            var parent = this.parentNode;
            if (!parent) return;
            var next = this.nextSibling;
            parent.removeChild(this);
            for (var i = 0; i < arguments.length; i++) {
                var arg = arguments[i];
                if (typeof arg === 'string') arg = document.createTextNode(arg);
                if (next) parent.insertBefore(arg, next);
                else parent.appendChild(arg);
            }
        };
        EP.toggleAttribute = function(name, force) {
            if (force !== undefined) {
                if (force) { this.setAttribute(name, ''); return true; }
                else { this.removeAttribute(name); return false; }
            }
            if (this.hasAttribute(name)) { this.removeAttribute(name); return false; }
            this.setAttribute(name, ''); return true;
        };
        EP.setAttributeNS = function(ns, name, value) { this.setAttribute(name, String(value)); };
        EP.getAttributeNS = function(ns, name) { return this.getAttribute(name); };
        EP.removeAttributeNS = function(ns, name) { this.removeAttribute(name); };
        EP.hasAttributeNS = function(ns, name) { return this.hasAttribute(name); };
        EP.insertAdjacentHTML = function(position, html) {
            var temp = document.createElement('div');
            __n_setInnerHTML(temp.__nid, html);
            var frag = document.createDocumentFragment();
            while (temp.firstChild) frag.appendChild(temp.firstChild);
            if (position === 'beforebegin') this.before(frag);
            else if (position === 'afterbegin') this.prepend(frag);
            else if (position === 'beforeend') this.append(frag);
            else if (position === 'afterend') this.after(frag);
        };
        EP.insertAdjacentElement = function(position, el) {
            if (position === 'beforebegin') this.before(el);
            else if (position === 'afterbegin') this.prepend(el);
            else if (position === 'beforeend') this.append(el);
            else if (position === 'afterend') this.after(el);
            return el;
        };
        EP.getAnimations = function() { return []; };
        EP.animate = function() {
            var anim = { finished: Promise.resolve(), cancel: function(){}, play: function(){}, pause: function(){}, onfinish: null };
            anim.finish = function() { if (typeof anim.onfinish === 'function') anim.onfinish(); };
            return anim;
        };
        EP.attachShadow = function() { return document.createDocumentFragment(); };
        EP.getAttributeNode = function(name) {
            if (!this.hasAttribute(name)) return null;
            return { name: name, value: this.getAttribute(name), specified: true };
        };
        EP.remove = function() {
            if (this.__nid !== undefined) {
                var pid = __n_getParent(this.__nid);
                if (pid >= 0) __n_removeChild(pid, this.__nid);
            }
        };
        EP.getRootNode = function() { return document; };
        EP.compareDocumentPosition = function(other) {
            if (!other || other.__nid === undefined || this.__nid === undefined) return 0;
            if (this.__nid === other.__nid) return 0;
            // Check if other is contained by this
            if (__n_contains(this.__nid, other.__nid)) return 16 | 4; // CONTAINED_BY | FOLLOWING
            // Check if this is contained by other
            if (__n_contains(other.__nid, this.__nid)) return 8 | 2; // CONTAINS | PRECEDING
            return 4; // FOLLOWING (simplified)
        };

        Object.defineProperties(EP, {
            textContent: {
                get: function() { return __n_getTextContent(this.__nid); },
                set: function(v) { __n_setTextContent(this.__nid, String(v)); },
                configurable: true
            },
            tagName: { get: function() { return __n_getTagName(this.__nid); }, configurable: true },
            nodeName: { get: function() {
                var nt = __n_getNodeType(this.__nid);
                if (nt === 3) return '#text';
                if (nt === 8) return '#comment';
                if (nt === 9) return '#document';
                if (nt === 11) return '#document-fragment';
                return __n_getTagName(this.__nid) || '#node';
            }, configurable: true },
            nodeType: { get: function() { return __n_getNodeType(this.__nid); }, configurable: true },
            id: {
                get: function() { return this.getAttribute('id') || ''; },
                set: function(v) { this.setAttribute('id', v); },
                configurable: true
            },
            className: {
                get: function() { return this.getAttribute('class') || ''; },
                set: function(v) { this.setAttribute('class', v); },
                configurable: true
            },
            value: {
                get: function() {
                    if (this.__props && this.__props._value !== undefined) return this.__props._value;
                    if (this.tagName === 'SELECT') {
                        var opts = this.querySelectorAll('option');
                        for (var i = 0; i < opts.length; i++) {
                            if ((opts[i].__props && opts[i].__props._selected) || opts[i].hasAttribute('selected')) {
                                return opts[i].getAttribute('value') || opts[i].textContent || '';
                            }
                        }
                        return opts.length > 0 ? (opts[0].getAttribute('value') || opts[0].textContent || '') : '';
                    }
                    return this.getAttribute('value') || '';
                },
                set: function(v) {
                    if (!this.__props) this.__props = {};
                    this.__props._value = String(v);
                    if (this.tagName === 'SELECT') {
                        var opts = this.querySelectorAll('option');
                        for (var i = 0; i < opts.length; i++) {
                            if (!opts[i].__props) opts[i].__props = {};
                            opts[i].__props._selected = ((opts[i].getAttribute('value') || opts[i].textContent || '') === String(v));
                        }
                    }
                    // Also sync to attribute so Rust-side snapshot can read the current value
                    __n_setAttribute(this.__nid, 'value', String(v));
                    // For textarea, also update text content so the snapshot can see it
                    if (this.tagName === 'TEXTAREA') __n_setTextContent(this.__nid, String(v));
                    // Fire input event (bubbles, not cancelable) per spec
                    this.dispatchEvent(new Event('input', {bubbles: true, cancelable: false}));
                },
                configurable: true
            },
            defaultValue: {
                get: function() { return this.getAttribute('value') || ''; },
                set: function(v) { this.setAttribute('value', String(v)); },
                configurable: true
            },
            checked: {
                get: function() {
                    if (this.__props && this.__props._checked !== undefined) return this.__props._checked;
                    return this.hasAttribute('checked');
                },
                set: function(v) { if (!this.__props) this.__props = {}; this.__props._checked = !!v; },
                configurable: true
            },
            defaultChecked: {
                get: function() { return this.hasAttribute('checked'); },
                set: function(v) { if(v) this.setAttribute('checked',''); else this.removeAttribute('checked'); },
                configurable: true
            },
            selected: {
                get: function() {
                    if (this.__props && this.__props._selected !== undefined) return this.__props._selected;
                    return this.hasAttribute('selected');
                },
                set: function(v) { if (!this.__props) this.__props = {}; this.__props._selected = !!v; },
                configurable: true
            },
            disabled: {
                get: function() { return this.hasAttribute('disabled'); },
                set: function(v) { if(v) this.setAttribute('disabled',''); else this.removeAttribute('disabled'); },
                configurable: true
            },
            noModule: {
                get: function() { return this.hasAttribute('nomodule'); },
                set: function(v) { if(v) this.setAttribute('nomodule',''); else this.removeAttribute('nomodule'); },
                configurable: true
            },
            async: {
                get: function() { return this.hasAttribute('async'); },
                set: function(v) { if(v) this.setAttribute('async',''); else this.removeAttribute('async'); },
                configurable: true
            },
            defer: {
                get: function() { return this.hasAttribute('defer'); },
                set: function(v) { if(v) this.setAttribute('defer',''); else this.removeAttribute('defer'); },
                configurable: true
            },
            reversed: {
                get: function() { return this.hasAttribute('reversed'); },
                set: function(v) { if(v) this.setAttribute('reversed',''); else this.removeAttribute('reversed'); },
                configurable: true
            },
            type: {
                get: function() {
                    var t = this.getAttribute('type');
                    // HTML spec: <input> without type defaults to 'text'
                    if (t === null && this.tagName === 'INPUT') return 'text';
                    return t || '';
                },
                set: function(v) { this.setAttribute('type', v); },
                configurable: true
            },
            href: {
                get: function() { return this.getAttribute('href') || ''; },
                set: function(v) { this.setAttribute('href', v); },
                configurable: true
            },
            src: {
                get: function() { return this.getAttribute('src') || ''; },
                set: function(v) { this.setAttribute('src', v); },
                configurable: true
            },
            innerHTML: {
                get: function() { return __n_getInnerHTML(this.__nid); },
                set: function(v) { __n_setInnerHTML(this.__nid, String(v)); },
                configurable: true
            },
            parentNode: {
                get: function() { var p = __n_getParent(this.__nid); return p >= 0 ? __w(p) : null; },
                configurable: true
            },
            parentElement: {
                get: function() { var p = __n_getParent(this.__nid); return p >= 0 ? __w(p) : null; },
                configurable: true
            },
            children: {
                get: function() { return __n_getChildElementIds(this.__nid).map(__w); },
                configurable: true
            },
            childNodes: {
                get: function() { return __n_getAllChildIds(this.__nid).map(__w); },
                configurable: true
            },
            firstChild: {
                get: function() { var id = __n_getFirstChild(this.__nid); return id >= 0 ? __w(id) : null; },
                configurable: true
            },
            lastChild: {
                get: function() { var id = __n_getLastChild(this.__nid); return id >= 0 ? __w(id) : null; },
                configurable: true
            },
            nextSibling: {
                get: function() { var id = __n_getNextSibling(this.__nid); return id >= 0 ? __w(id) : null; },
                configurable: true
            },
            previousSibling: {
                get: function() { var id = __n_getPrevSibling(this.__nid); return id >= 0 ? __w(id) : null; },
                configurable: true
            },
            nodeValue: {
                get: function() {
                    var nt = __n_getNodeType(this.__nid);
                    if (nt === 3 || nt === 8) return __n_getNodeValue(this.__nid);
                    return null;
                },
                set: function(v) {
                    var nt = __n_getNodeType(this.__nid);
                    if (nt === 3 || nt === 8) __n_setCharData(this.__nid, String(v));
                },
                configurable: true
            },
            data: {
                get: function() {
                    var nt = __n_getNodeType(this.__nid);
                    if (nt === 3 || nt === 8) return __n_getCharData(this.__nid);
                    return undefined;
                },
                set: function(v) {
                    var nt = __n_getNodeType(this.__nid);
                    if (nt === 3 || nt === 8) __n_setCharData(this.__nid, String(v));
                },
                configurable: true
            },
            style: {
                get: function() {
                    if (!this._s) {
                        var nid = this.__nid;
                        // helpers to parse / serialize the style attribute
                        function parseStyle() {
                            var s = __n_getAttribute(nid, 'style');
                            var arr = [];
                            if (!s) return arr;
                            var parts = s.split(';');
                            for (var i = 0; i < parts.length; i++) {
                                var p = parts[i].trim();
                                if (!p) continue;
                                var ci = p.indexOf(':');
                                if (ci < 0) continue;
                                arr.push([p.substring(0, ci).trim(), p.substring(ci + 1).trim()]);
                            }
                            return arr;
                        }
                        function serializeStyle(arr) {
                            return arr.map(function(e) { return e[0] + ': ' + e[1]; }).join('; ');
                        }
                        function writeStyle(arr) {
                            var s = serializeStyle(arr);
                            if (s) __n_setAttribute(nid, 'style', s);
                            else __n_removeAttribute(nid, 'style');
                        }
                        // camelCase <-> kebab-case
                        function toKebab(cc) {
                            if (cc === 'cssFloat') return 'float';
                            return cc.replace(/[A-Z]/g, function(c) { return '-' + c.toLowerCase(); });
                        }
                        var store = {
                            setProperty: function(prop, val) {
                                var arr = parseStyle();
                                var found = false;
                                for (var i = 0; i < arr.length; i++) {
                                    if (arr[i][0] === prop) { arr[i][1] = val; found = true; break; }
                                }
                                if (!found) arr.push([prop, val]);
                                writeStyle(arr);
                            },
                            removeProperty: function(prop) {
                                var arr = parseStyle();
                                var old = '';
                                for (var i = 0; i < arr.length; i++) {
                                    if (arr[i][0] === prop) { old = arr[i][1]; arr.splice(i, 1); break; }
                                }
                                writeStyle(arr);
                                return old;
                            },
                            getPropertyValue: function(prop) {
                                var arr = parseStyle();
                                for (var i = 0; i < arr.length; i++) {
                                    if (arr[i][0] === prop) return arr[i][1];
                                }
                                return '';
                            },
                            getPropertyPriority: function() { return ''; },
                        };
                        this._s = new Proxy(store, {
                            set: function(t, p, v) {
                                if (typeof p !== 'string') return true;
                                if (p === 'cssText') {
                                    if (v && String(v).trim()) __n_setAttribute(nid, 'style', String(v));
                                    else __n_removeAttribute(nid, 'style');
                                    return true;
                                }
                                // setting a camelCase or kebab prop writes to the DOM
                                var kebab = toKebab(p);
                                var arr = parseStyle();
                                if (v === '' || v === null || v === undefined) {
                                    // empty string removes property per spec
                                    for (var i = 0; i < arr.length; i++) {
                                        if (arr[i][0] === kebab) { arr.splice(i, 1); break; }
                                    }
                                } else {
                                    var found = false;
                                    for (var i = 0; i < arr.length; i++) {
                                        if (arr[i][0] === kebab) { arr[i][1] = String(v); found = true; break; }
                                    }
                                    if (!found) arr.push([kebab, String(v)]);
                                }
                                writeStyle(arr);
                                return true;
                            },
                            get: function(t, p) {
                                if (p in t) return t[p];
                                if (typeof p !== 'string') return undefined;
                                if (p === 'cssText') {
                                    return __n_getAttribute(nid, 'style') || '';
                                }
                                if (p === 'length') {
                                    return parseStyle().length;
                                }
                                if (p === 'item') {
                                    return function(idx) {
                                        var arr = parseStyle();
                                        return idx < arr.length ? arr[idx][0] : '';
                                    };
                                }
                                // camelCase property read
                                var kebab = toKebab(p);
                                var arr = parseStyle();
                                for (var i = 0; i < arr.length; i++) {
                                    if (arr[i][0] === kebab) return arr[i][1];
                                }
                                return '';
                            }
                        });
                    }
                    return this._s;
                },
                configurable: true
            },
            classList: {
                get: function() {
                    var el = this;
                    return {
                        add: function() { var c=(el.getAttribute('class')||'').split(/\s+/).filter(Boolean); for(var i=0;i<arguments.length;i++) if(c.indexOf(arguments[i])<0) c.push(arguments[i]); el.setAttribute('class',c.join(' ')); },
                        remove: function() { var c=(el.getAttribute('class')||'').split(/\s+/).filter(Boolean); for(var i=0;i<arguments.length;i++){var idx=c.indexOf(arguments[i]);if(idx>=0)c.splice(idx,1);} el.setAttribute('class',c.join(' ')); },
                        contains: function(cls) { return (el.getAttribute('class')||'').split(/\s+/).indexOf(cls)>=0; },
                        toggle: function(cls,force) { if(force!==undefined){if(force)this.add(cls);else this.remove(cls);return force;} if(this.contains(cls)){this.remove(cls);return false;} this.add(cls);return true; },
                        forEach: function(cb) { var c=(el.getAttribute('class')||'').split(/\s+/).filter(Boolean); for(var i=0;i<c.length;i++) cb(c[i],i,c); },
                        get length() { return (el.getAttribute('class')||'').split(/\s+/).filter(Boolean).length; },
                        item: function(i) { var c=(el.getAttribute('class')||'').split(/\s+/).filter(Boolean); return i<c.length?c[i]:null; },
                        toString: function() { return el.getAttribute('class')||''; },
                        get value() { return el.getAttribute('class')||''; },
                        set value(v) { el.setAttribute('class', v); },
                    };
                },
                configurable: true
            },
            dataset: {
                get: function() {
                    var el = this;
                    return new Proxy({}, {
                        get: function(t, prop) {
                            if (typeof prop !== 'string') return undefined;
                            return __n_getDataAttr(el.__nid, prop) || undefined;
                        },
                        set: function(t, prop, val) {
                            // Convert camelCase to data-kebab-case
                            var name = 'data-' + prop.replace(/[A-Z]/g, function(c){return '-'+c.toLowerCase();});
                            __n_setAttribute(el.__nid, name, String(val));
                            return true;
                        }
                    });
                },
                configurable: true
            },
            scrollTop: { get: function() { return 0; }, set: function(){}, configurable: true },
            scrollLeft: { get: function() { return 0; }, set: function(){}, configurable: true },
            scrollWidth: { get: function() { return this.getBoundingClientRect().width; }, configurable: true },
            scrollHeight: { get: function() { return this.getBoundingClientRect().height; }, configurable: true },
            offsetTop: { get: function() { return 0; }, configurable: true },
            offsetLeft: { get: function() { return 0; }, configurable: true },
            offsetWidth: { get: function() { return this.getBoundingClientRect().width; }, configurable: true },
            offsetHeight: { get: function() { return this.getBoundingClientRect().height; }, configurable: true },
            clientWidth: { get: function() { if (this.tagName === 'HTML') return 1280; return this.getBoundingClientRect().width; }, configurable: true },
            clientHeight: { get: function() { if (this.tagName === 'HTML') return 800; return this.getBoundingClientRect().height; }, configurable: true },
            clientTop: { get: function() { return 0; }, configurable: true },
            clientLeft: { get: function() { return 0; }, configurable: true },
            offsetParent: { get: function() { return this.parentNode; }, configurable: true },
            innerText: {
                get: function() {
                    // Walk tree, skipping hidden elements (display:none, visibility:hidden)
                    function walk(nid) {
                        var nt = __n_getNodeType(nid);
                        if (nt === 3) return __n_getCharData(nid);
                        if (nt !== 1) return '';
                        var disp = __n_getComputedStyle(nid, 'display');
                        if (disp === 'none') return '';
                        var vis = __n_getComputedStyle(nid, 'visibility');
                        if (vis === 'hidden') return '';
                        var kids = __n_getAllChildIds(nid);
                        var parts = [];
                        for (var i = 0; i < kids.length; i++) parts.push(walk(kids[i]));
                        return parts.join('');
                    }
                    return walk(this.__nid);
                },
                set: function(v) { this.textContent = v; },
                configurable: true
            },
            outerHTML: {
                get: function() {
                    var tag = (this.tagName || 'div').toLowerCase();
                    var attrs = this.getAttributeNames();
                    var s = '<' + tag;
                    for (var i = 0; i < attrs.length; i++) {
                        s += ' ' + attrs[i] + '="' + (this.getAttribute(attrs[i]) || '').replace(/"/g, '&quot;') + '"';
                    }
                    s += '>' + (this.innerHTML || '') + '</' + tag + '>';
                    return s;
                },
                configurable: true
            },
            ownerDocument: { get: function() { return document; }, configurable: true },
            isConnected: {
                get: function() {
                    // Walk up to see if we reach the document root
                    var cur = this.__nid;
                    while (cur >= 0) {
                        if (__n_getNodeType(cur) === 9) return true; // document node
                        cur = __n_getParent(cur);
                    }
                    return false;
                },
                configurable: true
            },
            // Attribute-reflecting properties
            tabIndex: {
                get: function() {
                    var v = this.getAttribute('tabindex');
                    if (v !== null) return parseInt(v) || 0;
                    var tag = this.tagName;
                    if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT' || tag === 'BUTTON' || tag === 'A' || tag === 'AREA') return 0;
                    return -1;
                },
                set: function(v) { this.setAttribute('tabindex', String(v)); },
                configurable: true
            },
            title: {
                get: function() { return this.getAttribute('title') || ''; },
                set: function(v) { this.setAttribute('title', String(v)); },
                configurable: true
            },
            lang: {
                get: function() { return this.getAttribute('lang') || ''; },
                set: function(v) { this.setAttribute('lang', String(v)); },
                configurable: true
            },
            dir: {
                get: function() { return this.getAttribute('dir') || ''; },
                set: function(v) { this.setAttribute('dir', String(v)); },
                configurable: true
            },
            hidden: {
                get: function() { return this.hasAttribute('hidden'); },
                set: function(v) { if (v) this.setAttribute('hidden', ''); else this.removeAttribute('hidden'); },
                configurable: true
            },
            name: {
                get: function() { return this.getAttribute('name') || ''; },
                set: function(v) { this.setAttribute('name', String(v)); },
                configurable: true
            },
            type: {
                get: function() {
                    if (this.tagName === 'INPUT') return (this.getAttribute('type') || 'text').toLowerCase();
                    if (this.tagName === 'BUTTON') return (this.getAttribute('type') || 'submit').toLowerCase();
                    return this.getAttribute('type') || '';
                },
                set: function(v) { this.setAttribute('type', String(v)); },
                configurable: true
            },
            disabled: {
                get: function() { return this.hasAttribute('disabled'); },
                set: function(v) { if (v) this.setAttribute('disabled', ''); else this.removeAttribute('disabled'); },
                configurable: true
            },
            placeholder: {
                get: function() { return this.getAttribute('placeholder') || ''; },
                set: function(v) { this.setAttribute('placeholder', String(v)); },
                configurable: true
            },
            href: {
                get: function() { return this.getAttribute('href') || ''; },
                set: function(v) { this.setAttribute('href', String(v)); },
                configurable: true
            },
            src: {
                get: function() { return this.getAttribute('src') || ''; },
                set: function(v) { this.setAttribute('src', String(v)); },
                configurable: true
            },
            rel: {
                get: function() { return this.getAttribute('rel') || ''; },
                set: function(v) { this.setAttribute('rel', String(v)); },
                configurable: true
            },
            validity: {
                get: function() {
                    var el = this;
                    var val = el.value || '';
                    var tag = el.tagName;
                    if (tag !== 'INPUT' && tag !== 'TEXTAREA' && tag !== 'SELECT') {
                        return { valid: true, valueMissing: false, typeMismatch: false, patternMismatch: false,
                            tooLong: false, tooShort: false, rangeUnderflow: false, rangeOverflow: false,
                            stepMismatch: false, badInput: false, customError: false };
                    }
                    var customMsg = (el.__props && el.__props._customValidity) || '';
                    var customError = customMsg.length > 0;
                    var valueMissing = !!(el.hasAttribute('required') && val === '');
                    var typeMismatch = false;
                    var inputType = (el.getAttribute('type') || '').toLowerCase();
                    if (val && inputType === 'email') typeMismatch = !/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(val);
                    if (val && inputType === 'url') typeMismatch = !/^https?:\/\/.+/.test(val);
                    var patternMismatch = false;
                    var pat = el.getAttribute('pattern');
                    if (pat && val) { try { patternMismatch = !new RegExp('^(?:' + pat + ')$').test(val); } catch(e) {} }
                    var tooLong = false, tooShort = false;
                    var maxl = el.getAttribute('maxlength'); if (maxl !== null && val.length > parseInt(maxl)) tooLong = true;
                    var minl = el.getAttribute('minlength'); if (minl !== null && val.length > 0 && val.length < parseInt(minl)) tooShort = true;
                    var rangeUnderflow = false, rangeOverflow = false, stepMismatch = false, badInput = false;
                    var mn = el.getAttribute('min');
                    var mx = el.getAttribute('max');
                    var stepAttr = el.getAttribute('step');
                    var numericTypes = { number: 1, range: 1 };
                    var dateTimeTypes = { date: 1, time: 1, 'datetime-local': 1, month: 1, week: 1 };
                    if (tag === 'INPUT' && inputType in numericTypes) {
                        var isRange = inputType === 'range';
                        var defMin = isRange ? 0 : null;
                        var defMax = isRange ? 100 : null;
                        var defStep = 1;
                        if (val !== '') {
                            var nv = parseFloat(val);
                            if (isNaN(nv) || !isFinite(nv)) {
                                if (!isRange) badInput = true;
                            } else {
                                var minVal = mn !== null ? parseFloat(mn) : defMin;
                                var maxVal = mx !== null ? parseFloat(mx) : defMax;
                                if (minVal !== null && nv < minVal) rangeUnderflow = true;
                                if (maxVal !== null && nv > maxVal) rangeOverflow = true;
                                var stepVal = stepAttr !== null ? parseFloat(stepAttr) : defStep;
                                if (stepVal !== null && stepAttr !== 'any' && !isNaN(stepVal) && stepVal > 0) {
                                    var base = minVal !== null ? minVal : 0;
                                    var diff = Math.abs((nv - base) % stepVal);
                                    if (diff > 1e-10 && Math.abs(diff - stepVal) > 1e-10) stepMismatch = true;
                                }
                            }
                        }
                    } else if (tag === 'INPUT' && inputType in dateTimeTypes) {
                        if (val !== '') {
                            var dtValid = true;
                            var dtVal = 0, dtMin = null, dtMax = null;
                            if (inputType === 'date') {
                                if (!/^\d{4}-\d{2}-\d{2}$/.test(val)) { badInput = true; dtValid = false; }
                                else { dtVal = new Date(val + 'T00:00:00Z').getTime(); if (isNaN(dtVal)) { badInput = true; dtValid = false; } }
                                if (dtValid && mn !== null) { dtMin = new Date(mn + 'T00:00:00Z').getTime(); }
                                if (dtValid && mx !== null) { dtMax = new Date(mx + 'T00:00:00Z').getTime(); }
                            } else if (inputType === 'time') {
                                if (!/^\d{2}:\d{2}(:\d{2})?$/.test(val)) { badInput = true; dtValid = false; }
                                else {
                                    var tp = val.split(':'); dtVal = parseInt(tp[0]) * 3600 + parseInt(tp[1]) * 60 + (tp[2] ? parseInt(tp[2]) : 0);
                                    if (parseInt(tp[0]) > 23 || parseInt(tp[1]) > 59 || (tp[2] && parseInt(tp[2]) > 59)) { badInput = true; dtValid = false; }
                                }
                                if (dtValid && mn !== null) { var mp = mn.split(':'); dtMin = parseInt(mp[0]) * 3600 + parseInt(mp[1]) * 60 + (mp[2] ? parseInt(mp[2]) : 0); }
                                if (dtValid && mx !== null) { var xp = mx.split(':'); dtMax = parseInt(xp[0]) * 3600 + parseInt(xp[1]) * 60 + (xp[2] ? parseInt(xp[2]) : 0); }
                            } else if (inputType === 'datetime-local') {
                                if (!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}(:\d{2})?$/.test(val)) { badInput = true; dtValid = false; }
                                else { dtVal = new Date(val + 'Z').getTime(); if (isNaN(dtVal)) { badInput = true; dtValid = false; } }
                                if (dtValid && mn !== null) { dtMin = new Date(mn + 'Z').getTime(); }
                                if (dtValid && mx !== null) { dtMax = new Date(mx + 'Z').getTime(); }
                            } else if (inputType === 'month') {
                                if (!/^\d{4}-\d{2}$/.test(val)) { badInput = true; dtValid = false; }
                                else {
                                    var mParts = val.split('-'); dtVal = parseInt(mParts[0]) * 12 + parseInt(mParts[1]);
                                    if (parseInt(mParts[1]) < 1 || parseInt(mParts[1]) > 12) { badInput = true; dtValid = false; }
                                }
                                if (dtValid && mn !== null) { var mnP = mn.split('-'); dtMin = parseInt(mnP[0]) * 12 + parseInt(mnP[1]); }
                                if (dtValid && mx !== null) { var mxP = mx.split('-'); dtMax = parseInt(mxP[0]) * 12 + parseInt(mxP[1]); }
                            } else if (inputType === 'week') {
                                if (!/^\d{4}-W\d{2}$/.test(val)) { badInput = true; dtValid = false; }
                                else {
                                    var wParts = val.split('-W'); dtVal = parseInt(wParts[0]) * 53 + parseInt(wParts[1]);
                                    if (parseInt(wParts[1]) < 1 || parseInt(wParts[1]) > 53) { badInput = true; dtValid = false; }
                                }
                                if (dtValid && mn !== null) { var wnP = mn.split('-W'); dtMin = parseInt(wnP[0]) * 53 + parseInt(wnP[1]); }
                                if (dtValid && mx !== null) { var wxP = mx.split('-W'); dtMax = parseInt(wxP[0]) * 53 + parseInt(wxP[1]); }
                            }
                            if (dtValid) {
                                if (dtMin !== null && !isNaN(dtMin) && dtVal < dtMin) rangeUnderflow = true;
                                if (dtMax !== null && !isNaN(dtMax) && dtVal > dtMax) rangeOverflow = true;
                            }
                        }
                    } else if (tag === 'INPUT' && inputType === 'color') {
                        if (val !== '' && !/^#[0-9a-fA-F]{6}$/.test(val)) badInput = true;
                    } else {
                        if (mn !== null && val !== '' && parseFloat(val) < parseFloat(mn)) rangeUnderflow = true;
                        if (mx !== null && val !== '' && parseFloat(val) > parseFloat(mx)) rangeOverflow = true;
                    }
                    var valid = !valueMissing && !typeMismatch && !patternMismatch && !tooLong && !tooShort && !rangeUnderflow && !rangeOverflow && !stepMismatch && !badInput && !customError;
                    return { valid: valid, valueMissing: valueMissing, typeMismatch: typeMismatch,
                        patternMismatch: patternMismatch, tooLong: tooLong, tooShort: tooShort,
                        rangeUnderflow: rangeUnderflow, rangeOverflow: rangeOverflow,
                        stepMismatch: stepMismatch, badInput: badInput, customError: customError };
                },
                configurable: true
            },
            validationMessage: {
                get: function() {
                    var v = this.validity;
                    if (v.valid) return '';
                    if (v.customError) return (this.__props && this.__props._customValidity) || '';
                    if (v.valueMissing) return 'Please fill out this field.';
                    if (v.typeMismatch) return 'Please enter a valid value.';
                    if (v.patternMismatch) return 'Please match the requested format.';
                    if (v.tooShort) return 'Please use at least ' + this.getAttribute('minlength') + ' characters.';
                    if (v.tooLong) return 'Please use no more than ' + this.getAttribute('maxlength') + ' characters.';
                    if (v.rangeUnderflow) return 'Value must be greater than or equal to ' + this.getAttribute('min') + '.';
                    if (v.rangeOverflow) return 'Value must be less than or equal to ' + this.getAttribute('max') + '.';
                    if (v.stepMismatch) return 'Please enter a valid value. The nearest valid values are those aligned with the step.';
                    if (v.badInput) return 'Please enter a valid value.';
                    return '';
                },
                configurable: true
            },
        });

        // open property for DIALOG and DETAILS
        Object.defineProperty(EP, 'open', {
            get: function() {
                if (this.tagName === 'DIALOG' || this.tagName === 'DETAILS') return this.hasAttribute('open');
                return undefined;
            },
            set: function(v) {
                if (this.tagName === 'DIALOG' || this.tagName === 'DETAILS') {
                    if (v) this.setAttribute('open', '');
                    else this.removeAttribute('open');
                }
            },
            configurable: true
        });
        Object.defineProperty(EP, 'returnValue', {
            get: function() {
                if (this.tagName !== 'DIALOG') return undefined;
                return (this.__props && this.__props._returnValue) || '';
            },
            set: function(v) {
                if (this.tagName === 'DIALOG') { if (!this.__props) this.__props = {}; this.__props._returnValue = String(v); }
            },
            configurable: true
        });

        // --- Form-related properties and methods ---
        // form property: walk up to find ancestor <form>
        Object.defineProperty(EP, 'form', {
            get: function() {
                var cur = __n_getParent(this.__nid);
                while (cur >= 0) {
                    if (__n_getTagName(cur) === 'FORM') return __w(cur);
                    cur = __n_getParent(cur);
                }
                return null;
            },
            configurable: true
        });

        // Form-specific methods (only meaningful on <form> elements but safe on all)
        EP.submit = function() {
            if (this.tagName === 'FORM') {
                var evt = new Event('submit', {bubbles: true, cancelable: true});
                evt.target = this;
                this.dispatchEvent(evt);
            }
        };
        EP.requestSubmit = function(submitter) {
            if (this.tagName !== 'FORM') return;
            // If submitter is provided, validate it
            if (submitter !== undefined && submitter !== null) {
                if (!submitter.form || submitter.form !== this) {
                    throw new DOMException(
                        "The specified element is not a submit button of this form",
                        "NotFoundError"
                    );
                }
            }
            // Run constraint validation on all controls
            var controls = this.querySelectorAll('input, textarea, select');
            var allValid = true;
            for (var i = 0; i < controls.length; i++) {
                if (!controls[i].checkValidity()) {
                    allValid = false;
                }
            }
            if (!allValid) return;
            // Fire the submit event (cancelable)
            var evt = new Event('submit', {bubbles: true, cancelable: true});
            evt.submitter = submitter || null;
            var dispatched = this.dispatchEvent(evt);
            // If preventDefault was called, do not submit
            if (!dispatched) return;
        };
        EP.reset = function() {
            if (this.tagName !== 'FORM') return;
            // Clear dirty flags on all descendant controls
            var controls = this.querySelectorAll('input, textarea, select');
            for (var i = 0; i < controls.length; i++) {
                var c = controls[i];
                if (c.__props) {
                    delete c.__props._value;
                    delete c.__props._checked;
                    delete c.__props._selected;
                }
                // Sync attribute back (value falls back to defaultValue)
                var dv = c.getAttribute('value');
                if (dv !== null) __n_setAttribute(c.__nid, 'value', dv);
                else __n_removeAttribute(c.__nid, 'value');
            }
            var evt = new Event('reset', {bubbles: true, cancelable: true});
            evt.target = this;
            this.dispatchEvent(evt);
        };
        EP.setCustomValidity = function(msg) {
            if (!this.__props) this.__props = {};
            this.__props._customValidity = String(msg);
        };
        EP.checkValidity = function() {
            var v = this.validity;
            if (!v.valid) {
                this.dispatchEvent(new Event('invalid', {bubbles: false, cancelable: true}));
                return false;
            }
            return true;
        };
        EP.reportValidity = function() { return this.checkValidity(); };

        // elements property for <form>: returns descendant controls with named access
        Object.defineProperty(EP, 'elements', {
            get: function() {
                if (this.tagName !== 'FORM') return undefined;
                var controls = this.querySelectorAll('input, textarea, select, button');
                return new Proxy(controls, {
                    get: function(arr, prop) {
                        if (prop in arr) return arr[prop];
                        if (typeof prop === 'string' && isNaN(prop)) {
                            // Named access: find by name attribute
                            for (var i = 0; i < arr.length; i++) {
                                if (arr[i].getAttribute('name') === prop || arr[i].getAttribute('id') === prop) return arr[i];
                            }
                            return undefined;
                        }
                        return arr[prop];
                    }
                });
            },
            configurable: true
        });

        // action/method properties for all elements (meaningful on <form>)
        Object.defineProperty(EP, 'action', {
            get: function() { return this.getAttribute('action') || ''; },
            set: function(v) { this.setAttribute('action', String(v)); },
            configurable: true
        });
        Object.defineProperty(EP, 'method', {
            get: function() { return (this.getAttribute('method') || 'get').toLowerCase(); },
            set: function(v) { this.setAttribute('method', String(v)); },
            configurable: true
        });

        // <select> selectedIndex property
        Object.defineProperty(EP, 'selectedIndex', {
            get: function() {
                if (this.tagName !== 'SELECT') return -1;
                var opts = this.querySelectorAll('option');
                for (var i = 0; i < opts.length; i++) {
                    if (opts[i].__props && opts[i].__props._selected) return i;
                    if (opts[i].hasAttribute('selected')) return i;
                }
                return opts.length > 0 ? 0 : -1;
            },
            set: function(idx) {
                if (this.tagName !== 'SELECT') return;
                var opts = this.querySelectorAll('option');
                for (var i = 0; i < opts.length; i++) {
                    if (!opts[i].__props) opts[i].__props = {};
                    opts[i].__props._selected = (i === idx);
                }
            },
            configurable: true
        });

        // <select> options property
        Object.defineProperty(EP, 'options', {
            get: function() {
                if (this.tagName !== 'SELECT') return undefined;
                var sel = this;
                var opts = this.querySelectorAll('option');
                return new Proxy(opts, {
                    get: function(arr, p) {
                        if (p === 'length') return arr.length;
                        if (p === 'selectedIndex') return sel.selectedIndex;
                        if (p === 'item') return function(i) { return arr[i] || null; };
                        if (p === 'namedItem') return function(name) {
                            for (var i = 0; i < arr.length; i++) {
                                if (arr[i].getAttribute('name') === name || arr[i].getAttribute('id') === name) return arr[i];
                            }
                            return null;
                        };
                        if (typeof p === 'string' && !isNaN(p)) return arr[parseInt(p)];
                        if (p === Symbol.iterator) return function() { return arr[Symbol.iterator](); };
                        return arr[p];
                    }
                });
            },
            configurable: true
        });

        // <option> text property
        Object.defineProperty(EP, 'text', {
            get: function() {
                if (this.tagName === 'OPTION') return (this.textContent || '').trim();
                return undefined;
            },
            set: function(v) {
                if (this.tagName === 'OPTION') this.textContent = String(v);
            },
            configurable: true
        });

        // <option> index property
        Object.defineProperty(EP, 'index', {
            get: function() {
                if (this.tagName !== 'OPTION') return undefined;
                var parent = this.parentNode;
                if (!parent || parent.tagName !== 'SELECT') return 0;
                var opts = parent.querySelectorAll('option');
                for (var i = 0; i < opts.length; i++) {
                    if (opts[i].__nid === this.__nid) return i;
                }
                return 0;
            },
            configurable: true
        });

        // <option> label property
        Object.defineProperty(EP, 'label', {
            get: function() {
                if (this.tagName !== 'OPTION') return '';
                return this.getAttribute('label') || (this.textContent || '').trim();
            },
            set: function(v) {
                if (this.tagName === 'OPTION') this.setAttribute('label', String(v));
            },
            configurable: true
        });

        // --- Label association properties ---
        // label.htmlFor — reflects the `for` attribute
        Object.defineProperty(EP, 'htmlFor', {
            get: function() {
                if (this.tagName !== 'LABEL') return undefined;
                return this.getAttribute('for') || '';
            },
            set: function(v) {
                if (this.tagName === 'LABEL') this.setAttribute('for', String(v));
            },
            configurable: true
        });

        // label.control — returns the associated form control element
        Object.defineProperty(EP, 'control', {
            get: function() {
                if (this.tagName !== 'LABEL') return undefined;
                var id = __n_findLabelControl(this.__nid);
                return id >= 0 ? __w(id) : null;
            },
            configurable: true
        });

        // input.labels — returns a NodeList of all <label> elements associated with this input
        Object.defineProperty(EP, 'labels', {
            get: function() {
                var tag = this.tagName;
                if (tag !== 'INPUT' && tag !== 'SELECT' && tag !== 'TEXTAREA' && tag !== 'BUTTON') return undefined;
                // Hidden inputs have no labels per spec
                if (tag === 'INPUT' && (this.getAttribute('type') || '').toLowerCase() === 'hidden') return [];
                var ids = __n_findLabelsForControl(this.__nid);
                return ids.map(__w);
            },
            configurable: true
        });

        // Tag → constructor map for React's node.constructor.prototype lookup
        var _ctorMap = {
            INPUT: HTMLInputElement, TEXTAREA: HTMLTextAreaElement,
            SELECT: HTMLSelectElement, FORM: HTMLFormElement,
            A: HTMLAnchorElement, IMG: HTMLImageElement,
            BUTTON: HTMLButtonElement, OPTION: HTMLOptionElement,
            IFRAME: HTMLIFrameElement,
        };

        // Wrapper factory
        function __w(nodeId) {
            if (_cache[nodeId]) return _cache[nodeId];
            var obj = Object.create(EP);
            obj.__nid = nodeId;
            obj.__props = {}; // per-element property store (dirty value/checked/selected)
            // Set constructor so React's inputValueTracking can find
            // the native value descriptor via node.constructor.prototype
            var tag = __n_getTagName(nodeId);
            var ctor = _ctorMap[tag];
            if (ctor) obj.constructor = ctor;
            _cache[nodeId] = obj;
            return obj;
        }
        globalThis.__braille_get_element_wrapper = __w;

        // Event dispatch with capture + bubble phases
        function __dispatch(nodeId, event) {
            // Build path: target -> parent -> ... -> root
            var path = [];
            var cur = nodeId;
            while (cur >= 0) {
                path.push(cur);
                cur = __n_getParent(cur);
            }

            event.target = __w(nodeId);
            event.eventPhase = 0;

            // Build composedPath: wrapped elements + document + window
            var composedPath = [];
            for (var pi = 0; pi < path.length; pi++) composedPath.push(__w(path[pi]));
            composedPath.push(document);
            composedPath.push(window);
            event._path = composedPath;

            // Helper to fire a list of callbacks
            function fireCbs(cbs, thisObj) {
                if (!cbs || !cbs.length) return;
                var snapshot = cbs.slice();
                for (var j = 0; j < snapshot.length; j++) {
                    snapshot[j].call(thisObj, event);
                    if (event._stopImmediate) return;
                }
            }

            // === CAPTURE PHASE (root → target) ===
            // Window capture
            event.eventPhase = 1;
            event.currentTarget = window;
            fireCbs(_winCapture[event.type], window);
            if (event._stopImmediate || event._stopPropagation) return;

            // Document capture
            event.currentTarget = document;
            fireCbs(_docCapture[event.type], document);
            if (event._stopImmediate || event._stopPropagation) return;

            // DOM elements capture: from root down to (but not including) target
            for (var i = path.length - 1; i > 0; i--) {
                var nid = path[i];
                event.currentTarget = __w(nid);
                fireCbs(_captureKeys[nid + ':' + event.type], event.currentTarget);
                if (event._stopImmediate || event._stopPropagation) return;
            }

            // === AT-TARGET PHASE ===
            event.eventPhase = 2;
            var targetNid = path[0];
            var targetEl = __w(targetNid);
            event.currentTarget = targetEl;

            // Inline event handler (e.g. onclick="...")
            var attrHandler = __n_getAttribute(targetNid, 'on' + event.type);
            if (attrHandler) {
                (new Function('event', attrHandler)).call(targetEl, event);
                if (event._stopImmediate) return;
            }

            // Fire both capture and bubble listeners at target (per spec)
            fireCbs(_captureKeys[targetNid + ':' + event.type], targetEl);
            if (event._stopImmediate) return;
            fireCbs(_bubbleKeys[targetNid + ':' + event.type], targetEl);
            if (event._stopImmediate) return;

            if (!event.bubbles) return;

            // === BUBBLE PHASE (target+1 → root → document → window) ===
            event.eventPhase = 3;
            for (var i = 1; i < path.length; i++) {
                if (event._stopPropagation) break;
                var nid = path[i];
                event.currentTarget = __w(nid);
                fireCbs(_bubbleKeys[nid + ':' + event.type], event.currentTarget);
                if (event._stopImmediate) return;
            }

            // Document bubble
            if (!event._stopPropagation) {
                event.currentTarget = document;
                fireCbs(doc.__listeners[event.type], document);
                if (event._stopImmediate) return;
            }

            // Window bubble
            if (!event._stopPropagation) {
                event.currentTarget = window;
                fireCbs(_winListeners[event.type], window);
            }
        }

        // __braille_click(nodeId) — called from Rust
        globalThis.__braille_click = function(nodeId) {
            var el = __w(nodeId);
            el.click();
        };

        // Fire load event on <link> elements (CSS, prefetch, etc.)
        // We don't actually load CSS, but frameworks need the onload to resolve promises.
        globalThis.__braille_maybe_load_link = function(node) {
            if (!node || node.tagName !== 'LINK') return;
            var rel = node.rel || node.getAttribute('rel') || '';
            if (rel === 'stylesheet' || rel === 'prefetch' || rel === 'preload') {
                setTimeout(function() {
                    if (typeof node.onload === 'function') {
                        node.onload({type: 'load', target: node});
                    }
                    node.dispatchEvent(new Event('load'));
                }, 0);
            }
        };

        // Dynamic script loading: fetch and eval <script src="..."> on insertion
        globalThis.__braille_script_log = [];
        globalThis.__braille_maybe_load_script = function(node) {
            if (!node || node.tagName !== 'SCRIPT') return;
            var src = node.getAttribute('src');
            if (!src) return;
            var shortSrc = src.substring(src.lastIndexOf('/') + 1).substring(0, 40);
            __braille_script_log.push('FETCH: ' + shortSrc);
            fetch(src).then(function(resp) {
                __braille_script_log.push('RESP: ' + shortSrc + ' ok=' + resp.ok + ' status=' + resp.status);
                if (!resp.ok) throw new Error('HTTP ' + resp.status);
                return resp.text();
            }).then(function(code) {
                __braille_script_log.push('EVAL: ' + shortSrc + ' len=' + code.length);
                document.currentScript = node;
                (0, eval)(code);
                document.currentScript = null;
                __braille_script_log.push('OK: ' + shortSrc);
                if (typeof node.onload === 'function') {
                    node.onload({type: 'load', target: node});
                }
                node.dispatchEvent(new Event('load'));
            }).catch(function(err) {
                document.currentScript = null;
                __braille_script_log.push('ERR: ' + shortSrc + ' -> ' + String(err).substring(0, 100));
                if (typeof node.onerror === 'function') {
                    node.onerror({type: 'error', target: node, message: String(err)});
                }
                node.dispatchEvent(new Event('error'));
            });
        };

        // Element mutation methods that operate on the real DomTree
        EP.appendChild = function(child) {
            if (child && child.__nid !== undefined && this.__nid !== undefined) {
                if (child.nodeType === 11) {
                    var kids = __n_getAllChildIds(child.__nid);
                    var added = [];
                    for (var i = 0; i < kids.length; i++) {
                        __n_appendChild(this.__nid, kids[i]);
                        added.push(__w(kids[i]));
                    }
                    if (typeof __mo_notify === 'function' && added.length) __mo_notify('childList', this, {addedNodes: added});
                } else {
                    __n_appendChild(this.__nid, child.__nid);
                    if (typeof __mo_notify === 'function') __mo_notify('childList', this, {addedNodes: [child]});
                }
            }
            __braille_maybe_load_script(child);
            __braille_maybe_load_link(child);
            return child;
        };
        EP.removeChild = function(child) {
            if (child && child.__nid !== undefined && this.__nid !== undefined) {
                __n_removeChild(this.__nid, child.__nid);
                if (typeof __mo_notify === 'function') __mo_notify('childList', this, {removedNodes: [child]});
            }
            return child;
        };
        EP.insertBefore = function(newChild, refChild) {
            if (newChild && newChild.__nid !== undefined && this.__nid !== undefined) {
                var refId = (refChild && refChild.__nid !== undefined) ? refChild.__nid : -1;
                if (newChild.nodeType === 11) {
                    var kids = __n_getAllChildIds(newChild.__nid);
                    var added = [];
                    for (var i = 0; i < kids.length; i++) {
                        __n_insertBefore(this.__nid, kids[i], refId);
                        added.push(__w(kids[i]));
                    }
                    if (typeof __mo_notify === 'function' && added.length) __mo_notify('childList', this, {addedNodes: added});
                } else {
                    __n_insertBefore(this.__nid, newChild.__nid, refId);
                    if (typeof __mo_notify === 'function') __mo_notify('childList', this, {addedNodes: [newChild]});
                }
            }
            __braille_maybe_load_script(newChild);
            __braille_maybe_load_link(newChild);
            return newChild;
        };

        // Fullscreen tracking
        var __fullscreenElement = null;
        EP.requestFullscreen = function() { __fullscreenElement = this; doc.dispatchEvent(new Event('fullscreenchange')); return Promise.resolve(); };

        // Override document methods
        var doc = globalThis.document;
        doc.__listeners = {};
        doc.getElementById = function(id) {
            var nid = __n_getElementById(id);
            return nid >= 0 ? __w(nid) : null;
        };
        doc.querySelector = function(sel) {
            var nid = __n_querySelector(0, sel);
            return nid >= 0 ? __w(nid) : null;
        };
        doc.querySelectorAll = function(sel) {
            return __n_querySelectorAll(0, sel).map(__w);
        };
        doc.createElement = function(tag) {
            var nid = __n_createElement(tag);
            return __w(nid);
        };
        doc.createElementNS = function(ns, tag) {
            var nid = __n_createElement(tag);
            var el = __w(nid);
            el.namespaceURI = ns;
            return el;
        };
        doc.createTextNode = function(text) {
            var nid = __n_createTextNode(text);
            var node = __w(nid);
            return node;
        };
        doc.createComment = function(text) { return { nodeType: 8, textContent: text }; };
        doc.createDocumentFragment = function() {
            var nid = __n_createDocFragment();
            return __w(nid);
        };
        doc.getElementsByTagName = function(tag) {
            return new Proxy([], {
                get: function(t, p) {
                    var live = doc.querySelectorAll(tag);
                    if (p === 'length') return live.length;
                    if (p === 'item') return function(i) { return live[i] || null; };
                    if (p === 'namedItem') return function(name) {
                        for (var i = 0; i < live.length; i++) {
                            if (live[i].getAttribute('name') === name || live[i].getAttribute('id') === name) return live[i];
                        }
                        return null;
                    };
                    if (p === Symbol.iterator) return function() { return live[Symbol.iterator](); };
                    if (typeof p === 'string' && !isNaN(p)) return live[parseInt(p)];
                    if (p === 'forEach') return function(cb) { for (var i = 0; i < live.length; i++) cb(live[i], i); };
                    return live[p];
                }
            });
        };
        doc.getElementsByClassName = function(cls) {
            return new Proxy([], {
                get: function(t, p) {
                    var live = doc.querySelectorAll('.' + cls);
                    if (p === 'length') return live.length;
                    if (p === 'item') return function(i) { return live[i] || null; };
                    if (p === Symbol.iterator) return function() { return live[Symbol.iterator](); };
                    if (typeof p === 'string' && !isNaN(p)) return live[parseInt(p)];
                    if (p === 'forEach') return function(cb) { for (var i = 0; i < live.length; i++) cb(live[i], i); };
                    return live[p];
                }
            });
        };
        doc.addEventListener = function(type, cb, opts) {
            if (typeof cb !== 'function') return;
            var capture = !!(opts === true || (opts && opts.capture));
            var once = !!(opts && typeof opts === 'object' && opts.once);
            var store = capture ? _docCapture : doc.__listeners;
            if (!store[type]) store[type] = [];
            if (once) {
                var wrapper = function(e) { cb.call(document, e); doc.removeEventListener(type, wrapper, capture); };
                wrapper._origCb = cb;
                store[type].push(wrapper);
            } else {
                store[type].push(cb);
            }
        };
        doc.removeEventListener = function(type, cb, opts) {
            var capture = !!(opts === true || (opts && opts.capture));
            var store = capture ? _docCapture : doc.__listeners;
            if (store[type]) store[type] = store[type].filter(function(f){return f!==cb && f._origCb!==cb;});
        };

        doc.createComment = function(text) {
            var nid = __n_createComment(text || '');
            return __w(nid);
        };

        function BrailleRange() {
            this.startContainer = null; this.startOffset = 0;
            this.endContainer = null; this.endOffset = 0;
            this.collapsed = true; this.commonAncestorContainer = null;
        }
        BrailleRange.START_TO_START = 0; BrailleRange.START_TO_END = 1;
        BrailleRange.END_TO_END = 2; BrailleRange.END_TO_START = 3;
        BrailleRange.prototype.setStart = function(node, offset) { this.startContainer = node; this.startOffset = offset; this._update(); };
        BrailleRange.prototype.setEnd = function(node, offset) { this.endContainer = node; this.endOffset = offset; this._update(); };
        BrailleRange.prototype.setStartBefore = function(node) { this.startContainer = node.parentNode; this.startOffset = node.parentNode ? Array.prototype.indexOf.call(node.parentNode.childNodes, node) : 0; this._update(); };
        BrailleRange.prototype.setStartAfter = function(node) { this.startContainer = node.parentNode; this.startOffset = node.parentNode ? Array.prototype.indexOf.call(node.parentNode.childNodes, node) + 1 : 0; this._update(); };
        BrailleRange.prototype.setEndBefore = function(node) { this.endContainer = node.parentNode; this.endOffset = node.parentNode ? Array.prototype.indexOf.call(node.parentNode.childNodes, node) : 0; this._update(); };
        BrailleRange.prototype.setEndAfter = function(node) { this.endContainer = node.parentNode; this.endOffset = node.parentNode ? Array.prototype.indexOf.call(node.parentNode.childNodes, node) + 1 : 0; this._update(); };
        BrailleRange.prototype.selectNode = function(node) { this.setStartBefore(node); this.setEndAfter(node); };
        BrailleRange.prototype.selectNodeContents = function(node) { this.startContainer = node; this.startOffset = 0; this.endContainer = node; this.endOffset = node.childNodes ? node.childNodes.length : 0; this._update(); };
        BrailleRange.prototype.collapse = function(toStart) { if (toStart || toStart === undefined) { this.endContainer = this.startContainer; this.endOffset = this.startOffset; } else { this.startContainer = this.endContainer; this.startOffset = this.endOffset; } this.collapsed = true; };
        BrailleRange.prototype.cloneRange = function() { var r = new BrailleRange(); r.startContainer = this.startContainer; r.startOffset = this.startOffset; r.endContainer = this.endContainer; r.endOffset = this.endOffset; r._update(); return r; };
        BrailleRange.prototype.detach = function() {};
        BrailleRange.prototype.getBoundingClientRect = function() {
            var el = this.startContainer;
            if (el && el.nodeType === 3) el = el.parentNode;
            return el && el.getBoundingClientRect ? el.getBoundingClientRect() : {top:0,left:0,width:0,height:0,right:0,bottom:0,x:0,y:0};
        };
        BrailleRange.prototype.getClientRects = function() { return [this.getBoundingClientRect()]; };
        BrailleRange.prototype.toString = function() {
            if (this.startContainer && this.endContainer && this.startContainer === this.endContainer && this.startContainer.nodeType === 3) {
                return (this.startContainer.textContent || '').substring(this.startOffset, this.endOffset);
            }
            return this.startContainer ? (this.startContainer.textContent || '') : '';
        };
        BrailleRange.prototype.createContextualFragment = function(html) {
            var temp = document.createElement('div');
            __n_setInnerHTML(temp.__nid, html);
            var frag = document.createDocumentFragment();
            while (temp.firstChild) frag.appendChild(temp.firstChild);
            return frag;
        };
        BrailleRange.prototype._update = function() {
            this.collapsed = (this.startContainer === this.endContainer && this.startOffset === this.endOffset);
            // Walk ancestors of startContainer and endContainer to find common ancestor
            if (this.startContainer && this.endContainer) {
                var ancestors = [];
                var cur = this.startContainer;
                while (cur) { ancestors.push(cur); cur = cur.parentNode; }
                cur = this.endContainer;
                while (cur) { if (ancestors.indexOf(cur) >= 0) { this.commonAncestorContainer = cur; return; } cur = cur.parentNode; }
            }
            this.commonAncestorContainer = null;
        };
        globalThis.Range = BrailleRange;
        doc.createRange = function() { return new BrailleRange(); };

        // window.addEventListener / removeEventListener
        window.addEventListener = function(type, cb, opts) {
            if (typeof cb !== 'function') return;
            var capture = !!(opts === true || (opts && opts.capture));
            var once = !!(opts && typeof opts === 'object' && opts.once);
            var store = capture ? _winCapture : _winListeners;
            if (!store[type]) store[type] = [];
            if (once) {
                var wrapper = function(e) { cb.call(window, e); window.removeEventListener(type, wrapper, capture); };
                wrapper._origCb = cb;
                store[type].push(wrapper);
            } else {
                store[type].push(cb);
            }
        };
        window.removeEventListener = function(type, cb, opts) {
            var capture = !!(opts === true || (opts && opts.capture));
            var store = capture ? _winCapture : _winListeners;
            if (store[type]) {
                store[type] = store[type].filter(function(f){return f!==cb && f._origCb!==cb;});
            }
        };

        doc.dispatchEvent = function(event) {
            event.target = document;
            event.currentTarget = document;
            var cbs = doc.__listeners[event.type];
            if (cbs) {
                var snapshot = cbs.slice();
                for (var i = 0; i < snapshot.length; i++) snapshot[i].call(document, event);
            }
            return !event.defaultPrevented;
        };
        doc.createEvent = function(type) { return new Event(type); };
        doc.createTreeWalker = function(root, whatToShow, filter) {
            // Minimal TreeWalker: pre-order traversal of element nodes
            var current = root;
            return {
                currentNode: root,
                nextNode: function() {
                    // depth-first walk
                    if (current.firstChild) { current = current.firstChild; this.currentNode = current; return current; }
                    while (current) {
                        if (current.nextSibling) { current = current.nextSibling; this.currentNode = current; return current; }
                        current = current.parentNode;
                        if (current === root) { current = null; this.currentNode = null; return null; }
                    }
                    return null;
                },
                previousNode: function() { return null; },
                firstChild: function() { var c = current.firstChild; if (c) { current = c; this.currentNode = c; } return c; },
                lastChild: function() { var c = current.lastChild; if (c) { current = c; this.currentNode = c; } return c; },
                nextSibling: function() { var s = current.nextSibling; if (s) { current = s; this.currentNode = s; } return s; },
                previousSibling: function() { var s = current.previousSibling; if (s) { current = s; this.currentNode = s; } return s; },
                parentNode: function() { var p = current.parentNode; if (p && p !== root) { current = p; this.currentNode = p; return p; } return null; },
            };
        };
        doc.createNodeIterator = function(root) { return doc.createTreeWalker(root); };
        doc.importNode = function(node, deep) {
            if (!node) return node;
            if (node.__nid !== undefined) return node.cloneNode(!!deep);
            return node;
        };
        doc.adoptNode = function(node) { return node; };
        doc.exitFullscreen = function() { __fullscreenElement = null; doc.dispatchEvent(new Event('fullscreenchange')); return Promise.resolve(); };
        doc.getAnimations = function() { return []; };

        window.dispatchEvent = function(event) {
            event.target = window;
            event.currentTarget = window;
            var cbs = _winListeners[event.type];
            if (cbs) {
                var snapshot = cbs.slice();
                for (var i = 0; i < snapshot.length; i++) snapshot[i].call(window, event);
            }
            return !event.defaultPrevented;
        };

        // Track focused element for document.activeElement
        var __focusedElement = null;
        EP.focus = function() { __focusedElement = this; };
        EP.blur = function() { if (__focusedElement === this) __focusedElement = null; };

        // document.cookie implementation (JS-side cookie jar)
        var _cookieJar = {};
        Object.defineProperties(doc, {
            body: { get: function() { return doc.querySelector('body'); }, configurable: true },
            head: { get: function() { return doc.querySelector('head'); }, configurable: true },
            documentElement: { get: function() { return doc.querySelector('html'); }, configurable: true },
            activeElement: { get: function() { return __focusedElement || doc.querySelector('body'); }, configurable: true },
            cookie: {
                get: function() {
                    var now = Date.now();
                    var parts = [];
                    for (var name in _cookieJar) {
                        var c = _cookieJar[name];
                        if (c.expires && c.expires < now) { delete _cookieJar[name]; continue; }
                        parts.push(name + '=' + c.value);
                    }
                    return parts.join('; ');
                },
                set: function(s) {
                    if (typeof s !== 'string') return;
                    var parts = s.split(';');
                    var nv = parts[0].trim().split('=');
                    if (nv.length < 2) return;
                    var name = nv[0].trim();
                    var value = nv.slice(1).join('=').trim();
                    var expires = null;
                    for (var i = 1; i < parts.length; i++) {
                        var p = parts[i].trim().toLowerCase();
                        if (p.indexOf('expires=') === 0) {
                            expires = Date.parse(parts[i].trim().substring(8));
                        } else if (p.indexOf('max-age=') === 0) {
                            var sec = parseInt(parts[i].trim().substring(8));
                            if (!isNaN(sec)) expires = Date.now() + sec * 1000;
                        }
                    }
                    if (expires !== null && expires < Date.now()) {
                        delete _cookieJar[name];
                    } else {
                        _cookieJar[name] = { value: value, expires: expires };
                    }
                },
                configurable: true
            },
            title: {
                get: function() {
                    var t = doc.querySelector('title');
                    return t ? t.textContent : '';
                },
                set: function(v) {
                    var t = doc.querySelector('title');
                    if (t) t.textContent = String(v);
                },
                configurable: true
            },
            currentScript: { value: null, writable: true, configurable: true },
            doctype: {
                get: function() {
                    var json = __n_getDoctypeInfo();
                    if (!json) return null;
                    var info = JSON.parse(json);
                    return { name: info.name, publicId: info.publicId, systemId: info.systemId, nodeType: 10, nodeName: info.name };
                },
                configurable: true
            },
            domain: {
                get: function() { return doc.__domain || location.hostname; },
                set: function(v) {
                    var cur = location.hostname;
                    if (cur === v || cur.endsWith('.' + v)) doc.__domain = v;
                },
                configurable: true
            },
            fullscreenElement: { get: function() { return __fullscreenElement; }, configurable: true },
            fullscreenEnabled: { value: true, configurable: true },
            referrer: { value: '', writable: true, configurable: true },
            characterSet: { value: 'UTF-8', configurable: true },
            contentType: { value: 'text/html', configurable: true },
            hidden: { value: false, configurable: true },
            visibilityState: { value: 'visible', configurable: true },
            implementation: { value: {
                createHTMLDocument: function(title) {
                    var div = document.createElement('div');
                    return {
                        documentElement: div, body: div, head: null,
                        title: title || '', readyState: 'complete',
                        querySelector: function(sel) { return div.querySelector(sel); },
                        querySelectorAll: function(sel) { return div.querySelectorAll(sel); },
                        getElementById: function(id) { return div.querySelector('#' + id) || null; },
                        getElementsByTagName: function(tag) { return div.getElementsByTagName(tag); },
                        getElementsByClassName: function(cls) { return div.getElementsByClassName(cls); },
                        createElement: function(tag) { return document.createElement(tag); },
                        createTextNode: function(text) { return document.createTextNode(text); },
                        createDocumentFragment: function() { return document.createDocumentFragment(); },
                    };
                },
                hasFeature: function() { return true; },
            }, configurable: true },
        });
    })();
    "#).unwrap_or_else(|e| {
        let msg = match e {
            rquickjs::Error::Exception => {
                let exc = ctx.catch();
                if let Some(exc) = exc.as_exception() {
                    format!("{}: {}", exc.message().unwrap_or_default(), exc.stack().unwrap_or_default())
                } else {
                    format!("{exc:?}")
                }
            }
            other => format!("{other:?}"),
        };
        panic!("DOM bridge JS init failed: {msg}");
    });
}

/// Recursively copy a node from a source tree into a destination tree.
fn import_node_recursive(
    dst: &mut DomTree,
    src: &DomTree,
    src_node_id: NodeId,
    dst_parent_id: NodeId,
) {
    let src_node = src.get_node(src_node_id);
    let new_id = match &src_node.data {
        NodeData::Element {
            tag_name,
            attributes,
            namespace,
            ..
        } => {
            let attrs: Vec<crate::dom::node::DomAttribute> = attributes.clone();
            dst.create_element_ns(tag_name, attrs, namespace)
        }
        NodeData::Text { content } => dst.create_text(content),
        NodeData::Comment { content } => dst.create_comment(content),
        _ => return,
    };
    dst.append_child(dst_parent_id, new_id);

    let children: Vec<NodeId> = src_node.children.clone();
    for &child_id in &children {
        import_node_recursive(dst, src, child_id, new_id);
    }
}

#[cfg(test)]
mod tests {
    use crate::dom::DomTree;
    use crate::js::runtime::JsRuntime;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn make_runtime() -> JsRuntime {
        let tree = Rc::new(RefCell::new(DomTree::new()));
        {
            let mut t = tree.borrow_mut();
            let html = t.create_element("html");
            let body = t.create_element("body");
            let doc = t.document();
            t.append_child(doc, html);
            t.append_child(html, body);
        }
        JsRuntime::new(tree)
    }

    fn validity_field(rt: &mut JsRuntime, setup: &str, field: &str) -> String {
        rt.eval(setup).unwrap();
        rt.eval_to_string(&format!(
            "String(document.querySelector('input').validity.{field})"
        ))
        .unwrap()
    }

    // -----------------------------------------------------------------------
    // number
    // -----------------------------------------------------------------------

    #[test]
    fn number_valid_value() {
        let mut rt = make_runtime();
        let v = validity_field(
            &mut rt,
            r#"var i = document.createElement('input'); i.setAttribute('type','number'); i.setAttribute('value','42'); document.body.appendChild(i);"#,
            "valid",
        );
        assert_eq!(v, "true");
    }

    #[test]
    fn number_bad_input() {
        let mut rt = make_runtime();
        let v = validity_field(
            &mut rt,
            r#"var i = document.createElement('input'); i.setAttribute('type','number'); i.setAttribute('value','abc'); document.body.appendChild(i);"#,
            "badInput",
        );
        assert_eq!(v, "true");
    }

    #[test]
    fn number_range_underflow() {
        let mut rt = make_runtime();
        let v = validity_field(
            &mut rt,
            r#"var i = document.createElement('input'); i.setAttribute('type','number'); i.setAttribute('min','10'); i.setAttribute('value','5'); document.body.appendChild(i);"#,
            "rangeUnderflow",
        );
        assert_eq!(v, "true");
    }

    #[test]
    fn number_range_overflow() {
        let mut rt = make_runtime();
        let v = validity_field(
            &mut rt,
            r#"var i = document.createElement('input'); i.setAttribute('type','number'); i.setAttribute('max','10'); i.setAttribute('value','15'); document.body.appendChild(i);"#,
            "rangeOverflow",
        );
        assert_eq!(v, "true");
    }

    #[test]
    fn number_step_mismatch() {
        let mut rt = make_runtime();
        let v = validity_field(
            &mut rt,
            r#"var i = document.createElement('input'); i.setAttribute('type','number'); i.setAttribute('step','3'); i.setAttribute('min','0'); i.setAttribute('value','5'); document.body.appendChild(i);"#,
            "stepMismatch",
        );
        assert_eq!(v, "true");
    }

    #[test]
    fn number_step_valid() {
        let mut rt = make_runtime();
        let v = validity_field(
            &mut rt,
            r#"var i = document.createElement('input'); i.setAttribute('type','number'); i.setAttribute('step','3'); i.setAttribute('min','0'); i.setAttribute('value','6'); document.body.appendChild(i);"#,
            "stepMismatch",
        );
        assert_eq!(v, "false");
    }

    #[test]
    fn number_step_any_skips_check() {
        let mut rt = make_runtime();
        let v = validity_field(
            &mut rt,
            r#"var i = document.createElement('input'); i.setAttribute('type','number'); i.setAttribute('step','any'); i.setAttribute('min','0'); i.setAttribute('value','5.5'); document.body.appendChild(i);"#,
            "stepMismatch",
        );
        assert_eq!(v, "false");
    }

    // -----------------------------------------------------------------------
    // range
    // -----------------------------------------------------------------------

    #[test]
    fn range_valid_value() {
        let mut rt = make_runtime();
        let v = validity_field(
            &mut rt,
            r#"var i = document.createElement('input'); i.setAttribute('type','range'); i.setAttribute('value','50'); document.body.appendChild(i);"#,
            "valid",
        );
        assert_eq!(v, "true");
    }

    #[test]
    fn range_underflow_default_min() {
        let mut rt = make_runtime();
        let v = validity_field(
            &mut rt,
            r#"var i = document.createElement('input'); i.setAttribute('type','range'); i.setAttribute('value','-1'); document.body.appendChild(i);"#,
            "rangeUnderflow",
        );
        assert_eq!(v, "true");
    }

    #[test]
    fn range_overflow_default_max() {
        let mut rt = make_runtime();
        let v = validity_field(
            &mut rt,
            r#"var i = document.createElement('input'); i.setAttribute('type','range'); i.setAttribute('value','101'); document.body.appendChild(i);"#,
            "rangeOverflow",
        );
        assert_eq!(v, "true");
    }

    #[test]
    fn range_step_mismatch() {
        let mut rt = make_runtime();
        let v = validity_field(
            &mut rt,
            r#"var i = document.createElement('input'); i.setAttribute('type','range'); i.setAttribute('value','5.5'); document.body.appendChild(i);"#,
            "stepMismatch",
        );
        assert_eq!(v, "true");
    }

    // -----------------------------------------------------------------------
    // date
    // -----------------------------------------------------------------------

    #[test]
    fn date_valid() {
        let mut rt = make_runtime();
        let v = validity_field(
            &mut rt,
            r#"var i = document.createElement('input'); i.setAttribute('type','date'); i.setAttribute('value','2024-01-15'); document.body.appendChild(i);"#,
            "valid",
        );
        assert_eq!(v, "true");
    }

    #[test]
    fn date_bad_format() {
        let mut rt = make_runtime();
        let v = validity_field(
            &mut rt,
            r#"var i = document.createElement('input'); i.setAttribute('type','date'); i.setAttribute('value','01-15-2024'); document.body.appendChild(i);"#,
            "badInput",
        );
        assert_eq!(v, "true");
    }

    #[test]
    fn date_range_underflow() {
        let mut rt = make_runtime();
        let v = validity_field(
            &mut rt,
            r#"var i = document.createElement('input'); i.setAttribute('type','date'); i.setAttribute('min','2024-06-01'); i.setAttribute('value','2024-01-15'); document.body.appendChild(i);"#,
            "rangeUnderflow",
        );
        assert_eq!(v, "true");
    }

    #[test]
    fn date_range_overflow() {
        let mut rt = make_runtime();
        let v = validity_field(
            &mut rt,
            r#"var i = document.createElement('input'); i.setAttribute('type','date'); i.setAttribute('max','2024-06-01'); i.setAttribute('value','2024-12-15'); document.body.appendChild(i);"#,
            "rangeOverflow",
        );
        assert_eq!(v, "true");
    }

    // -----------------------------------------------------------------------
    // time
    // -----------------------------------------------------------------------

    #[test]
    fn time_valid_hhmm() {
        let mut rt = make_runtime();
        let v = validity_field(
            &mut rt,
            r#"var i = document.createElement('input'); i.setAttribute('type','time'); i.setAttribute('value','14:30'); document.body.appendChild(i);"#,
            "valid",
        );
        assert_eq!(v, "true");
    }

    #[test]
    fn time_valid_hhmmss() {
        let mut rt = make_runtime();
        let v = validity_field(
            &mut rt,
            r#"var i = document.createElement('input'); i.setAttribute('type','time'); i.setAttribute('value','14:30:45'); document.body.appendChild(i);"#,
            "valid",
        );
        assert_eq!(v, "true");
    }

    #[test]
    fn time_bad_format() {
        let mut rt = make_runtime();
        let v = validity_field(
            &mut rt,
            r#"var i = document.createElement('input'); i.setAttribute('type','time'); i.setAttribute('value','2:30pm'); document.body.appendChild(i);"#,
            "badInput",
        );
        assert_eq!(v, "true");
    }

    #[test]
    fn time_range_underflow() {
        let mut rt = make_runtime();
        let v = validity_field(
            &mut rt,
            r#"var i = document.createElement('input'); i.setAttribute('type','time'); i.setAttribute('min','09:00'); i.setAttribute('value','08:00'); document.body.appendChild(i);"#,
            "rangeUnderflow",
        );
        assert_eq!(v, "true");
    }

    #[test]
    fn time_range_overflow() {
        let mut rt = make_runtime();
        let v = validity_field(
            &mut rt,
            r#"var i = document.createElement('input'); i.setAttribute('type','time'); i.setAttribute('max','17:00'); i.setAttribute('value','18:00'); document.body.appendChild(i);"#,
            "rangeOverflow",
        );
        assert_eq!(v, "true");
    }

    // -----------------------------------------------------------------------
    // datetime-local
    // -----------------------------------------------------------------------

    #[test]
    fn datetime_local_valid() {
        let mut rt = make_runtime();
        let v = validity_field(
            &mut rt,
            r#"var i = document.createElement('input'); i.setAttribute('type','datetime-local'); i.setAttribute('value','2024-01-15T14:30'); document.body.appendChild(i);"#,
            "valid",
        );
        assert_eq!(v, "true");
    }

    #[test]
    fn datetime_local_bad_format() {
        let mut rt = make_runtime();
        let v = validity_field(
            &mut rt,
            r#"var i = document.createElement('input'); i.setAttribute('type','datetime-local'); i.setAttribute('value','2024-01-15 14:30'); document.body.appendChild(i);"#,
            "badInput",
        );
        assert_eq!(v, "true");
    }

    #[test]
    fn datetime_local_range_underflow() {
        let mut rt = make_runtime();
        let v = validity_field(
            &mut rt,
            r#"var i = document.createElement('input'); i.setAttribute('type','datetime-local'); i.setAttribute('min','2024-06-01T00:00'); i.setAttribute('value','2024-01-15T14:30'); document.body.appendChild(i);"#,
            "rangeUnderflow",
        );
        assert_eq!(v, "true");
    }

    #[test]
    fn datetime_local_range_overflow() {
        let mut rt = make_runtime();
        let v = validity_field(
            &mut rt,
            r#"var i = document.createElement('input'); i.setAttribute('type','datetime-local'); i.setAttribute('max','2024-06-01T00:00'); i.setAttribute('value','2024-12-15T14:30'); document.body.appendChild(i);"#,
            "rangeOverflow",
        );
        assert_eq!(v, "true");
    }

    // -----------------------------------------------------------------------
    // month
    // -----------------------------------------------------------------------

    #[test]
    fn month_valid() {
        let mut rt = make_runtime();
        let v = validity_field(
            &mut rt,
            r#"var i = document.createElement('input'); i.setAttribute('type','month'); i.setAttribute('value','2024-01'); document.body.appendChild(i);"#,
            "valid",
        );
        assert_eq!(v, "true");
    }

    #[test]
    fn month_bad_format() {
        let mut rt = make_runtime();
        let v = validity_field(
            &mut rt,
            r#"var i = document.createElement('input'); i.setAttribute('type','month'); i.setAttribute('value','Jan 2024'); document.body.appendChild(i);"#,
            "badInput",
        );
        assert_eq!(v, "true");
    }

    #[test]
    fn month_range_underflow() {
        let mut rt = make_runtime();
        let v = validity_field(
            &mut rt,
            r#"var i = document.createElement('input'); i.setAttribute('type','month'); i.setAttribute('min','2024-06'); i.setAttribute('value','2024-01'); document.body.appendChild(i);"#,
            "rangeUnderflow",
        );
        assert_eq!(v, "true");
    }

    #[test]
    fn month_range_overflow() {
        let mut rt = make_runtime();
        let v = validity_field(
            &mut rt,
            r#"var i = document.createElement('input'); i.setAttribute('type','month'); i.setAttribute('max','2024-06'); i.setAttribute('value','2024-12'); document.body.appendChild(i);"#,
            "rangeOverflow",
        );
        assert_eq!(v, "true");
    }

    // -----------------------------------------------------------------------
    // week
    // -----------------------------------------------------------------------

    #[test]
    fn week_valid() {
        let mut rt = make_runtime();
        let v = validity_field(
            &mut rt,
            r#"var i = document.createElement('input'); i.setAttribute('type','week'); i.setAttribute('value','2024-W03'); document.body.appendChild(i);"#,
            "valid",
        );
        assert_eq!(v, "true");
    }

    #[test]
    fn week_bad_format() {
        let mut rt = make_runtime();
        let v = validity_field(
            &mut rt,
            r#"var i = document.createElement('input'); i.setAttribute('type','week'); i.setAttribute('value','2024-3'); document.body.appendChild(i);"#,
            "badInput",
        );
        assert_eq!(v, "true");
    }

    #[test]
    fn week_range_underflow() {
        let mut rt = make_runtime();
        let v = validity_field(
            &mut rt,
            r#"var i = document.createElement('input'); i.setAttribute('type','week'); i.setAttribute('min','2024-W20'); i.setAttribute('value','2024-W03'); document.body.appendChild(i);"#,
            "rangeUnderflow",
        );
        assert_eq!(v, "true");
    }

    #[test]
    fn week_range_overflow() {
        let mut rt = make_runtime();
        let v = validity_field(
            &mut rt,
            r#"var i = document.createElement('input'); i.setAttribute('type','week'); i.setAttribute('max','2024-W20'); i.setAttribute('value','2024-W40'); document.body.appendChild(i);"#,
            "rangeOverflow",
        );
        assert_eq!(v, "true");
    }

    // -----------------------------------------------------------------------
    // color
    // -----------------------------------------------------------------------

    #[test]
    fn color_valid() {
        let mut rt = make_runtime();
        let v = validity_field(
            &mut rt,
            r#"var i = document.createElement('input'); i.setAttribute('type','color'); i.setAttribute('value','#ff0000'); document.body.appendChild(i);"#,
            "valid",
        );
        assert_eq!(v, "true");
    }

    #[test]
    fn color_bad_input_short_hex() {
        let mut rt = make_runtime();
        let v = validity_field(
            &mut rt,
            r#"var i = document.createElement('input'); i.setAttribute('type','color'); i.setAttribute('value','#fff'); document.body.appendChild(i);"#,
            "badInput",
        );
        assert_eq!(v, "true");
    }

    #[test]
    fn color_bad_input_no_hash() {
        let mut rt = make_runtime();
        let v = validity_field(
            &mut rt,
            r#"var i = document.createElement('input'); i.setAttribute('type','color'); i.setAttribute('value','ff0000'); document.body.appendChild(i);"#,
            "badInput",
        );
        assert_eq!(v, "true");
    }

    #[test]
    fn color_bad_input_invalid_chars() {
        let mut rt = make_runtime();
        let v = validity_field(
            &mut rt,
            r#"var i = document.createElement('input'); i.setAttribute('type','color'); i.setAttribute('value','#gggggg'); document.body.appendChild(i);"#,
            "badInput",
        );
        assert_eq!(v, "true");
    }

    // -----------------------------------------------------------------------
    // empty values should not trigger type-specific validation
    // -----------------------------------------------------------------------

    #[test]
    fn number_empty_is_valid() {
        let mut rt = make_runtime();
        let v = validity_field(
            &mut rt,
            r#"var i = document.createElement('input'); i.setAttribute('type','number'); document.body.appendChild(i);"#,
            "valid",
        );
        assert_eq!(v, "true");
    }

    #[test]
    fn date_empty_is_valid() {
        let mut rt = make_runtime();
        let v = validity_field(
            &mut rt,
            r#"var i = document.createElement('input'); i.setAttribute('type','date'); document.body.appendChild(i);"#,
            "valid",
        );
        assert_eq!(v, "true");
    }

    use crate::Engine;

    #[test]
    fn js_value_setter_fires_input_event() {
        let mut engine = Engine::new();
        engine.load_html(r#"<html><body>
            <input id="i" type="text">
            <script>
                window.__inputFired = false;
                window.__inputBubbles = null;
                document.getElementById('i').addEventListener('input', function(e) {
                    window.__inputFired = true;
                    window.__inputBubbles = e.bubbles;
                });
            </script>
        </body></html>"#);

        engine.eval_js("document.getElementById('i').value = 'hello'").unwrap();

        let fired = engine.eval_js("window.__inputFired").unwrap();
        assert_eq!(fired, "true", "input event should fire when value is set via JS");
        let bubbles = engine.eval_js("window.__inputBubbles").unwrap();
        assert_eq!(bubbles, "true", "input event should bubble");
    }

    #[test]
    fn js_value_setter_fires_input_event_on_textarea() {
        let mut engine = Engine::new();
        engine.load_html(r#"<html><body>
            <textarea id="t"></textarea>
            <script>
                window.__inputFired = false;
                document.getElementById('t').addEventListener('input', function() {
                    window.__inputFired = true;
                });
            </script>
        </body></html>"#);

        engine.eval_js("document.getElementById('t').value = 'hello'").unwrap();

        let fired = engine.eval_js("window.__inputFired").unwrap();
        assert_eq!(fired, "true", "input event should fire on textarea value set");
    }

    #[test]
    fn invalid_event_fires_on_check_validity() {
        let mut engine = Engine::new();
        engine.load_html(r#"<html><body>
            <input id="i" type="text" required>
            <script>
                window.__invalidFired = false;
                window.__invalidBubbles = null;
                window.__invalidCancelable = null;
                document.getElementById('i').addEventListener('invalid', function(e) {
                    window.__invalidFired = true;
                    window.__invalidBubbles = e.bubbles;
                    window.__invalidCancelable = e.cancelable;
                });
            </script>
        </body></html>"#);

        // checkValidity on a required empty input should fire invalid
        let result = engine.eval_js("document.getElementById('i').checkValidity()").unwrap();
        assert_eq!(result, "false", "checkValidity should return false for empty required input");

        let fired = engine.eval_js("window.__invalidFired").unwrap();
        assert_eq!(fired, "true", "invalid event should fire when checkValidity fails");
        let bubbles = engine.eval_js("window.__invalidBubbles").unwrap();
        assert_eq!(bubbles, "false", "invalid event should NOT bubble");
        let cancelable = engine.eval_js("window.__invalidCancelable").unwrap();
        assert_eq!(cancelable, "true", "invalid event should be cancelable");
    }

    #[test]
    fn invalid_event_does_not_fire_on_valid_input() {
        let mut engine = Engine::new();
        engine.load_html(r#"<html><body>
            <input id="i" type="text" required value="filled">
            <script>
                window.__invalidFired = false;
                document.getElementById('i').addEventListener('invalid', function() {
                    window.__invalidFired = true;
                });
            </script>
        </body></html>"#);

        let result = engine.eval_js("document.getElementById('i').checkValidity()").unwrap();
        assert_eq!(result, "true", "checkValidity should return true for filled required input");

        let fired = engine.eval_js("window.__invalidFired").unwrap();
        assert_eq!(fired, "false", "invalid event should NOT fire when input is valid");
    }

    #[test]
    fn invalid_event_fires_with_custom_validity() {
        let mut engine = Engine::new();
        engine.load_html(r#"<html><body>
            <input id="i" type="text">
            <script>
                window.__invalidFired = false;
                var el = document.getElementById('i');
                el.setCustomValidity('custom error');
                el.addEventListener('invalid', function() {
                    window.__invalidFired = true;
                });
            </script>
        </body></html>"#);

        let result = engine.eval_js("document.getElementById('i').checkValidity()").unwrap();
        assert_eq!(result, "false", "checkValidity should return false with custom validity");

        let fired = engine.eval_js("window.__invalidFired").unwrap();
        assert_eq!(fired, "true", "invalid event should fire with setCustomValidity");
    }
}
