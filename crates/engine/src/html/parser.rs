use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;

use html5ever::interface::{Attribute, ElementFlags, NodeOrText, QuirksMode, TreeSink};
use html5ever::{parse_document, parse_fragment, Parser, ParseOpts, QualName};
use markup5ever::{ns, LocalName, Namespace};
use tendril::{StrTendril, TendrilSink};

use crate::dom::node::{DomAttribute, NodeData, NodeId};
use crate::dom::tree::DomTree;

/// A TreeSink implementation that builds our DomTree from html5ever's parser events.
///
/// Interior mutability is required because html5ever's TreeSink methods take `&self`.
/// We use `Rc<RefCell<DomTree>>` for the tree so that callers can retain a reference
/// after parsing completes. Element QualNames are stored in a parallel Vec so that
/// `elem_name` can return a borrow from `self` (not from inside the RefCell).
struct BrailleSink {
    tree: Rc<RefCell<DomTree>>,
    names: RefCell<Vec<Option<QualName>>>,
    // POLYFILL: stored per-node flag for is_mathml_annotation_xml_integration_point.
    // html5ever's TreeSink trait requires us to store this from create_element and
    // return it later. Remove when html5ever handles this internally.
    mathml_integration_points: RefCell<Vec<bool>>,
}

impl BrailleSink {
    fn new() -> Self {
        let tree = Rc::new(RefCell::new(DomTree::new()));
        // Index 0 is the Document root — no QualName for it.
        let names = RefCell::new(vec![None]);
        let mathml_integration_points = RefCell::new(vec![false]);
        BrailleSink {
            tree,
            names,
            mathml_integration_points,
        }
    }

    /// Create a sink that shares an existing tree (which already has a Document root at id 0).
    fn with_tree(tree: Rc<RefCell<DomTree>>) -> Self {
        // One entry (index 0) for the Document root — no QualName.
        let names = RefCell::new(vec![None]);
        let mathml_integration_points = RefCell::new(vec![false]);
        BrailleSink {
            tree,
            names,
            mathml_integration_points,
        }
    }

    /// Pad a parallel vec with `value` up to `len` (inclusive).
    fn pad_vec<T: Clone>(vec: &mut Vec<T>, target_len: usize, value: T) {
        while vec.len() <= target_len {
            vec.push(value.clone());
        }
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
        let ptr = names[*target].as_ref().expect("elem_name called on non-element node") as *const QualName;
        unsafe { &*ptr }
    }

