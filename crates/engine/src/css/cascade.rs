//! CSS cascade algorithm implementation.
//!
//! This module implements the CSS cascade: given a set of UA rules, author rules,
//! and inline styles, it determines which declaration wins for each property on
//! a given element.
//!
//! The cascade ordering (highest priority first):
//! 1. Important UA declarations
//! 2. Important inline declarations
//! 3. Important author declarations
//! 4. Normal inline declarations
//! 5. Normal author declarations
//! 6. Normal UA declarations
//!
//! Within the same origin+importance level, higher specificity wins.
//! At the same specificity, later source order wins.

use std::collections::HashMap;

use crate::css::matching::DomElement;
use crate::css::parser::Stylesheet;
use crate::css::selector_impl::BrailleSelectorImpl;
use crate::dom::node::NodeId;
use crate::dom::tree::DomTree;
use selectors::matching::{
    matches_selector, MatchingContext, MatchingForInvalidation, MatchingMode, NeedsSelectorFlags, QuirksMode,
    SelectorCaches,
};
use selectors::parser::SelectorList;

/// The result of cascading for a single element: property name -> winning entry.
pub type CascadedValues = HashMap<String, CascadedEntry>;

/// A single cascaded value with its priority information.
#[derive(Debug, Clone)]
pub struct CascadedEntry {
    pub value: String,
    pub important: bool,
}

/// A pre-processed rule ready for cascade matching.
#[derive(Debug, Clone)]
pub struct CascadeRule {
    pub selector: SelectorList<BrailleSelectorImpl>,
    pub declarations: Vec<CascadeDeclaration>,
    pub source_order: usize,
}

/// A single declaration within a cascade rule.
#[derive(Debug, Clone)]
pub struct CascadeDeclaration {
    pub property: String,
    pub value: String,
    pub important: bool,
}

/// Cascade priority levels for origin+importance combinations.
///
/// Ordering follows the CSS cascade specification (highest priority last):
/// Normal UA < Normal Author < Normal Inline < Important Author < Important Inline < Important UA
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
enum CascadePriority {
    NormalUA = 0,
    NormalAuthor = 1,
    NormalInline = 2,
    ImportantAuthor = 3,
    ImportantInline = 4,
    ImportantUA = 5,
}

/// Cascade priority key used for comparing declarations.
///
/// Higher values win. The derived Ord gives us lexicographic comparison
/// which is exactly what we want: origin_importance first, then specificity,
/// then source_order.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct CascadeKey {
    origin_importance: CascadePriority,
    specificity: u32,
    source_order: u32,
}

/// A candidate declaration collected during cascade resolution.
struct CascadeCandidate {
    key: CascadeKey,
    value: String,
    important: bool,
}

/// Origin of a stylesheet rule.
#[derive(Debug, Clone, Copy)]
enum Origin {
    UserAgent,
    Author,
}

/// Cascade all styles for a single element.
///
/// Collects matching rules from UA stylesheet, author stylesheets, and inline styles,
/// then resolves which declaration wins for each property based on cascade ordering.
pub fn cascade_element(
    tree: &DomTree,
    node_id: NodeId,
    ua_rules: &[CascadeRule],
    author_rules: &[CascadeRule],
    inline_declarations: &[(String, String, bool)],
) -> CascadedValues {
    let mut candidates: HashMap<String, Vec<CascadeCandidate>> = HashMap::new();

    let dom_element = DomElement::new(tree, node_id);

    // Collect matching UA rules
    collect_matching_rules(&dom_element, ua_rules, Origin::UserAgent, &mut candidates);

    // Collect matching author rules
    collect_matching_rules(&dom_element, author_rules, Origin::Author, &mut candidates);

    // Collect inline declarations (always apply, no selector matching needed)
    for (property, value, important) in inline_declarations {
        let origin_importance = if *important {
            CascadePriority::ImportantInline
        } else {
            CascadePriority::NormalInline
        };

        let candidate = CascadeCandidate {
            key: CascadeKey {
                origin_importance,
                specificity: 0,
                source_order: 0,
            },
            value: value.clone(),
            important: *important,
        };

        candidates.entry(property.clone()).or_default().push(candidate);
    }

    // Resolve: for each property, pick the candidate with the highest CascadeKey
    let mut result = CascadedValues::new();

    for (property, property_candidates) in candidates {
        let winner = property_candidates.into_iter().max_by(|a, b| a.key.cmp(&b.key));

        if let Some(winner) = winner {
            result.insert(
                property,
                CascadedEntry {
                    value: winner.value,
                    important: winner.important,
                },
            );
        }
    }

    result
}

