//! Style tree computation — DFS walk that populates computed styles on every element.
//!
//! This module provides `compute_all_styles`, which:
//! 1. Collects stylesheets from `<style>` elements
//! 2. Collects inline styles from `style=""` attributes
//! 3. Builds UA (user-agent) default rules
//! 4. Walks the DOM tree in DFS order (parent before child)
//! 5. For each Element node, cascades + resolves a `ComputedStyle` and stores it
//!    as a `HashMap<String, String>` on the node's `computed_style` field

use std::collections::HashMap;

use crate::css::cascade::{cascade_element, stylesheet_to_rules, CascadeDeclaration, CascadeRule, CascadedValues};
use crate::css::collection::{collect_inline_styles, ua_stylesheet};
use crate::css::computed::{
    ComputedColor, ComputedStyle, Display, FontStyle, Overflow, Position, TextAlign, TextDecoration, Visibility,
};
use crate::css::parser::parse_stylesheet;
use crate::css::selector_impl::BrailleSelectorParser;
use crate::dom::node::{NodeData, NodeId};
use crate::dom::tree::DomTree;
use cssparser::ParserInput;
use selectors::parser::{ParseRelative, SelectorList};

// ---------------------------------------------------------------------------
// UA rules — parsed through the real cssparser path
// ---------------------------------------------------------------------------

/// Build CascadeRules from the UA stylesheet returned by `collection::ua_stylesheet()`.
///
/// Each `CollectedRule` has a `selector_text` string. We parse that into a
/// `SelectorList<BrailleSelectorImpl>` via cssparser + BrailleSelectorParser,
/// then wrap it into a `CascadeRule`.
fn build_ua_rules() -> Vec<CascadeRule> {
    let collected = ua_stylesheet();
    let mut rules = Vec::new();

    for cr in &collected {
        let selector_list = match parse_selector_text(&cr.selector_text) {
            Some(sl) => sl,
            None => continue, // skip unparseable selectors
        };
        let declarations: Vec<CascadeDeclaration> = cr
            .declarations
            .iter()
            .map(|(prop, val, imp)| CascadeDeclaration {
                property: prop.clone(),
                value: val.clone(),
                important: *imp,
            })
            .collect();
        rules.push(CascadeRule {
            selector: selector_list,
            declarations,
            source_order: cr.source_order,
        });
    }

    rules
}

/// Build CascadeRules from the author stylesheets collected from `<style>` elements.
///
/// We re-parse each collected rule's CSS text through the proper `parse_stylesheet`
/// path so that selectors are parsed by cssparser/selectors rather than the simple
/// parser in collection.rs. We use the full stylesheet text gathered from each
/// `<style>` element for accurate parsing.
fn build_author_rules(tree: &DomTree) -> Vec<CascadeRule> {
    let mut all_rules = Vec::new();
    let mut source_order = 0;

    // Find all <style> elements and parse their CSS through the real parser
    let style_nodes = tree.get_elements_by_tag_name("style");
    for style_node_id in style_nodes {
        let css_text = tree.get_text_content(style_node_id);
        let stylesheet = parse_stylesheet(&css_text);
        let rules = stylesheet_to_rules(&stylesheet, source_order);
        source_order += rules.len();
        all_rules.extend(rules);
    }

    all_rules
}

/// Parse a CSS selector text string into a `SelectorList`.
fn parse_selector_text(selector_text: &str) -> Option<SelectorList<crate::css::selector_impl::BrailleSelectorImpl>> {
    let mut input = ParserInput::new(selector_text);
    let mut parser = cssparser::Parser::new(&mut input);
    SelectorList::parse(&BrailleSelectorParser, &mut parser, ParseRelative::No).ok()
}

// ---------------------------------------------------------------------------
// Inline style lookup
// ---------------------------------------------------------------------------

