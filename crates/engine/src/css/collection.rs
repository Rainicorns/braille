use crate::dom::node::{NodeData, NodeId};
use crate::dom::tree::DomTree;

/// Origin of a CSS style rule - determines specificity in the cascade
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StyleOrigin {
    UserAgent,  // Browser defaults
    Author,     // Page's CSS
    Inline,     // style="" attribute
}

/// A CSS rule collected from the DOM
#[derive(Debug, Clone, PartialEq)]
pub struct CollectedRule {
    pub origin: StyleOrigin,
    pub selector_text: String,  // Raw selector string, empty for inline
    pub declarations: Vec<(String, String, bool)>,  // (property, value, important)
    pub source_order: usize,
}

/// Collects all stylesheet rules from `<style>` elements in the DOM
///
/// NOTE: This uses a SIMPLE CSS parser for now. This should be replaced
/// with the proper CSS parser from Agent C-1A once it's available.
/// The simple parser only handles basic cases like `selector { prop: value; }`
pub fn collect_stylesheets(tree: &DomTree) -> Vec<CollectedRule> {
    let mut rules = Vec::new();
    let mut source_order = 0;

    // Find all <style> elements
    let style_nodes = tree.get_elements_by_tag_name("style");

    for style_node_id in style_nodes {
        // Get the text content of the <style> element
        let css_text = tree.get_text_content(style_node_id);

        // Parse the CSS text (simple parsing for now)
        let parsed_rules = parse_simple_css(&css_text, source_order);
        source_order += parsed_rules.len();

        rules.extend(parsed_rules);
    }

    rules
}

/// Collects inline styles from all elements with style attributes
///
/// Returns a Vec of (NodeId, Vec<(property, value)>)
pub fn collect_inline_styles(tree: &DomTree) -> Vec<(NodeId, Vec<(String, String)>)> {
    let mut inline_styles = Vec::new();

    // Walk all nodes in the tree
    for node_id in 0..tree.node_count() {
        let node = tree.get_node(node_id);

        // Check if it's an Element with a style attribute
        if let NodeData::Element { ref attributes, .. } = node.data {
            if let Some(style_value) = attributes.iter().find(|(k, _)| k == "style").map(|(_, v)| v) {
                let declarations = parse_inline_style(style_value);
                if !declarations.is_empty() {
                    inline_styles.push((node_id, declarations));
                }
            }
        }
    }

    inline_styles
}

/// Returns the user agent (browser default) stylesheet
///
/// These are the basic styles that browsers apply by default
pub fn ua_stylesheet() -> Vec<CollectedRule> {
    let mut rules = Vec::new();
    let mut source_order = 0;

    // Block-level elements
    rules.push(CollectedRule {
        origin: StyleOrigin::UserAgent,
        selector_text: "html, body, div, p, h1, h2, h3, h4, h5, h6, ul, ol, li, form, table".to_string(),
        declarations: vec![("display".to_string(), "block".to_string(), false)],
        source_order,
    });
    source_order += 1;

    // Inline elements
    rules.push(CollectedRule {
        origin: StyleOrigin::UserAgent,
        selector_text: "span, a, strong, em, b, i, code".to_string(),
        declarations: vec![("display".to_string(), "inline".to_string(), false)],
        source_order,
    });
    source_order += 1;

    // Hidden elements
    rules.push(CollectedRule {
        origin: StyleOrigin::UserAgent,
        selector_text: "head, script, style, meta, link, title".to_string(),
        declarations: vec![("display".to_string(), "none".to_string(), false)],
        source_order,
    });
    source_order += 1;

    // Heading styles
    rules.push(CollectedRule {
        origin: StyleOrigin::UserAgent,
        selector_text: "h1".to_string(),
        declarations: vec![
            ("font-size".to_string(), "2em".to_string(), false),
            ("font-weight".to_string(), "bold".to_string(), false),
        ],
        source_order,
    });
    source_order += 1;

    rules.push(CollectedRule {
        origin: StyleOrigin::UserAgent,
        selector_text: "h2".to_string(),
        declarations: vec![
            ("font-size".to_string(), "1.5em".to_string(), false),
            ("font-weight".to_string(), "bold".to_string(), false),
        ],
        source_order,
    });
    source_order += 1;

    rules.push(CollectedRule {
        origin: StyleOrigin::UserAgent,
        selector_text: "h3".to_string(),
        declarations: vec![
            ("font-size".to_string(), "1.17em".to_string(), false),
            ("font-weight".to_string(), "bold".to_string(), false),
        ],
        source_order,
    });
    source_order += 1;

    // Text styling
    rules.push(CollectedRule {
        origin: StyleOrigin::UserAgent,
        selector_text: "strong, b".to_string(),
        declarations: vec![("font-weight".to_string(), "bold".to_string(), false)],
        source_order,
    });
    source_order += 1;

    rules.push(CollectedRule {
        origin: StyleOrigin::UserAgent,
        selector_text: "em, i".to_string(),
        declarations: vec![("font-style".to_string(), "italic".to_string(), false)],
        source_order,
    });
    source_order += 1;

    // Link styling
    rules.push(CollectedRule {
        origin: StyleOrigin::UserAgent,
        selector_text: "a".to_string(),
        declarations: vec![
            ("color".to_string(), "blue".to_string(), false),
            ("text-decoration".to_string(), "underline".to_string(), false),
        ],
        source_order,
    });

    rules
}

