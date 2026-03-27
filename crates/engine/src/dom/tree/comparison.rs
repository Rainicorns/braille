use crate::dom::node::{NodeData, NodeId};

use super::DomTree;

impl DomTree {
    /// Walk to the shadow-including root: like `root_of()` but when it reaches a ShadowRoot,
    /// jumps to the host element and continues walking up.
    pub fn shadow_including_root_of(&self, id: NodeId) -> NodeId {
        let mut current = id;
        loop {
            // Walk to the root of the current tree
            while let Some(parent) = self.nodes[current].parent {
                current = parent;
            }
            // If we landed on a ShadowRoot, jump to its host and continue
            if let NodeData::ShadowRoot { host, .. } = self.nodes[current].data {
                current = host;
            } else {
                return current;
            }
        }
    }

    /// DOM spec retarget algorithm: walk `a` up through shadow boundaries until it reaches
    /// a tree that is visible from `b`. If `b` is None (non-node target like XMLHttpRequest
    /// or window), the shadow walk always continues to the outermost host.
    ///
    /// Spec: https://dom.spec.whatwg.org/#retarget
    /// Walk A up through shadow roots until A's root is not a shadow root or B is in the same
    /// shadow tree as A.
    pub fn retarget(&self, mut a: NodeId, b: Option<NodeId>) -> NodeId {
        loop {
            let root = self.root_of(a);
            if !matches!(self.nodes[root].data, NodeData::ShadowRoot { .. }) {
                return a;
            }
            if let Some(b_id) = b {
                // If B is in the same shadow tree as A, A is already visible from B
                if self.root_of(b_id) == root {
                    return a;
                }
            }
            // Jump through shadow boundary to the host
            if let NodeData::ShadowRoot { host, .. } = self.nodes[root].data {
                a = host;
            } else {
                return a;
            }
        }
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
            NodeData::DocumentFragment | NodeData::ShadowRoot { .. } => 11,
        }
    }

    /// Implements the DOM isEqualNode algorithm.
    /// Two nodes are equal if they have the same type, same type-specific data,
    /// same number of children, and each child is recursively equal.
    pub fn is_equal_node(&self, a: NodeId, b: NodeId) -> bool {
        let mut stack: Vec<(NodeId, NodeId)> = vec![(a, b)];
        while let Some((a, b)) = stack.pop() {
            if a == b {
                continue;
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

            // Compare children: must have same count, then push pairs for comparison
            if node_a.children.len() != node_b.children.len() {
                return false;
            }
            for (child_a, child_b) in node_a.children.iter().zip(node_b.children.iter()) {
                stack.push((*child_a, *child_b));
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
        // Iterative normalization using an explicit work stack.
        let mut work_stack: Vec<NodeId> = vec![node_id];

        while let Some(current_node) = work_stack.pop() {
            let mut i = 0;
            loop {
                // Clone needed: remove_child below mutates self.nodes[current_node].children
                let children = self.nodes[current_node].children.clone();
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
                        self.remove_child(current_node, child_id);
                        // Don't increment i — the next child slides into this position
                        continue;
                    }

                    // Merge with any following adjacent Text nodes
                    loop {
                        // Clone needed: remove_child below mutates self.nodes[current_node].children
                        let children = self.nodes[current_node].children.clone();
                        let next_idx = self.find_child_index(current_node, child_id).map(|pos| pos + 1);
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
                                self.remove_child(current_node, next);
                            }
                            _ => break,
                        }
                    }

                    i += 1;
                } else {
                    // Push non-Text child onto work stack instead of recursing
                    work_stack.push(child_id);
                    i += 1;
                }
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
                let pos_a = self.find_child_index(common_parent, chain_a[i]);
                let pos_b = self.find_child_index(common_parent, chain_b[i]);
                return pos_a < pos_b;
            }
        }

        // If one is a prefix of the other, the shorter chain is the ancestor
        chain_a.len() < chain_b.len()
    }
}
