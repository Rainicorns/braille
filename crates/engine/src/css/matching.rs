//! CSS selector matching implementation for Braille.
//!
//! This module implements the `selectors::Element` trait for our DOM tree,
//! enabling CSS selector matching and querySelector operations.

use super::selector_impl::{BrailleSelectorImpl, BrailleSelectorParser, CssString, PseudoClass};
use crate::dom::node::{NodeData, NodeId};
use crate::dom::tree::DomTree;
use cssparser::{Parser as CssParser, ParserInput};
use selectors::matching::{matches_selector, MatchingContext, MatchingMode, QuirksMode, NeedsSelectorFlags, MatchingForInvalidation, SelectorCaches};
use selectors::parser::{SelectorList, ParseRelative};
use selectors::{attr::CaseSensitivity, bloom::{CountingBloomFilter, BloomStorageU8}, Element, OpaqueElement};

/// A wrapper around a DomTree node that implements the selectors::Element trait.
///
/// This allows the selectors crate to query our DOM structure using CSS selectors.
#[derive(Clone, Copy, Debug)]
pub struct DomElement<'a> {
    pub tree: &'a DomTree,
    pub node_id: NodeId,
}

impl<'a> DomElement<'a> {
    /// Creates a new DomElement wrapper.
    pub fn new(tree: &'a DomTree, node_id: NodeId) -> Self {
        DomElement { tree, node_id }
    }

    /// Returns the tag name if this is an Element node.
    fn tag_name(&self) -> Option<&str> {
        let node = self.tree.get_node(self.node_id);
        if let NodeData::Element { ref tag_name, .. } = node.data {
            Some(tag_name)
        } else {
            None
        }
    }

    /// Checks if this form element (input/textarea/select) is invalid.
    /// Invalid means: has `required` attribute and the value is empty.
    fn is_form_element_invalid(&self) -> bool {
        let tag = match self.tag_name() {
            Some(t) => t,
            None => return false,
        };
        match tag {
            "input" | "textarea" => {
                if !self.tree.has_attribute(self.node_id, "required") {
                    return false;
                }
                // Check if value is empty
                let value = self.tree.get_attribute(self.node_id, "value").unwrap_or_default();
                value.is_empty()
            }
            "select" => {
                if !self.tree.has_attribute(self.node_id, "required") {
                    return false;
                }
                // A required select is invalid if its value is empty.
                // The value comes from the selected option's value attribute (or its text content).
                // For this minimal implementation: check if there's a selected option with non-empty value.
                !self.has_selected_option_with_value()
            }
            _ => false,
        }
    }

    /// Checks if a select element has a selected option with a non-empty value.
    fn has_selected_option_with_value(&self) -> bool {
        self.check_options_recursive(self.node_id)
    }

    /// Recursively check children for <option selected> with non-empty value.
    fn check_options_recursive(&self, node_id: NodeId) -> bool {
        let node = self.tree.get_node(node_id);
        for &child_id in &node.children {
            let child = self.tree.get_node(child_id);
            if let NodeData::Element { ref tag_name, .. } = child.data {
                if tag_name == "option" && self.tree.has_attribute(child_id, "selected") {
                    // Check the option's value attribute; if not present, use text content
                    let value = self.tree.get_attribute(child_id, "value")
                        .unwrap_or_else(|| self.tree.get_text_content(child_id));
                    if !value.is_empty() {
                        return true;
                    }
                }
                // Recurse into optgroup etc.
                if self.check_options_recursive(child_id) {
                    return true;
                }
            }
        }
        false
    }

    /// Checks if this element has any descendant form elements that are invalid.
    fn has_invalid_descendant(&self) -> bool {
        self.check_invalid_descendants(self.node_id)
    }

    /// Recursively walk descendants looking for invalid form elements.
    fn check_invalid_descendants(&self, node_id: NodeId) -> bool {
        let node = self.tree.get_node(node_id);
        for &child_id in &node.children {
            let child = self.tree.get_node(child_id);
            if let NodeData::Element { ref tag_name, .. } = child.data {
                if matches!(tag_name.as_str(), "input" | "textarea" | "select") {
                    let child_elem = DomElement::new(self.tree, child_id);
                    if child_elem.is_form_element_invalid() {
                        return true;
                    }
                }
                // Recurse into children
                if self.check_invalid_descendants(child_id) {
                    return true;
                }
            }
        }
        false
    }
}

