use super::node::{DomAttribute, Node, NodeData, NodeId};

#[derive(Debug)]
pub struct DomTree {
    nodes: Vec<Node>,
    is_html_document: bool,
}

impl Default for DomTree {
    fn default() -> Self {
        Self::new()
    }
}

impl DomTree {
    /// Creates a new DomTree with a Document root node at index 0.
    /// Defaults to HTML document (is_html_document = true).
    pub fn new() -> Self {
        let root = Node {
            id: 0,
            data: NodeData::Document,
            parent: None,
            children: Vec::new(),
            computed_style: None,
            template_contents: None,
        };
        DomTree {
            nodes: vec![root],
            is_html_document: true,
        }
    }

    /// Creates a new DomTree for an XML document (is_html_document = false).
    pub fn new_xml() -> Self {
        let root = Node {
            id: 0,
            data: NodeData::Document,
            parent: None,
            children: Vec::new(),
            computed_style: None,
            template_contents: None,
        };
        DomTree {
            nodes: vec![root],
            is_html_document: false,
        }
    }

    /// Returns true if this is an HTML document, false for XML documents.
    pub fn is_html_document(&self) -> bool {
        self.is_html_document
    }

    /// Allocates a new Element node (unattached) and returns its NodeId.
    pub fn create_element(&mut self, tag_name: &str) -> NodeId {
        let id = self.nodes.len();
        self.nodes.push(Node {
            id,
            data: NodeData::Element {
                tag_name: tag_name.to_string(),
                attributes: Vec::new(),
                namespace: "http://www.w3.org/1999/xhtml".to_string(),
            },
            parent: None,
            children: Vec::new(),
            computed_style: None,
            template_contents: None,
        });
        id
    }

    /// Allocates a new Text node (unattached) and returns its NodeId.
    pub fn create_text(&mut self, content: &str) -> NodeId {
        let id = self.nodes.len();
        self.nodes.push(Node {
            id,
            data: NodeData::Text {
                content: content.to_string(),
            },
            parent: None,
            children: Vec::new(),
            computed_style: None,
            template_contents: None,
        });
        id
    }

    /// Appends `child` as the last child of `parent`.
    /// If the child already has a parent, it is first removed from that parent.
    pub fn append_child(&mut self, parent: NodeId, child: NodeId) {
        // Detach from current parent if any.
        if let Some(old_parent) = self.nodes[child].parent {
            self.nodes[old_parent].children.retain(|&c| c != child);
        }
        self.nodes[child].parent = Some(parent);
        self.nodes[parent].children.push(child);
    }

    /// Removes `child` from `parent`'s children list and clears the child's parent.
    pub fn remove_child(&mut self, parent: NodeId, child: NodeId) {
        self.nodes[parent].children.retain(|&c| c != child);
        self.nodes[child].parent = None;
    }

    pub fn get_node(&self, id: NodeId) -> &Node {
        &self.nodes[id]
    }

    pub fn get_node_mut(&mut self, id: NodeId) -> &mut Node {
        &mut self.nodes[id]
    }

    /// Searches the entire tree for an Element whose "id" attribute matches `id`.
    pub fn get_element_by_id(&self, id: &str) -> Option<NodeId> {
        self.nodes.iter().find_map(|node| {
            if let NodeData::Element { ref attributes, .. } = node.data {
                if attributes.iter().any(|a| a.local_name == "id" && a.value == id) {
                    return Some(node.id);
                }
            }
            None
        })
    }

    /// Returns all Element nodes whose tag_name matches `tag` (case-insensitive).
    pub fn get_elements_by_tag_name(&self, tag: &str) -> Vec<NodeId> {
        let tag_lower = tag.to_ascii_lowercase();
        self.nodes
            .iter()
            .filter_map(|node| {
                if let NodeData::Element { ref tag_name, .. } = node.data {
                    if tag_name.to_ascii_lowercase() == tag_lower {
                        return Some(node.id);
                    }
                }
                None
            })
            .collect()
    }

    /// Returns the textContent of a node per the DOM spec:
    /// - Text / Comment: return the node's data
    /// - Element / DocumentFragment: concatenation of all descendant Text node data
    /// - Document / Doctype: return empty string (JS layer returns null)
    pub fn get_text_content(&self, node_id: NodeId) -> String {
        let node = &self.nodes[node_id];
        match &node.data {
            NodeData::Text { content } | NodeData::CDATASection { content } => content.clone(),
            NodeData::Comment { content } => content.clone(),
            NodeData::ProcessingInstruction { data, .. } => data.clone(),
            NodeData::Attr { value, .. } => value.clone(),
            NodeData::Element { .. } | NodeData::DocumentFragment => {
                let mut result = String::new();
                self.collect_descendant_text(node_id, &mut result);
                result
            }
            _ => {
                // Document / Doctype: empty string at tree level
                String::new()
            }
        }
    }

    /// Recursively collects text content from all descendant Text nodes.
    /// Per spec, only Text node data is included (not Comment or PI).
    fn collect_descendant_text(&self, node_id: NodeId, result: &mut String) {
        for &child_id in &self.nodes[node_id].children {
            match &self.nodes[child_id].data {
                NodeData::Text { content } | NodeData::CDATASection { content } => result.push_str(content),
                NodeData::Element { .. } | NodeData::DocumentFragment => {
                    self.collect_descendant_text(child_id, result);
                }
                // Skip Comment, Doctype, Document
                _ => {}
            }
        }
    }

    /// Removes all children of the node and replaces them with a single Text child.
    pub fn set_text_content(&mut self, node_id: NodeId, text: &str) {
        // Collect children to detach.
        let children: Vec<NodeId> = self.nodes[node_id].children.clone();
        for child_id in children {
            self.nodes[child_id].parent = None;
        }
        self.nodes[node_id].children.clear();

        // Create and append a new text node.
        let text_id = self.create_text(text);
        self.append_child(node_id, text_id);
    }

    /// The Document root is always at index 0.
    pub fn document(&self) -> NodeId {
        0
    }

    /// Finds the first `<body>` element in the tree.
    pub fn body(&self) -> Option<NodeId> {
        self.find_element_by_tag("body")
    }

    /// Finds the first `<head>` element in the tree.
    pub fn head(&self) -> Option<NodeId> {
        self.find_element_by_tag("head")
    }

    /// Allocates a new Comment node (unattached) and returns its NodeId.
    pub fn create_comment(&mut self, content: &str) -> NodeId {
        let id = self.nodes.len();
        self.nodes.push(Node {
            id,
            data: NodeData::Comment {
                content: content.to_string(),
            },
            parent: None,
            children: Vec::new(),
            computed_style: None,
            template_contents: None,
        });
        id
    }

    /// Allocates a new ProcessingInstruction node (unattached) and returns its NodeId.
    pub fn create_processing_instruction(&mut self, target: &str, data: &str) -> NodeId {
        let id = self.nodes.len();
        self.nodes.push(Node {
            id,
            data: NodeData::ProcessingInstruction {
                target: target.to_string(),
                data: data.to_string(),
            },
            parent: None,
            children: Vec::new(),
            computed_style: None,
            template_contents: None,
        });
        id
    }

    /// Allocates a new Attr node (unattached) and returns its NodeId.
    pub fn create_attr(&mut self, local_name: &str, namespace: &str, prefix: &str, value: &str) -> NodeId {
        let id = self.nodes.len();
        self.nodes.push(Node {
            id,
            data: NodeData::Attr {
                local_name: local_name.to_string(),
                namespace: namespace.to_string(),
                prefix: prefix.to_string(),
                value: value.to_string(),
            },
            parent: None,
            children: Vec::new(),
            computed_style: None,
            template_contents: None,
        });
        id
    }

