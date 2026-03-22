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
        var _listeners = {};  // key: nodeId + ":" + eventType -> array of callbacks
        var _winListeners = {};  // window event listeners

        // Element prototype
        var EP = {};
        EP.getAttribute = function(name) {
            var v = __n_getAttribute(this.__nid, name);
            return __n_hasAttrValue(this.__nid, name) ? v : null;
        };
        EP.setAttribute = function(name, value) { __n_setAttribute(this.__nid, name, String(value)); };
        EP.removeAttribute = function(name) { __n_removeAttribute(this.__nid, name); };
        EP.hasAttribute = function(name) { return __n_hasAttribute(this.__nid, name); };

        EP.addEventListener = function(type, cb, opts) {
            if (typeof cb !== 'function') return;
            var key = this.__nid + ':' + type;
            if (!_listeners[key]) _listeners[key] = [];
            _listeners[key].push(cb);
        };
        EP.removeEventListener = function(type, cb) {
            var key = this.__nid + ':' + type;
            if (_listeners[key]) {
                _listeners[key] = _listeners[key].filter(function(f) { return f !== cb; });
            }
        };
        EP.dispatchEvent = function(event) {
            __dispatch(this.__nid, event);
            return !event.defaultPrevented;
        };
        EP.click = function() {
            var event = new MouseEvent('click', {bubbles: true, cancelable: true});
            event.target = this;
            event.currentTarget = this;
            __dispatch(this.__nid, event);
        };
        EP.querySelector = function(sel) {
            var id = __n_querySelector(this.__nid, sel);
            return id >= 0 ? __w(id) : null;
        };
        EP.querySelectorAll = function(sel) {
            return __n_querySelectorAll(this.__nid, sel).map(__w);
        };
        EP.getElementsByTagName = function(tag) { return this.querySelectorAll(tag); };
        EP.getElementsByClassName = function(cls) { return this.querySelectorAll('.' + cls); };
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
        EP.getBoundingClientRect = function() { return {top:0,left:0,width:0,height:0,right:0,bottom:0}; };
        EP.focus = function() {};
        EP.blur = function() {};
        EP.scrollIntoView = function() {};
        EP.matches = function(sel) { return __n_matchesSelector(this.__nid, sel); };
        EP.closest = function(sel) {
            var id = __n_closest(this.__nid, sel);
            return id >= 0 ? __w(id) : null;
        };
        EP.getAttributeNames = function() { return []; };

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
                get: function() { return this.getAttribute('value') || ''; },
                set: function(v) { this.setAttribute('value', v); },
                configurable: true
            },
            checked: {
                get: function() { return this.hasAttribute('checked'); },
                set: function(v) { if(v) this.setAttribute('checked',''); else this.removeAttribute('checked'); },
                configurable: true
            },
            disabled: {
                get: function() { return this.hasAttribute('disabled'); },
                set: function(v) { if(v) this.setAttribute('disabled',''); else this.removeAttribute('disabled'); },
                configurable: true
            },
            type: {
                get: function() { return this.getAttribute('type') || ''; },
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
                    if (!this._s) this._s = new Proxy({}, { set: function(t,p,v){t[p]=v;return true;}, get: function(t,p){return t[p]||'';} });
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
            ownerDocument: { get: function() { return document; }, configurable: true },
        });

        // Wrapper factory
        function __w(nodeId) {
            if (_cache[nodeId]) return _cache[nodeId];
            var obj = Object.create(EP);
            obj.__nid = nodeId;
            _cache[nodeId] = obj;
            return obj;
        }
        globalThis.__braille_get_element_wrapper = __w;

        // Event dispatch with bubbling through DOM, then document, then window
        function __dispatch(nodeId, event) {
            // Build path: node -> parent -> ... -> root
            var path = [];
            var cur = nodeId;
            while (cur >= 0) {
                path.push(cur);
                cur = __n_getParent(cur);
            }

            // Set target
            event.target = __w(nodeId);

            // Bubble through DOM elements
            for (var i = 0; i < path.length; i++) {
                var nid = path[i];
                event.currentTarget = __w(nid);
                var key = nid + ':' + event.type;
                var cbs = _listeners[key];
                if (cbs) {
                    var snapshot = cbs.slice();
                    for (var j = 0; j < snapshot.length; j++) {
                        snapshot[j].call(event.currentTarget, event);
                        if (event._stopImmediate) return;
                    }
                }
                if (event._stopPropagation || !event.bubbles) break;
            }

            // Bubble to document listeners
            if (!event._stopPropagation && event.bubbles) {
                event.currentTarget = document;
                var docCbs = doc.__listeners[event.type];
                if (docCbs) {
                    var snapshot = docCbs.slice();
                    for (var j = 0; j < snapshot.length; j++) {
                        snapshot[j].call(document, event);
                        if (event._stopImmediate) return;
                    }
                }
            }

            // Bubble to window listeners
            if (!event._stopPropagation && event.bubbles) {
                event.currentTarget = window;
                var winCbs = _winListeners[event.type];
                if (winCbs) {
                    var snapshot = winCbs.slice();
                    for (var j = 0; j < snapshot.length; j++) {
                        snapshot[j].call(window, event);
                        if (event._stopImmediate) return;
                    }
                }
            }
        }

        // __braille_click(nodeId) — called from Rust
        globalThis.__braille_click = function(nodeId) {
            var el = __w(nodeId);
            el.click();
        };

        // Element mutation methods that operate on the real DomTree
        EP.appendChild = function(child) {
            if (child && child.__nid !== undefined && this.__nid !== undefined) {
                if (child.nodeType === 11) {
                    // DocumentFragment: transfer all children
                    var kids = __n_getAllChildIds(child.__nid);
                    for (var i = 0; i < kids.length; i++) {
                        __n_appendChild(this.__nid, kids[i]);
                    }
                } else {
                    __n_appendChild(this.__nid, child.__nid);
                }
            }
            return child;
        };
        EP.removeChild = function(child) {
            if (child && child.__nid !== undefined && this.__nid !== undefined) {
                __n_removeChild(this.__nid, child.__nid);
            }
            return child;
        };
        EP.insertBefore = function(newChild, refChild) {
            if (newChild && newChild.__nid !== undefined && this.__nid !== undefined) {
                var refId = (refChild && refChild.__nid !== undefined) ? refChild.__nid : -1;
                if (newChild.nodeType === 11) {
                    // DocumentFragment: transfer all children
                    var kids = __n_getAllChildIds(newChild.__nid);
                    for (var i = 0; i < kids.length; i++) {
                        __n_insertBefore(this.__nid, kids[i], refId);
                    }
                } else {
                    __n_insertBefore(this.__nid, newChild.__nid, refId);
                }
            }
            return newChild;
        };

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
        doc.getElementsByTagName = function(tag) { return doc.querySelectorAll(tag); };
        doc.getElementsByClassName = function(cls) { return doc.querySelectorAll('.' + cls); };
        doc.addEventListener = function(type, cb) {
            if (!doc.__listeners[type]) doc.__listeners[type] = [];
            doc.__listeners[type].push(cb);
        };
        doc.removeEventListener = function(type, cb) {
            if (doc.__listeners[type]) doc.__listeners[type] = doc.__listeners[type].filter(function(f){return f!==cb;});
        };

        doc.createComment = function(text) {
            var nid = __n_createComment(text || '');
            return __w(nid);
        };

        // window.addEventListener / removeEventListener
        window.addEventListener = function(type, cb) {
            if (typeof cb !== 'function') return;
            if (!_winListeners[type]) _winListeners[type] = [];
            _winListeners[type].push(cb);
        };
        window.removeEventListener = function(type, cb) {
            if (_winListeners[type]) {
                _winListeners[type] = _winListeners[type].filter(function(f){return f!==cb;});
            }
        };

        Object.defineProperties(doc, {
            body: { get: function() { return doc.querySelector('body'); }, configurable: true },
            head: { get: function() { return doc.querySelector('head'); }, configurable: true },
            documentElement: { get: function() { return doc.querySelector('html'); }, configurable: true },
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
