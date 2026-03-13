//! CSS stylesheet and inline style parsing using the cssparser crate.
//!
//! This module provides functionality to parse CSS stylesheets into Braille's
//! internal representation, handling both external stylesheets and inline style attributes.
//!
//! ## API Uncertainty Note
//!
//! The cssparser 0.35 API for handling the !important flag in DeclarationParser is not
//! fully documented. This implementation manually detects !important by checking for its
//! presence in the raw value text, which may not be the canonical approach. Future agents
//! may need to refine this based on actual testing or cssparser source code review.

use crate::css::selector_impl::{BrailleSelectorImpl, BrailleSelectorParser};
use cssparser::{CowRcStr, ParseError, Parser, ParserInput, ParserState};
use selectors::parser::{SelectorList, ParseRelative};

/// A parsed CSS stylesheet containing a list of rules.
#[derive(Debug, Clone, PartialEq)]
pub struct Stylesheet {
    pub rules: Vec<Rule>,
}

/// A CSS rule with selectors and declarations.
#[derive(Debug, Clone, PartialEq)]
pub struct Rule {
    pub selectors: SelectorList<BrailleSelectorImpl>,
    pub declarations: Vec<Declaration>,
}

/// A CSS property declaration with optional !important flag.
#[derive(Debug, Clone, PartialEq)]
pub struct Declaration {
    pub property: String,  // raw CSS property name
    pub value: String,     // raw CSS value string
    pub important: bool,
}

/// Parse a CSS stylesheet string into a Stylesheet structure.
///
/// # Examples
///
/// ```
/// use braille_engine::css::parser::parse_stylesheet;
///
/// let css = ".foo { color: red; font-size: 16px; }";
/// let stylesheet = parse_stylesheet(css);
/// assert_eq!(stylesheet.rules.len(), 1);
/// assert_eq!(stylesheet.rules[0].declarations.len(), 2);
/// ```
pub fn parse_stylesheet(css: &str) -> Stylesheet {
    let mut input = ParserInput::new(css);
    let mut parser = Parser::new(&mut input);
    let mut rules = Vec::new();

    // Use StyleSheetParser to iterate through top-level rules
    let mut rule_parser = BrailleRuleParser;
    let iter = cssparser::StyleSheetParser::new(&mut parser, &mut rule_parser);
    for result in iter {
        match result {
            Ok(rule) => rules.push(rule),
            Err(_) => {
                // Skip invalid rules (fail fast on parse errors in individual rules)
                // but continue parsing the rest of the stylesheet
            }
        }
    }

    Stylesheet { rules }
}

/// Parse an inline style attribute string into a list of declarations.
///
/// # Examples
///
/// ```
/// use braille_engine::css::parser::parse_inline_style;
///
/// let style = "color: red; font-size: 16px;";
/// let declarations = parse_inline_style(style);
/// assert_eq!(declarations.len(), 2);
/// ```
pub fn parse_inline_style(style_attr: &str) -> Vec<Declaration> {
    let mut input = ParserInput::new(style_attr);
    let mut parser = Parser::new(&mut input);
    let mut declarations = Vec::new();

    // Use RuleBodyParser to parse declarations without selectors
    let mut decl_parser = BrailleDeclarationParser;
    let iter = cssparser::RuleBodyParser::new(&mut parser, &mut decl_parser);
    for result in iter {
        match result {
            Ok(decl) => declarations.push(decl),
            Err(_) => {
                // Skip invalid declarations but continue parsing
            }
        }
    }

    declarations
}

/// Implementation of cssparser's DeclarationParser trait for Braille.
struct BrailleDeclarationParser;

impl<'i> cssparser::DeclarationParser<'i> for BrailleDeclarationParser {
    type Declaration = Declaration;
    type Error = ();

    fn parse_value<'t>(
        &mut self,
        name: CowRcStr<'i>,
        input: &mut Parser<'i, 't>,
        _declaration_start: &ParserState,
    ) -> Result<Declaration, ParseError<'i, ()>> {
        // Collect all tokens as raw text for the value
        // We'll parse these values properly in later agents (C-2, etc.)

        // Record the starting position
        let start_pos = input.position();

        // Consume all remaining tokens
        // The parser is already delimited to this declaration's value
        while input.next().is_ok() {}

        // Get the slice of text from start to current position
        let end_pos = input.position();
        let value_text = input.slice(start_pos..end_pos);

        // Check if !important is present at the end and strip it
        // cssparser may include it in the value text
        let (value, important) = if let Some(stripped) = value_text.trim_end().strip_suffix("!important") {
            (stripped.trim(), true)
        } else if let Some(stripped) = value_text.trim_end().strip_suffix("! important") {
            (stripped.trim(), true)
        } else {
            (value_text.trim(), false)
        };

        Ok(Declaration {
            property: name.to_string(),
            value: value.to_string(),
            important,
        })
    }
}

impl<'i> cssparser::AtRuleParser<'i> for BrailleDeclarationParser {
    type Prelude = ();
    type AtRule = Declaration;
    type Error = ();
}

impl<'i> cssparser::QualifiedRuleParser<'i> for BrailleDeclarationParser {
    type Prelude = ();
    type QualifiedRule = Declaration;
    type Error = ();
}

impl<'i> cssparser::RuleBodyItemParser<'i, Declaration, ()> for BrailleDeclarationParser {
    fn parse_qualified(&self) -> bool { false }
    fn parse_declarations(&self) -> bool { true }
}

/// Implementation of cssparser's QualifiedRuleParser trait for Braille.
struct BrailleRuleParser;