/// Build a lookup from NodeId -> inline declarations (property, value, important).
///
/// The `collect_inline_styles` function returns `(NodeId, Vec<(String, String)>)` pairs
/// without !important information. We convert them to `(String, String, bool)` tuples
/// where the bool is always `false` (inline styles collected this way have had
/// `!important` stripped — see collection.rs `parse_inline_style`).
fn build_inline_map(tree: &DomTree) -> HashMap<NodeId, Vec<(String, String, bool)>> {
    let collected = collect_inline_styles(tree);
    let mut map = HashMap::new();
    for (node_id, decls) in collected {
        let tuples: Vec<(String, String, bool)> = decls.into_iter().map(|(prop, val)| (prop, val, false)).collect();
        map.insert(node_id, tuples);
    }
    map
}

// ---------------------------------------------------------------------------
// ComputedStyle -> HashMap<String, String> conversion
// ---------------------------------------------------------------------------

fn format_display(d: Display) -> &'static str {
    match d {
        Display::Block => "block",
        Display::Inline => "inline",
        Display::InlineBlock => "inline-block",
        Display::Flex => "flex",
        Display::Grid => "grid",
        Display::None => "none",
        Display::Table => "table",
        Display::TableRow => "table-row",
        Display::TableCell => "table-cell",
        Display::ListItem => "list-item",
    }
}

fn format_visibility(v: Visibility) -> &'static str {
    match v {
        Visibility::Visible => "visible",
        Visibility::Hidden => "hidden",
        Visibility::Collapse => "collapse",
    }
}

fn format_position(p: Position) -> &'static str {
    match p {
        Position::Static => "static",
        Position::Relative => "relative",
        Position::Absolute => "absolute",
        Position::Fixed => "fixed",
        Position::Sticky => "sticky",
    }
}

fn format_text_align(ta: TextAlign) -> &'static str {
    match ta {
        TextAlign::Left => "left",
        TextAlign::Right => "right",
        TextAlign::Center => "center",
        TextAlign::Justify => "justify",
    }
}

fn format_text_decoration(td: TextDecoration) -> &'static str {
    match td {
        TextDecoration::None => "none",
        TextDecoration::Underline => "underline",
        TextDecoration::Overline => "overline",
        TextDecoration::LineThrough => "line-through",
    }
}

fn format_font_style(fs: FontStyle) -> &'static str {
    match fs {
        FontStyle::Normal => "normal",
        FontStyle::Italic => "italic",
        FontStyle::Oblique => "oblique",
    }
}

fn format_overflow(o: Overflow) -> &'static str {
    match o {
        Overflow::Visible => "visible",
        Overflow::Hidden => "hidden",
        Overflow::Scroll => "scroll",
        Overflow::Auto => "auto",
    }
}

fn format_color(c: &ComputedColor) -> String {
    if c.a == 1.0 {
        format!("rgb({}, {}, {})", c.r, c.g, c.b)
    } else {
        format!("rgba({}, {}, {}, {})", c.r, c.g, c.b, c.a)
    }
}

fn format_length_clean(v: f32) -> String {
    if v == 0.0 {
        "0px".to_string()
    } else {
        // Format with up to 2 decimal places, trimming trailing zeros
        let s = format!("{:.2}", v);
        let s = s.trim_end_matches('0').trim_end_matches('.');
        format!("{}px", s)
    }
}

