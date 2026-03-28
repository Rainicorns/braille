use crate::dom::node::{DomAttribute, NodeData, NodeId, ShadowRootMode};

use super::DomTree;

impl DomTree {
    /// Allocates a new Element node (unattached) and returns its NodeId.
    pub fn create_element(&mut self, tag_name: &str) -> NodeId {
        self.alloc_node(NodeData::Element {
            tag_name: tag_name.to_string(),
            attributes: Vec::new(),
            namespace: "http://www.w3.org/1999/xhtml".to_string(),
        })
    }

    /// Allocates a new Text node (unattached) and returns its NodeId.
    pub fn create_text(&mut self, content: &str) -> NodeId {
        self.alloc_node(NodeData::Text {
            content: content.to_string(),
        })
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

    /// Allocates a new Comment node (unattached) and returns its NodeId.
    pub fn create_comment(&mut self, content: &str) -> NodeId {
        self.alloc_node(NodeData::Comment {
            content: content.to_string(),
        })
    }

    /// Allocates a new ProcessingInstruction node (unattached) and returns its NodeId.
    pub fn create_processing_instruction(&mut self, target: &str, data: &str) -> NodeId {
        self.alloc_node(NodeData::ProcessingInstruction {
            target: target.to_string(),
            data: data.to_string(),
        })
    }

    /// Allocates a new Attr node (unattached) and returns its NodeId.
    pub fn create_attr(&mut self, local_name: &str, namespace: &str, prefix: &str, value: &str) -> NodeId {
        self.alloc_node(NodeData::Attr {
            local_name: local_name.to_string(),
            namespace: namespace.to_string(),
            prefix: prefix.to_string(),
            value: value.to_string(),
        })
    }

    /// Allocates a new CDATASection node (unattached) and returns its NodeId.
    pub fn create_cdata_section(&mut self, content: &str) -> NodeId {
        self.alloc_node(NodeData::CDATASection {
            content: content.to_string(),
        })
    }

    /// Allocates a new DocumentFragment node (unattached) and returns its NodeId.
    pub fn create_document_fragment(&mut self) -> NodeId {
        self.alloc_node(NodeData::DocumentFragment)
    }

    /// Allocates a new ShadowRoot node for the given host element.
    /// The ShadowRoot is NOT a child of the host — it is referenced only via `Node.shadow_root`.
    /// The ShadowRoot's parent is None (it is a separate tree root).
    pub fn create_shadow_root(&mut self, mode: ShadowRootMode, host: NodeId) -> NodeId {
        let id = self.alloc_node(NodeData::ShadowRoot { mode, host });
        self.nodes[host].shadow_root = Some(id);
        id
    }

    /// Allocates a new Element node with attributes (unattached) and returns its NodeId.
    pub fn create_element_with_attrs(&mut self, tag_name: &str, attributes: Vec<DomAttribute>) -> NodeId {
        self.alloc_node(NodeData::Element {
            tag_name: tag_name.to_string(),
            attributes,
            namespace: "http://www.w3.org/1999/xhtml".to_string(),
        })
    }

    /// Allocates a new Doctype node (unattached) and returns its NodeId.
    pub fn create_doctype(&mut self, name: &str, public_id: &str, system_id: &str) -> NodeId {
        self.alloc_node(NodeData::Doctype {
            name: name.to_string(),
            public_id: public_id.to_string(),
            system_id: system_id.to_string(),
        })
    }

    /// Allocates a new Element node with namespace (unattached) and returns its NodeId.
    pub fn create_element_ns(&mut self, tag_name: &str, attributes: Vec<DomAttribute>, namespace: &str) -> NodeId {
        self.alloc_node(NodeData::Element {
            tag_name: tag_name.to_string(),
            attributes,
            namespace: namespace.to_string(),
        })
    }

    /// Creates a template content fragment node (Document-like container).
    /// Returns the NodeId of the new fragment.
    pub fn create_template_contents(&mut self) -> NodeId {
        self.alloc_node(NodeData::DocumentFragment) // template content is a DocumentFragment per spec
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
        let pos = self
            .find_child_index(parent, sibling)
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
        // Clone needed: loop mutates source.children (cleared) and new_parent.children (pushed)
        let children: Vec<NodeId> = self.nodes[source].children.clone();
        self.nodes[source].children.clear();
        for child_id in children {
            self.nodes[child_id].parent = Some(new_parent);
            self.nodes[new_parent].children.push(child_id);
        }
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
        let pos = self
            .find_child_index(parent, reference_child)
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
        let pos = self
            .find_child_index(parent, old_child)
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
        let new_id = self.alloc_node(data);

        if deep {
            // Iterative deep clone using explicit stack of (src_id, dst_parent_id)
            let mut stack: Vec<(NodeId, NodeId)> = self.nodes[node_id]
                .children
                .iter()
                .rev()
                .map(|&c| (c, new_id))
                .collect();
            while let Some((src_id, dst_parent)) = stack.pop() {
                let child_data = self.nodes[src_id].data.clone();
                let cloned_id = self.alloc_node(child_data);
                self.append_child(dst_parent, cloned_id);
                for &grandchild in self.nodes[src_id].children.iter().rev() {
                    stack.push((grandchild, cloned_id));
                }
            }
        }

        new_id
    }

    pub fn clear_children(&mut self, nid: NodeId) {
        // Clone needed: loop mutates self.nodes[child_id] while iterating nid's children
        let children: Vec<NodeId> = self.nodes[nid].children.clone();
        for child_id in children {
            self.nodes[child_id].parent = None;
        }
        self.nodes[nid].children.clear();
    }

    pub fn import_subtree(&mut self, source: &DomTree, src_nid: NodeId) -> NodeId {
        let root_id = self.import_single_node(source, src_nid);

        // Iterative deep import using explicit stack of (src_id_in_source, dst_parent_in_self)
        let mut stack: Vec<(NodeId, NodeId)> = source
            .get_node(src_nid)
            .children
            .iter()
            .rev()
            .map(|&c| (c, root_id))
            .collect();
        while let Some((src_id, dst_parent)) = stack.pop() {
            let new_id = self.import_single_node(source, src_id);
            self.append_child(dst_parent, new_id);
            for &child_id in source.get_node(src_id).children.iter().rev() {
                stack.push((child_id, new_id));
            }
        }

        root_id
    }

    /// Import a single node from a source tree (no children).
    fn import_single_node(&mut self, source: &DomTree, src_nid: NodeId) -> NodeId {
        let src_node = source.get_node(src_nid);
        match &src_node.data {
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
            NodeData::DocumentFragment | NodeData::ShadowRoot { .. } => self.create_document_fragment(),
        }
    }

    pub fn insert_after(&mut self, sibling: NodeId, child: NodeId) {
        let parent = self.nodes[sibling].parent.expect("insert_after: sibling has no parent");
        if let Some(old_parent) = self.nodes[child].parent {
            self.nodes[old_parent].children.retain(|&c| c != child);
        }
        let pos = self
            .find_child_index(parent, sibling)
            .expect("insert_after: sibling not found");
        self.nodes[parent].children.insert(pos + 1, child);
        self.nodes[child].parent = Some(parent);
    }

    /// Removes all children of the node and replaces them with a single Text child.
    pub fn set_text_content(&mut self, node_id: NodeId, text: &str) {
        // Clone needed: loop mutates self.nodes[child_id] while iterating node_id's children
        let children: Vec<NodeId> = self.nodes[node_id].children.clone();
        for child_id in children {
            self.nodes[child_id].parent = None;
        }
        self.nodes[node_id].children.clear();

        // Create and append a new text node.
        let text_id = self.create_text(text);
        self.append_child(node_id, text_id);
    }

    /// Checks if `ancestor_id` is an inclusive ancestor of `node_id`.
    fn is_inclusive_ancestor(&self, ancestor_id: NodeId, node_id: NodeId) -> bool {
        let mut current = node_id;
        loop {
            if current == ancestor_id {
                return true;
            }
            match self.nodes[current].parent {
                Some(parent) => current = parent,
                None => return false,
            }
        }
    }

    /// Returns true if there's a Doctype node after `ref_id` in `parent_id`'s children.
    fn has_doctype_after(&self, parent_id: NodeId, ref_id: NodeId) -> bool {
        let children = &self.nodes[parent_id].children;
        let mut found_ref = false;
        for &c in children {
            if c == ref_id {
                found_ref = true;
                continue;
            }
            if found_ref && matches!(self.nodes[c].data, NodeData::Doctype { .. }) {
                return true;
            }
        }
        false
    }

    /// Returns true if there's an Element node before `ref_id` in `parent_id`'s children.
    fn has_element_before(&self, parent_id: NodeId, ref_id: NodeId) -> bool {
        let children = &self.nodes[parent_id].children;
        for &c in children {
            if c == ref_id {
                return false;
            }
            if matches!(self.nodes[c].data, NodeData::Element { .. }) {
                return true;
            }
        }
        false
    }

    /// Pre-insertion validation per https://dom.spec.whatwg.org/#concept-node-ensure-pre-insertion-validity
    /// Returns Ok(()) if valid, Err(error_name, message) if invalid.
    /// `ref_child` is None for appendChild (append at end).
    pub fn validate_pre_insert(
        &self,
        parent_id: NodeId,
        node_id: NodeId,
        ref_child: Option<NodeId>,
    ) -> Result<(), (&'static str, &'static str)> {
        let parent_data = &self.nodes[parent_id].data;

        // Step 1: parent must be Document, DocumentFragment, or Element
        match parent_data {
            NodeData::Document
            | NodeData::DocumentFragment
            | NodeData::ShadowRoot { .. }
            | NodeData::Element { .. } => {}
            _ => {
                return Err((
                    "HierarchyRequestError",
                    "parent is not a Document, DocumentFragment, or Element",
                ));
            }
        }

        // Step 2: node must not be an inclusive ancestor of parent
        if self.is_inclusive_ancestor(node_id, parent_id) {
            return Err((
                "HierarchyRequestError",
                "The new child is an ancestor of the parent",
            ));
        }

        // Step 3: if ref_child is not null, its parent must be parent
        if let Some(ref_id) = ref_child {
            if self.nodes[ref_id].parent != Some(parent_id) {
                return Err((
                    "NotFoundError",
                    "The node before which the new node is to be inserted is not a child of this node",
                ));
            }
        }

        let node_data = &self.nodes[node_id].data;

        // Step 4: node must be a valid insertion type
        match node_data {
            NodeData::DocumentFragment
            | NodeData::ShadowRoot { .. }
            | NodeData::Doctype { .. }
            | NodeData::Element { .. }
            | NodeData::Text { .. }
            | NodeData::Comment { .. }
            | NodeData::ProcessingInstruction { .. }
            | NodeData::CDATASection { .. } => {}
            NodeData::Attr { .. } => {
                return Err(("HierarchyRequestError", "Cannot insert an Attr node"));
            }
            NodeData::Document => {
                return Err(("HierarchyRequestError", "Cannot insert a Document node"));
            }
        }

        // Step 5: If node is Text and parent is Document, throw
        if matches!(node_data, NodeData::Text { .. }) && matches!(parent_data, NodeData::Document) {
            return Err((
                "HierarchyRequestError",
                "Cannot insert Text as a child of Document",
            ));
        }

        // Step 5 continued: If node is Doctype and parent is not Document, throw
        if matches!(node_data, NodeData::Doctype { .. }) && !matches!(parent_data, NodeData::Document) {
            return Err((
                "HierarchyRequestError",
                "Cannot insert Doctype as a child of a non-Document node",
            ));
        }

        // Step 6: If parent is Document, additional constraints
        if matches!(parent_data, NodeData::Document) {
            self.validate_document_insert(parent_id, node_id, ref_child)?;
        }

        Ok(())
    }

    /// Additional validation when inserting into a Document node (step 6 of pre-insert).
    fn validate_document_insert(
        &self,
        parent_id: NodeId,
        node_id: NodeId,
        ref_child: Option<NodeId>,
    ) -> Result<(), (&'static str, &'static str)> {
        let node_data = &self.nodes[node_id].data;

        match node_data {
            NodeData::DocumentFragment | NodeData::ShadowRoot { .. } => {
                let frag_children = &self.nodes[node_id].children;
                let elem_count = frag_children
                    .iter()
                    .filter(|&&c| matches!(self.nodes[c].data, NodeData::Element { .. }))
                    .count();
                let has_text = frag_children
                    .iter()
                    .any(|&c| matches!(self.nodes[c].data, NodeData::Text { .. }));

                if has_text {
                    return Err((
                        "HierarchyRequestError",
                        "Cannot insert DocumentFragment containing Text into Document",
                    ));
                }
                if elem_count > 1 {
                    return Err((
                        "HierarchyRequestError",
                        "Cannot insert DocumentFragment with multiple elements into Document",
                    ));
                }
                if elem_count == 1 {
                    let has_existing_element = self.nodes[parent_id]
                        .children
                        .iter()
                        .any(|&c| matches!(self.nodes[c].data, NodeData::Element { .. }));
                    if has_existing_element {
                        return Err((
                            "HierarchyRequestError",
                            "Document already has an element child",
                        ));
                    }
                    if let Some(ref_id) = ref_child {
                        if matches!(self.nodes[ref_id].data, NodeData::Doctype { .. }) {
                            return Err((
                                "HierarchyRequestError",
                                "Cannot insert element before doctype",
                            ));
                        }
                        if self.has_doctype_after(parent_id, ref_id) {
                            return Err((
                                "HierarchyRequestError",
                                "Cannot insert element before a doctype",
                            ));
                        }
                    }
                }
            }
            NodeData::Element { .. } => {
                let has_existing_element = self.nodes[parent_id]
                    .children
                    .iter()
                    .any(|&c| matches!(self.nodes[c].data, NodeData::Element { .. }));
                if has_existing_element {
                    return Err((
                        "HierarchyRequestError",
                        "Document already has an element child",
                    ));
                }
                if let Some(ref_id) = ref_child {
                    if matches!(self.nodes[ref_id].data, NodeData::Doctype { .. }) {
                        return Err((
                            "HierarchyRequestError",
                            "Cannot insert element before doctype",
                        ));
                    }
                    if self.has_doctype_after(parent_id, ref_id) {
                        return Err((
                            "HierarchyRequestError",
                            "Cannot insert element before a doctype",
                        ));
                    }
                }
            }
            NodeData::Doctype { .. } => {
                let has_existing_doctype = self.nodes[parent_id]
                    .children
                    .iter()
                    .any(|&c| matches!(self.nodes[c].data, NodeData::Doctype { .. }));
                if has_existing_doctype {
                    return Err((
                        "HierarchyRequestError",
                        "Document already has a doctype child",
                    ));
                }
                if let Some(ref_id) = ref_child {
                    if self.has_element_before(parent_id, ref_id) {
                        return Err((
                            "HierarchyRequestError",
                            "Cannot insert doctype after an element",
                        ));
                    }
                } else {
                    let has_element = self.nodes[parent_id]
                        .children
                        .iter()
                        .any(|&c| matches!(self.nodes[c].data, NodeData::Element { .. }));
                    if has_element {
                        return Err((
                            "HierarchyRequestError",
                            "Cannot insert doctype after an element",
                        ));
                    }
                }
            }
            _ => {}
        }

        Ok(())
    }

    /// Pre-replace validation per https://dom.spec.whatwg.org/#concept-node-replace
    /// Returns Ok(()) if valid, Err(error_name, message) if invalid.
    pub fn validate_pre_replace(
        &self,
        parent_id: NodeId,
        node_id: NodeId,
        old_child_id: NodeId,
    ) -> Result<(), (&'static str, &'static str)> {
        let parent_data = &self.nodes[parent_id].data;

        // Step 1: parent must be Document, DocumentFragment, or Element
        match parent_data {
            NodeData::Document
            | NodeData::DocumentFragment
            | NodeData::ShadowRoot { .. }
            | NodeData::Element { .. } => {}
            _ => {
                return Err((
                    "HierarchyRequestError",
                    "parent is not a Document, DocumentFragment, or Element",
                ));
            }
        }

        // Step 2: node must not be an inclusive ancestor of parent
        if self.is_inclusive_ancestor(node_id, parent_id) {
            return Err((
                "HierarchyRequestError",
                "The new child is an ancestor of the parent",
            ));
        }

        // Step 3: old child's parent must be parent
        if self.nodes[old_child_id].parent != Some(parent_id) {
            return Err((
                "NotFoundError",
                "The node to be replaced is not a child of this node",
            ));
        }

        let node_data = &self.nodes[node_id].data;

        // Step 4/5: node must be a valid insertion type
        match node_data {
            NodeData::DocumentFragment
            | NodeData::ShadowRoot { .. }
            | NodeData::Doctype { .. }
            | NodeData::Element { .. }
            | NodeData::Text { .. }
            | NodeData::Comment { .. }
            | NodeData::ProcessingInstruction { .. }
            | NodeData::CDATASection { .. } => {}
            NodeData::Attr { .. } => {
                return Err(("HierarchyRequestError", "Cannot insert an Attr node"));
            }
            NodeData::Document => {
                return Err(("HierarchyRequestError", "Cannot insert a Document node"));
            }
        }

        // Step 5: If node is Text and parent is Document, throw
        if matches!(node_data, NodeData::Text { .. }) && matches!(parent_data, NodeData::Document) {
            return Err((
                "HierarchyRequestError",
                "Cannot insert Text as a child of Document",
            ));
        }

        // If node is Doctype and parent is not Document, throw
        if matches!(node_data, NodeData::Doctype { .. }) && !matches!(parent_data, NodeData::Document) {
            return Err((
                "HierarchyRequestError",
                "Cannot insert Doctype as a child of a non-Document node",
            ));
        }

        // Step 6: If parent is Document, additional constraints
        if matches!(parent_data, NodeData::Document) {
            self.validate_document_replace(parent_id, node_id, old_child_id)?;
        }

        Ok(())
    }

    /// Additional validation when replacing within a Document node (step 6 of replace).
    fn validate_document_replace(
        &self,
        parent_id: NodeId,
        node_id: NodeId,
        old_child_id: NodeId,
    ) -> Result<(), (&'static str, &'static str)> {
        let node_data = &self.nodes[node_id].data;

        match node_data {
            NodeData::DocumentFragment | NodeData::ShadowRoot { .. } => {
                let frag_children = &self.nodes[node_id].children;
                let elem_count = frag_children
                    .iter()
                    .filter(|&&c| matches!(self.nodes[c].data, NodeData::Element { .. }))
                    .count();
                let has_text = frag_children
                    .iter()
                    .any(|&c| matches!(self.nodes[c].data, NodeData::Text { .. }));

                if has_text {
                    return Err((
                        "HierarchyRequestError",
                        "Cannot insert DocumentFragment containing Text into Document",
                    ));
                }
                if elem_count > 1 {
                    return Err((
                        "HierarchyRequestError",
                        "Cannot insert DocumentFragment with multiple elements into Document",
                    ));
                }
                if elem_count == 1 {
                    let has_other_element = self.nodes[parent_id]
                        .children
                        .iter()
                        .any(|&c| {
                            c != old_child_id
                                && matches!(self.nodes[c].data, NodeData::Element { .. })
                        });
                    if has_other_element {
                        return Err((
                            "HierarchyRequestError",
                            "Document already has an element child",
                        ));
                    }
                    if self.has_doctype_after(parent_id, old_child_id) {
                        return Err((
                            "HierarchyRequestError",
                            "Cannot insert element before a doctype",
                        ));
                    }
                }
            }
            NodeData::Element { .. } => {
                let has_other_element = self.nodes[parent_id]
                    .children
                    .iter()
                    .any(|&c| {
                        c != old_child_id
                            && matches!(self.nodes[c].data, NodeData::Element { .. })
                    });
                if has_other_element {
                    return Err((
                        "HierarchyRequestError",
                        "Document already has an element child",
                    ));
                }
                if self.has_doctype_after(parent_id, old_child_id) {
                    return Err((
                        "HierarchyRequestError",
                        "Cannot insert element before a doctype",
                    ));
                }
            }
            NodeData::Doctype { .. } => {
                let has_other_doctype = self.nodes[parent_id]
                    .children
                    .iter()
                    .any(|&c| {
                        c != old_child_id
                            && matches!(self.nodes[c].data, NodeData::Doctype { .. })
                    });
                if has_other_doctype {
                    return Err((
                        "HierarchyRequestError",
                        "Document already has a doctype child",
                    ));
                }
                if self.has_element_before(parent_id, old_child_id) {
                    return Err((
                        "HierarchyRequestError",
                        "Cannot insert doctype after an element",
                    ));
                }
            }
            _ => {}
        }

        Ok(())
    }
}