/// Simple CSS parser - handles basic "selector { prop: value; }" format
///
/// TEMPORARY: This should be replaced with the proper CSS parser from Agent C-1A.
/// This simple implementation only handles:
/// - Single selectors (not compound)
/// - Simple property: value declarations
/// - Basic !important handling
///
/// It does NOT handle:
/// - Multiple selectors (comma-separated)
/// - Nested rules
/// - @-rules
/// - Complex values with braces or semicolons
/// - Comments (beyond basic strip)
fn parse_simple_css(css_text: &str, start_order: usize) -> Vec<CollectedRule> {
    let mut rules = Vec::new();
    let mut source_order = start_order;

    // Remove comments (simple /* */ removal)
    let css_text = remove_comments(css_text);

    // Split by closing brace to get rule blocks
    let parts: Vec<&str> = css_text.split('}').collect();

    for part in parts {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        // Find the opening brace
        if let Some(brace_pos) = part.find('{') {
            let selector = part[..brace_pos].trim().to_string();
            let declarations_text = part[brace_pos + 1..].trim();

            if selector.is_empty() {
                continue;
            }

            // Parse declarations
            let declarations = parse_declarations(declarations_text);

            if !declarations.is_empty() {
                rules.push(CollectedRule {
                    origin: StyleOrigin::Author,
                    selector_text: selector,
                    declarations,
                    source_order,
                });
                source_order += 1;
            }
        }
    }

    rules
}

/// Parse inline style attribute value into property:value pairs
///
/// Example: "color: red; font-size: 16px" -> [("color", "red"), ("font-size", "16px")]
fn parse_inline_style(style_text: &str) -> Vec<(String, String)> {
    let mut declarations = Vec::new();

    // Split by semicolon
    for decl in style_text.split(';') {
        let decl = decl.trim();
        if decl.is_empty() {
            continue;
        }

        // Split on first colon
        if let Some(colon_pos) = decl.find(':') {
            let property = decl[..colon_pos].trim().to_string();
            let value = decl[colon_pos + 1..].trim();

            // Remove !important if present (inline styles are already highest specificity)
            let value = value.replace("!important", "").trim().to_string();

            if !property.is_empty() && !value.is_empty() {
                declarations.push((property, value));
            }
        }
    }

    declarations
}

/// Parse declaration block into (property, value, important) tuples
fn parse_declarations(text: &str) -> Vec<(String, String, bool)> {
    let mut declarations = Vec::new();

    // Split by semicolon
    for decl in text.split(';') {
        let decl = decl.trim();
        if decl.is_empty() {
            continue;
        }

        // Split on first colon
        if let Some(colon_pos) = decl.find(':') {
            let property = decl[..colon_pos].trim().to_string();
            let value_part = decl[colon_pos + 1..].trim();

            // Check for !important
            let (value, important) = if value_part.ends_with("!important") {
                let val = value_part[..value_part.len() - 10].trim().to_string();
                (val, true)
            } else {
                (value_part.to_string(), false)
            };

            if !property.is_empty() && !value.is_empty() {
                declarations.push((property, value, important));
            }
        }
    }

    declarations
}