impl<'i> cssparser::QualifiedRuleParser<'i> for BrailleRuleParser {
    type Prelude = SelectorList<BrailleSelectorImpl>;
    type QualifiedRule = Rule;
    type Error = ();

    fn parse_prelude<'t>(
        &mut self,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::Prelude, ParseError<'i, ()>> {
        // Parse selector list using the selectors crate
        SelectorList::parse(&BrailleSelectorParser, input, ParseRelative::No)
            .map_err(|_| input.new_custom_error(()))
    }

    fn parse_block<'t>(
        &mut self,
        selectors: Self::Prelude,
        _start: &ParserState,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::QualifiedRule, ParseError<'i, ()>> {
        // Parse the declaration block
        let mut declarations = Vec::new();
        let mut decl_parser = BrailleDeclarationParser;
        let iter = cssparser::RuleBodyParser::new(input, &mut decl_parser);

        for result in iter {
            match result {
                Ok(decl) => declarations.push(decl),
                Err(_) => {
                    // Skip invalid declarations
                }
            }
        }

        Ok(Rule {
            selectors,
            declarations,
        })
    }
}

impl<'i> cssparser::AtRuleParser<'i> for BrailleRuleParser {
    type Prelude = ();
    type AtRule = Rule;
    type Error = ();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_single_declaration() {
        let decls = parse_inline_style("color: red;");
        assert_eq!(decls.len(), 1);
        assert_eq!(decls[0].property, "color");
        assert_eq!(decls[0].value, "red");
        assert_eq!(decls[0].important, false);
    }

    #[test]
    fn test_parse_two_declarations() {
        let decls = parse_inline_style("color: red; font-size: 16px;");
        assert_eq!(decls.len(), 2);

        assert_eq!(decls[0].property, "color");
        assert_eq!(decls[0].value, "red");

        assert_eq!(decls[1].property, "font-size");
        assert_eq!(decls[1].value, "16px");
    }

    #[test]
    fn test_parse_declaration_with_important() {
        let decls = parse_inline_style("color: red !important;");
        assert_eq!(decls.len(), 1);
        assert_eq!(decls[0].property, "color");
        assert_eq!(decls[0].value, "red");
        assert_eq!(decls[0].important, true);
    }

    #[test]
    fn test_parse_simple_rule() {
        let stylesheet = parse_stylesheet(".foo { color: red; }");
        assert_eq!(stylesheet.rules.len(), 1);

        let rule = &stylesheet.rules[0];
        assert_eq!(rule.declarations.len(), 1);
        assert_eq!(rule.declarations[0].property, "color");
        assert_eq!(rule.declarations[0].value, "red");
    }

    #[test]
    fn test_parse_rule_with_multiple_declarations() {
        let stylesheet = parse_stylesheet(".foo { color: red; font-size: 16px; }");
        assert_eq!(stylesheet.rules.len(), 1);

        let rule = &stylesheet.rules[0];
        assert_eq!(rule.declarations.len(), 2);
        assert_eq!(rule.declarations[0].property, "color");
        assert_eq!(rule.declarations[1].property, "font-size");
    }

    #[test]
    fn test_parse_rule_with_multiple_selectors() {
        let stylesheet = parse_stylesheet("h1, h2 { font-weight: bold; }");
        assert_eq!(stylesheet.rules.len(), 1);

        let rule = &stylesheet.rules[0];
        assert_eq!(rule.selectors.len(), 2);
        assert_eq!(rule.declarations.len(), 1);
        assert_eq!(rule.declarations[0].property, "font-weight");
        assert_eq!(rule.declarations[0].value, "bold");
    }

    #[test]
    fn test_parse_empty_stylesheet() {
        let stylesheet = parse_stylesheet("");
        assert_eq!(stylesheet.rules.len(), 0);
    }

    #[test]
    fn test_parse_empty_inline_style() {
        let decls = parse_inline_style("");
        assert_eq!(decls.len(), 0);
    }

    #[test]
    fn test_parse_multiple_rules() {
        let css = r#"
            .foo { color: red; }
            .bar { font-size: 16px; }
        "#;
        let stylesheet = parse_stylesheet(css);
        assert_eq!(stylesheet.rules.len(), 2);
    }

    #[test]
    fn test_parse_complex_value() {
        let decls = parse_inline_style("margin: 10px 20px 30px 40px;");
        assert_eq!(decls.len(), 1);
        assert_eq!(decls[0].property, "margin");
        assert_eq!(decls[0].value, "10px 20px 30px 40px");
    }

    #[test]
    fn test_parse_rgb_color() {
        let decls = parse_inline_style("color: rgb(255, 0, 0);");
        assert_eq!(decls.len(), 1);
        assert_eq!(decls[0].property, "color");
        assert_eq!(decls[0].value, "rgb(255, 0, 0)");
    }

    #[test]
    fn test_parse_important_flag() {
        let stylesheet = parse_stylesheet(".foo { color: red !important; }");
        assert_eq!(stylesheet.rules.len(), 1);
        assert_eq!(stylesheet.rules[0].declarations.len(), 1);
        assert!(stylesheet.rules[0].declarations[0].important);
    }

    #[test]
    fn test_parse_inline_style_without_semicolon() {
        let decls = parse_inline_style("color: red");
        assert_eq!(decls.len(), 1);
        assert_eq!(decls[0].property, "color");
        assert_eq!(decls[0].value, "red");
    }

    #[test]
    fn test_parse_whitespace_handling() {
        let decls = parse_inline_style("  color  :  red  ;  ");
        assert_eq!(decls.len(), 1);
        assert_eq!(decls[0].property, "color");
        assert_eq!(decls[0].value, "red");
    }
}
