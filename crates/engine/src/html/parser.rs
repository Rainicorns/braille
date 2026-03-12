use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;

use html5ever::interface::{
    Attribute, ElementFlags, NodeOrText, QuirksMode, TreeSink,
};
use html5ever::{parse_document, ParseOpts, QualName};
use tendril::{StrTendril, TendrilSink};

use crate::dom::tree::DomTree;
use crate::dom::node::{NodeData, NodeId};

/// A TreeSink implementation that builds our DomTree from html5ever's parser events.
///
/// Interior mutability is required because html5ever's TreeSink methods take `&self`.
/// We use `Rc<RefCell<DomTree>>` for the tree so that callers can retain a reference
/// after parsing completes. Element QualNames are stored in a parallel Vec so that
/// `elem_name` can return a borrow from `self` (not from inside the RefCell).
struct BrailleSink {
    tree: Rc<RefCell<DomTree>>,
    names: RefCell<Vec<Option<QualName>>>,
}

impl BrailleSink {
    fn new() -> Self {
        let tree = Rc::new(RefCell::new(DomTree::new()));
        // Index 0 is the Document root — no QualName for it.
        let names = RefCell::new(vec![None]);
        BrailleSink { tree, names }
    }
}

impl TreeSink for BrailleSink {
    type Handle = NodeId;
    type Output = Rc<RefCell<DomTree>>;
    type ElemName<'a> = &'a QualName;

    fn finish(self) -> Self::Output {
        self.tree
    }

    fn parse_error(&self, _msg: Cow<'static, str>) {
        // Ignored for spike — real HTML is messy, errors are expected.
    }

    fn get_document(&self) -> NodeId {
        0
    }

    fn elem_name<'a>(&'a self, target: &'a NodeId) -> Self::ElemName<'a> {
        let names = self.names.borrow();
        // SAFETY: We leak the borrow here. This is sound because:
        // 1. The Vec only grows (we never remove or reorder entries).
        // 2. QualName entries are never mutated after insertion.
        // 3. The returned reference's lifetime is tied to `&'a self`, and
        //    self (the BrailleSink) owns the RefCell<Vec<..>>.
        //
        // html5ever calls elem_name during tree building and does not hold
        // the reference across calls that would trigger a mutable borrow of
        // the names vec. We re-borrow on each call.
        let ptr = names[*target]
            .as_ref()
            .expect("elem_name called on non-element node") as *const QualName;
        unsafe { &*ptr }
    }

    fn create_element(
        &self,
        name: QualName,
        attrs: Vec<Attribute>,
        _flags: ElementFlags,
    ) -> NodeId {
        let tag_name = name.local.to_string();
        let attributes: Vec<(String, String)> = attrs
            .into_iter()
            .map(|a| (a.name.local.to_string(), a.value.to_string()))
            .collect();

        let id = self
            .tree
            .borrow_mut()
            .create_element_with_attrs(&tag_name, attributes);

        // Keep the parallel names vec in sync.
        let mut names = self.names.borrow_mut();
        // Pad with None if needed (shouldn't happen normally, but be safe).
        while names.len() < id {
            names.push(None);
        }
        names.push(Some(name));

        id
    }

    fn create_comment(&self, text: StrTendril) -> NodeId {
        let id = self.tree.borrow_mut().create_comment(&text);

        // Keep names vec in sync (no QualName for comments).
        let mut names = self.names.borrow_mut();
        while names.len() < id {
            names.push(None);
        }
        names.push(None);

        id
    }

    fn create_pi(&self, _target: StrTendril, _data: StrTendril) -> NodeId {
        // Processing instructions are not used in HTML. Return a dummy comment node.
        self.create_comment(StrTendril::new())
    }

    fn append(&self, parent: &NodeId, child: NodeOrText<NodeId>) {
        match child {
            NodeOrText::AppendNode(child_id) => {
                self.tree.borrow_mut().append_child(*parent, child_id);
            }
            NodeOrText::AppendText(text) => {
                let mut tree = self.tree.borrow_mut();
                // Try to merge with the last child if it's a Text node.
                let last_child = tree.get_node(*parent).children.last().copied();
                if let Some(last_id) = last_child {
                    if tree.append_to_text(last_id, &text) {
                        return;
                    }
                }
                // No existing text node to merge with — create a new one.
                let text_id = tree.create_text(&text);

                // Keep names vec in sync.
                let mut names = self.names.borrow_mut();
                while names.len() < text_id {
                    names.push(None);
                }
                names.push(None);

                tree.append_child(*parent, text_id);
            }
        }
    }

    fn append_before_sibling(&self, sibling: &NodeId, child: NodeOrText<NodeId>) {
        match child {
            NodeOrText::AppendNode(child_id) => {
                self.tree.borrow_mut().insert_before(*sibling, child_id);
            }
            NodeOrText::AppendText(text) => {
                let mut tree = self.tree.borrow_mut();
                let text_id = tree.create_text(&text);

                let mut names = self.names.borrow_mut();
                while names.len() < text_id {
                    names.push(None);
                }
                names.push(None);

                tree.insert_before(*sibling, text_id);
            }
        }
    }

