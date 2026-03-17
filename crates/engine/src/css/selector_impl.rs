//! Braille's implementation of the selectors crate SelectorImpl trait.
//!
//! This module defines the types and parsers needed to use Servo's selectors
//! crate for CSS selector matching and querySelector operations.

use cssparser::ToCss as CssparserToCss;
use selectors::parser::{NonTSPseudoClass as NonTSPseudoClassTrait, PseudoElement as PseudoElementTrait};
use selectors::{parser, SelectorImpl};
use std::borrow::Borrow;
use std::fmt;

/// A string wrapper that implements the traits required by the selectors crate
/// (ToCss, PrecomputedHash, etc.) so it can be used as associated types in SelectorImpl.
#[derive(Debug, Clone, PartialEq, Eq, Default, Hash)]
pub struct CssString(pub String);

impl<'a> From<&'a str> for CssString {
    fn from(s: &'a str) -> Self {
        CssString(s.to_string())
    }
}

impl CssparserToCss for CssString {
    fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
        dest.write_str(&self.0)
    }
}

impl precomputed_hash::PrecomputedHash for CssString {
    fn precomputed_hash(&self) -> u32 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.0.hash(&mut hasher);
        hasher.finish() as u32
    }
}

impl Borrow<str> for CssString {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for CssString {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CssString {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Braille's selector implementation type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BrailleSelectorImpl;

/// Pseudo-classes supported by Braille.
///
/// These represent CSS pseudo-classes like `:hover`, `:focus`, etc.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PseudoClass {
    Hover,
    Focus,
    Active,
    Visited,
    Link,
    Checked,
    Disabled,
    Enabled,
    FirstChild,
    LastChild,
    NthChild(i32, i32),
    OnlyChild,
    Empty,
    Root,
    Scope,
    Invalid,
    Valid,
    Target,
    Lang(String),
}

/// Pseudo-elements supported by Braille.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PseudoElement {
    Before,
    After,
}

impl NonTSPseudoClassTrait for PseudoClass {
    type Impl = BrailleSelectorImpl;

    fn is_active_or_hover(&self) -> bool {
        matches!(self, PseudoClass::Active | PseudoClass::Hover)
    }

    fn is_user_action_state(&self) -> bool {
        matches!(self, PseudoClass::Active | PseudoClass::Hover | PseudoClass::Focus)
    }
}

impl CssparserToCss for PseudoClass {
    fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
        match self {
            PseudoClass::Hover => dest.write_str(":hover"),
            PseudoClass::Focus => dest.write_str(":focus"),
            PseudoClass::Active => dest.write_str(":active"),
            PseudoClass::Visited => dest.write_str(":visited"),
            PseudoClass::Link => dest.write_str(":link"),
            PseudoClass::Checked => dest.write_str(":checked"),
            PseudoClass::Disabled => dest.write_str(":disabled"),
            PseudoClass::Enabled => dest.write_str(":enabled"),
            PseudoClass::FirstChild => dest.write_str(":first-child"),
            PseudoClass::LastChild => dest.write_str(":last-child"),
            PseudoClass::NthChild(a, b) => write!(dest, ":nth-child({}n+{})", a, b),
            PseudoClass::OnlyChild => dest.write_str(":only-child"),
            PseudoClass::Empty => dest.write_str(":empty"),
            PseudoClass::Root => dest.write_str(":root"),
            PseudoClass::Scope => dest.write_str(":scope"),
            PseudoClass::Invalid => dest.write_str(":invalid"),
            PseudoClass::Valid => dest.write_str(":valid"),
            PseudoClass::Target => dest.write_str(":target"),
            PseudoClass::Lang(ref lang) => write!(dest, ":lang({})", lang),
        }
    }
}

impl PseudoElementTrait for PseudoElement {
    type Impl = BrailleSelectorImpl;
}

impl CssparserToCss for PseudoElement {
    fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
        match self {
            PseudoElement::Before => dest.write_str("::before"),
            PseudoElement::After => dest.write_str("::after"),
        }
    }
}

impl fmt::Display for PseudoClass {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        <Self as CssparserToCss>::to_css(self, f)
    }
}

impl fmt::Display for PseudoElement {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        <Self as CssparserToCss>::to_css(self, f)
    }
}

impl SelectorImpl for BrailleSelectorImpl {
    type ExtraMatchingData<'a> = ();
    type AttrValue = CssString;
    type Identifier = CssString;
    type LocalName = CssString;
    type NamespaceUrl = CssString;
    type NamespacePrefix = CssString;
    type BorrowedLocalName = str;
    type BorrowedNamespaceUrl = str;
    type NonTSPseudoClass = PseudoClass;
    type PseudoElement = PseudoElement;
}

/// Parser for Braille CSS selectors.
#[derive(Debug, Clone)]
pub struct BrailleSelectorParser;

impl<'i> parser::Parser<'i> for BrailleSelectorParser {
    type Impl = BrailleSelectorImpl;
    type Error = parser::SelectorParseErrorKind<'i>;

    fn parse_has(&self) -> bool {
        true
    }

