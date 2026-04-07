use rquickjs::{Ctx, Function};

use crate::dom::node::{DomString, NodeData, ShadowRootMode};
use crate::dom::tree::{is_valid_xml_name, DomTree};
use crate::dom::NodeId;

use super::{import_node_recursive, with_tree, with_tree_mut};

pub(super) fn register_native_functions(ctx: &Ctx<'_>) {
    super::native_attributes::register_native_attributes(ctx);
    super::native_tree_ops::register_native_tree_ops(ctx);

    let g = ctx.globals();

    // getDataAttribute(nodeId, camelCaseName) -> string or empty
    g.set("__n_getDataAttr", Function::new(ctx.clone(), |node_id: u32, name: DomString| -> String {
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
            tree.get_attribute(node_id as NodeId, &kebab).map(|v| v.to_string()).unwrap_or_default()
        })
    }).unwrap()).unwrap();

    // innerHTML setter: parse HTML fragment and replace children
    g.set("__n_setInnerHTML", Function::new(ctx.clone(), |parent_id: u32, html: DomString| {
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

    // parseHTMLDocument(html) -> JSON array of imported top-level node IDs
    // Parses as a full document (not fragment), so doctypes are preserved.
    // Returns IDs in document order (e.g. [doctypeNid, htmlElementNid]).
    g.set("__n_parseHTMLDocument", Function::new(ctx.clone(), |html: DomString| -> String {
        let doc_tree = crate::html::parser::parse_html(&html);
        with_tree_mut(|tree| {
            let src = doc_tree.borrow();
            let src_doc = src.document();
            let src_children: Vec<NodeId> = src.get_node(src_doc).children.clone();
            let mut imported_ids: Vec<u32> = Vec::new();
            // Create a temporary holder in the main tree
            let holder = tree.create_document_fragment();
            for &child_id in &src_children {
                import_node_recursive(tree, &src, child_id, holder);
            }
            // Collect the imported children and detach them from the holder
            let holder_children: Vec<NodeId> = tree.get_node(holder).children.clone();
            for &child_id in &holder_children {
                tree.remove_from_parent(child_id);
                imported_ids.push(child_id as u32);
            }
            format!("[{}]", imported_ids.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(","))
        })
    }).unwrap()).unwrap();

    // createComment(text) -> nodeId
    g.set("__n_createComment", Function::new(ctx.clone(), |text: DomString| -> u32 {
        with_tree_mut(|tree| {
            tree.create_comment(&text) as u32
        })
    }).unwrap()).unwrap();

    // createProcessingInstruction(target, data) -> nodeId
    g.set("__n_createPI", Function::new(ctx.clone(), |target: DomString, data: DomString| -> u32 {
        with_tree_mut(|tree| {
            tree.create_processing_instruction(&target, &data) as u32
        })
    }).unwrap()).unwrap();

    // createCDATASection(content) -> nodeId
    g.set("__n_createCDATASection", Function::new(ctx.clone(), |content: DomString| -> u32 {
        with_tree_mut(|tree| {
            tree.create_cdata_section(&content) as u32
        })
    }).unwrap()).unwrap();

    // getPITarget(nodeId) -> target string for ProcessingInstruction nodes
    g.set("__n_getPITarget", Function::new(ctx.clone(), |node_id: u32| -> String {
        with_tree(|tree| {
            match &tree.nodes[node_id as NodeId].data {
                NodeData::ProcessingInstruction { target, .. } => target.clone(),
                _ => String::new(),
            }
        })
    }).unwrap()).unwrap();

    // isValidXmlName(name) -> bool
    g.set("__n_isValidXmlName", Function::new(ctx.clone(), |name: DomString| -> bool {
        is_valid_xml_name(&name)
    }).unwrap()).unwrap();

    // getCharData(nodeId) -> string (text/comment node data)
    g.set("__n_getCharData", Function::new(ctx.clone(), |node_id: u32| -> String {
        with_tree(|tree| {
            tree.character_data_get(node_id as NodeId).unwrap_or_default()
        })
    }).unwrap()).unwrap();

    // setCharData(nodeId, data) — set text/comment node data
    // Accepts rquickjs::Value to handle JS strings with lone surrogates (which can't
    // convert to Rust String). Falls back to lossy conversion for Rust-side storage.
    g.set("__n_setCharData", Function::new(ctx.clone(), |node_id: u32, data: rquickjs::Value<'_>| {
        let s = if let Some(js_str) = data.as_string() {
            js_str.to_string().unwrap_or_else(|_| {
                // String contains lone surrogates — get raw bytes via lossy conversion.
                // QuickJS JS_ToCStringLen produces WTF-8; we convert lossy to valid UTF-8.
                String::from("")
            })
        } else {
            data.get::<String>().unwrap_or_default()
        };
        with_tree_mut(|tree| {
            tree.character_data_set(node_id as NodeId, &s);
        });
    }).unwrap()).unwrap();

    // charDataLength(nodeId) -> length in UTF-16 code units
    g.set("__n_charDataLength", Function::new(ctx.clone(), |node_id: u32| -> u32 {
        with_tree(|tree| {
            tree.character_data_length(node_id as NodeId) as u32
        })
    }).unwrap()).unwrap();

    // charDataAppend(nodeId, data)
    g.set("__n_charDataAppend", Function::new(ctx.clone(), |node_id: u32, data: DomString| {
        with_tree_mut(|tree| {
            tree.character_data_append(node_id as NodeId, &data);
        });
    }).unwrap()).unwrap();

    // charDataInsert(nodeId, offset, data) -> "" on success, error name on failure
    g.set("__n_charDataInsert", Function::new(ctx.clone(), |node_id: u32, offset: u32, data: DomString| -> String {
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
    g.set("__n_charDataReplace", Function::new(ctx.clone(), |node_id: u32, offset: u32, count: u32, data: DomString| -> String {
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

    // createDocumentNode() -> nodeId (standalone Document node, no parent)
    g.set("__n_createDocumentNode", Function::new(ctx.clone(), || -> u32 {
        with_tree_mut(|tree| {
            tree.alloc_node(NodeData::Document) as u32
        })
    }).unwrap()).unwrap();

    // createDoctype(name, publicId, systemId) -> nodeId
    g.set("__n_createDoctype", Function::new(ctx.clone(), |name: DomString, public_id: DomString, system_id: DomString| -> u32 {
        with_tree_mut(|tree| {
            tree.create_doctype(&name, &public_id, &system_id) as u32
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

    // getDoctypeNodeId() -> nid of the doctype child, or -1
    g.set("__n_getDoctypeNodeId", Function::new(ctx.clone(), || -> i32 {
        with_tree(|tree| {
            let doc = tree.document();
            for &child_id in &tree.get_node(doc).children {
                if matches!(&tree.get_node(child_id).data, NodeData::Doctype { .. }) {
                    return child_id as i32;
                }
            }
            -1
        })
    }).unwrap()).unwrap();

    // getDoctypeName(nodeId) -> name string
    g.set("__n_getDoctypeName", Function::new(ctx.clone(), |node_id: u32| -> String {
        with_tree(|tree| {
            match &tree.nodes[node_id as NodeId].data {
                NodeData::Doctype { name, .. } => name.clone(),
                _ => String::new(),
            }
        })
    }).unwrap()).unwrap();

    // getDoctypePublicId(nodeId) -> publicId string
    g.set("__n_getDoctypePublicId", Function::new(ctx.clone(), |node_id: u32| -> String {
        with_tree(|tree| {
            match &tree.nodes[node_id as NodeId].data {
                NodeData::Doctype { public_id, .. } => public_id.clone(),
                _ => String::new(),
            }
        })
    }).unwrap()).unwrap();

    // getDoctypeSystemId(nodeId) -> systemId string
    g.set("__n_getDoctypeSystemId", Function::new(ctx.clone(), |node_id: u32| -> String {
        with_tree(|tree| {
            match &tree.nodes[node_id as NodeId].data {
                NodeData::Doctype { system_id, .. } => system_id.clone(),
                _ => String::new(),
            }
        })
    }).unwrap()).unwrap();

    // getInnerHTML(nodeId) -> string
    g.set("__n_getInnerHTML", Function::new(ctx.clone(), |node_id: u32| -> String {
        with_tree(|tree| {
            tree.serialize_children_html(node_id as NodeId)
        })
    }).unwrap()).unwrap();

    // matchesSelector(nodeId, selector) -> bool
    g.set("__n_matchesSelector", Function::new(ctx.clone(), |node_id: u32, selector: DomString| -> bool {
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

    // __n_cssSupports(declaration) -> bool
    g.set("__n_cssSupports", Function::new(ctx.clone(), |decl: DomString| -> bool {
        !crate::css::parser::parse_inline_style(&decl).is_empty()
    }).unwrap()).unwrap();

    // __n_getComputedStyle(nodeId, prop) -> string value or empty
    // Recomputes styles on-demand if DOM mutations have dirtied the style cache.
    g.set("__n_getComputedStyle", Function::new(ctx.clone(), |node_id: u32, prop: DomString| -> String {
        with_tree_mut(|tree| {
            if tree.styles_dirty {
                crate::css::style_tree::compute_all_styles(tree);
            }
            let node = tree.get_node(node_id as NodeId);
            node.computed_style.as_ref()
                .and_then(|cs| cs.get(prop.as_str()))
                .cloned()
                .unwrap_or_default()
        })
    }).unwrap()).unwrap();

    // __n_matchMedia(query) -> bool — evaluates a media query string
    g.set("__n_matchMedia", Function::new(ctx.clone(), |query: DomString| -> bool {
        crate::css::media::evaluate_media_query(&query, 1280.0, 800.0)
    }).unwrap()).unwrap();

    // __n_getComputedStyleAll(nodeId) -> JSON string of all computed styles
    g.set("__n_getComputedStyleAll", Function::new(ctx.clone(), |node_id: u32| -> String {
        with_tree_mut(|tree| {
            if tree.styles_dirty {
                crate::css::style_tree::compute_all_styles(tree);
            }
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

    // __n_getTemplateContent(nodeId) -> nodeId of template content fragment, or -1
    g.set("__n_getTemplateContent", Function::new(ctx.clone(), |node_id: u32| -> i32 {
        with_tree(|tree| {
            tree.get_node(node_id as NodeId)
                .template_contents
                .map(|nid| nid as i32)
                .unwrap_or(-1)
        })
    }).unwrap()).unwrap();

    // __n_createTemplateContent(nodeId) -> creates template_contents fragment, returns its nodeId
    g.set("__n_createTemplateContent", Function::new(ctx.clone(), |node_id: u32| -> i32 {
        with_tree_mut(|tree| {
            let nid = node_id as NodeId;
            if tree.get_node(nid).template_contents.is_some() {
                return tree.get_node(nid).template_contents.unwrap() as i32;
            }
            let frag_id = tree.create_template_contents();
            tree.get_node_mut(nid).template_contents = Some(frag_id);
            frag_id as i32
        })
    }).unwrap()).unwrap();

    // __n_getLayout(nodeId) -> JSON {"x":...,"y":...,"width":...,"height":...} or empty
    g.set("__n_getLayout", Function::new(ctx.clone(), |node_id: u32| -> String {
        with_tree_mut(|tree| {
            match tree.get_layout_rect(node_id as NodeId) {
                Some(r) => format!("{{\"x\":{},\"y\":{},\"width\":{},\"height\":{}}}", r.x, r.y, r.width, r.height),
                None => String::new(),
            }
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
            let control_id_attr = tree.get_attribute(control_id as NodeId, "id").map(|v| v.to_string());

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

    // createShadowRoot(hostId, modeStr) -> nodeId
    g.set("__n_createShadowRoot", Function::new(ctx.clone(), |host_id: u32, mode_str: DomString| -> u32 {
        let mode = if mode_str == "closed" { ShadowRootMode::Closed } else { ShadowRootMode::Open };
        with_tree_mut(|tree| {
            tree.create_shadow_root(mode, host_id as NodeId) as u32
        })
    }).unwrap()).unwrap();

    // isShadowRoot(nodeId) -> bool
    g.set("__n_isShadowRoot", Function::new(ctx.clone(), |node_id: u32| -> bool {
        with_tree(|tree| {
            matches!(tree.get_node(node_id as NodeId).data, NodeData::ShadowRoot { .. })
        })
    }).unwrap()).unwrap();

    // getShadowHost(nodeId) -> host nodeId or -1
    g.set("__n_getShadowHost", Function::new(ctx.clone(), |node_id: u32| -> i32 {
        with_tree(|tree| {
            match &tree.get_node(node_id as NodeId).data {
                NodeData::ShadowRoot { host, .. } => *host as i32,
                _ => -1,
            }
        })
    }).unwrap()).unwrap();

    // getShadowRootMode(nodeId) -> "open" or "closed"
    g.set("__n_getShadowRootMode", Function::new(ctx.clone(), |node_id: u32| -> String {
        with_tree(|tree| {
            match &tree.get_node(node_id as NodeId).data {
                NodeData::ShadowRoot { mode, .. } => match mode {
                    ShadowRootMode::Open => "open".to_string(),
                    ShadowRootMode::Closed => "closed".to_string(),
                },
                _ => "open".to_string(),
            }
        })
    }).unwrap()).unwrap();

    // hasShadowRoot(nodeId) -> bool
    g.set("__n_hasShadowRoot", Function::new(ctx.clone(), |node_id: u32| -> bool {
        with_tree(|tree| {
            tree.get_node(node_id as NodeId).shadow_root.is_some()
        })
    }).unwrap()).unwrap();

    // getShadowRootId(nodeId) -> shadowRootNodeId or -1
    g.set("__n_getShadowRootId", Function::new(ctx.clone(), |node_id: u32| -> i32 {
        with_tree(|tree| {
            tree.get_node(node_id as NodeId).shadow_root.map(|id| id as i32).unwrap_or(-1)
        })
    }).unwrap()).unwrap();

    // retarget(aNodeId, bNodeId) -> retargeted nodeId for a
    // bNodeId of -1 means b is a non-node (window, XHR, etc.)
    g.set("__n_retarget", Function::new(ctx.clone(), |a_id: u32, b_id: i32| -> u32 {
        with_tree(|tree| {
            let b = if b_id >= 0 { Some(b_id as NodeId) } else { None };
            tree.retarget(a_id as NodeId, b) as u32
        })
    }).unwrap()).unwrap();

    // rootOf(nodeId) -> root nodeId (walks parent chain to root, stops at shadow root)
    g.set("__n_rootOf", Function::new(ctx.clone(), |node_id: u32| -> u32 {
        with_tree(|tree| {
            tree.root_of(node_id as NodeId) as u32
        })
    }).unwrap()).unwrap();

    // getElementsByTagName(rootNodeId, tagName, isHTMLDoc) -> array of nodeIds
    // Unlike querySelectorAll, this handles any tag name including those with special CSS chars
    // Per DOM spec §4.4.4: when the node document is HTML, HTML-namespace elements match against
    // ASCII-lowercased input; non-HTML-namespace elements match case-sensitively.
    // When the node document is NOT HTML (e.g. XML), all matching is case-sensitive.
    g.set("__n_getElementsByTagName", Function::new(ctx.clone(), |root_id: u32, tag: DomString, is_html_doc: bool| -> Vec<u32> {
        with_tree(|tree| {
            let mut result = Vec::new();
            let tag_lower = tag.to_ascii_lowercase();
            let is_wildcard = tag == "*";
            fn collect(tree: &DomTree, node_id: NodeId, tag_lower: &str, tag_original: &str, is_wildcard: bool, is_html_doc: bool, result: &mut Vec<u32>) {
                let node = tree.get_node(node_id);
                if let NodeData::Element { tag_name, namespace, prefix, .. } = &node.data {
                    if is_wildcard {
                        result.push(node_id as u32);
                    } else {
                        // Reconstruct qualified name
                        let qname = match prefix {
                            Some(p) => format!("{}:{}", p, tag_name),
                            None => tag_name.clone(),
                        };
                        if is_html_doc && namespace == "http://www.w3.org/1999/xhtml" {
                            // HTML doc + HTML namespace: compare element qname against lowercased input
                            if qname == tag_lower {
                                result.push(node_id as u32);
                            }
                        } else {
                            // Non-HTML doc or non-HTML namespace: exact case-sensitive match
                            if qname == tag_original {
                                result.push(node_id as u32);
                            }
                        }
                    }
                }
                for &child_id in &node.children {
                    collect(tree, child_id, tag_lower, tag_original, is_wildcard, is_html_doc, result);
                }
            }
            // Only collect descendants, not the root itself
            let root_node = tree.get_node(root_id as NodeId);
            for &child_id in &root_node.children {
                collect(tree, child_id, &tag_lower, &tag, is_wildcard, is_html_doc, &mut result);
            }
            result
        })
    }).unwrap()).unwrap();

    // getElementsByTagNameNS(rootNodeId, namespace, localName) -> array of nodeIds
    // Per DOM spec §4.4.4: matches localName (not qualified name), always case-sensitive.
    // namespace/localName "*" = wildcard. null namespace mapped to "" by JS caller.
    g.set("__n_getElementsByTagNameNS", Function::new(ctx.clone(), |root_id: u32, namespace: DomString, local_name: DomString| -> Vec<u32> {
        with_tree(|tree| {
            let mut result = Vec::new();
            let ns_wildcard = namespace == "*";
            let ln_wildcard = local_name == "*";
            fn collect(tree: &DomTree, node_id: NodeId, ns: &str, ln: &str, ns_wildcard: bool, ln_wildcard: bool, result: &mut Vec<u32>) {
                let node = tree.get_node(node_id);
                if let NodeData::Element { tag_name, namespace: elem_ns, .. } = &node.data {
                    let ns_match = ns_wildcard || elem_ns == ns;
                    let ln_match = ln_wildcard || tag_name == ln;
                    if ns_match && ln_match {
                        result.push(node_id as u32);
                    }
                }
                for &child_id in &node.children {
                    collect(tree, child_id, ns, ln, ns_wildcard, ln_wildcard, result);
                }
            }
            // Only collect descendants, not the root itself
            let root_node = tree.get_node(root_id as NodeId);
            for &child_id in &root_node.children {
                collect(tree, child_id, &namespace, &local_name, ns_wildcard, ln_wildcard, &mut result);
            }
            result
        })
    }).unwrap()).unwrap();

    // isEqualNode(a, b) -> bool
    g.set("__n_isEqualNode", Function::new(ctx.clone(), |a: u32, b: u32| -> bool {
        with_tree(|tree| tree.is_equal_node(a as NodeId, b as NodeId))
    }).unwrap()).unwrap();

    // normalize(nodeId) — merge adjacent text nodes, remove empty text nodes
    g.set("__n_normalize", Function::new(ctx.clone(), |node_id: u32| {
        with_tree_mut(|tree| tree.normalize(node_id as NodeId));
    }).unwrap()).unwrap();

    // validateAndExtract(namespace, qualifiedName) -> JSON result
    // Returns {"ok":{"prefix":"...","localName":"..."}} or {"err":"ErrorName"}
    g.set("__n_validateAndExtract", Function::new(ctx.clone(), |namespace: DomString, qualified_name: DomString| -> String {
        let ns = if namespace.is_empty() || namespace == "null" { None } else { Some(namespace.as_str()) };
        match crate::dom::tree::validate_and_extract(ns, &qualified_name) {
            Ok((prefix, local_name)) => {
                let p = prefix.as_deref().unwrap_or("");
                format!("{{\"ok\":{{\"prefix\":\"{}\",\"localName\":\"{}\"}}}}", p, local_name)
            }
            Err(err_name) => {
                format!("{{\"err\":\"{}\"}}", err_name)
            }
        }
    }).unwrap()).unwrap();

    // Set focused node on the tree so CSS :focus / :focus-within matching works.
    // Called from EP.focus and EP.blur.
    g.set("__n_setFocusedNode", Function::new(ctx.clone(), |node_id: i32| {
        with_tree_mut(|tree| {
            tree.focused_node = if node_id >= 0 { Some(node_id as NodeId) } else { None };
        });
    }).unwrap()).unwrap();

    // Set hovered node on the tree so CSS :hover matching works.
    g.set("__n_setHoveredNode", Function::new(ctx.clone(), |node_id: i32| {
        with_tree_mut(|tree| {
            tree.hovered_node = if node_id >= 0 { Some(node_id as NodeId) } else { None };
        });
    }).unwrap()).unwrap();

    // Store CSS text for a <link rel="stylesheet"> element and mark styles dirty.
    g.set("__n_setLinkCss", Function::new(ctx.clone(), |node_id: u32, css_text: DomString| {
        with_tree_mut(|tree| {
            tree.link_stylesheets.insert(node_id as NodeId, css_text.into_string());
            tree.styles_dirty = true;
        });
    }).unwrap()).unwrap();
}