/// Convert a `ComputedStyle` into a `HashMap<String, String>` suitable for
/// storing on `Node::computed_style`.
fn computed_style_to_map(style: &ComputedStyle) -> HashMap<String, String> {
    let mut map = HashMap::new();

    map.insert("display".to_string(), format_display(style.display).to_string());
    map.insert(
        "visibility".to_string(),
        format_visibility(style.visibility).to_string(),
    );
    map.insert("color".to_string(), format_color(&style.color));
    map.insert("background-color".to_string(), format_color(&style.background_color));
    map.insert("font-size".to_string(), format_length_clean(style.font_size));
    map.insert("font-weight".to_string(), style.font_weight.to_string());
    map.insert(
        "font-style".to_string(),
        format_font_style(style.font_style).to_string(),
    );
    map.insert("font-family".to_string(), style.font_family.clone());
    map.insert("line-height".to_string(), format_length_clean(style.line_height));
    map.insert(
        "text-align".to_string(),
        format_text_align(style.text_align).to_string(),
    );
    map.insert(
        "text-decoration".to_string(),
        format_text_decoration(style.text_decoration).to_string(),
    );
    map.insert("margin-top".to_string(), format_length_clean(style.margin_top));
    map.insert("margin-right".to_string(), format_length_clean(style.margin_right));
    map.insert("margin-bottom".to_string(), format_length_clean(style.margin_bottom));
    map.insert("margin-left".to_string(), format_length_clean(style.margin_left));
    map.insert("padding-top".to_string(), format_length_clean(style.padding_top));
    map.insert("padding-right".to_string(), format_length_clean(style.padding_right));
    map.insert("padding-bottom".to_string(), format_length_clean(style.padding_bottom));
    map.insert("padding-left".to_string(), format_length_clean(style.padding_left));

    match style.width {
        Some(w) => map.insert("width".to_string(), format_length_clean(w)),
        None => map.insert("width".to_string(), "auto".to_string()),
    };
    match style.height {
        Some(h) => map.insert("height".to_string(), format_length_clean(h)),
        None => map.insert("height".to_string(), "auto".to_string()),
    };

    map.insert("position".to_string(), format_position(style.position).to_string());
    map.insert("opacity".to_string(), format!("{}", style.opacity));
    map.insert("overflow".to_string(), format_overflow(style.overflow).to_string());

    map
}

// ---------------------------------------------------------------------------
// Bridge: cascade::CascadedEntry -> computed::CascadedEntry
// ---------------------------------------------------------------------------