    /// Allocates a new CDATASection node (unattached) and returns its NodeId.
    pub fn create_cdata_section(&mut self, content: &str) -> NodeId {
        let id = self.nodes.len();
        self.nodes.push(Node {
            id,
            data: NodeData::CDATASection {
                content: content.to_string(),
            },
            parent: None,
            children: Vec::new(),
            computed_style: None,
            template_contents: None,
        });
        id
    }

    /// Allocates a new DocumentFragment node (unattached) and returns its NodeId.
    pub fn create_document_fragment(&mut self) -> NodeId {
        let id = self.nodes.len();
        self.nodes.push(Node {
            id,
            data: NodeData::DocumentFragment,
            parent: None,
            children: Vec::new(),
            computed_style: None,
            template_contents: None,
        });
        id
    }

    /// Allocates a new Element node with attributes (unattached) and returns its NodeId.
    pub fn create_element_with_attrs(&mut self, tag_name: &str, attributes: Vec<DomAttribute>) -> NodeId {
        let id = self.nodes.len();
        self.nodes.push(Node {
            id,
            data: NodeData::Element {
                tag_name: tag_name.to_string(),
                attributes,
                namespace: "http://www.w3.org/1999/xhtml".to_string(),
            },
            parent: None,
            children: Vec::new(),
            computed_style: None,
            template_contents: None,
        });
        id
    }

    /// Allocates a new Doctype node (unattached) and returns its NodeId.
    pub fn create_doctype(&mut self, name: &str, public_id: &str, system_id: &str) -> NodeId {
        let id = self.nodes.len();
        self.nodes.push(Node {
            id,
            data: NodeData::Doctype {
                name: name.to_string(),
                public_id: public_id.to_string(),
                system_id: system_id.to_string(),
            },
            parent: None,
            children: Vec::new(),
            computed_style: None,
            template_contents: None,
        });
        id
    }

    /// Allocates a new Element node with namespace (unattached) and returns its NodeId.
    pub fn create_element_ns(&mut self, tag_name: &str, attributes: Vec<DomAttribute>, namespace: &str) -> NodeId {
        let id = self.nodes.len();
        self.nodes.push(Node {
            id,
            data: NodeData::Element {
                tag_name: tag_name.to_string(),
                attributes,
                namespace: namespace.to_string(),
            },
            parent: None,
            children: Vec::new(),
            computed_style: None,
            template_contents: None,
        });
        id
    }

    /// Creates a template content fragment node (Document-like container).
    /// Returns the NodeId of the new fragment.
    pub fn create_template_contents(&mut self) -> NodeId {
        let id = self.nodes.len();
        self.nodes.push(Node {
            id,
            data: NodeData::DocumentFragment, // template content is a DocumentFragment per spec
            parent: None,
            children: Vec::new(),
            computed_style: None,
            template_contents: None,
        });
        id
    }

    /// Inserts `child` as a sibling immediately before `sibling`.
    /// If `child` already has a parent, it is first detached.
    pub fn insert_before(&mut self, sibling: NodeId, child: NodeId) {
        let parent = self.nodes[sibling]
            .parent
            .expect("insert_before: sibling has no parent");

        // Detach child from current parent if any.
        if let Some(old_parent) = self.nodes[child].parent {
            self.nodes[old_parent].children.retain(|&c| c != child);
        }

        // Find sibling's position in parent's children list and insert before it.
        let pos = self.nodes[parent]
            .children
            .iter()
            .position(|&c| c == sibling)
            .expect("insert_before: sibling not found in parent's children");
        self.nodes[parent].children.insert(pos, child);
        self.nodes[child].parent = Some(parent);
    }

    /// Removes a node from its parent (if it has one).
    pub fn remove_from_parent(&mut self, target: NodeId) {
        if let Some(parent) = self.nodes[target].parent {
            self.nodes[parent].children.retain(|&c| c != target);
            self.nodes[target].parent = None;
        }
    }

    /// Moves all children of `source` to become children of `new_parent`.
    pub fn reparent_children(&mut self, source: NodeId, new_parent: NodeId) {
        let children: Vec<NodeId> = self.nodes[source].children.clone();
        self.nodes[source].children.clear();
        for child_id in children {
            self.nodes[child_id].parent = Some(new_parent);
            self.nodes[new_parent].children.push(child_id);
        }
    }

    /// Returns the total number of nodes in the tree.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// If the node is a Text node, appends `extra` to its content. Returns true if successful.
    /// If the node is a Text node, appends `extra` to its content. Returns true if successful.
    /// CDATASection nodes are NOT merged (returns false).
    pub fn append_to_text(&mut self, node_id: NodeId, extra: &str) -> bool {
        if let NodeData::Text { ref mut content } = self.nodes[node_id].data {
            content.push_str(extra);
            true
        } else {
            false
        }
    }

    /// Inserts new_child before reference_child in parent's children list.
    /// If new_child already has a parent, it is first detached.
    /// Panics if reference_child is not found in parent's children.
    pub fn insert_child_before(&mut self, parent: NodeId, new_child: NodeId, reference_child: NodeId) {
        // Detach new_child from its current parent if any.
        if let Some(old_parent) = self.nodes[new_child].parent {
            self.nodes[old_parent].children.retain(|&c| c != new_child);
        }

        // Find reference_child position in parent's children list and insert before it.
        let pos = self.nodes[parent]
            .children
            .iter()
            .position(|&c| c == reference_child)
            .expect("insert_child_before: reference_child not found in parent's children");
        self.nodes[parent].children.insert(pos, new_child);
        self.nodes[new_child].parent = Some(parent);
    }

    /// Replaces old_child with new_child in parent's children list.
    /// If new_child already has a parent, it is first detached.
    /// Clears old_child's parent. Panics if old_child is not in parent's children.
    pub fn replace_child(&mut self, parent: NodeId, new_child: NodeId, old_child: NodeId) {
        // Detach new_child from its current parent if any.
        if let Some(old_parent) = self.nodes[new_child].parent {
            self.nodes[old_parent].children.retain(|&c| c != new_child);
        }

        // Find old_child position and replace it.
        let pos = self.nodes[parent]
            .children
            .iter()
            .position(|&c| c == old_child)
            .expect("replace_child: old_child not found in parent's children");
        self.nodes[parent].children[pos] = new_child;
        self.nodes[new_child].parent = Some(parent);
        self.nodes[old_child].parent = None;
    }

    /// Clones a node and optionally all its descendants.
    /// The cloned node has no parent (unattached).
    /// If deep is true, all children are recursively cloned and attached to the clone.
    pub fn clone_node(&mut self, node_id: NodeId, deep: bool) -> NodeId {
        let data = self.nodes[node_id].data.clone();
        let new_id = self.nodes.len();
        self.nodes.push(Node {
            id: new_id,
            data,
            parent: None,
            children: Vec::new(),
            computed_style: None,
            template_contents: None,
        });

        if deep {
            let child_ids: Vec<NodeId> = self.nodes[node_id].children.clone();
            for child_id in child_ids {
                let cloned_child = self.clone_node(child_id, true);
                self.append_child(new_id, cloned_child);
            }
        }

        new_id
    }

    fn find_element_by_tag(&self, tag: &str) -> Option<NodeId> {
        let tag_lower = tag.to_ascii_lowercase();
        self.nodes.iter().find_map(|node| {
            if let NodeData::Element { ref tag_name, .. } = node.data {
                if tag_name.to_ascii_lowercase() == tag_lower {
                    return Some(node.id);
                }
            }
            None
        })
    }