impl<'a> Element for DomElement<'a> {
    type Impl = BrailleSelectorImpl;

    fn opaque(&self) -> OpaqueElement {
        // Use the stable address of the node in the arena, not the stack-allocated DomElement.
        // This ensures that two DomElements wrapping the same node_id in the same tree
        // produce the same OpaqueElement, which is needed for :scope matching.
        let node_ref = self.tree.get_node(self.node_id);
        OpaqueElement::new(node_ref)
    }

    fn parent_element(&self) -> Option<Self> {
        self.tree
            .parent_element(self.node_id)
            .map(|id| DomElement::new(self.tree, id))
    }

    fn parent_node_is_shadow_root(&self) -> bool {
        // We don't support shadow DOM
        false
    }

    fn containing_shadow_host(&self) -> Option<Self> {
        // We don't support shadow DOM
        None
    }

    fn is_pseudo_element(&self) -> bool {
        // Our nodes are not pseudo-elements
        false
    }

    fn prev_sibling_element(&self) -> Option<Self> {
        self.tree
            .prev_sibling_element(self.node_id)
            .map(|id| DomElement::new(self.tree, id))
    }

    fn next_sibling_element(&self) -> Option<Self> {
        self.tree
            .next_sibling_element(self.node_id)
            .map(|id| DomElement::new(self.tree, id))
    }

    fn first_element_child(&self) -> Option<Self> {
        self.tree
            .element_children(self.node_id)
            .first()
            .map(|&id| DomElement::new(self.tree, id))
    }

    fn is_html_element_in_html_document(&self) -> bool {
        // We're always in an HTML context
        true
    }

    fn has_local_name(&self, local_name: &str) -> bool {
        let tag = match self.tag_name() {
            Some(t) => t,
            None => return false,
        };

        // HTML tag names are case-insensitive
        tag.eq_ignore_ascii_case(local_name)
    }

    fn has_namespace(&self, _ns: &str) -> bool {
        // We don't use namespaces in our simple HTML implementation
        true
    }

    fn is_same_type(&self, other: &Self) -> bool {
        // Compare tag names
        self.tag_name() == other.tag_name()
    }

    fn attr_matches(
        &self,
        ns: &selectors::attr::NamespaceConstraint<&CssString>,
        local_name: &CssString,
        operation: &selectors::attr::AttrSelectorOperation<&CssString>,
    ) -> bool {
        // Allow Any namespace, or Specific with empty namespace (null namespace).
        // Only reject non-empty specific namespaces since we don't store namespaced attributes.
        match ns {
            selectors::attr::NamespaceConstraint::Any => {}
            selectors::attr::NamespaceConstraint::Specific(ns_url) => {
                if !ns_url.0.is_empty() {
                    return false;
                }
            }
        }

        let attr_value = match self.tree.get_attribute(self.node_id, &local_name.0) {
            Some(v) => v,
            None => return false,
        };

        operation.eval_str(&attr_value)
    }

