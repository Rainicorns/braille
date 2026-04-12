use rquickjs::{Ctx, Function};

use crate::dom::node::{DomString, NodeData};
use crate::dom::NodeId;

use super::{with_tree, with_tree_mut};

pub(super) fn register_native_tree_ops(ctx: &Ctx<'_>) {
    let g = ctx.globals();

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

    // getLocalName(nodeId) -> string (raw tag_name, no case conversion)
    g.set("__n_getLocalName", Function::new(ctx.clone(), |node_id: u32| -> String {
        with_tree(|tree| {
            let node = tree.get_node(node_id as NodeId);
            match &node.data {
                NodeData::Element { tag_name, .. } => tag_name.clone(),
                _ => String::new(),
            }
        })
    }).unwrap()).unwrap();

    // getNamespace(nodeId) -> namespace URI string (empty for non-elements)
    g.set("__n_getNamespace", Function::new(ctx.clone(), |node_id: u32| -> String {
        with_tree(|tree| {
            let node = tree.get_node(node_id as NodeId);
            match &node.data {
                NodeData::Element { namespace, .. } => namespace.clone(),
                _ => String::new(),
            }
        })
    }).unwrap()).unwrap();

    // getPrefix(nodeId) -> prefix string (empty for no prefix)
    g.set("__n_getPrefix", Function::new(ctx.clone(), |node_id: u32| -> String {
        with_tree(|tree| {
            let node = tree.get_node(node_id as NodeId);
            match &node.data {
                NodeData::Element { prefix, .. } => prefix.clone().unwrap_or_default(),
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
                NodeData::ShadowRoot { .. } => 11,
                NodeData::ProcessingInstruction { .. } => 7,
                NodeData::CDATASection { .. } => 4,
                NodeData::Attr { .. } => 2,
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
    g.set("__n_getElementById", Function::new(ctx.clone(), |id: DomString| -> i32 {
        with_tree(|tree| {
            tree.get_element_by_id(&id).map(|nid| nid as i32).unwrap_or(-1)
        })
    }).unwrap()).unwrap();

    // querySelector(rootNodeId, selector, scopeNodeId) -> nodeId or -1
    g.set("__n_querySelector", Function::new(ctx.clone(), |root_id: u32, selector: DomString, scope_id: u32| -> i32 {
        with_tree(|tree| {
            crate::css::matching::query_selector(tree, root_id as NodeId, &selector, Some(scope_id as NodeId))
                .map(|nid| nid as i32)
                .unwrap_or(-1)
        })
    }).unwrap()).unwrap();

    // querySelectorAll(rootNodeId, selector, scopeNodeId) -> array of nodeIds
    g.set("__n_querySelectorAll", Function::new(ctx.clone(), |root_id: u32, selector: DomString, scope_id: u32| -> Vec<u32> {
        with_tree(|tree| {
            crate::css::matching::query_selector_all(tree, root_id as NodeId, &selector, Some(scope_id as NodeId))
                .into_iter()
                .map(|nid| nid as u32)
                .collect()
        })
    }).unwrap()).unwrap();

    // hasAttrValue(nodeId, name) -> bool (has the attribute at all?)
    g.set("__n_hasAttrValue", Function::new(ctx.clone(), |node_id: u32, name: DomString| -> bool {
        with_tree(|tree| tree.get_attribute(node_id as NodeId, &name).is_some())
    }).unwrap()).unwrap();

    // createElement(tagName) -> nodeId
    g.set("__n_createElement", Function::new(ctx.clone(), |tag: DomString| -> u32 {
        with_tree_mut(|tree| {
            tree.create_element(&tag.to_ascii_lowercase()) as u32
        })
    }).unwrap()).unwrap();

    // createElementNS(localName, namespace, prefix) -> nodeId
    // Stores raw localName (no lowercasing), namespace, and optional prefix in the DomTree.
    g.set("__n_createElementNS", Function::new(ctx.clone(), |local_name: DomString, namespace: DomString, prefix: DomString| -> u32 {
        with_tree_mut(|tree| {
            let pfx = if prefix.is_empty() { None } else { Some(prefix.as_str()) };
            tree.create_element_ns_with_prefix(&local_name, Vec::new(), &namespace, pfx) as u32
        })
    }).unwrap()).unwrap();

    // createTextNode(text) -> nodeId
    g.set("__n_createTextNode", Function::new(ctx.clone(), |text: DomString| -> u32 {
        with_tree_mut(|tree| {
            tree.create_text(&text) as u32
        })
    }).unwrap()).unwrap();

    // appendChild(parentId, childId) -> insertionIndex
    g.set("__n_appendChild", Function::new(ctx.clone(), |parent_id: u32, child_id: u32| -> u32 {
        with_tree_mut(|tree| {
            tree.append_child(parent_id as NodeId, child_id as NodeId) as u32
        })
    }).unwrap()).unwrap();

    // removeChild(parentId, childId)
    g.set("__n_removeChild", Function::new(ctx.clone(), |parent_id: u32, child_id: u32| {
        with_tree_mut(|tree| {
            tree.remove_child(parent_id as NodeId, child_id as NodeId);
        });
    }).unwrap()).unwrap();

    // insertBefore(parentId, newChildId, refChildId) — refChildId -1 means append. Returns insertion index.
    g.set("__n_insertBefore", Function::new(ctx.clone(), |parent_id: u32, new_child_id: u32, ref_child_id: i32| -> u32 {
        with_tree_mut(|tree| {
            if ref_child_id < 0 {
                tree.append_child(parent_id as NodeId, new_child_id as NodeId) as u32
            } else {
                tree.insert_before(ref_child_id as NodeId, new_child_id as NodeId) as u32
            }
        })
    }).unwrap()).unwrap();

    // removeAllChildren(nodeId) — removes all children of a node
    g.set("__n_removeAllChildren", Function::new(ctx.clone(), |node_id: u32| {
        with_tree_mut(|tree| {
            tree.remove_all_children(node_id as NodeId);
        });
    }).unwrap()).unwrap();

    // setTextContent(nodeId, text) — removes all children and sets text
    g.set("__n_setTextContent", Function::new(ctx.clone(), |node_id: u32, text: DomString| {
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

    // compareDocumentPosition(referenceId, otherId) -> u16 bitmask
    g.set("__n_compareDocumentPosition", Function::new(ctx.clone(), |reference_id: u32, other_id: u32| -> u16 {
        with_tree(|tree| {
            tree.compare_document_position(reference_id as NodeId, other_id as NodeId)
        })
    }).unwrap()).unwrap();

    // closest(nodeId, selector) -> nodeId or -1
    g.set("__n_closest", Function::new(ctx.clone(), |node_id: u32, selector: DomString| -> i32 {
        with_tree(|tree| {
            let mut current = Some(node_id as NodeId);
            while let Some(id) = current {
                if matches!(tree.get_node(id).data, NodeData::Element { .. })
                    && crate::css::matching::matches_selector_str(tree, id, &selector, Some(node_id as NodeId))
                {
                    return id as i32;
                }
                current = tree.get_node(id).parent;
            }
            -1
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
}