    fn is_void_element(tag: &str) -> bool {
        matches!(
            tag.to_ascii_lowercase().as_str(),
            "area"
                | "base"
                | "br"
                | "col"
                | "embed"
                | "hr"
                | "img"
                | "input"
                | "link"
                | "meta"
                | "param"
                | "source"
                | "track"
                | "wbr"
        )
    }

    fn escape_html(text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        for ch in text.chars() {
            match ch {
                '&' => out.push_str("&amp;"),
                '<' => out.push_str("&lt;"),
                '>' => out.push_str("&gt;"),
                '"' => out.push_str("&quot;"),
                _ => out.push(ch),
            }
        }
        out
    }

    pub fn serialize_children_html(&self, nid: NodeId) -> String {
        let mut out = String::new();
        for &child in &self.nodes[nid].children {
            out.push_str(&self.serialize_node_html(child));
        }
        out
    }

    pub fn serialize_node_html(&self, nid: NodeId) -> String {
        let nd = &self.nodes[nid];
        match &nd.data {
            NodeData::Text { content } => Self::escape_html(content),
            NodeData::Comment { content } => format!("<!--{}-->", content),
            NodeData::Doctype { name, .. } => format!("<!DOCTYPE {}>", name),
            NodeData::Element {
                tag_name, attributes, ..
            } => {
                let mut o = String::new();
                o.push('<');
                o.push_str(tag_name);
                for a in attributes {
                    o.push(' ');
                    o.push_str(&a.qualified_name());
                    o.push_str("=\"");
                    o.push_str(&Self::escape_html(&a.value));
                    o.push('"');
                }
                o.push('>');
                if Self::is_void_element(tag_name) {
                    return o;
                }
                for &c in &nd.children {
                    o.push_str(&self.serialize_node_html(c));
                }
                o.push_str("</");
                o.push_str(tag_name);
                o.push('>');
                o
            }
            NodeData::ProcessingInstruction { target, data } => format!(
                "<?{}{}?>",
                target,
                if data.is_empty() {
                    String::new()
                } else {
                    format!(" {}", data)
                }
            ),
            NodeData::CDATASection { content } => format!("<![CDATA[{}]]>", content),
            NodeData::Attr { .. } => String::new(), // Attr nodes are not serialized as children
            NodeData::Document | NodeData::DocumentFragment => self.serialize_children_html(nid),
        }
    }

    pub fn clear_children(&mut self, nid: NodeId) {
        let children: Vec<NodeId> = self.nodes[nid].children.clone();
        for child_id in children {
            self.nodes[child_id].parent = None;
        }
        self.nodes[nid].children.clear();
    }

    pub fn import_subtree(&mut self, source: &DomTree, src_nid: NodeId) -> NodeId {
        let src_node = source.get_node(src_nid);
        let new_id = match &src_node.data {
            NodeData::Element {
                tag_name,
                attributes,
                namespace,
            } => self.create_element_ns(tag_name, attributes.clone(), namespace),
            NodeData::Text { content } => self.create_text(content),
            NodeData::Comment { content } => self.create_comment(content),
            NodeData::ProcessingInstruction { target, data } => self.create_processing_instruction(target, data),
            NodeData::Attr {
                local_name,
                namespace,
                prefix,
                value,
            } => self.create_attr(local_name, namespace, prefix, value),
            NodeData::Doctype {
                name,
                public_id,
                system_id,
            } => self.create_doctype(name, public_id, system_id),
            NodeData::CDATASection { content } => self.create_cdata_section(content),
            NodeData::Document => panic!("cannot import Document node"),
            NodeData::DocumentFragment => self.create_document_fragment(),
        };
        let src_children: Vec<NodeId> = src_node.children.clone();
        for &child_id in &src_children {
            let new_child = self.import_subtree(source, child_id);
            self.append_child(new_id, new_child);
        }
        new_id
    }

    // -----------------------------------------------------------------------
    // CharacterData interface methods
    // All offsets and counts are in UTF-16 code units.
    // -----------------------------------------------------------------------

    /// Returns the text content of a Text or Comment node, or None for other node types.
    pub fn character_data_get(&self, id: NodeId) -> Option<String> {
        match &self.nodes[id].data {
            NodeData::Text { content } | NodeData::CDATASection { content } => Some(content.clone()),
            NodeData::Comment { content } => Some(content.clone()),
            NodeData::ProcessingInstruction { data, .. } => Some(data.clone()),
            _ => None,
        }
    }

    /// Sets the text content of a Text or Comment node.
    pub fn character_data_set(&mut self, id: NodeId, new_data: &str) {
        match &mut self.nodes[id].data {
            NodeData::Text { content } | NodeData::CDATASection { content } => *content = new_data.to_string(),
            NodeData::Comment { content } => *content = new_data.to_string(),
            NodeData::ProcessingInstruction { data, .. } => *data = new_data.to_string(),
            _ => {}
        }
    }

    /// Returns the length of the CharacterData in UTF-16 code units.
    pub fn character_data_length(&self, id: NodeId) -> usize {
        match &self.nodes[id].data {
            NodeData::Text { content } | NodeData::CDATASection { content } => content.encode_utf16().count(),
            NodeData::Comment { content } => content.encode_utf16().count(),
            NodeData::ProcessingInstruction { data, .. } => data.encode_utf16().count(),
            _ => 0,
        }
    }

    /// Appends data to a Text or Comment node.
    pub fn character_data_append(&mut self, id: NodeId, append_data: &str) {
        match &mut self.nodes[id].data {
            NodeData::Text { content } | NodeData::CDATASection { content } => content.push_str(append_data),
            NodeData::Comment { content } => content.push_str(append_data),
            NodeData::ProcessingInstruction { data, .. } => data.push_str(append_data),
            _ => {}
        }
    }