/// Collect all matching declarations from a set of rules for an element.
fn collect_matching_rules(
    element: &DomElement<'_>,
    rules: &[CascadeRule],
    origin: Origin,
    candidates: &mut HashMap<String, Vec<CascadeCandidate>>,
) {
    for rule in rules {
        let mut best_specificity: Option<u32> = None;

        let mut caches = SelectorCaches::default();
        let mut context = MatchingContext::new(
            MatchingMode::Normal,
            None,
            &mut caches,
            QuirksMode::NoQuirks,
            NeedsSelectorFlags::No,
            MatchingForInvalidation::No,
        );

        for selector in rule.selector.slice().iter() {
            if matches_selector(selector, 0, None, element, &mut context) {
                let spec = selector.specificity();
                best_specificity = Some(match best_specificity {
                    Some(existing) => existing.max(spec),
                    None => spec,
                });
            }
        }

        if let Some(specificity) = best_specificity {
            for decl in &rule.declarations {
                let origin_importance = match (origin, decl.important) {
                    (Origin::UserAgent, false) => CascadePriority::NormalUA,
                    (Origin::Author, false) => CascadePriority::NormalAuthor,
                    (Origin::Author, true) => CascadePriority::ImportantAuthor,
                    (Origin::UserAgent, true) => CascadePriority::ImportantUA,
                };

                let candidate = CascadeCandidate {
                    key: CascadeKey {
                        origin_importance,
                        specificity,
                        source_order: rule.source_order as u32,
                    },
                    value: decl.value.clone(),
                    important: decl.important,
                };

                candidates.entry(decl.property.clone()).or_default().push(candidate);
            }
        }
    }
}