    fn create_element(&self, name: QualName, attrs: Vec<Attribute>, flags: ElementFlags) -> NodeId {
        let tag_name = name.local.to_string();
        let attributes: Vec<DomAttribute> = attrs
            .into_iter()
            .map(|a| {
                let local_name = a.name.local.to_string();
                let prefix = a.name.prefix.as_ref().map(|p| p.to_string()).unwrap_or_else(|| {
                    let p = ns_to_attr_prefix(&a.name.ns);
                    p.to_string()
                });
                let namespace = ns_to_attr_ns_uri(&a.name.ns).to_string();
                DomAttribute {
                    local_name,
                    prefix,
                    namespace,
                    value: crate::dom::node::DomString::from(a.value.as_ref()),
                }
            })
            .collect();

        let namespace = ns_to_label(&name.ns);
        let elem_prefix = name.prefix.as_ref().map(|p| p.to_string());

        let mut tree = self.tree.borrow_mut();
        let id = tree.create_element_ns_with_prefix(&tag_name, attributes, namespace, elem_prefix.as_deref());

        // Populate meta cache for <meta> elements.
        if tag_name.eq_ignore_ascii_case("meta") {
            let node = tree.get_node(id);
            if let NodeData::Element { attributes: attrs, .. } = &node.data {
                let name_val = attrs.iter().find(|a| a.local_name == "name").map(|a| a.value.to_string());
                let content_val = attrs.iter().find(|a| a.local_name == "content").map(|a| a.value.to_string());
                let http_equiv_val = attrs.iter().find(|a| a.local_name == "http-equiv").map(|a| a.value.to_string());
                if let (Some(name), Some(content)) = (name_val, content_val) {
                    tree.insert_meta(crate::dom::meta::MetaEntry {
                        name,
                        content,
                        http_equiv: http_equiv_val,
                    });
                }
            }
        }

        // For <template> elements, create an associated content fragment.
        if flags.template {
            let content_id = tree.create_template_contents();
            tree.get_node_mut(id).template_contents = Some(content_id);

            // Keep names vec in sync for the content fragment node too.
            let mut names = self.names.borrow_mut();
            while names.len() < id {
                names.push(None);
            }
            names.push(Some(name));
            // Also pad for the content_id
            while names.len() <= content_id {
                names.push(None);
            }
        } else {
            // Keep the parallel names vec in sync.
            let mut names = self.names.borrow_mut();
            // Pad with None if needed (shouldn't happen normally, but be safe).
            while names.len() < id {
                names.push(None);
            }
            names.push(Some(name));
        }

        // POLYFILL: store mathml annotation-xml integration point flag.
        let mut mip = self.mathml_integration_points.borrow_mut();
        Self::pad_vec(&mut mip, id, false);
        mip[id] = flags.mathml_annotation_xml_integration_point;

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

                // Try to merge with the sibling immediately before `sibling`.
                let parent = tree
                    .get_node(*sibling)
                    .parent
                    .expect("append_before_sibling: sibling has no parent");
                let children = &tree.get_node(parent).children;
                let pos = children.iter().position(|&c| c == *sibling);
                if let Some(p) = pos {
                    if p > 0 {
                        let prev = children[p - 1];
                        if tree.append_to_text(prev, &text) {
                            return;
                        }
                    }
                }

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

    fn append_based_on_parent_node(&self, element: &NodeId, prev_element: &NodeId, child: NodeOrText<NodeId>) {
        // If the element has a parent, insert before the element.
        // Otherwise, append to prev_element.
        let has_parent = self.tree.borrow().get_node(*element).parent.is_some();
        if has_parent {
            self.append_before_sibling(element, child);
        } else {
            self.append(prev_element, child);
        }
    }

    fn append_doctype_to_document(&self, name: StrTendril, public_id: StrTendril, system_id: StrTendril) {
        let mut tree = self.tree.borrow_mut();
        let doctype_id = tree.create_doctype(&name, &public_id, &system_id);

        // Keep names vec in sync.
        let mut names = self.names.borrow_mut();
        while names.len() < doctype_id {
            names.push(None);
        }
        names.push(None);

        let doc = tree.document();
        tree.append_child(doc, doctype_id);
    }

    fn get_template_contents(&self, target: &NodeId) -> NodeId {
        let tree = self.tree.borrow();
        tree.get_node(*target)
            .template_contents
            .expect("get_template_contents called on non-template element")
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
        if let NodeData::Element { ref mut attributes, .. } = node.data {
            let existing: Vec<String> = attributes.iter().map(|a| a.qualified_name()).collect();
            for attr in attrs {
                let local_name = attr.name.local.to_string();
                let prefix = attr.name.prefix.as_ref().map(|p| p.to_string()).unwrap_or_else(|| {
                    let p = ns_to_attr_prefix(&attr.name.ns);
                    p.to_string()
                });
                let namespace = ns_to_attr_ns_uri(&attr.name.ns).to_string();
                let dom_attr = DomAttribute {
                    local_name,
                    prefix,
                    namespace,
                    value: crate::dom::node::DomString::from(attr.value.as_ref()),
                };
                let qname = dom_attr.qualified_name();
                if !existing.contains(&qname) {
                    attributes.push(dom_attr);
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

    // -----------------------------------------------------------------------
    // POLYFILL: is_mathml_annotation_xml_integration_point
    //
    // html5ever calls this to decide whether <annotation-xml> is an HTML
    // integration point (based on its encoding attribute). The TreeSink
    // default returns false. We store the flag from ElementFlags in
    // create_element and return it here.
    //
    // Remove when html5ever handles this internally without requiring
    // TreeSink participation.
    // -----------------------------------------------------------------------
    fn is_mathml_annotation_xml_integration_point(&self, handle: &NodeId) -> bool {
        let mip = self.mathml_integration_points.borrow();
        mip.get(*handle).copied().unwrap_or(false)
    }
}

// ---------------------------------------------------------------------------
// POLYFILL: selectedcontent cloning (post-processing)
//
// Per spec, when an <option> is closed inside a <select> that has a
// <button> > <selectedcontent> descendant, the selected option's children
// are deep-cloned into <selectedcontent>. html5ever 0.38 only calls
// maybe_clone_an_option_into_selectedcontent for *explicit* </option> close
// tags, not implicit closes (html5ever issue #712). Since most HTML uses
// implicit closes, we handle this as a post-parse fixup instead.
//
// Remove when html5ever handles implicit option closing for selectedcontent.
// ---------------------------------------------------------------------------

fn polyfill_selectedcontent(tree: &mut DomTree) {
    // Collect all <select> elements.
    let selects: Vec<NodeId> = (0..tree.node_count())
        .filter(|&id| {
            matches!(&tree.get_node(id).data,
            NodeData::Element { tag_name, .. } if tag_name == "select")
        })
        .collect();

    for select_id in selects {
        // Find <selectedcontent> inside a <button> child of this <select>.
        let selectedcontent_id = {
            let mut found = None;
            let select_children: Vec<NodeId> = tree.get_node(select_id).children.clone();
            'outer: for &child_id in &select_children {
                if let NodeData::Element { ref tag_name, .. } = tree.get_node(child_id).data {
                    if tag_name == "button" {
                        let mut stack: Vec<NodeId> = tree.get_node(child_id).children.clone();
                        while let Some(nid) = stack.pop() {
                            if let NodeData::Element { ref tag_name, .. } = tree.get_node(nid).data {
                                if tag_name == "selectedcontent" {
                                    found = Some(nid);
                                    break 'outer;
                                }
                            }
                            stack.extend_from_slice(&tree.get_node(nid).children);
                        }
                    }
                }
            }
            found
        };

        let selectedcontent_id = match selectedcontent_id {
            Some(id) => id,
            None => continue,
        };

        // Already populated (e.g. by an explicit </option> calling the trait method)?
        if !tree.get_node(selectedcontent_id).children.is_empty() {
            continue;
        }

        // Find the selected <option>: last one with `selected` attr, or first option.
        let mut first_option: Option<NodeId> = None;
        let mut selected_option: Option<NodeId> = None;
        collect_options(tree, select_id, &mut first_option, &mut selected_option);

        let winner = selected_option.or(first_option);
        let winner = match winner {
            Some(id) => id,
            None => continue,
        };

        // Deep-clone winner's children into selectedcontent.
        let option_children: Vec<NodeId> = tree.get_node(winner).children.clone();
        for &child_id in &option_children {
            let cloned = tree.clone_node(child_id, true);
            tree.append_child(selectedcontent_id, cloned);
        }
    }
}

/// Recursively collect <option> info under a subtree, skipping <button> descendants.
fn collect_options(
    tree: &DomTree,
    node_id: NodeId,
    first_option: &mut Option<NodeId>,
    selected_option: &mut Option<NodeId>,
) {
    let children: Vec<NodeId> = tree.get_node(node_id).children.clone();
    for &child_id in &children {
        if let NodeData::Element {
            ref tag_name,
            ref attributes,
            ..
        } = tree.get_node(child_id).data
        {
            if tag_name == "button" {
                continue; // don't look inside <button> for options
            }
            if tag_name == "option" {
                if first_option.is_none() {
                    *first_option = Some(child_id);
                }
                if attributes.iter().any(|a| a.local_name == "selected") {
                    *selected_option = Some(child_id);
                }
                continue; // options don't nest
            }
        }
        collect_options(tree, child_id, first_option, selected_option);
    }
}

// ---------------------------------------------------------------------------
// POLYFILL: declarative shadow DOM (post-processing)
//
// Per the HTML spec, `<template shadowrootmode="open|closed">` inside a host
// element should attach a ShadowRoot to the host, move the template's content
// children into it, and remove the <template> element. html5ever does not
// support this natively, so we handle it as a post-parse fixup.
// ---------------------------------------------------------------------------

fn polyfill_declarative_shadow_roots(tree: &mut DomTree) {
    use crate::dom::node::ShadowRootMode;

    // Collect all <template> elements with a `shadowrootmode` attribute.
    let templates: Vec<(NodeId, String)> = (0..tree.node_count())
        .filter_map(|id| {
            if let NodeData::Element {
                ref tag_name,
                ref attributes,
                ..
            } = tree.get_node(id).data
            {
                if tag_name == "template" {
                    if let Some(attr) = attributes.iter().find(|a| a.local_name == "shadowrootmode") {
                        let mode_str = attr.value.to_string();
                        if mode_str == "open" || mode_str == "closed" {
                            return Some((id, mode_str));
                        }
                    }
                }
            }
            None
        })
        .collect();

    for (template_id, mode_str) in templates {
        let mode = if mode_str == "open" {
            ShadowRootMode::Open
        } else {
            ShadowRootMode::Closed
        };

        // The host is the template's parent element.
        let host_id = match tree.get_node(template_id).parent {
            Some(id) => id,
            None => continue,
        };

        // Skip if host already has a shadow root.
        if tree.get_node(host_id).shadow_root.is_some() {
            continue;
        }

        // Create the shadow root attached to the host.
        let shadow_id = tree.create_shadow_root(mode, host_id);

        // Move children from the template's content fragment into the shadow root.
        if let Some(content_id) = tree.get_node(template_id).template_contents {
            let content_children: Vec<NodeId> = tree.get_node(content_id).children.clone();
            for child_id in content_children {
                tree.remove_from_parent(child_id);
                tree.append_child(shadow_id, child_id);
            }
        }

        // Remove the <template> element from the DOM.
        tree.remove_from_parent(template_id);
    }
}

/// Maps an html5ever namespace URL to a full namespace URI string.
fn ns_to_label(ns: &Namespace) -> &'static str {
    if *ns == ns!(svg) {
        "http://www.w3.org/2000/svg"
    } else if *ns == ns!(mathml) {
        "http://www.w3.org/1998/Math/MathML"
    } else {
        "http://www.w3.org/1999/xhtml"
    }
}

/// Maps an html5ever namespace URL to a prefix string for attributes.
fn ns_to_attr_prefix(ns: &Namespace) -> &'static str {
    if *ns == ns!(xlink) {
        "xlink"
    } else if *ns == ns!(xml) {
        "xml"
    } else if *ns == ns!(xmlns) {
        "xmlns"
    } else {
        ""
    }
}

/// Maps an html5ever namespace URL to a full namespace URI string for attributes.
fn ns_to_attr_ns_uri(ns: &Namespace) -> &'static str {
    if *ns == ns!(xlink) {
        "http://www.w3.org/1999/xlink"
    } else if *ns == ns!(xml) {
        "http://www.w3.org/XML/1998/namespace"
    } else if *ns == ns!(xmlns) {
        "http://www.w3.org/2000/xmlns/"
    } else {
        ""
    }
}

fn make_opts(scripting_enabled: bool) -> ParseOpts {
    let mut opts = ParseOpts::default();
    opts.tree_builder.scripting_enabled = scripting_enabled;
    opts
}

/// Parses an HTML string and returns a shared reference to the resulting DomTree.
pub fn parse_html(html: &str) -> Rc<RefCell<DomTree>> {
    parse_html_scripting(html, true)
}

/// Parses HTML with explicit scripting flag.
pub fn parse_html_scripting(html: &str, scripting_enabled: bool) -> Rc<RefCell<DomTree>> {
    let sink = BrailleSink::new();
    let parser = parse_document(sink, make_opts(scripting_enabled));
    let tree = parser.one(html);
    // POLYFILL: selectedcontent cloning (see polyfill_selectedcontent docs).
    polyfill_selectedcontent(&mut tree.borrow_mut());
    // POLYFILL: declarative shadow DOM (see polyfill_declarative_shadow_roots docs).
    polyfill_declarative_shadow_roots(&mut tree.borrow_mut());
    tree
}

/// Parses an HTML fragment with the given context element tag and namespace.
/// Returns a shared reference to the resulting DomTree.
/// The context element is used to set the parsing context (e.g., parsing inside a `<td>`).
pub fn parse_html_fragment(html: &str, context_tag: &str, context_ns: &str) -> Rc<RefCell<DomTree>> {
    parse_html_fragment_scripting(html, context_tag, context_ns, true)
}

/// Parses HTML fragment with explicit scripting flag.
pub fn parse_html_fragment_scripting(
    html: &str,
    context_tag: &str,
    context_ns: &str,
    scripting_enabled: bool,
) -> Rc<RefCell<DomTree>> {
    let ns = match context_ns {
        "svg" => ns!(svg),
        "math" => ns!(mathml),
        _ => ns!(html),
    };
    let context_name = QualName::new(None, ns, LocalName::from(context_tag));
    let sink = BrailleSink::new();
    let parser = parse_fragment(
        sink,
        make_opts(scripting_enabled),
        context_name,
        Vec::new(),
        scripting_enabled,
    );
    let tree = parser.one(html);
    // POLYFILL: selectedcontent cloning (see polyfill_selectedcontent docs).
    polyfill_selectedcontent(&mut tree.borrow_mut());
    // POLYFILL: declarative shadow DOM (see polyfill_declarative_shadow_roots docs).
    polyfill_declarative_shadow_roots(&mut tree.borrow_mut());
    tree
}

// ---------------------------------------------------------------------------
// Incremental parser — feeds HTML in chunks for interleaved script execution
// ---------------------------------------------------------------------------

/// A parser that feeds HTML in chunks, allowing scripts to execute between chunks.
/// The parser and the caller share the same `Rc<RefCell<DomTree>>`.
pub struct IncrementalParser {
    parser: Parser<BrailleSink>,
    tree: Rc<RefCell<DomTree>>,
}

impl Default for IncrementalParser {
    fn default() -> Self {
        Self::new()
    }
}

impl IncrementalParser {
    /// Create a new incremental parser. The returned parser and tree share the same `Rc`.
    pub fn new() -> Self {
        let tree = Rc::new(RefCell::new(DomTree::new()));
        let sink = BrailleSink::with_tree(Rc::clone(&tree));
        let parser = parse_document(sink, make_opts(true));
        IncrementalParser { parser, tree }
    }