    /// Deletes count UTF-16 code units starting at offset.
    /// Returns Err if offset > length.
    pub fn character_data_delete(&mut self, id: NodeId, offset: usize, count: usize) -> Result<(), &'static str> {
        let content = match &self.nodes[id].data {
            NodeData::Text { content } | NodeData::CDATASection { content } => content.clone(),
            NodeData::Comment { content } => content.clone(),
            NodeData::ProcessingInstruction { data, .. } => data.clone(),
            _ => return Ok(()),
        };
        let utf16_len = content.encode_utf16().count();
        if offset > utf16_len {
            return Err("IndexSizeError");
        }
        let end = std::cmp::min(offset + count, utf16_len);
        let new_content = Self::utf16_splice(&content, offset, end, "");
        self.character_data_set(id, &new_content);
        Ok(())
    }

    /// Inserts data at offset (in UTF-16 code units).
    /// Returns Err if offset > length.
    pub fn character_data_insert(&mut self, id: NodeId, offset: usize, data: &str) -> Result<(), &'static str> {
        let content = match &self.nodes[id].data {
            NodeData::Text { content } | NodeData::CDATASection { content } => content.clone(),
            NodeData::Comment { content } => content.clone(),
            NodeData::ProcessingInstruction { data: pi_data, .. } => pi_data.clone(),
            _ => return Ok(()),
        };
        let utf16_len = content.encode_utf16().count();
        if offset > utf16_len {
            return Err("IndexSizeError");
        }
        let new_content = Self::utf16_splice(&content, offset, offset, data);
        self.character_data_set(id, &new_content);
        Ok(())
    }

    /// Replaces count UTF-16 code units starting at offset with data.
    /// Returns Err if offset > length.
    pub fn character_data_replace(
        &mut self,
        id: NodeId,
        offset: usize,
        count: usize,
        data: &str,
    ) -> Result<(), &'static str> {
        let content = match &self.nodes[id].data {
            NodeData::Text { content } | NodeData::CDATASection { content } => content.clone(),
            NodeData::Comment { content } => content.clone(),
            NodeData::ProcessingInstruction { data: pi_data, .. } => pi_data.clone(),
            _ => return Ok(()),
        };
        let utf16_len = content.encode_utf16().count();
        if offset > utf16_len {
            return Err("IndexSizeError");
        }
        let end = std::cmp::min(offset + count, utf16_len);
        let new_content = Self::utf16_splice(&content, offset, end, data);
        self.character_data_set(id, &new_content);
        Ok(())
    }

    /// Returns a substring of count UTF-16 code units starting at offset.
    /// Returns Err if offset > length.
    pub fn character_data_substring(&self, id: NodeId, offset: usize, count: usize) -> Result<String, &'static str> {
        let content = match &self.nodes[id].data {
            NodeData::Text { content } | NodeData::CDATASection { content } => content,
            NodeData::Comment { content } => content,
            NodeData::ProcessingInstruction { data, .. } => data,
            _ => return Ok(String::new()),
        };
        let utf16_len = content.encode_utf16().count();
        if offset > utf16_len {
            return Err("IndexSizeError");
        }
        let end = std::cmp::min(offset + count, utf16_len);
        let utf16_units: Vec<u16> = content.encode_utf16().collect();
        let slice = &utf16_units[offset..end];
        Ok(String::from_utf16_lossy(slice))
    }

    /// Splices a UTF-8 string at UTF-16 code unit boundaries.
    /// Replaces UTF-16 code units in range [start..end) with `replacement`.
    fn utf16_splice(s: &str, start: usize, end: usize, replacement: &str) -> String {
        let utf16_units: Vec<u16> = s.encode_utf16().collect();
        let mut result_utf16: Vec<u16> = Vec::with_capacity(utf16_units.len() + replacement.encode_utf16().count());
        result_utf16.extend_from_slice(&utf16_units[..start]);
        result_utf16.extend(replacement.encode_utf16());
        result_utf16.extend_from_slice(&utf16_units[end..]);
        String::from_utf16_lossy(&result_utf16)
    }

    /// Converts a UTF-16 code unit offset to a byte offset in a UTF-8 string.
    /// Returns None if the offset is out of range.
    fn utf16_offset_to_byte_offset(s: &str, utf16_offset: usize) -> Option<usize> {
        let mut utf16_pos = 0;
        for (byte_pos, ch) in s.char_indices() {
            if utf16_pos == utf16_offset {
                return Some(byte_pos);
            }
            utf16_pos += ch.len_utf16();
        }
        if utf16_pos == utf16_offset {
            return Some(s.len());
        }
        None // offset out of range
    }

    /// Splits a Text node at the given offset (in UTF-16 code units).
    ///
    /// - Keeps data[..offset] in the original node
    /// - Creates a new Text node with data[offset..]
    /// - If the original node has a parent, inserts the new node as next sibling
    /// - Returns Ok(new_node_id) or Err("IndexSizeError") if offset > length
    pub fn split_text(&mut self, node_id: NodeId, utf16_offset: usize) -> Result<NodeId, &'static str> {
        // Get the text content
        let content = match &self.nodes[node_id].data {
            NodeData::Text { content } => content.clone(),
            _ => return Err("InvalidNodeTypeError"),
        };

        let utf16_len = content.encode_utf16().count();
        if utf16_offset > utf16_len {
            return Err("IndexSizeError");
        }

        let byte_offset = Self::utf16_offset_to_byte_offset(&content, utf16_offset).ok_or("IndexSizeError")?;

        // Split the data
        let kept = &content[..byte_offset];
        let split_off = &content[byte_offset..];

        // Create new Text node with the split-off portion
        let new_id = self.create_text(split_off);

        // Update the original node's data to keep only the first part
        if let NodeData::Text { ref mut content } = self.nodes[node_id].data {
            *content = kept.to_string();
        }

        // If original node has a parent, insert the new node after it
        let parent = self.nodes[node_id].parent;
        if parent.is_some() {
            self.insert_after(node_id, new_id);
        }

        Ok(new_id)
    }

    /// Returns the concatenation of all contiguous Text node siblings' data,
    /// including this node, in document order.
    ///
    /// Walks backwards from this node through previous siblings (stopping at
    /// non-Text nodes), then forwards through next siblings (stopping at non-Text
    /// nodes), collecting all text content.
    pub fn whole_text(&self, node_id: NodeId) -> String {
        // Verify this is a Text node
        if !matches!(&self.nodes[node_id].data, NodeData::Text { .. }) {
            return String::new();
        }

        let parent = self.nodes[node_id].parent;

        // If no parent, wholeText is just this node's text
        if parent.is_none() {
            return match &self.nodes[node_id].data {
                NodeData::Text { content } => content.clone(),
                _ => String::new(),
            };
        }

        let parent_id = parent.unwrap();
        let siblings = &self.nodes[parent_id].children;

        // Find position of this node in parent's children
        let pos = match siblings.iter().position(|&c| c == node_id) {
            Some(p) => p,
            None => {
                return match &self.nodes[node_id].data {
                    NodeData::Text { content } => content.clone(),
                    _ => String::new(),
                };
            }
        };

        // Walk backwards to find the start of the contiguous Text run
        let mut start = pos;
        while start > 0 {
            let prev = siblings[start - 1];
            if matches!(&self.nodes[prev].data, NodeData::Text { .. }) {
                start -= 1;
            } else {
                break;
            }
        }

        // Walk forwards to find the end of the contiguous Text run
        let mut end = pos;
        while end + 1 < siblings.len() {
            let next = siblings[end + 1];
            if matches!(&self.nodes[next].data, NodeData::Text { .. }) {
                end += 1;
            } else {
                break;
            }
        }

        // Concatenate all text content in the range [start..=end]
        let mut result = String::new();
        for sib in siblings.iter().take(end + 1).skip(start) {
            if let NodeData::Text { content } = &self.nodes[*sib].data {
                result.push_str(content);
            }
        }

        result
    }

    pub fn insert_after(&mut self, sibling: NodeId, child: NodeId) {
        let parent = self.nodes[sibling].parent.expect("insert_after: sibling has no parent");
        if let Some(old_parent) = self.nodes[child].parent {
            self.nodes[old_parent].children.retain(|&c| c != child);
        }
        let pos = self.nodes[parent]
            .children
            .iter()
            .position(|&c| c == sibling)
            .expect("insert_after: sibling not found");
        self.nodes[parent].children.insert(pos + 1, child);
        self.nodes[child].parent = Some(parent);
    }

    // -----------------------------------------------------------------------
    // Node comparison methods
    // -----------------------------------------------------------------------

    /// Returns the nodeType integer for a node.
    pub fn node_type(&self, id: NodeId) -> u16 {
        match &self.nodes[id].data {
            NodeData::Element { .. } => 1,
            NodeData::Text { .. } => 3,
            NodeData::CDATASection { .. } => 4,
            NodeData::ProcessingInstruction { .. } => 7,
            NodeData::Attr { .. } => 2,
            NodeData::Comment { .. } => 8,
            NodeData::Document => 9,
            NodeData::Doctype { .. } => 10,
            NodeData::DocumentFragment => 11,
        }
    }

    /// Implements the DOM isEqualNode algorithm.
    /// Two nodes are equal if they have the same type, same type-specific data,
    /// same number of children, and each child is recursively equal.
    pub fn is_equal_node(&self, a: NodeId, b: NodeId) -> bool {
        if a == b {
            return true;
        }
        let node_a = &self.nodes[a];
        let node_b = &self.nodes[b];

        // Must be same nodeType
        if self.node_type(a) != self.node_type(b) {
            return false;
        }

        // Compare type-specific data
        match (&node_a.data, &node_b.data) {
            (
                NodeData::Element {
                    tag_name: t1,
                    attributes: a1,
                    namespace: ns1,
                },
                NodeData::Element {
                    tag_name: t2,
                    attributes: a2,
                    namespace: ns2,
                },
            ) => {
                // Compare localName, namespace, and prefix (we store them in tag_name)
                if t1 != t2 || ns1 != ns2 {
                    return false;
                }
                // Compare attributes: same count, and each attr in a1 has a match in a2
                if a1.len() != a2.len() {
                    return false;
                }
                for attr in a1 {
                    let found = a2.iter().any(|a| {
                        a.local_name == attr.local_name && a.namespace == attr.namespace && a.value == attr.value
                    });
                    if !found {
                        return false;
                    }
                }
            }
            (
                NodeData::Doctype {
                    name: n1,
                    public_id: p1,
                    system_id: s1,
                },
                NodeData::Doctype {
                    name: n2,
                    public_id: p2,
                    system_id: s2,
                },
            ) => {
                if n1 != n2 || p1 != p2 || s1 != s2 {
                    return false;
                }
            }
            (NodeData::Text { content: c1 }, NodeData::Text { content: c2 })
            | (NodeData::CDATASection { content: c1 }, NodeData::CDATASection { content: c2 }) => {
                if c1 != c2 {
                    return false;
                }
            }
            (NodeData::Comment { content: c1 }, NodeData::Comment { content: c2 }) => {
                if c1 != c2 {
                    return false;
                }
            }
            (
                NodeData::ProcessingInstruction { target: t1, data: d1 },
                NodeData::ProcessingInstruction { target: t2, data: d2 },
            ) => {
                if t1 != t2 || d1 != d2 {
                    return false;
                }
            }
            (
                NodeData::Attr {
                    local_name: l1,
                    namespace: n1,
                    prefix: p1,
                    value: v1,
                },
                NodeData::Attr {
                    local_name: l2,
                    namespace: n2,
                    prefix: p2,
                    value: v2,
                },
            ) => {
                if l1 != l2 || n1 != n2 || p1 != p2 || v1 != v2 {
                    return false;
                }
            }
            (NodeData::Document, NodeData::Document) => {}
            (NodeData::DocumentFragment, NodeData::DocumentFragment) => {}
            _ => return false,
        }

        // Compare children recursively
        if node_a.children.len() != node_b.children.len() {
            return false;
        }
        for (child_a, child_b) in node_a.children.iter().zip(node_b.children.iter()) {
            if !self.is_equal_node(*child_a, *child_b) {
                return false;
            }
        }

        true
    }

    /// Implements the DOM Node.normalize() algorithm.
    ///
    /// For each descendant of `node_id` (in tree order):
    /// - Remove empty Text nodes
    /// - Merge adjacent Text nodes (append next sibling's data to current, remove next)
    ///
    /// Only "exclusive Text nodes" are touched (not CDATASection, Comment, etc.).
    pub fn normalize(&mut self, node_id: NodeId) {
        // We process each direct child of node_id, then recurse into non-Text children.
        // Because normalize is recursive and merging/removing changes the children list,
        // we use index-based iteration and re-read children as we go.

        let mut i = 0;
        loop {
            let children = self.nodes[node_id].children.clone();
            if i >= children.len() {
                break;
            }
            let child_id = children[i];

            if matches!(&self.nodes[child_id].data, NodeData::Text { .. }) {
                // Check if the text node is empty
                let is_empty = match &self.nodes[child_id].data {
                    NodeData::Text { content } => content.is_empty(),
                    _ => false,
                };

                if is_empty {
                    // Remove empty text node
                    self.remove_child(node_id, child_id);
                    // Don't increment i — the next child slides into this position
                    continue;
                }

                // Merge with any following adjacent Text nodes
                loop {
                    let children = self.nodes[node_id].children.clone();
                    let next_idx = children.iter().position(|&c| c == child_id).map(|pos| pos + 1);
                    let next_id = next_idx.and_then(|idx| children.get(idx).copied());

                    match next_id {
                        Some(next) if matches!(&self.nodes[next].data, NodeData::Text { .. }) => {
                            // Get next sibling's text data
                            let next_text = match &self.nodes[next].data {
                                NodeData::Text { content } => content.clone(),
                                _ => unreachable!(),
                            };
                            // Append to current text node
                            if let NodeData::Text { ref mut content } = self.nodes[child_id].data {
                                content.push_str(&next_text);
                            }
                            // Remove the next sibling
                            self.remove_child(node_id, next);
                        }
                        _ => break,
                    }
                }

                i += 1;
            } else {
                // Recurse into non-Text child
                self.normalize(child_id);
                i += 1;
            }
        }
    }

    /// Walk to the root of the tree from a given node, returning the root NodeId.
    pub fn root_of(&self, id: NodeId) -> NodeId {
        let mut current = id;
        while let Some(parent) = self.nodes[current].parent {
            current = parent;
        }
        current
    }

    /// Implements the DOM compareDocumentPosition algorithm.
    /// Returns a bitmask of position flags.
    pub fn compare_document_position(&self, reference: NodeId, other: NodeId) -> u16 {
        const DISCONNECTED: u16 = 0x01;
        const PRECEDING: u16 = 0x02;
        const FOLLOWING: u16 = 0x04;
        const CONTAINS: u16 = 0x08;
        const CONTAINED_BY: u16 = 0x10;
        const IMPLEMENTATION_SPECIFIC: u16 = 0x20;

        // Same node => 0
        if reference == other {
            return 0;
        }

        // Check if they are in the same tree
        let root_ref = self.root_of(reference);
        let root_other = self.root_of(other);

        if root_ref != root_other {
            // Different trees -- disconnected. Use NodeId for consistent ordering.
            let dir = if other < reference { PRECEDING } else { FOLLOWING };
            return DISCONNECTED | IMPLEMENTATION_SPECIFIC | dir;
        }

        // Check if other is an ancestor of reference (other contains reference)
        {
            let mut current = reference;
            while let Some(parent) = self.nodes[current].parent {
                if parent == other {
                    // other is an ancestor of reference
                    return CONTAINS | PRECEDING;
                }
                current = parent;
            }
        }

        // Check if other is a descendant of reference (reference contains other)
        {
            let mut current = other;
            while let Some(parent) = self.nodes[current].parent {
                if parent == reference {
                    // other is a descendant of reference
                    return CONTAINED_BY | FOLLOWING;
                }
                current = parent;
            }
        }

        // Neither ancestor nor descendant -- determine document order
        if self.is_preceding(other, reference) {
            PRECEDING
        } else {
            FOLLOWING
        }
    }

    /// Returns true if `a` precedes `b` in tree order (depth-first pre-order).
    fn is_preceding(&self, a: NodeId, b: NodeId) -> bool {
        // Build ancestor chains for both nodes
        let mut chain_a = Vec::new();
        let mut current = a;
        chain_a.push(current);
        while let Some(parent) = self.nodes[current].parent {
            chain_a.push(parent);
            current = parent;
        }
        chain_a.reverse();

        let mut chain_b = Vec::new();
        current = b;
        chain_b.push(current);
        while let Some(parent) = self.nodes[current].parent {
            chain_b.push(parent);
            current = parent;
        }
        chain_b.reverse();

        // Find the first point of divergence
        let min_len = chain_a.len().min(chain_b.len());
        for i in 0..min_len {
            if chain_a[i] != chain_b[i] {
                // Find which comes first among the children of the common parent
                let common_parent = if i > 0 {
                    chain_a[i - 1]
                } else {
                    return a < b;
                };
                let parent_children = &self.nodes[common_parent].children;
                let pos_a = parent_children.iter().position(|&c| c == chain_a[i]);
                let pos_b = parent_children.iter().position(|&c| c == chain_b[i]);
                return pos_a < pos_b;
            }
        }

        // If one is a prefix of the other, the shorter chain is the ancestor
        chain_a.len() < chain_b.len()
    }

    // -----------------------------------------------------------------------
    // Namespace lookup algorithms (DOM spec §4.4.8)
    // -----------------------------------------------------------------------

    const XMLNS_NS: &str = "http://www.w3.org/2000/xmlns/";
    const XML_NS: &str = "http://www.w3.org/XML/1998/namespace";

    /// Implements the "locate a namespace" algorithm from the DOM spec.
    /// `prefix` is `None` for null/empty prefix.
    /// Returns `Some(namespace_uri)` or `None` for null.
    pub fn locate_namespace(&self, node_id: NodeId, prefix: Option<&str>) -> Option<String> {
        let node = self.get_node(node_id);
        match &node.data {
            NodeData::Element {
                tag_name,
                namespace,
                attributes,
            } => self.locate_namespace_for_element(node_id, tag_name, namespace, attributes, prefix),
            NodeData::Document => {
                // Delegate to document element if it exists
                for &child_id in &node.children {
                    if matches!(self.get_node(child_id).data, NodeData::Element { .. }) {
                        return self.locate_namespace(child_id, prefix);
                    }
                }
                None
            }
            NodeData::Doctype { .. } | NodeData::DocumentFragment => None,
            NodeData::Attr { .. } => {
                // Attr nodes: delegate to ownerElement.
                // In this implementation, Attr nodes don't track their ownerElement,
                // so we return null (None) for disconnected Attrs.
                // If the Attr has a parent (which currently doesn't happen), delegate to it.
                if let Some(parent_id) = node.parent {
                    if matches!(self.get_node(parent_id).data, NodeData::Element { .. }) {
                        return self.locate_namespace(parent_id, prefix);
                    }
                }
                None
            }
            NodeData::Text { .. }
            | NodeData::CDATASection { .. }
            | NodeData::Comment { .. }
            | NodeData::ProcessingInstruction { .. } => {
                // Delegate to parent element
                if let Some(parent_id) = node.parent {
                    if matches!(self.get_node(parent_id).data, NodeData::Element { .. }) {
                        return self.locate_namespace(parent_id, prefix);
                    }
                }
                None
            }
        }
    }

    /// Core "locate a namespace" for Element nodes.
    fn locate_namespace_for_element(
        &self,
        node_id: NodeId,
        tag_name: &str,
        namespace: &str,
        attributes: &[DomAttribute],
        prefix: Option<&str>,
    ) -> Option<String> {
        // Step 1: If element's namespace is not empty and element's prefix matches, return namespace
        if !namespace.is_empty() {
            let elem_prefix = tag_name.find(':').map(|pos| &tag_name[..pos]);
            if elem_prefix == prefix {
                return Some(namespace.to_string());
            }
        }

        // Step 2: Check xmlns attributes
        for attr in attributes {
            if attr.namespace != Self::XMLNS_NS {
                continue;
            }
            if let Some(pfx) = prefix {
                // Looking for xmlns:pfx attribute
                if attr.prefix == "xmlns" && attr.local_name == pfx {
                    return if attr.value.is_empty() {
                        None
                    } else {
                        Some(attr.value.clone())
                    };
                }
            } else {
                // Looking for xmlns attribute (no prefix) for null/empty prefix lookup
                if attr.prefix.is_empty() && attr.local_name == "xmlns" {
                    return if attr.value.is_empty() {
                        None
                    } else {
                        Some(attr.value.clone())
                    };
                }
            }
        }

        // Step 3: Built-in xml prefix
        if prefix == Some("xml") {
            return Some(Self::XML_NS.to_string());
        }

        // Step 4: Built-in xmlns prefix
        if prefix == Some("xmlns") {
            return Some(Self::XMLNS_NS.to_string());
        }

        // Step 5: Recurse to parent element
        let node = self.get_node(node_id);
        if let Some(parent_id) = node.parent {
            if matches!(self.get_node(parent_id).data, NodeData::Element { .. }) {
                return self.locate_namespace(parent_id, prefix);
            }
        }

        // Step 6: Not found
        None
    }

    /// Implements the "locate a namespace prefix" algorithm from the DOM spec.
    /// `namespace` is the namespace URI to look up.
    /// Returns `Some(prefix)` or `None`.
    pub fn locate_prefix(&self, node_id: NodeId, namespace: &str) -> Option<String> {
        let node = self.get_node(node_id);
        match &node.data {
            NodeData::Element {
                tag_name,
                namespace: elem_ns,
                attributes,
            } => self.locate_prefix_for_element(node_id, tag_name, elem_ns, attributes, namespace),
            NodeData::Document => {
                // Delegate to document element if it exists
                for &child_id in &node.children {
                    if matches!(self.get_node(child_id).data, NodeData::Element { .. }) {
                        return self.locate_prefix(child_id, namespace);
                    }
                }
                None
            }
            NodeData::Doctype { .. } | NodeData::DocumentFragment => None,
            NodeData::Attr { .. } => {
                // Delegate to ownerElement if known (currently always None)
                if let Some(parent_id) = node.parent {
                    if matches!(self.get_node(parent_id).data, NodeData::Element { .. }) {
                        return self.locate_prefix(parent_id, namespace);
                    }
                }
                None
            }
            NodeData::Text { .. }
            | NodeData::CDATASection { .. }
            | NodeData::Comment { .. }
            | NodeData::ProcessingInstruction { .. } => {
                // Delegate to parent element
                if let Some(parent_id) = node.parent {
                    if matches!(self.get_node(parent_id).data, NodeData::Element { .. }) {
                        return self.locate_prefix(parent_id, namespace);
                    }
                }
                None
            }
        }
    }

    /// Core "locate a namespace prefix" for Element nodes.
    fn locate_prefix_for_element(
        &self,
        node_id: NodeId,
        tag_name: &str,
        elem_ns: &str,
        attributes: &[DomAttribute],
        namespace: &str,
    ) -> Option<String> {
        // Step 1: If element's namespace matches and element has a prefix, return the prefix
        if !elem_ns.is_empty() && elem_ns == namespace {
            if let Some(pos) = tag_name.find(':') {
                let pfx = &tag_name[..pos];
                if !pfx.is_empty() {
                    return Some(pfx.to_string());
                }
            }
        }

        // Step 2: Check xmlns:* attributes whose value matches the namespace
        for attr in attributes {
            if attr.namespace == Self::XMLNS_NS && attr.prefix == "xmlns" && attr.value == namespace {
                return Some(attr.local_name.clone());
            }
        }

        // Step 3: Recurse to parent element
        let node = self.get_node(node_id);
        if let Some(parent_id) = node.parent {
            if matches!(self.get_node(parent_id).data, NodeData::Element { .. }) {
                return self.locate_prefix(parent_id, namespace);
            }
        }

        // Step 4: Not found
        None
    }
}