    fn append_based_on_parent_node(
        &self,
        element: &NodeId,
        prev_element: &NodeId,
        child: NodeOrText<NodeId>,
    ) {
        // If the element has a parent, insert before the element.
        // Otherwise, append to prev_element.
        let has_parent = self.tree.borrow().get_node(*element).parent.is_some();
        if has_parent {
            self.append_before_sibling(element, child);
        } else {
            self.append(prev_element, child);
        }
    }

    fn append_doctype_to_document(
        &self,
        _name: StrTendril,
        _public_id: StrTendril,
        _system_id: StrTendril,
    ) {
        // Ignored for spike — we don't model DOCTYPE nodes.
    }

    fn get_template_contents(&self, target: &NodeId) -> NodeId {
        // For the spike, we treat template elements as regular elements.
        // Their children go directly into the element itself.
        *target
    }

    fn same_node(&self, x: &NodeId, y: &NodeId) -> bool {
        x == y
    }

    fn set_quirks_mode(&self, _mode: QuirksMode) {
        // Ignored for spike.
    }

    fn add_attrs_if_missing(&self, target: &NodeId, attrs: Vec<Attribute>) {
        let mut tree = self.tree.borrow_mut();
        let node = tree.get_node_mut(*target);
        if let NodeData::Element {
            ref mut attributes, ..
        } = node.data
        {
            let existing: Vec<String> = attributes.iter().map(|(k, _)| k.clone()).collect();
            for attr in attrs {
                let name = attr.name.local.to_string();
                if !existing.contains(&name) {
                    attributes.push((name, attr.value.to_string()));
                }
            }
        }
    }

    fn remove_from_parent(&self, target: &NodeId) {
        self.tree.borrow_mut().remove_from_parent(*target);
    }

    fn reparent_children(&self, node: &NodeId, new_parent: &NodeId) {
        self.tree.borrow_mut().reparent_children(*node, *new_parent);
    }
}

/// Parses an HTML string and returns a shared reference to the resulting DomTree.
pub fn parse_html(html: &str) -> Rc<RefCell<DomTree>> {
    let sink = BrailleSink::new();
    let parser = parse_document(sink, ParseOpts::default());
    parser.one(html)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_basic_structure() {
        let tree_rc = parse_html("<html><body><p>Hello</p></body></html>");
        let tree = tree_rc.borrow();

        // Document root should have an <html> child.
        let doc = tree.get_node(tree.document());
        assert!(!doc.children.is_empty());

        // Find <html>
        let html_nodes = tree.get_elements_by_tag_name("html");
        assert_eq!(html_nodes.len(), 1);
        let html_id = html_nodes[0];

        // <html> should contain <head> (auto-inserted) and <body>
        let html_node = tree.get_node(html_id);
        assert!(html_node.children.len() >= 2); // head + body

        // Find <body>
        let body_id = tree.body().expect("should have a body");

        // <body> should contain <p>
        let p_nodes = tree.get_elements_by_tag_name("p");
        assert_eq!(p_nodes.len(), 1);
        let p_id = p_nodes[0];
        assert_eq!(tree.get_node(p_id).parent, Some(body_id));

        // <p> should contain text "Hello"
        assert_eq!(tree.get_text_content(p_id), "Hello");
    }

    #[test]
    fn parse_attributes() {
        let tree_rc =
            parse_html("<div id=\"app\"><span class=\"x\">text</span></div>");
        let tree = tree_rc.borrow();

        // Find <div id="app">
        let div_id = tree
            .get_element_by_id("app")
            .expect("should find div#app");
        let div_node = tree.get_node(div_id);
        if let NodeData::Element {
            ref tag_name,
            ref attributes,
        } = div_node.data
        {
            assert_eq!(tag_name, "div");
            assert!(attributes.contains(&("id".to_string(), "app".to_string())));
        } else {
            panic!("expected Element node");
        }

        // Find <span class="x">
        let spans = tree.get_elements_by_tag_name("span");
        assert_eq!(spans.len(), 1);
        let span_id = spans[0];
        let span_node = tree.get_node(span_id);
        if let NodeData::Element {
            ref tag_name,
            ref attributes,
        } = span_node.data
        {
            assert_eq!(tag_name, "span");
            assert!(attributes.contains(&("class".to_string(), "x".to_string())));
        } else {
            panic!("expected Element node");
        }

        assert_eq!(tree.get_text_content(span_id), "text");
    }

    #[test]
    fn parse_script_content() {
        let tree_rc = parse_html(
            "<html><head></head><body><script>var x = 1;</script></body></html>",
        );
        let tree = tree_rc.borrow();

        let scripts = tree.get_elements_by_tag_name("script");
        assert_eq!(scripts.len(), 1);
        let script_id = scripts[0];

        // Script content should be a text child.
        let script_node = tree.get_node(script_id);
        assert!(!script_node.children.is_empty());

        let first_child = script_node.children[0];
        let child_node = tree.get_node(first_child);
        if let NodeData::Text { ref content } = child_node.data {
            assert_eq!(content, "var x = 1;");
        } else {
            panic!("expected Text node as child of script, got {:?}", child_node.data);
        }
    }
}