    /// Returns a reference to the shared tree.
    pub fn tree(&self) -> &Rc<RefCell<DomTree>> {
        &self.tree
    }

    /// Feed a chunk of HTML to the parser. The tree is updated immediately.
    pub fn process(&mut self, chunk: &str) {
        self.parser.process(StrTendril::from(chunk));
    }

    /// Signal end-of-input and run post-parse fixups.
    /// Consumes self, returns the final tree.
    pub fn finish(self) -> Rc<RefCell<DomTree>> {
        let tree = self.parser.finish();
        polyfill_selectedcontent(&mut tree.borrow_mut());
        // POLYFILL: declarative shadow DOM (see polyfill_declarative_shadow_roots docs).
        polyfill_declarative_shadow_roots(&mut tree.borrow_mut());
        tree
    }
}

/// Split HTML at `</script>` boundaries (case-insensitive).
/// Each chunk (except possibly the last) ends with `</script>`.
/// Concatenating all chunks yields the original HTML.
pub fn split_html_at_scripts(html: &str) -> Vec<String> {
    let lower = html.to_ascii_lowercase();
    let mut chunks = Vec::new();
    let mut start = 0;

    while start < html.len() {
        match lower[start..].find("</script>") {
            Some(offset) => {
                let end = start + offset + "</script>".len();
                chunks.push(html[start..end].to_string());
                start = end;
            }
            None => {
                // Remainder after last </script>
                chunks.push(html[start..].to_string());
                start = html.len();
            }
        }
    }

    chunks
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
        let tree_rc = parse_html("<div id=\"app\"><span class=\"x\">text</span></div>");
        let tree = tree_rc.borrow();

        // Find <div id="app">
        let div_id = tree.get_element_by_id("app").expect("should find div#app");
        let div_node = tree.get_node(div_id);
        if let NodeData::Element {
            ref tag_name,
            ref attributes,
            ..
        } = div_node.data
        {
            assert_eq!(tag_name, "div");
            assert!(attributes.iter().any(|a| a.local_name == "id" && a.value == "app"));
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
            ..
        } = span_node.data
        {
            assert_eq!(tag_name, "span");
            assert!(attributes.iter().any(|a| a.local_name == "class" && a.value == "x"));
        } else {
            panic!("expected Element node");
        }

        assert_eq!(tree.get_text_content(span_id), "text");
    }