/// Check whether a character is an XML NameStartChar per the XML spec:
/// https://www.w3.org/TR/xml/#NT-NameStartChar
fn is_name_start_char(c: char) -> bool {
    matches!(c,
        ':' | 'A'..='Z' | '_' | 'a'..='z'
        | '\u{C0}'..='\u{D6}' | '\u{D8}'..='\u{F6}' | '\u{F8}'..='\u{2FF}'
        | '\u{370}'..='\u{37D}' | '\u{37F}'..='\u{1FFF}'
        | '\u{200C}'..='\u{200D}' | '\u{2070}'..='\u{218F}'
        | '\u{2C00}'..='\u{2FEF}' | '\u{3001}'..='\u{D7FF}'
        | '\u{F900}'..='\u{FDCF}' | '\u{FDF0}'..='\u{FFFD}'
        | '\u{10000}'..='\u{EFFFF}'
    )
}

/// Check whether a character is an XML NameChar per the XML spec:
/// https://www.w3.org/TR/xml/#NT-NameChar
fn is_name_char(c: char) -> bool {
    is_name_start_char(c)
        || matches!(c,
            '-' | '.' | '0'..='9' | '\u{B7}'
            | '\u{0300}'..='\u{036F}' | '\u{203F}'..='\u{2040}'
        )
}

