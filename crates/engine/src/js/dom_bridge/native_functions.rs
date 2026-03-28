use rquickjs::{Ctx, Function};

use crate::dom::node::NodeData;
use crate::dom::tree::DomTree;
use crate::dom::NodeId;

use super::{import_node_recursive, with_tree, with_tree_mut};

pub(super) fn register_native_functions(ctx: &Ctx<'_>) {
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

    // hasAttributes(nodeId) -> bool (any attributes at all)
    g.set("__n_hasAttributes", Function::new(ctx.clone(), |node_id: u32| -> bool {
        with_tree(|tree| tree.has_attributes(node_id as NodeId))
    }).unwrap()).unwrap();

    // setAttribute(nodeId, name, value)
    g.set("__n_setAttribute", Function::new(ctx.clone(), |node_id: u32, name: String, value: String| {
        with_tree_mut(|tree| tree.set_attribute(node_id as NodeId, &name, &value));
    }).unwrap()).unwrap();

    // removeAttribute(nodeId, name)
    g.set("__n_removeAttribute", Function::new(ctx.clone(), |node_id: u32, name: String| {
        with_tree_mut(|tree| { tree.remove_attribute(node_id as NodeId, &name); });
    }).unwrap()).unwrap();

    // setAttributeNS(nodeId, namespace, qualifiedName, value)
    g.set("__n_setAttributeNS", Function::new(ctx.clone(), |node_id: u32, namespace: String, qualified_name: String, value: String| {
        with_tree_mut(|tree| tree.set_attribute_ns(node_id as NodeId, &namespace, &qualified_name, &value));
    }).unwrap()).unwrap();

    // getAttributeNS(nodeId, namespace, localName) -> string (empty = not found)
    g.set("__n_getAttributeNS", Function::new(ctx.clone(), |node_id: u32, namespace: String, local_name: String| -> String {
        with_tree(|tree| tree.get_attribute_ns(node_id as NodeId, &namespace, &local_name).unwrap_or_default())
    }).unwrap()).unwrap();

    // hasAttributeNS(nodeId, namespace, localName) -> bool
    g.set("__n_hasAttributeNS", Function::new(ctx.clone(), |node_id: u32, namespace: String, local_name: String| -> bool {
        with_tree(|tree| tree.has_attribute_ns(node_id as NodeId, &namespace, &local_name))
    }).unwrap()).unwrap();

    // removeAttributeNS(nodeId, namespace, localName)
    g.set("__n_removeAttributeNS", Function::new(ctx.clone(), |node_id: u32, namespace: String, local_name: String| {
        with_tree_mut(|tree| { tree.remove_attribute_ns(node_id as NodeId, &namespace, &local_name); });
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

    // compareDocumentPosition(referenceId, otherId) -> u16 bitmask
    g.set("__n_compareDocumentPosition", Function::new(ctx.clone(), |reference_id: u32, other_id: u32| -> u16 {
        with_tree(|tree| {
            tree.compare_document_position(reference_id as NodeId, other_id as NodeId)
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
        let fragment_tree = crate::html::parser::parse_html_fragment(&html, "div", "");
        with_tree_mut(|tree| {
            let old_children: Vec<NodeId> = tree.get_node(parent_id as NodeId).children.clone();
            for child_id in old_children {
                tree.remove_child(parent_id as NodeId, child_id);
            }
            let frag = fragment_tree.borrow();
            // html5ever's parse_fragment creates: Document -> <html> -> actual content.
            // We need to skip the <html> wrapper and import the actual content nodes.
            let frag_doc = frag.document();
            let doc_children: Vec<NodeId> = frag.get_node(frag_doc).children.clone();
            let content_parent = doc_children.iter().find(|&&child_id| {
                matches!(
                    &frag.get_node(child_id).data,
                    NodeData::Element { tag_name, .. } if tag_name == "html"
                )
            }).copied().unwrap_or(frag_doc);
            let frag_children: Vec<NodeId> = frag.get_node(content_parent).children.clone();
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

    // createProcessingInstruction(target, data) -> nodeId
    g.set("__n_createPI", Function::new(ctx.clone(), |target: String, data: String| -> u32 {
        with_tree_mut(|tree| {
            tree.create_processing_instruction(&target, &data) as u32
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

    // charDataLength(nodeId) -> length in UTF-16 code units
    g.set("__n_charDataLength", Function::new(ctx.clone(), |node_id: u32| -> u32 {
        with_tree(|tree| {
            tree.character_data_length(node_id as NodeId) as u32
        })
    }).unwrap()).unwrap();

    // charDataAppend(nodeId, data)
    g.set("__n_charDataAppend", Function::new(ctx.clone(), |node_id: u32, data: String| {
        with_tree_mut(|tree| {
            tree.character_data_append(node_id as NodeId, &data);
        });
    }).unwrap()).unwrap();

    // charDataInsert(nodeId, offset, data) -> "" on success, error name on failure
    g.set("__n_charDataInsert", Function::new(ctx.clone(), |node_id: u32, offset: u32, data: String| -> String {
        with_tree_mut(|tree| {
            match tree.character_data_insert(node_id as NodeId, offset as usize, &data) {
                Ok(()) => String::new(),
                Err(e) => e.to_string(),
            }
        })
    }).unwrap()).unwrap();

    // charDataDelete(nodeId, offset, count) -> "" on success, error name on failure
    g.set("__n_charDataDelete", Function::new(ctx.clone(), |node_id: u32, offset: u32, count: u32| -> String {
        with_tree_mut(|tree| {
            match tree.character_data_delete(node_id as NodeId, offset as usize, count as usize) {
                Ok(()) => String::new(),
                Err(e) => e.to_string(),
            }
        })
    }).unwrap()).unwrap();

    // charDataReplace(nodeId, offset, count, data) -> "" on success, error name on failure
    g.set("__n_charDataReplace", Function::new(ctx.clone(), |node_id: u32, offset: u32, count: u32, data: String| -> String {
        with_tree_mut(|tree| {
            match tree.character_data_replace(node_id as NodeId, offset as usize, count as usize, &data) {
                Ok(()) => String::new(),
                Err(e) => e.to_string(),
            }
        })
    }).unwrap()).unwrap();

    // charDataSubstring(nodeId, offset, count) -> substring or throws
    // Returns JSON: {"ok":"result"} or {"err":"IndexSizeError"}
    g.set("__n_charDataSubstring", Function::new(ctx.clone(), |node_id: u32, offset: u32, count: u32| -> String {
        with_tree(|tree| {
            match tree.character_data_substring(node_id as NodeId, offset as usize, count as usize) {
                Ok(s) => format!("{{\"ok\":{}}}", serde_json::to_string(&s).unwrap_or_default()),
                Err(e) => format!("{{\"err\":\"{e}\"}}"),
            }
        })
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

    // validatePreInsert(parentId, nodeId, refChildId) -> "" if valid, "ErrorName:message" if invalid
    // refChildId < 0 means null (append)
    g.set("__n_validatePreInsert", Function::new(ctx.clone(), |parent_id: u32, node_id: u32, ref_child_id: i32| -> String {
        with_tree(|tree| {
            let ref_child = if ref_child_id < 0 { None } else { Some(ref_child_id as NodeId) };
            match tree.validate_pre_insert(parent_id as NodeId, node_id as NodeId, ref_child) {
                Ok(()) => String::new(),
                Err((name, msg)) => format!("{}:{}", name, msg),
            }
        })
    }).unwrap()).unwrap();

    // validatePreReplace(parentId, nodeId, oldChildId) -> "" if valid, "ErrorName:message" if invalid
    g.set("__n_validatePreReplace", Function::new(ctx.clone(), |parent_id: u32, node_id: u32, old_child_id: u32| -> String {
        with_tree(|tree| {
            match tree.validate_pre_replace(parent_id as NodeId, node_id as NodeId, old_child_id as NodeId) {
                Ok(()) => String::new(),
                Err((name, msg)) => format!("{}:{}", name, msg),
            }
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

    // __n_cssSupports(declaration) -> bool
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
    g.set("__n_findLabelControl", Function::new(ctx.clone(), |label_id: u32| -> i32 {
        with_tree(|tree| {
            let node = tree.get_node(label_id as NodeId);
            if let NodeData::Element { tag_name, .. } = &node.data {
                if !tag_name.eq_ignore_ascii_case("label") {
                    return -1;
                }
            } else {
                return -1;
            }

            if let Some(for_id) = tree.get_attribute(label_id as NodeId, "for") {
                if !for_id.is_empty() {
                    return tree.get_element_by_id(&for_id).map(|nid| nid as i32).unwrap_or(-1);
                }
            }

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
    g.set("__n_findLabelsForControl", Function::new(ctx.clone(), |control_id: u32| -> Vec<u32> {
        with_tree(|tree| {
            let node = tree.get_node(control_id as NodeId);
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
            let control_id_attr = tree.get_attribute(control_id as NodeId, "id");

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
                        if let Some(for_id) = tree.get_attribute(node_id, "for") {
                            if let Some(ref cid) = control_id_attr {
                                if !for_id.is_empty() && &for_id == cid {
                                    labels.push(node_id as u32);
                                }
                            }
                        } else {
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