    #[test]
    fn parse_script_content() {
        let tree_rc = parse_html("<html><head></head><body><script>var x = 1;</script></body></html>");
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

    #[test]
    fn declarative_shadow_dom_full_document() {
        let tree_rc = parse_html("<html><body><div><template shadowrootmode=\"open\">hello</template></div></body></html>");
        let tree = tree_rc.borrow();
        let divs = tree.get_elements_by_tag_name("div");
        assert_eq!(divs.len(), 1);
        let div_id = divs[0];
        let shadow = tree.get_node(div_id).shadow_root;
        assert!(shadow.is_some(), "div should have a shadow root");
        let shadow_id = shadow.unwrap();
        // Shadow root should have children (the text "hello")
        assert!(!tree.get_node(shadow_id).children.is_empty(), "shadow root should have children");
        assert_eq!(tree.get_text_content(shadow_id), "hello");
        // The <template> should be removed from the div
        assert!(tree.get_node(div_id).children.is_empty(), "template should be removed from div");
    }

    #[test]
    fn declarative_shadow_dom_fragment() {
        let tree_rc = parse_html_fragment(
            "<div><template shadowrootmode=\"open\">frag</template></div>",
            "div",
            "",
        );
        let tree = tree_rc.borrow();
        // Fragment parse creates: Document -> <html> -> <div> (our content div)
        // The polyfill should attach a shadow root to our <div>
        let divs = tree.get_elements_by_tag_name("div");
        // Find the div that has the shadow root (skip the context element)
        let shadow_div = divs.iter().find(|&&id| tree.get_node(id).shadow_root.is_some());
        assert!(shadow_div.is_some(), "one div should have a shadow root from fragment parse");
        let shadow_id = tree.get_node(*shadow_div.unwrap()).shadow_root.unwrap();
        assert_eq!(tree.get_text_content(shadow_id), "frag");
    }
}