    fn match_non_ts_pseudo_class(
        &self,
        pseudo: &PseudoClass,
        context: &mut MatchingContext<Self::Impl>,
    ) -> bool {
        match pseudo {
            PseudoClass::Scope => {
                match context.scope_element {
                    Some(scope) => self.opaque() == scope,
                    None => self.tree.is_root_element(self.node_id),
                }
            }
            PseudoClass::Root => self.tree.is_root_element(self.node_id),
            PseudoClass::Empty => self.is_empty(),
            PseudoClass::Link => self.is_link(),
            PseudoClass::FirstChild => {
                // This node should be the first element child of its parent
                let parent_id = match self.tree.get_node(self.node_id).parent {
                    Some(p) => p,
                    None => return false,
                };
                let element_children = self.tree.element_children(parent_id);
                element_children.first() == Some(&self.node_id)
            }
            PseudoClass::LastChild => {
                // This node should be the last element child of its parent
                let parent_id = match self.tree.get_node(self.node_id).parent {
                    Some(p) => p,
                    None => return false,
                };
                let element_children = self.tree.element_children(parent_id);
                element_children.last() == Some(&self.node_id)
            }
            PseudoClass::OnlyChild => {
                // This node should be the only element child of its parent
                let parent_id = match self.tree.get_node(self.node_id).parent {
                    Some(p) => p,
                    None => return false,
                };
                let element_children = self.tree.element_children(parent_id);
                element_children.len() == 1 && element_children.first() == Some(&self.node_id)
            }
            PseudoClass::NthChild(a, b) => {
                // Find position among element siblings
                let parent_id = match self.tree.get_node(self.node_id).parent {
                    Some(p) => p,
                    None => return false,
                };
                let element_children = self.tree.element_children(parent_id);
                let position = match element_children.iter().position(|&id| id == self.node_id) {
                    Some(p) => (p + 1) as i32, // 1-indexed
                    None => return false,
                };

                // an + b formula: check if (position - b) is divisible by a
                if *a == 0 {
                    position == *b
                } else {
                    let remainder = (position - b) % a;
                    remainder == 0 && (position - b) / a >= 0
                }
            }
            // Dynamic pseudo-classes (user interaction states)
            // FLAG: These are not yet implemented in the engine's state tracking
            PseudoClass::Hover | PseudoClass::Focus | PseudoClass::Active => false,
            // Link-related pseudo-classes
            PseudoClass::Visited => false, // We don't track visited state
            // Form-related pseudo-classes
            // FLAG: These require form state tracking which is not yet implemented
            PseudoClass::Checked => {
                // Check if input/option is checked
                if let Some(tag) = self.tag_name() {
                    if tag == "input" || tag == "option" {
                        self.tree.has_attribute(self.node_id, "checked")
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            PseudoClass::Disabled => {
                // Check disabled attribute
                self.tree.get_attribute(self.node_id, "disabled").is_some()
            }
            PseudoClass::Enabled => {
                // Enabled if it's a form element without disabled attribute
                if let Some(tag) = self.tag_name() {
                    matches!(tag, "input" | "button" | "select" | "textarea" | "option")
                        && !self.tree.has_attribute(self.node_id, "disabled")
                } else {
                    false
                }
            }
            PseudoClass::Invalid => {
                match self.tag_name() {
                    Some("input") | Some("textarea") | Some("select") => {
                        self.is_form_element_invalid()
                    }
                    Some("fieldset") | Some("form") => {
                        self.has_invalid_descendant()
                    }
                    _ => false,
                }
            }
            PseudoClass::Valid => {
                match self.tag_name() {
                    Some("input") | Some("textarea") | Some("select") => {
                        !self.is_form_element_invalid()
                    }
                    Some("fieldset") | Some("form") => {
                        !self.has_invalid_descendant()
                    }
                    _ => false,
                }
            }
        }
    }

    fn match_pseudo_element(
        &self,
        _pseudo: &<Self::Impl as selectors::SelectorImpl>::PseudoElement,
        _context: &mut MatchingContext<Self::Impl>,
    ) -> bool {
        // FLAG: Pseudo-elements (::before, ::after) are not yet implemented
        false
    }

    fn apply_selector_flags(&self, _flags: selectors::matching::ElementSelectorFlags) {
        // No-op: we don't track selector flags for invalidation
    }

    fn is_link(&self) -> bool {
        // An element is a link if it's an <a> with an href attribute
        match self.tag_name() {
            Some("a") => self.tree.has_attribute(self.node_id, "href"),
            _ => false,
        }
    }

    fn is_html_slot_element(&self) -> bool {
        // We don't support <slot> elements yet
        false
    }

    fn has_id(&self, id: &CssString, case_sensitivity: CaseSensitivity) -> bool {
        let attr_value = match self.tree.get_attribute(self.node_id, "id") {
            Some(v) => v,
            None => return false,
        };

        match case_sensitivity {
            CaseSensitivity::CaseSensitive => attr_value == id.0,
            CaseSensitivity::AsciiCaseInsensitive => {
                attr_value.eq_ignore_ascii_case(&id.0)
            }
        }
    }

    fn has_class(&self, name: &CssString, case_sensitivity: CaseSensitivity) -> bool {
        let class_attr = match self.tree.get_attribute(self.node_id, "class") {
            Some(v) => v,
            None => return false,
        };

        // Split class attribute by whitespace and check if any match
        class_attr.split_whitespace().any(|class| {
            match case_sensitivity {
                CaseSensitivity::CaseSensitive => class == name.0,
                CaseSensitivity::AsciiCaseInsensitive => {
                    class.eq_ignore_ascii_case(&name.0)
                }
            }
        })
    }

    fn imported_part(&self, _name: &CssString) -> Option<CssString> {
        // We don't support ::part() pseudo-element
        None
    }

    fn is_part(&self, _name: &CssString) -> bool {
        // We don't support parts
        false
    }

    fn is_empty(&self) -> bool {
        // Empty if no element children and no non-whitespace text content
        let element_children = self.tree.element_children(self.node_id);
        if !element_children.is_empty() {
            return false;
        }

        // Check if there's any non-whitespace text content
        let text = self.tree.get_text_content(self.node_id);
        text.trim().is_empty()
    }

    fn is_root(&self) -> bool {
        self.tree.is_root_element(self.node_id)
    }

    fn has_custom_state(&self, _name: &CssString) -> bool {
        // FLAG: Custom states are not yet implemented
        false
    }

    fn add_element_unique_hashes(&self, _filter: &mut CountingBloomFilter<BloomStorageU8>) -> bool {
        // FLAG: Bloom filter optimization not yet implemented
        true
    }
}

/// Finds the first element matching the given CSS selector, starting from `root`.
///
/// Returns the NodeId of the first matching element, or None if no match is found.
///
/// # Arguments
/// * `tree` - The DOM tree to search
/// * `root` - The NodeId to start searching from (searches descendants)
/// * `selector` - The CSS selector string to match
pub fn query_selector(tree: &DomTree, root: NodeId, selector: &str, scope_node_id: Option<NodeId>) -> Option<NodeId> {
    // Parse the selector
    let mut parser_input = ParserInput::new(selector);
    let mut parser = CssParser::new(&mut parser_input);
    let selector_list = SelectorList::parse(&BrailleSelectorParser, &mut parser, ParseRelative::No).ok()?;

    // Create matching context with scope
    let scope_opaque = scope_node_id.map(|id| DomElement::new(tree, id).opaque());
    let mut caches = SelectorCaches::default();
    let mut context = MatchingContext::new(
        MatchingMode::Normal,
        None,
        &mut caches,
        QuirksMode::NoQuirks,
        NeedsSelectorFlags::No,
        MatchingForInvalidation::No,
    );
    context.scope_element = scope_opaque;

    // Walk descendants and find first match
    find_first_match(tree, root, &selector_list, &mut context)
}

/// Finds all elements matching the given CSS selector, starting from `root`.
///
/// Returns a Vec of NodeIds of all matching elements.
///
/// # Arguments
/// * `tree` - The DOM tree to search
/// * `root` - The NodeId to start searching from (searches descendants)
/// * `selector` - The CSS selector string to match
pub fn query_selector_all(tree: &DomTree, root: NodeId, selector: &str, scope_node_id: Option<NodeId>) -> Vec<NodeId> {
    // Parse the selector
    let mut parser_input = ParserInput::new(selector);
    let mut parser = CssParser::new(&mut parser_input);
    let selector_list = match SelectorList::parse(&BrailleSelectorParser, &mut parser, ParseRelative::No) {
        Ok(list) => list,
        Err(_) => return vec![],
    };

    // Create matching context with scope
    let scope_opaque = scope_node_id.map(|id| DomElement::new(tree, id).opaque());
    let mut caches = SelectorCaches::default();
    let mut context = MatchingContext::new(
        MatchingMode::Normal,
        None,
        &mut caches,
        QuirksMode::NoQuirks,
        NeedsSelectorFlags::No,
        MatchingForInvalidation::No,
    );
    context.scope_element = scope_opaque;

    // Walk descendants and collect all matches
    find_all_matches(tree, root, &selector_list, &mut context)
}

/// Tests if a single element matches the given CSS selector string.
pub fn matches_selector_str(tree: &DomTree, node_id: NodeId, selector: &str, scope_node_id: Option<NodeId>) -> bool {
    let node = tree.get_node(node_id);
    if !matches!(node.data, NodeData::Element { .. }) {
        return false;
    }

    let mut parser_input = ParserInput::new(selector);
    let mut parser = CssParser::new(&mut parser_input);
    let selector_list = match SelectorList::parse(&BrailleSelectorParser, &mut parser, ParseRelative::No) {
        Ok(list) => list,
        Err(_) => return false,
    };

    let scope_opaque = scope_node_id.map(|id| DomElement::new(tree, id).opaque());
    let mut caches = SelectorCaches::default();
    let mut context = MatchingContext::new(
        MatchingMode::Normal,
        None,
        &mut caches,
        QuirksMode::NoQuirks,
        NeedsSelectorFlags::No,
        MatchingForInvalidation::No,
    );
    context.scope_element = scope_opaque;

    let element = DomElement::new(tree, node_id);
    for selector in selector_list.slice().iter() {
        if matches_selector(selector, 0, None, &element, &mut context) {
            return true;
        }
    }
    false
}

/// Helper function to recursively find the first matching element.
fn find_first_match(
    tree: &DomTree,
    node_id: NodeId,
    selector_list: &SelectorList<BrailleSelectorImpl>,
    context: &mut MatchingContext<BrailleSelectorImpl>,
) -> Option<NodeId> {
    let node = tree.get_node(node_id);

    // Check if this node is an element and matches
    if matches!(node.data, NodeData::Element { .. }) {
        let element = DomElement::new(tree, node_id);
        for selector in selector_list.slice().iter() {
            if matches_selector(selector, 0, None, &element, context) {
                return Some(node_id);
            }
        }
    }

    // Recursively search children
    for &child_id in &node.children {
        if let Some(found) = find_first_match(tree, child_id, selector_list, context) {
            return Some(found);
        }
    }

    None
}

/// Helper function to recursively find all matching elements.
fn find_all_matches(
    tree: &DomTree,
    node_id: NodeId,
    selector_list: &SelectorList<BrailleSelectorImpl>,
    context: &mut MatchingContext<BrailleSelectorImpl>,
) -> Vec<NodeId> {
    let mut results = Vec::new();
    collect_matches(tree, node_id, selector_list, context, &mut results);
    results
}

/// Helper function to recursively collect all matching elements.
fn collect_matches(
    tree: &DomTree,
    node_id: NodeId,
    selector_list: &SelectorList<BrailleSelectorImpl>,
    context: &mut MatchingContext<BrailleSelectorImpl>,
    results: &mut Vec<NodeId>,
) {
    let node = tree.get_node(node_id);

    // Check if this node is an element and matches
    if matches!(node.data, NodeData::Element { .. }) {
        let element = DomElement::new(tree, node_id);
        for selector in selector_list.slice().iter() {
            if matches_selector(selector, 0, None, &element, context) {
                results.push(node_id);
                break; // Don't add the same element multiple times
            }
        }
    }

    // Recursively search children
    for &child_id in &node.children {
        collect_matches(tree, child_id, selector_list, context, results);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_test_tree() -> DomTree {
        let mut tree = DomTree::new();

        // Create structure:
        // <html>
        //   <body>
        //     <div class="container">
        //       <p id="first">First paragraph</p>
        //       <p class="highlight">Second paragraph</p>
        //       <div class="nested">
        //         <span>Nested span</span>
        //       </div>
        //     </div>
        //     <p>Third paragraph</p>
        //   </body>
        // </html>

        let html = tree.create_element("html");
        let body = tree.create_element("body");
        let div = tree.create_element_with_attrs("div", vec![
            ("class".to_string(), "container".to_string()),
        ]);
        let p1 = tree.create_element_with_attrs("p", vec![
            ("id".to_string(), "first".to_string()),
        ]);
        let p1_text = tree.create_text("First paragraph");
        let p2 = tree.create_element_with_attrs("p", vec![
            ("class".to_string(), "highlight".to_string()),
        ]);
        let p2_text = tree.create_text("Second paragraph");
        let nested_div = tree.create_element_with_attrs("div", vec![
            ("class".to_string(), "nested".to_string()),
        ]);
        let span = tree.create_element("span");
        let span_text = tree.create_text("Nested span");
        let p3 = tree.create_element("p");
        let p3_text = tree.create_text("Third paragraph");

        tree.append_child(tree.document(), html);
        tree.append_child(html, body);
        tree.append_child(body, div);
        tree.append_child(div, p1);
        tree.append_child(p1, p1_text);
        tree.append_child(div, p2);
        tree.append_child(p2, p2_text);
        tree.append_child(div, nested_div);
        tree.append_child(nested_div, span);
        tree.append_child(span, span_text);
        tree.append_child(body, p3);
        tree.append_child(p3, p3_text);

        tree
    }

    #[test]
    fn test_query_selector_by_tag_name() {
        let tree = setup_test_tree();
        let body = tree.body().expect("body should exist");

        let result = query_selector(&tree, body, "p", None);
        assert!(result.is_some());

        // Should find the first <p> element
        let node = tree.get_node(result.unwrap());
        if let NodeData::Element { ref tag_name, .. } = node.data {
            assert_eq!(tag_name, "p");
        } else {
            panic!("Expected Element node");
        }
    }

    #[test]
    fn test_query_selector_by_class() {
        let tree = setup_test_tree();
        let body = tree.body().expect("body should exist");

        let result = query_selector(&tree, body, ".highlight", None);
        assert!(result.is_some());

        let node_id = result.unwrap();
        assert_eq!(tree.get_attribute(node_id, "class"), Some("highlight".to_string()));
    }

    #[test]
    fn test_query_selector_by_id() {
        let tree = setup_test_tree();
        let body = tree.body().expect("body should exist");

        let result = query_selector(&tree, body, "#first", None);
        assert!(result.is_some());

        let node_id = result.unwrap();
        assert_eq!(tree.get_attribute(node_id, "id"), Some("first".to_string()));
    }

    #[test]
    fn test_query_selector_all_returns_multiple() {
        let tree = setup_test_tree();
        let body = tree.body().expect("body should exist");

        let results = query_selector_all(&tree, body, "p", None);
        assert_eq!(results.len(), 3); // Three <p> elements in the tree

        // All should be <p> elements
        for &node_id in &results {
            let node = tree.get_node(node_id);
            if let NodeData::Element { ref tag_name, .. } = node.data {
                assert_eq!(tag_name, "p");
            } else {
                panic!("Expected Element node");
            }
        }
    }

    #[test]
    fn test_query_selector_returns_none_for_no_match() {
        let tree = setup_test_tree();
        let body = tree.body().expect("body should exist");

        let result = query_selector(&tree, body, ".nonexistent", None);
        assert!(result.is_none());
    }

    #[test]
    fn test_query_selector_complex_descendant() {
        let tree = setup_test_tree();
        let body = tree.body().expect("body should exist");

        // Select span inside .container
        let result = query_selector(&tree, body, ".container span", None);
        assert!(result.is_some());

        let node = tree.get_node(result.unwrap());
        if let NodeData::Element { ref tag_name, .. } = node.data {
            assert_eq!(tag_name, "span");
        } else {
            panic!("Expected Element node");
        }
    }

    #[test]
    fn test_query_selector_all_with_class() {
        let tree = setup_test_tree();
        let body = tree.body().expect("body should exist");

        let results = query_selector_all(&tree, body, "div", None);
        assert_eq!(results.len(), 2); // Two <div> elements
    }

    #[test]
    fn test_dom_element_wrapper() {
        let tree = setup_test_tree();
        let body = tree.body().expect("body should exist");

        let elem = DomElement::new(&tree, body);
        assert_eq!(elem.tag_name(), Some("body"));
        assert!(elem.is_html_element_in_html_document());
    }

    #[test]
    fn test_has_class_method() {
        let tree = setup_test_tree();
        let body = tree.body().expect("body should exist");

        // Find element with class
        let result = query_selector(&tree, body, ".container", None);
        assert!(result.is_some());

        let elem = DomElement::new(&tree, result.unwrap());
        assert!(elem.has_class(&CssString("container".into()), CaseSensitivity::AsciiCaseInsensitive));
        assert!(!elem.has_class(&CssString("nonexistent".into()), CaseSensitivity::AsciiCaseInsensitive));
    }

    #[test]
    fn test_has_id_method() {
        let tree = setup_test_tree();
        let body = tree.body().expect("body should exist");

        let result = query_selector(&tree, body, "#first", None);
        assert!(result.is_some());

        let elem = DomElement::new(&tree, result.unwrap());
        assert!(elem.has_id(&CssString("first".into()), CaseSensitivity::AsciiCaseInsensitive));
        assert!(!elem.has_id(&CssString("second".into()), CaseSensitivity::AsciiCaseInsensitive));
    }

    #[test]
    fn test_is_empty_pseudo_class() {
        let mut tree = DomTree::new();
        let empty_div = tree.create_element("div");
        let non_empty_div = tree.create_element("div");
        let text = tree.create_text("content");

        tree.append_child(tree.document(), empty_div);
        tree.append_child(tree.document(), non_empty_div);
        tree.append_child(non_empty_div, text);

        let empty_elem = DomElement::new(&tree, empty_div);
        let non_empty_elem = DomElement::new(&tree, non_empty_div);

        assert!(empty_elem.is_empty());
        assert!(!non_empty_elem.is_empty());
    }

    #[test]
    fn test_is_root_pseudo_class() {
        let tree = setup_test_tree();
        let html = tree.get_elements_by_tag_name("html").into_iter().next().unwrap();
        let body = tree.body().unwrap();

        let html_elem = DomElement::new(&tree, html);
        let body_elem = DomElement::new(&tree, body);

        assert!(html_elem.is_root());
        assert!(!body_elem.is_root());
    }

    #[test]
    fn test_first_child_pseudo_class() {
        let tree = setup_test_tree();
        let body = tree.body().expect("body should exist");

        // The .container div should be the first element child of body
        let result = query_selector(&tree, body, "div:first-child", None);
        assert!(result.is_some());

        let node_id = result.unwrap();
        assert_eq!(tree.get_attribute(node_id, "class"), Some("container".to_string()));
    }

    #[test]
    fn test_last_child_pseudo_class() {
        let tree = setup_test_tree();
        let body = tree.body().expect("body should exist");

        // The third <p> should be the last element child of body
        let result = query_selector(&tree, body, "p:last-child", None);
        assert!(result.is_some());

        let text = tree.get_text_content(result.unwrap());
        assert_eq!(text, "Third paragraph");
    }

    #[test]
    fn test_is_link_method() {
        let mut tree = DomTree::new();
        let link = tree.create_element_with_attrs("a", vec![
            ("href".to_string(), "https://example.com".to_string()),
        ]);
        let not_link = tree.create_element("a"); // <a> without href
        let div = tree.create_element("div");

        tree.append_child(tree.document(), link);
        tree.append_child(tree.document(), not_link);
        tree.append_child(tree.document(), div);

        let link_elem = DomElement::new(&tree, link);
        let not_link_elem = DomElement::new(&tree, not_link);
        let div_elem = DomElement::new(&tree, div);

        assert!(link_elem.is_link());
        assert!(!not_link_elem.is_link());
        assert!(!div_elem.is_link());
    }

    #[test]
    fn test_query_selector_all_empty_on_invalid_selector() {
        let tree = setup_test_tree();
        let body = tree.body().expect("body should exist");

        // Invalid selector should return empty vec
        let results = query_selector_all(&tree, body, "::invalid::::syntax", None);
        assert_eq!(results.len(), 0);
    }
}