/// Convert a parsed Stylesheet into CascadeRules.
///
/// Each Rule in the Stylesheet becomes a CascadeRule with incrementing source_order
/// starting from `start_order`.
pub fn stylesheet_to_rules(sheet: &Stylesheet, start_order: usize) -> Vec<CascadeRule> {
    sheet
        .rules
        .iter()
        .enumerate()
        .map(|(i, rule)| CascadeRule {
            selector: rule.selectors.clone(),
            declarations: rule
                .declarations
                .iter()
                .map(|d| CascadeDeclaration {
                    property: d.property.clone(),
                    value: d.value.clone(),
                    important: d.important,
                })
                .collect(),
            source_order: start_order + i,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::css::parser::parse_stylesheet;

    fn build_tree_with_element(tag: &str, attrs: Vec<(String, String)>) -> (DomTree, NodeId) {
        use crate::dom::node::DomAttribute;
        let mut tree = DomTree::new();
        let html = tree.create_element("html");
        let body = tree.create_element("body");
        let dom_attrs: Vec<DomAttribute> = attrs.into_iter().map(|(k, v)| DomAttribute::new(&k, &v)).collect();
        let target = tree.create_element_with_attrs(tag, dom_attrs);
        tree.append_child(tree.document(), html);
        tree.append_child(html, body);
        tree.append_child(body, target);
        (tree, target)
    }

    fn parse_author_rules(css: &str) -> Vec<CascadeRule> {
        let sheet = parse_stylesheet(css);
        stylesheet_to_rules(&sheet, 0)
    }

    #[test]
    fn single_matching_rule_produces_correct_values() {
        let (tree, target) = build_tree_with_element("div", vec![]);
        let rules = parse_author_rules("div { color: red; font-size: 16px; }");
        let result = cascade_element(&tree, target, &[], &rules, &[]);
        assert_eq!(result.len(), 2);
        assert_eq!(result["color"].value, "red");
        assert_eq!(result["font-size"].value, "16px");
        assert!(!result["color"].important);
    }

    #[test]
    fn higher_specificity_wins() {
        let (tree, target) = build_tree_with_element("div", vec![("class".to_string(), "foo".to_string())]);
        let rules = parse_author_rules("div { color: red; } div.foo { color: blue; }");
        let result = cascade_element(&tree, target, &[], &rules, &[]);
        assert_eq!(result["color"].value, "blue");
    }

    #[test]
    fn same_specificity_later_source_order_wins() {
        let (tree, target) = build_tree_with_element("div", vec![]);
        let rules = parse_author_rules("div { color: red; } div { color: blue; }");
        let result = cascade_element(&tree, target, &[], &rules, &[]);
        assert_eq!(result["color"].value, "blue");
    }

    #[test]
    fn important_beats_normal_regardless_of_specificity() {
        let (tree, target) = build_tree_with_element(
            "div",
            vec![
                ("id".to_string(), "main".to_string()),
                ("class".to_string(), "foo".to_string()),
            ],
        );
        let rules = parse_author_rules("#main { color: red; } div { color: blue !important; }");
        let result = cascade_element(&tree, target, &[], &rules, &[]);
        assert_eq!(result["color"].value, "blue");
        assert!(result["color"].important);
    }

    #[test]
    fn normal_inline_beats_normal_author() {
        let (tree, target) = build_tree_with_element("div", vec![]);
        let rules = parse_author_rules("div { color: red; }");
        let inline = vec![("color".to_string(), "green".to_string(), false)];
        let result = cascade_element(&tree, target, &[], &rules, &inline);
        assert_eq!(result["color"].value, "green");
    }

    #[test]
    fn important_author_beats_normal_inline() {
        let (tree, target) = build_tree_with_element("div", vec![]);
        let rules = parse_author_rules("div { color: red !important; }");
        let inline = vec![("color".to_string(), "green".to_string(), false)];
        let result = cascade_element(&tree, target, &[], &rules, &inline);
        assert_eq!(result["color"].value, "red");
        assert!(result["color"].important);
    }

    #[test]
    fn important_ua_beats_everything() {
        let (tree, target) = build_tree_with_element("div", vec![]);
        let ua_rules = parse_author_rules("div { display: block !important; }");
        let author_rules = parse_author_rules("div { display: none !important; }");
        let inline = vec![("display".to_string(), "flex".to_string(), true)];
        let result = cascade_element(&tree, target, &ua_rules, &author_rules, &inline);
        assert_eq!(result["display"].value, "block");
        assert!(result["display"].important);
    }

    #[test]
    fn non_matching_rules_excluded() {
        let (tree, target) = build_tree_with_element("div", vec![]);
        let rules = parse_author_rules("span { color: red; } p { font-size: 14px; } div { margin: 0; }");
        let result = cascade_element(&tree, target, &[], &rules, &[]);
        assert_eq!(result.len(), 1);
        assert_eq!(result["margin"].value, "0");
        assert!(!result.contains_key("color"));
        assert!(!result.contains_key("font-size"));
    }

    #[test]
    fn multiple_properties_from_one_rule() {
        let (tree, target) = build_tree_with_element("div", vec![]);
        let rules = parse_author_rules("div { color: red; font-size: 16px; margin: 10px; display: block; }");
        let result = cascade_element(&tree, target, &[], &rules, &[]);
        assert_eq!(result.len(), 4);
        assert_eq!(result["color"].value, "red");
        assert_eq!(result["font-size"].value, "16px");
        assert_eq!(result["margin"].value, "10px");
        assert_eq!(result["display"].value, "block");
    }

    #[test]
    fn empty_stylesheet_produces_empty_values() {
        let (tree, target) = build_tree_with_element("div", vec![]);
        let result = cascade_element(&tree, target, &[], &[], &[]);
        assert!(result.is_empty());
    }

    #[test]
    fn inline_styles_appear_in_output() {
        let (tree, target) = build_tree_with_element("div", vec![]);
        let inline = vec![
            ("color".to_string(), "red".to_string(), false),
            ("font-size".to_string(), "20px".to_string(), false),
        ];
        let result = cascade_element(&tree, target, &[], &[], &inline);
        assert_eq!(result.len(), 2);
        assert_eq!(result["color"].value, "red");
        assert_eq!(result["font-size"].value, "20px");
    }

    #[test]
    fn stylesheet_to_rules_preserves_source_order() {
        let sheet = parse_stylesheet("h1 { color: red; } p { color: blue; } div { color: green; }");
        let rules = stylesheet_to_rules(&sheet, 10);
        assert_eq!(rules.len(), 3);
        assert_eq!(rules[0].source_order, 10);
        assert_eq!(rules[1].source_order, 11);
        assert_eq!(rules[2].source_order, 12);
    }

    #[test]
    fn stylesheet_to_rules_converts_declarations() {
        let sheet = parse_stylesheet("div { color: red !important; font-size: 16px; }");
        let rules = stylesheet_to_rules(&sheet, 0);
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].declarations.len(), 2);
        let color_decl = rules[0].declarations.iter().find(|d| d.property == "color").unwrap();
        assert_eq!(color_decl.value, "red");
        assert!(color_decl.important);
        let font_decl = rules[0]
            .declarations
            .iter()
            .find(|d| d.property == "font-size")
            .unwrap();
        assert_eq!(font_decl.value, "16px");
        assert!(!font_decl.important);
    }

    #[test]
    fn important_inline_beats_important_author() {
        let (tree, target) = build_tree_with_element("div", vec![]);
        let rules = parse_author_rules("div { color: red !important; }");
        let inline = vec![("color".to_string(), "green".to_string(), true)];
        let result = cascade_element(&tree, target, &[], &rules, &inline);
        assert_eq!(result["color"].value, "green");
        assert!(result["color"].important);
    }

    #[test]
    fn normal_ua_loses_to_normal_author() {
        let (tree, target) = build_tree_with_element("div", vec![]);
        let ua_rules = parse_author_rules("div { display: block; }");
        let author_rules = parse_author_rules("div { display: flex; }");
        let result = cascade_element(&tree, target, &ua_rules, &author_rules, &[]);
        assert_eq!(result["display"].value, "flex");
    }

    #[test]
    fn class_beats_type_selector() {
        let (tree, target) = build_tree_with_element("p", vec![("class".to_string(), "intro".to_string())]);
        let rules = parse_author_rules("p { color: red; } .intro { color: blue; }");
        let result = cascade_element(&tree, target, &[], &rules, &[]);
        assert_eq!(result["color"].value, "blue");
    }

    #[test]
    fn id_selector_beats_class_and_type() {
        let (tree, target) = build_tree_with_element(
            "div",
            vec![
                ("id".to_string(), "main".to_string()),
                ("class".to_string(), "container".to_string()),
            ],
        );
        let rules = parse_author_rules("div { color: red; } .container { color: green; } #main { color: blue; }");
        let result = cascade_element(&tree, target, &[], &rules, &[]);
        assert_eq!(result["color"].value, "blue");
    }
}