/// Convert cascade module's CascadedValues into the computed module's CascadedEntry map.
///
/// The cascade and computed modules define their own `CascadedEntry` types with identical
/// fields (`value: String`, `important: bool`). This function bridges between them.
fn cascade_to_computed_entries(cascaded: &CascadedValues) -> HashMap<String, crate::css::computed::CascadedEntry> {
    cascaded
        .iter()
        .map(|(prop, entry)| {
            (
                prop.clone(),
                crate::css::computed::CascadedEntry {
                    value: entry.value.clone(),
                    important: entry.important,
                },
            )
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Core: compute_all_styles
// ---------------------------------------------------------------------------

/// Compute CSS styles for every element in the DOM tree.
///
/// After this function returns, every Element node in the tree has its
/// `computed_style` field populated with a `HashMap<String, String>` containing
/// the fully resolved computed style values.
///
/// The walk is DFS (parent before child) so that inheritance works correctly:
/// a child can look up its parent's already-computed style.
pub fn compute_all_styles(tree: &mut DomTree) {
    // 1. Build UA rules (parsed through the real cssparser path)
    let ua_rules = build_ua_rules();

    // 2. Build author rules from <style> elements (parsed through real cssparser)
    let author_rules = build_author_rules(tree);

    // 3. Build inline style lookup
    let inline_map = build_inline_map(tree);

    // 4. DFS walk from the document root
    //    We can't hold a mutable borrow on the tree while also reading it for
    //    cascade_element (which needs &DomTree). So we do a two-phase approach
    //    per level: collect children, compute style, store, then recurse.
    //
    //    Actually, cascade_element needs &DomTree (immutable) but we need &mut DomTree
    //    to store results. We'll collect all computations first, then apply them.
    //    But that breaks inheritance since children need parent's computed style.
    //
    //    Solution: We store ComputedStyle objects in a side map indexed by NodeId,
    //    then after the full walk, we write them all back to the tree.

    let mut computed_styles: HashMap<NodeId, ComputedStyle> = HashMap::new();

    // Build a DFS order list of (node_id, is_element) pairs
    let mut dfs_order: Vec<NodeId> = Vec::new();
    {
        let mut stack = vec![tree.document()];
        while let Some(nid) = stack.pop() {
            dfs_order.push(nid);
            let children: Vec<NodeId> = tree.get_node(nid).children.clone();
            // Push in reverse so that leftmost child is processed first
            for &child in children.iter().rev() {
                stack.push(child);
            }
        }
    }

    // Phase 1: compute styles for all elements in DFS order
    for &node_id in &dfs_order {
        let is_element = matches!(tree.get_node(node_id).data, NodeData::Element { .. });
        if !is_element {
            continue;
        }

        // Find parent's ComputedStyle (if parent is an element with computed style)
        let parent_style = find_parent_computed_style(tree, node_id, &computed_styles);

        // Get inline declarations for this element
        let empty_inline = Vec::new();
        let inline_decls = inline_map.get(&node_id).unwrap_or(&empty_inline);

        // Cascade: determine which declaration wins for each property
        let cascaded = cascade_element(tree, node_id, &ua_rules, &author_rules, inline_decls);

        // Convert cascade::CascadedEntry -> computed::CascadedEntry
        let computed_entries = cascade_to_computed_entries(&cascaded);

        // Resolve: turn cascaded values + inheritance into fully computed style
        let style = crate::css::computed::resolve_style(&computed_entries, parent_style);

        computed_styles.insert(node_id, style);
    }

    // Phase 2: write computed styles to the tree as HashMap<String, String>
    for (node_id, style) in &computed_styles {
        let map = computed_style_to_map(style);
        tree.get_node_mut(*node_id).computed_style = Some(map);
    }
}

/// Walk up the tree from `node_id` to find the nearest ancestor Element
/// that has a computed style, and return a reference to it.
fn find_parent_computed_style<'a>(
    tree: &DomTree,
    node_id: NodeId,
    computed_styles: &'a HashMap<NodeId, ComputedStyle>,
) -> Option<&'a ComputedStyle> {
    let mut current = tree.get_node(node_id).parent;
    while let Some(pid) = current {
        if matches!(tree.get_node(pid).data, NodeData::Element { .. }) {
            if let Some(style) = computed_styles.get(&pid) {
                return Some(style);
            }
        }
        current = tree.get_node(pid).parent;
    }
    None
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::html::parse_html;
    use std::rc::Rc;

    /// Helper: parse HTML and compute styles, returning the tree by extracting from RefCell.
    fn styled_tree(html: &str) -> DomTree {
        let tree_rc = parse_html(html);
        let tree_refcell = Rc::try_unwrap(tree_rc).expect("should have single Rc owner");
        let mut tree = tree_refcell.into_inner();
        compute_all_styles(&mut tree);
        tree
    }

    /// Find the first element with the given tag name.
    fn find_element<'a>(tree: &'a DomTree, tag: &str) -> Option<(NodeId, &'a HashMap<String, String>)> {
        let elements = tree.get_elements_by_tag_name(tag);
        elements
            .into_iter()
            .next()
            .and_then(|nid| tree.get_node(nid).computed_style.as_ref().map(|cs| (nid, cs)))
    }

    // -----------------------------------------------------------------------
    // 1. Basic UA styles
    // -----------------------------------------------------------------------

    #[test]
    fn basic_ua_styles_block_elements() {
        let tree = styled_tree("<html><body><p>text</p></body></html>");

        let (_, p_style) = find_element(&tree, "p").expect("p should have computed style");
        assert_eq!(p_style.get("display").unwrap(), "block", "p should be display:block");

        let (_, body_style) = find_element(&tree, "body").expect("body should have computed style");
        assert_eq!(
            body_style.get("display").unwrap(),
            "block",
            "body should be display:block"
        );
    }

    // -----------------------------------------------------------------------
    // 2. Hidden elements
    // -----------------------------------------------------------------------

    #[test]
    fn hidden_elements_head_display_none() {
        let tree = styled_tree("<html><head><title>T</title></head><body></body></html>");

        let (_, head_style) = find_element(&tree, "head").expect("head should have computed style");
        assert_eq!(
            head_style.get("display").unwrap(),
            "none",
            "head should be display:none"
        );
    }

    // -----------------------------------------------------------------------
    // 3. Inline style override
    // -----------------------------------------------------------------------

    #[test]
    fn inline_style_override() {
        let tree = styled_tree(r#"<html><body><div style="color: red">text</div></body></html>"#);

        let (_, div_style) = find_element(&tree, "div").expect("div should have computed style");
        let color = div_style.get("color").unwrap();
        // red = rgb(255, 0, 0)
        assert_eq!(color, "rgb(255, 0, 0)", "div color should be red");
    }

    // -----------------------------------------------------------------------
    // 4. Style tag
    // -----------------------------------------------------------------------

    #[test]
    fn style_tag_applies_rules() {
        let tree = styled_tree("<html><head><style>p { color: blue }</style></head><body><p>text</p></body></html>");

        let (_, p_style) = find_element(&tree, "p").expect("p should have computed style");
        let color = p_style.get("color").unwrap();
        // blue = rgb(0, 0, 255)
        assert_eq!(color, "rgb(0, 0, 255)", "p color should be blue from <style>");
    }

    // -----------------------------------------------------------------------
    // 5. Inheritance
    // -----------------------------------------------------------------------

    #[test]
    fn color_inheritance() {
        let tree = styled_tree(r#"<html><body><div style="color: red"><p>text</p></div></body></html>"#);

        let (_, p_style) = find_element(&tree, "p").expect("p should have computed style");
        let color = p_style.get("color").unwrap();
        // p should inherit color: red from parent div
        assert_eq!(color, "rgb(255, 0, 0)", "p should inherit color red from div");
    }

    // -----------------------------------------------------------------------
    // 6. Heading sizes
    // -----------------------------------------------------------------------

    #[test]
    fn heading_sizes_h1_larger_than_h2() {
        let tree = styled_tree("<html><body><h1>Big</h1><h2>Medium</h2></body></html>");

        let (_, h1_style) = find_element(&tree, "h1").expect("h1 should have computed style");
        let (_, h2_style) = find_element(&tree, "h2").expect("h2 should have computed style");

        let h1_size: f32 = h1_style
            .get("font-size")
            .unwrap()
            .trim_end_matches("px")
            .parse()
            .unwrap();
        let h2_size: f32 = h2_style
            .get("font-size")
            .unwrap()
            .trim_end_matches("px")
            .parse()
            .unwrap();

        assert!(
            h1_size > h2_size,
            "h1 font-size ({}) should be larger than h2 font-size ({})",
            h1_size,
            h2_size
        );

        // h1 should also be bold
        let h1_weight: u16 = h1_style.get("font-weight").unwrap().parse().unwrap();
        assert_eq!(h1_weight, 700, "h1 should have font-weight: bold (700)");
    }

    // -----------------------------------------------------------------------
    // 7. display:none from stylesheet
    // -----------------------------------------------------------------------

    #[test]
    fn display_none_from_stylesheet() {
        let tree = styled_tree(
            r#"<html><head><style>.hidden { display: none }</style></head><body><div class="hidden">X</div></body></html>"#,
        );

        let divs = tree.get_elements_by_tag_name("div");
        assert!(!divs.is_empty(), "should find a div");
        let div_node = tree.get_node(divs[0]);
        let style = div_node
            .computed_style
            .as_ref()
            .expect("div should have computed style");
        assert_eq!(
            style.get("display").unwrap(),
            "none",
            "div.hidden should be display:none"
        );
    }

    // -----------------------------------------------------------------------
    // 8. Multiple stylesheets
    // -----------------------------------------------------------------------

    #[test]
    fn multiple_stylesheets() {
        let tree = styled_tree(
            r#"<html><head>
                <style>p { color: red }</style>
                <style>p { font-weight: bold }</style>
            </head><body><p>text</p></body></html>"#,
        );

        let (_, p_style) = find_element(&tree, "p").expect("p should have computed style");

        // The second stylesheet sets font-weight: bold but color: red comes from first
        // Actually, both should apply since they set different properties
        let weight: u16 = p_style.get("font-weight").unwrap().parse().unwrap();
        assert_eq!(weight, 700, "p should have font-weight bold from second stylesheet");

        // Color should be red from first stylesheet
        let color = p_style.get("color").unwrap();
        assert_eq!(color, "rgb(255, 0, 0)", "p should have color red from first stylesheet");
    }

    // -----------------------------------------------------------------------
    // Additional tests
    // -----------------------------------------------------------------------

    #[test]
    fn strong_is_bold() {
        let tree = styled_tree("<html><body><strong>bold text</strong></body></html>");

        let (_, strong_style) = find_element(&tree, "strong").expect("strong should have computed style");
        let weight: u16 = strong_style.get("font-weight").unwrap().parse().unwrap();
        assert_eq!(weight, 700, "strong should be bold (700)");
    }

    #[test]
    fn em_is_italic() {
        let tree = styled_tree("<html><body><em>italic text</em></body></html>");

        let (_, em_style) = find_element(&tree, "em").expect("em should have computed style");
        assert_eq!(
            em_style.get("font-style").unwrap(),
            "italic",
            "em should be font-style: italic"
        );
    }

    #[test]
    fn link_is_blue_underline() {
        let tree = styled_tree(r#"<html><body><a href="/">link</a></body></html>"#);

        let (_, a_style) = find_element(&tree, "a").expect("a should have computed style");
        assert_eq!(
            a_style.get("color").unwrap(),
            "rgb(0, 0, 255)",
            "a should have color blue"
        );
        assert_eq!(
            a_style.get("text-decoration").unwrap(),
            "underline",
            "a should have text-decoration underline"
        );
    }

    #[test]
    fn non_element_nodes_have_no_computed_style() {
        let tree = styled_tree("<html><body><p>some text</p></body></html>");

        // Text nodes should not have computed style
        for nid in 0..tree.node_count() {
            let node = tree.get_node(nid);
            match &node.data {
                NodeData::Text { .. }
                | NodeData::Comment { .. }
                | NodeData::ProcessingInstruction { .. }
                | NodeData::CDATASection { .. }
                | NodeData::Document
                | NodeData::Doctype { .. }
                | NodeData::DocumentFragment
                | NodeData::Attr { .. } => {
                    assert!(
                        node.computed_style.is_none(),
                        "non-element node {} should not have computed_style",
                        nid
                    );
                }
                NodeData::Element { .. } => {
                    // Elements should have computed style
                    assert!(
                        node.computed_style.is_some(),
                        "element node {} should have computed_style",
                        nid
                    );
                }
            }
        }
    }

    #[test]
    fn author_style_overrides_ua() {
        let tree =
            styled_tree(r#"<html><head><style>p { display: inline }</style></head><body><p>text</p></body></html>"#);

        let (_, p_style) = find_element(&tree, "p").expect("p should have computed style");
        assert_eq!(
            p_style.get("display").unwrap(),
            "inline",
            "author display:inline should override UA display:block"
        );
    }

    #[test]
    fn inline_style_overrides_author() {
        let tree = styled_tree(
            r#"<html><head><style>p { color: blue }</style></head><body><p style="color: green">text</p></body></html>"#,
        );

        let (_, p_style) = find_element(&tree, "p").expect("p should have computed style");
        let color = p_style.get("color").unwrap();
        // green = rgb(0, 128, 0)
        assert_eq!(
            color, "rgb(0, 128, 0)",
            "inline color:green should override author color:blue"
        );
    }

    #[test]
    fn deep_inheritance_chain() {
        let tree = styled_tree(
            r#"<html><body><div style="color: red"><section><article><p>deep</p></article></section></div></body></html>"#,
        );

        // section and article are not in the UA stylesheet as block elements,
        // but p should still inherit color from the div through the chain
        let (_, p_style) = find_element(&tree, "p").expect("p should have computed style");
        let color = p_style.get("color").unwrap();
        assert_eq!(color, "rgb(255, 0, 0)", "p should inherit color through deep chain");
    }

    #[test]
    fn initial_style_for_plain_element() {
        let tree = styled_tree("<html><body><span>text</span></body></html>");

        let (_, span_style) = find_element(&tree, "span").expect("span should have computed style");
        // span is inline per UA stylesheet
        assert_eq!(span_style.get("display").unwrap(), "inline");
        // default visibility
        assert_eq!(span_style.get("visibility").unwrap(), "visible");
        // default opacity
        assert_eq!(span_style.get("opacity").unwrap(), "1");
    }
}