/// Lenient start-character check matching actual browser behavior for DOM APIs.
/// Browsers accept any non-ASCII character (>= U+0080) as a valid name start
/// character, plus ASCII letters, `_`, and `:`. This is broader than the strict
/// XML NameStartChar production, which excludes certain Unicode ranges (e.g.
/// U+037E, U+0300, U+FFFF). Colon is included because local parts of QNames
/// can start with `:` (e.g. `f::oo`); the QName colon splitting is handled
/// separately in validate_and_extract.
fn is_lenient_name_start_char(c: char) -> bool {
    matches!(c, ':' | 'A'..='Z' | '_' | 'a'..='z') || c as u32 >= 0x80
}

/// Lenient name validation matching actual browser behavior for DOM APIs like
/// createElementNS and createDocument. Checks that the first character passes
/// the lenient NameStartChar check (ASCII letter, `_`, or any char >= U+0080)
/// and that the name contains no whitespace or '>' chars. Subsequent characters
/// are otherwise completely unchecked. This contradicts the spec (which says to
/// validate against the full QName production) but it's what every browser does,
/// and what WPT tests demand. C'est la vie.
///
/// NOTE: '>' is rejected in ALL positions (not just first). Browsers treat '>'
/// as invalid anywhere in a DOM name, unlike most other non-NameChar characters
/// which are leniently accepted in non-first positions.
pub fn is_valid_dom_name(name: &str) -> bool {
    name.chars().next().is_some_and(is_lenient_name_start_char)
        && !name.contains(char::is_whitespace)
        && !name.contains('>')
}