    fn parse_non_ts_pseudo_class(
        &self,
        _location: cssparser::SourceLocation,
        name: cssparser::CowRcStr<'i>,
    ) -> Result<PseudoClass, cssparser::ParseError<'i, Self::Error>> {
        match &*name {
            "hover" => Ok(PseudoClass::Hover),
            "focus" => Ok(PseudoClass::Focus),
            "active" => Ok(PseudoClass::Active),
            "visited" => Ok(PseudoClass::Visited),
            "link" => Ok(PseudoClass::Link),
            "checked" => Ok(PseudoClass::Checked),
            "disabled" => Ok(PseudoClass::Disabled),
            "enabled" => Ok(PseudoClass::Enabled),
            "first-child" => Ok(PseudoClass::FirstChild),
            "last-child" => Ok(PseudoClass::LastChild),
            "only-child" => Ok(PseudoClass::OnlyChild),
            "empty" => Ok(PseudoClass::Empty),
            "root" => Ok(PseudoClass::Root),
            "scope" => Ok(PseudoClass::Scope),
            "invalid" => Ok(PseudoClass::Invalid),
            "valid" => Ok(PseudoClass::Valid),
            "target" => Ok(PseudoClass::Target),
            _ => Err(cssparser::ParseError {
                kind: cssparser::ParseErrorKind::Custom(parser::SelectorParseErrorKind::UnexpectedIdent(name.clone())),
                location: _location,
            }),
        }
    }

    fn parse_non_ts_functional_pseudo_class<'t>(
        &self,
        name: cssparser::CowRcStr<'i>,
        arguments: &mut cssparser::Parser<'i, 't>,
        _after_part: bool,
    ) -> Result<PseudoClass, cssparser::ParseError<'i, Self::Error>> {
        match &*name {
            "lang" => {
                let lang = arguments
                    .expect_ident_or_string()
                    .map(|v| v.to_string())
                    .map_err(|e| -> cssparser::ParseError<'i, Self::Error> {
                        cssparser::ParseError {
                            kind: cssparser::ParseErrorKind::Custom(
                                parser::SelectorParseErrorKind::UnexpectedIdent(name.clone()),
                            ),
                            location: e.location,
                        }
                    })?;
                Ok(PseudoClass::Lang(lang))
            }
            _ => Err(arguments.new_custom_error(parser::SelectorParseErrorKind::UnexpectedIdent(name.clone()))),
        }
    }

    fn parse_pseudo_element(
        &self,
        _location: cssparser::SourceLocation,
        name: cssparser::CowRcStr<'i>,
    ) -> Result<PseudoElement, cssparser::ParseError<'i, Self::Error>> {
        match &*name {
            "before" => Ok(PseudoElement::Before),
            "after" => Ok(PseudoElement::After),
            _ => Err(cssparser::ParseError {
                kind: cssparser::ParseErrorKind::Custom(parser::SelectorParseErrorKind::UnexpectedIdent(name.clone())),
                location: _location,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pseudo_class_construction() {
        let hover = PseudoClass::Hover;
        let focus = PseudoClass::Focus;
        assert_eq!(hover, PseudoClass::Hover);
        assert_ne!(hover, focus);
    }

    #[test]
    fn test_pseudo_class_user_action() {
        assert!(PseudoClass::Hover.is_user_action_state());
        assert!(PseudoClass::Focus.is_user_action_state());
        assert!(PseudoClass::Active.is_user_action_state());
        assert!(!PseudoClass::Visited.is_user_action_state());
        assert!(!PseudoClass::Link.is_user_action_state());
    }

    #[test]
    fn test_pseudo_class_active_or_hover() {
        assert!(PseudoClass::Hover.is_active_or_hover());
        assert!(PseudoClass::Active.is_active_or_hover());
        assert!(!PseudoClass::Focus.is_active_or_hover());
        assert!(!PseudoClass::Checked.is_active_or_hover());
    }

    #[test]
    fn test_pseudo_element_construction() {
        let before = PseudoElement::Before;
        let after = PseudoElement::After;
        assert_eq!(before, PseudoElement::Before);
        assert_ne!(before, after);
    }

    #[test]
    fn test_braille_selector_impl_instantiation() {
        let _impl = BrailleSelectorImpl;
    }

    #[test]
    fn test_pseudo_class_display() {
        assert_eq!(format!("{}", PseudoClass::Hover), ":hover");
        assert_eq!(format!("{}", PseudoClass::Focus), ":focus");
        assert_eq!(format!("{}", PseudoClass::FirstChild), ":first-child");
        assert_eq!(format!("{}", PseudoClass::NthChild(2, 1)), ":nth-child(2n+1)");
    }

    #[test]
    fn test_pseudo_element_display() {
        assert_eq!(format!("{}", PseudoElement::Before), "::before");
        assert_eq!(format!("{}", PseudoElement::After), "::after");
    }

    #[test]
    fn test_selector_parser_instantiation() {
        let _parser = BrailleSelectorParser;
    }

    #[test]
    fn test_all_pseudo_classes() {
        let classes = vec![
            PseudoClass::Hover,
            PseudoClass::Focus,
            PseudoClass::Active,
            PseudoClass::Visited,
            PseudoClass::Link,
            PseudoClass::Checked,
            PseudoClass::Disabled,
            PseudoClass::Enabled,
            PseudoClass::FirstChild,
            PseudoClass::LastChild,
            PseudoClass::NthChild(1, 0),
            PseudoClass::OnlyChild,
            PseudoClass::Empty,
            PseudoClass::Root,
        ];

        for (i, class1) in classes.iter().enumerate() {
            for (j, class2) in classes.iter().enumerate() {
                if i == j {
                    assert_eq!(class1, class2);
                } else {
                    if !matches!(
                        (class1, class2),
                        (PseudoClass::NthChild(_, _), PseudoClass::NthChild(_, _))
                    ) {
                        assert_ne!(class1, class2);
                    }
                }
            }
        }
    }
}