/// Remove CSS comments (/* ... */)
fn remove_comments(text: &str) -> String {
    let mut result = String::new();
    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '/' && chars.peek() == Some(&'*') {
            // Start of comment, skip until */
            chars.next(); // consume '*'
            let mut prev = ' ';
            while let Some(ch) = chars.next() {
                if prev == '*' && ch == '/' {
                    break;
                }
                prev = ch;
            }
        } else {
            result.push(ch);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ua_stylesheet_returns_defaults() {
        let rules = ua_stylesheet();

        // Should have multiple rules
        assert!(rules.len() > 0);

        // All should be UserAgent origin
        for rule in &rules {
            assert_eq!(rule.origin, StyleOrigin::UserAgent);
        }

        // Check for some expected rules
        let display_block = rules.iter().find(|r| r.selector_text.contains("div"));
        assert!(display_block.is_some());

        let h1_rule = rules.iter().find(|r| r.selector_text == "h1");
        assert!(h1_rule.is_some());
        if let Some(rule) = h1_rule {
            assert!(rule.declarations.iter().any(|(p, v, _)| p == "font-size" && v == "2em"));
            assert!(rule.declarations.iter().any(|(p, v, _)| p == "font-weight" && v == "bold"));
        }
    }

    #[test]
    fn test_collect_stylesheets_finds_style_elements() {
        let mut tree = DomTree::new();
        let html = tree.create_element("html");
        let head = tree.create_element("head");
        let style = tree.create_element("style");
        let css_text = tree.create_text("h1 { color: red; }");

        tree.append_child(tree.document(), html);
        tree.append_child(html, head);
        tree.append_child(head, style);
        tree.append_child(style, css_text);

        let rules = collect_stylesheets(&tree);

        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].origin, StyleOrigin::Author);
        assert_eq!(rules[0].selector_text, "h1");
        assert_eq!(rules[0].declarations.len(), 1);
        assert_eq!(rules[0].declarations[0].0, "color");
        assert_eq!(rules[0].declarations[0].1, "red");
        assert_eq!(rules[0].declarations[0].2, false); // not important
    }

    #[test]
    fn test_collect_stylesheets_handles_multiple_style_elements() {
        let mut tree = DomTree::new();
        let html = tree.create_element("html");
        let head = tree.create_element("head");
        let style1 = tree.create_element("style");
        let css1 = tree.create_text("h1 { color: red; }");
        let style2 = tree.create_element("style");
        let css2 = tree.create_text("p { margin: 10px; }");

        tree.append_child(tree.document(), html);
        tree.append_child(html, head);
        tree.append_child(head, style1);
        tree.append_child(style1, css1);
        tree.append_child(head, style2);
        tree.append_child(style2, css2);

        let rules = collect_stylesheets(&tree);

        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].selector_text, "h1");
        assert_eq!(rules[1].selector_text, "p");
    }

    #[test]
    fn test_collect_inline_styles_finds_style_attributes() {
        let mut tree = DomTree::new();
        let div = tree.create_element_with_attrs(
            "div",
            vec![("style".to_string(), "color: blue; font-size: 14px".to_string())]
        );
        tree.append_child(tree.document(), div);

        let inline_styles = collect_inline_styles(&tree);

        assert_eq!(inline_styles.len(), 1);
        assert_eq!(inline_styles[0].0, div);
        assert_eq!(inline_styles[0].1.len(), 2);

        let decls = &inline_styles[0].1;
        assert!(decls.contains(&("color".to_string(), "blue".to_string())));
        assert!(decls.contains(&("font-size".to_string(), "14px".to_string())));
    }

    #[test]
    fn test_simple_css_parsing_basic_rule() {
        let css = "h1 { color: red; }";
        let rules = parse_simple_css(css, 0);

        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].selector_text, "h1");
        assert_eq!(rules[0].declarations.len(), 1);
        assert_eq!(rules[0].declarations[0], ("color".to_string(), "red".to_string(), false));
    }

    #[test]
    fn test_simple_css_parsing_multiple_declarations() {
        let css = "h1 { color: red; font-size: 2em; font-weight: bold; }";
        let rules = parse_simple_css(css, 0);

        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].declarations.len(), 3);
        assert!(rules[0].declarations.contains(&("color".to_string(), "red".to_string(), false)));
        assert!(rules[0].declarations.contains(&("font-size".to_string(), "2em".to_string(), false)));
        assert!(rules[0].declarations.contains(&("font-weight".to_string(), "bold".to_string(), false)));
    }

    #[test]
    fn test_simple_css_parsing_important() {
        let css = "h1 { color: red !important; }";
        let rules = parse_simple_css(css, 0);

        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].declarations.len(), 1);
        assert_eq!(rules[0].declarations[0], ("color".to_string(), "red".to_string(), true));
    }

    #[test]
    fn test_simple_css_parsing_multiple_rules() {
        let css = "h1 { color: red; } p { margin: 10px; }";
        let rules = parse_simple_css(css, 0);

        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].selector_text, "h1");
        assert_eq!(rules[1].selector_text, "p");
    }

    #[test]
    fn test_simple_css_parsing_with_comments() {
        let css = "/* This is a comment */ h1 { color: red; } /* Another comment */";
        let rules = parse_simple_css(css, 0);

        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].selector_text, "h1");
    }

    #[test]
    fn test_simple_css_parsing_whitespace() {
        let css = "  h1  {  color:  red  ;  }  ";
        let rules = parse_simple_css(css, 0);

        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].selector_text, "h1");
        assert_eq!(rules[0].declarations[0].0, "color");
        assert_eq!(rules[0].declarations[0].1, "red");
    }

    #[test]
    fn test_empty_page_returns_only_ua_rules() {
        let tree = DomTree::new();

        let author_rules = collect_stylesheets(&tree);
        assert_eq!(author_rules.len(), 0);

        let ua_rules = ua_stylesheet();
        assert!(ua_rules.len() > 0);

        for rule in &ua_rules {
            assert_eq!(rule.origin, StyleOrigin::UserAgent);
        }
    }

    #[test]
    fn test_parse_inline_style_basic() {
        let declarations = parse_inline_style("color: red");
        assert_eq!(declarations.len(), 1);
        assert_eq!(declarations[0], ("color".to_string(), "red".to_string()));
    }

    #[test]
    fn test_parse_inline_style_multiple() {
        let declarations = parse_inline_style("color: red; font-size: 16px; margin: 0");
        assert_eq!(declarations.len(), 3);
        assert!(declarations.contains(&("color".to_string(), "red".to_string())));
        assert!(declarations.contains(&("font-size".to_string(), "16px".to_string())));
        assert!(declarations.contains(&("margin".to_string(), "0".to_string())));
    }

    #[test]
    fn test_parse_inline_style_trailing_semicolon() {
        let declarations = parse_inline_style("color: red;");
        assert_eq!(declarations.len(), 1);
        assert_eq!(declarations[0], ("color".to_string(), "red".to_string()));
    }

    #[test]
    fn test_parse_inline_style_empty() {
        let declarations = parse_inline_style("");
        assert_eq!(declarations.len(), 0);
    }

    #[test]
    fn test_parse_inline_style_removes_important() {
        let declarations = parse_inline_style("color: red !important");
        assert_eq!(declarations.len(), 1);
        assert_eq!(declarations[0], ("color".to_string(), "red".to_string()));
    }

    #[test]
    fn test_source_order_increments() {
        let css = "h1 { color: red; } p { color: blue; }";
        let rules = parse_simple_css(css, 0);

        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].source_order, 0);
        assert_eq!(rules[1].source_order, 1);
    }

    #[test]
    fn test_collect_stylesheets_preserves_source_order() {
        let mut tree = DomTree::new();
        let html = tree.create_element("html");
        let head = tree.create_element("head");
        let style = tree.create_element("style");
        let css_text = tree.create_text("h1 { color: red; } p { color: blue; } div { color: green; }");

        tree.append_child(tree.document(), html);
        tree.append_child(html, head);
        tree.append_child(head, style);
        tree.append_child(style, css_text);

        let rules = collect_stylesheets(&tree);

        assert_eq!(rules.len(), 3);
        assert_eq!(rules[0].source_order, 0);
        assert_eq!(rules[1].source_order, 1);
        assert_eq!(rules[2].source_order, 2);
    }
}