/// Validates whether a string is a valid element name per the HTML spec.
/// Rules match the WPT name-validation test regex:
///   /^(?:[A-Za-z][^\0\t\n\f\r\u0020/>]*|[:_\u0080-\u{10FFFF}][A-Za-z0-9-.:_\u0080-\u{10FFFF}]*)$/u
///
/// Two cases:
/// 1. ASCII alpha start → subsequent chars must not be: \0, \t, \n, \x0C, \r, space, /, >
/// 2. :, _, or >= U+0080 start → subsequent chars only: A-Za-z0-9, -, ., :, _, >= U+0080
///
/// Everything else (empty, digit start, other control chars) is invalid.
pub fn is_valid_element_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        None => false,
        Some(first) if first.is_ascii_alphabetic() => {
            // ASCII alpha start: reject \0, whitespace subset, /, >
            chars.all(|c| !matches!(c, '\0' | '\t' | '\n' | '\x0C' | '\r' | ' ' | '/' | '>'))
        }
        Some(first) if first == ':' || first == '_' || first as u32 >= 0x80 => {
            // :, _, or non-ASCII start: strict subsequent char set
            chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | ':' | '_') || c as u32 >= 0x80)
        }
        _ => false,
    }
}

/// Validates whether a string is a valid attribute name per the HTML spec.
/// Invalid chars: empty, \0, ASCII whitespace (\t, \n, \x0C, \r, space), /, >, =
pub fn is_valid_attribute_name(name: &str) -> bool {
    !name.is_empty() && !name.contains(['\0', '\t', '\n', '\x0C', '\r', ' ', '/', '>', '='])
}

/// Validates whether a string is a valid doctype name.
/// Invalid chars: empty allowed, \0, ASCII whitespace (\t, \n, \x0C, \r, space), >
pub fn is_valid_doctype_name(name: &str) -> bool {
    !name.contains(['\0', '\t', '\n', '\x0C', '\r', ' ', '>'])
}

/// Validates whether a string is a valid XML Name per the XML spec.
/// Used by createProcessingInstruction and other DOM APIs that require valid XML names.
pub fn is_valid_xml_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        None => false,
        Some(first) => is_name_start_char(first) && chars.all(is_name_char),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_tree_has_document_root() {
        let tree = DomTree::new();
        let root = tree.get_node(tree.document());
        assert!(matches!(root.data, NodeData::Document));
        assert_eq!(root.id, 0);
        assert!(root.parent.is_none());
        assert!(root.children.is_empty());
    }

    #[test]
    fn create_and_append_children() {
        let mut tree = DomTree::new();
        let html = tree.create_element("html");
        let body = tree.create_element("body");
        let p = tree.create_element("p");

        tree.append_child(tree.document(), html);
        tree.append_child(html, body);
        tree.append_child(body, p);

        // Verify parent links
        assert_eq!(tree.get_node(html).parent, Some(0));
        assert_eq!(tree.get_node(body).parent, Some(html));
        assert_eq!(tree.get_node(p).parent, Some(body));

        // Verify children lists
        assert_eq!(tree.get_node(tree.document()).children, vec![html]);
        assert_eq!(tree.get_node(html).children, vec![body]);
        assert_eq!(tree.get_node(body).children, vec![p]);
    }

    #[test]
    fn append_child_detaches_from_old_parent() {
        let mut tree = DomTree::new();
        let div1 = tree.create_element("div");
        let div2 = tree.create_element("div");
        let span = tree.create_element("span");

        tree.append_child(tree.document(), div1);
        tree.append_child(tree.document(), div2);
        tree.append_child(div1, span);

        assert_eq!(tree.get_node(span).parent, Some(div1));
        assert_eq!(tree.get_node(div1).children, vec![span]);

        // Move span from div1 to div2
        tree.append_child(div2, span);

        assert_eq!(tree.get_node(span).parent, Some(div2));
        assert_eq!(tree.get_node(div2).children, vec![span]);
        assert!(tree.get_node(div1).children.is_empty());
    }

    #[test]
    fn remove_child_clears_relationship() {
        let mut tree = DomTree::new();
        let div = tree.create_element("div");
        let span = tree.create_element("span");

        tree.append_child(tree.document(), div);
        tree.append_child(div, span);

        assert_eq!(tree.get_node(div).children, vec![span]);
        assert_eq!(tree.get_node(span).parent, Some(div));

        tree.remove_child(div, span);

        assert!(tree.get_node(div).children.is_empty());
        assert!(tree.get_node(span).parent.is_none());
    }

    #[test]
    fn get_element_by_id_finds_matching_attribute() {
        let mut tree = DomTree::new();
        let div = tree.create_element("div");
        tree.append_child(tree.document(), div);

        // Add an "id" attribute
        if let NodeData::Element { ref mut attributes, .. } = tree.get_node_mut(div).data {
            attributes.push(DomAttribute::new("id", "main"));
        }

        assert_eq!(tree.get_element_by_id("main"), Some(div));
        assert_eq!(tree.get_element_by_id("nonexistent"), None);
    }

    #[test]
    fn get_elements_by_tag_name_is_case_insensitive() {
        let mut tree = DomTree::new();
        let div1 = tree.create_element("div");
        let div2 = tree.create_element("DIV");
        let span = tree.create_element("span");

        tree.append_child(tree.document(), div1);
        tree.append_child(tree.document(), div2);
        tree.append_child(tree.document(), span);

        let divs = tree.get_elements_by_tag_name("div");
        assert_eq!(divs, vec![div1, div2]);

        let spans = tree.get_elements_by_tag_name("SPAN");
        assert_eq!(spans, vec![span]);
    }

    #[test]
    fn get_text_content_collects_recursively() {
        let mut tree = DomTree::new();
        let div = tree.create_element("div");
        let t1 = tree.create_text("Hello, ");
        let span = tree.create_element("span");
        let t2 = tree.create_text("world");
        let t3 = tree.create_text("!");

        tree.append_child(tree.document(), div);
        tree.append_child(div, t1);
        tree.append_child(div, span);
        tree.append_child(span, t2);
        tree.append_child(div, t3);

        assert_eq!(tree.get_text_content(div), "Hello, world!");
        assert_eq!(tree.get_text_content(span), "world");
    }

    #[test]
    fn set_text_content_replaces_children() {
        let mut tree = DomTree::new();
        let div = tree.create_element("div");
        let span = tree.create_element("span");
        let t1 = tree.create_text("old text");

        tree.append_child(tree.document(), div);
        tree.append_child(div, span);
        tree.append_child(span, t1);

        tree.set_text_content(div, "new text");

        assert_eq!(tree.get_text_content(div), "new text");
        // The old span should be detached
        assert!(tree.get_node(span).parent.is_none());
        // div should have exactly one child (the new text node)
        assert_eq!(tree.get_node(div).children.len(), 1);
    }

    #[test]
    fn body_and_head_find_elements() {
        let mut tree = DomTree::new();
        let html = tree.create_element("html");
        let head = tree.create_element("head");
        let body = tree.create_element("body");

        tree.append_child(tree.document(), html);
        tree.append_child(html, head);
        tree.append_child(html, body);

        assert_eq!(tree.head(), Some(head));
        assert_eq!(tree.body(), Some(body));
    }

    #[test]
    fn body_and_head_return_none_when_absent() {
        let tree = DomTree::new();
        assert_eq!(tree.head(), None);
        assert_eq!(tree.body(), None);
    }

    #[test]
    fn insert_child_before_inserts_at_correct_position() {
        let mut tree = DomTree::new();
        let div = tree.create_element("div");
        let a = tree.create_element("a");
        let b = tree.create_element("b");
        let c = tree.create_element("c");

        tree.append_child(tree.document(), div);
        tree.append_child(div, a);
        tree.append_child(div, c);

        tree.insert_child_before(div, b, c);

        assert_eq!(tree.get_node(div).children, vec![a, b, c]);
        assert_eq!(tree.get_node(b).parent, Some(div));
    }

    #[test]
    fn insert_child_before_detaches_from_old_parent() {
        let mut tree = DomTree::new();
        let div1 = tree.create_element("div");
        let div2 = tree.create_element("div");
        let a = tree.create_element("a");
        let b = tree.create_element("b");

        tree.append_child(tree.document(), div1);
        tree.append_child(tree.document(), div2);
        tree.append_child(div1, a);
        tree.append_child(div2, b);

        tree.insert_child_before(div2, a, b);

        assert!(tree.get_node(div1).children.is_empty());
        assert_eq!(tree.get_node(div2).children, vec![a, b]);
        assert_eq!(tree.get_node(a).parent, Some(div2));
    }

    #[test]
    fn replace_child_swaps_correctly() {
        let mut tree = DomTree::new();
        let div = tree.create_element("div");
        let old = tree.create_element("old");
        let new_el = tree.create_element("new");

        tree.append_child(tree.document(), div);
        tree.append_child(div, old);

        tree.replace_child(div, new_el, old);

        assert_eq!(tree.get_node(div).children, vec![new_el]);
        assert_eq!(tree.get_node(new_el).parent, Some(div));
        assert!(tree.get_node(old).parent.is_none());
    }

    #[test]
    fn replace_child_detaches_new_child_from_old_parent() {
        let mut tree = DomTree::new();
        let div1 = tree.create_element("div");
        let div2 = tree.create_element("div");
        let old_child = tree.create_element("old");
        let new_child = tree.create_element("new");

        tree.append_child(tree.document(), div1);
        tree.append_child(tree.document(), div2);
        tree.append_child(div1, new_child);
        tree.append_child(div2, old_child);

        tree.replace_child(div2, new_child, old_child);

        assert!(tree.get_node(div1).children.is_empty());
        assert_eq!(tree.get_node(div2).children, vec![new_child]);
        assert_eq!(tree.get_node(new_child).parent, Some(div2));
        assert!(tree.get_node(old_child).parent.is_none());
    }

    #[test]
    fn clone_node_shallow_no_children() {
        let mut tree = DomTree::new();
        let div = tree.create_element_with_attrs("div", vec![DomAttribute::new("class", "container")]);
        let span = tree.create_element("span");
        tree.append_child(div, span);

        let cloned = tree.clone_node(div, false);

        assert_ne!(cloned, div);
        assert!(tree.get_node(cloned).children.is_empty());
        assert!(tree.get_node(cloned).parent.is_none());
        match &tree.get_node(cloned).data {
            NodeData::Element {
                tag_name, attributes, ..
            } => {
                assert_eq!(tag_name, "div");
                assert_eq!(attributes, &vec![DomAttribute::new("class", "container")]);
            }
            _ => panic!("expected Element"),
        }
    }

    #[test]
    fn clone_node_deep_clones_descendants() {
        let mut tree = DomTree::new();
        let div = tree.create_element("div");
        let span = tree.create_element("span");
        let text = tree.create_text("hello");

        tree.append_child(div, span);
        tree.append_child(span, text);

        let cloned = tree.clone_node(div, true);

        assert_eq!(tree.get_node(cloned).children.len(), 1);
        let cloned_span = tree.get_node(cloned).children[0];
        assert_ne!(cloned_span, span);
        assert_eq!(tree.get_node(cloned_span).children.len(), 1);
        let cloned_text = tree.get_node(cloned_span).children[0];
        assert_ne!(cloned_text, text);
        assert_eq!(tree.get_text_content(cloned), "hello");
        assert!(tree.get_node(cloned).parent.is_none());
        assert_eq!(tree.get_node(cloned_span).parent, Some(cloned));
        assert_eq!(tree.get_node(cloned_text).parent, Some(cloned_span));
    }

    #[test]
    fn clone_node_preserves_attributes() {
        let mut tree = DomTree::new();
        let div = tree.create_element_with_attrs(
            "div",
            vec![
                DomAttribute::new("id", "main"),
                DomAttribute::new("class", "container"),
                DomAttribute::new("data-x", "42"),
            ],
        );

        let cloned = tree.clone_node(div, false);

        match &tree.get_node(cloned).data {
            NodeData::Element {
                tag_name, attributes, ..
            } => {
                assert_eq!(tag_name, "div");
                assert_eq!(attributes.len(), 3);
                assert!(attributes.contains(&DomAttribute::new("id", "main")));
                assert!(attributes.contains(&DomAttribute::new("class", "container")));
                assert!(attributes.contains(&DomAttribute::new("data-x", "42")));
            }
            _ => panic!("expected Element"),
        }
    }

    #[test]
    fn clone_node_text_node() {
        let mut tree = DomTree::new();
        let text = tree.create_text("hello world");

        let cloned = tree.clone_node(text, false);

        assert_ne!(cloned, text);
        match &tree.get_node(cloned).data {
            NodeData::Text { content } => assert_eq!(content, "hello world"),
            _ => panic!("expected Text"),
        }
        assert!(tree.get_node(cloned).parent.is_none());
    }
}
