use crate::css::values::{CssColor, CssValue, LengthUnit};

/// CSS property identifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PropertyId {
    // Display & Positioning
    Display,
    Position,
    Float,
    Clear,

    // Box Model - Size
    Width,
    Height,
    MinWidth,
    MinHeight,
    MaxWidth,
    MaxHeight,

    // Box Model - Margin
    Margin,
    MarginTop,
    MarginRight,
    MarginBottom,
    MarginLeft,

    // Box Model - Padding
    Padding,
    PaddingTop,
    PaddingRight,
    PaddingBottom,
    PaddingLeft,

    // Box Model - Border
    Border,
    BorderTop,
    BorderRight,
    BorderBottom,
    BorderLeft,
    BorderWidth,
    BorderStyle,
    BorderColor,

    // Color & Background
    Color,
    BackgroundColor,
    Background,

    // Typography
    FontSize,
    FontWeight,
    FontFamily,
    FontStyle,
    TextAlign,
    TextDecoration,
    TextTransform,
    LineHeight,
    LetterSpacing,
    WordSpacing,

    // Overflow & Visibility
    Overflow,
    OverflowX,
    OverflowY,
    Visibility,
    Opacity,

    // Positioning
    ZIndex,
    Top,
    Right,
    Bottom,
    Left,

    // Miscellaneous
    Cursor,
    ListStyleType,
    BoxSizing,

    // Flexbox
    FlexDirection,
    FlexWrap,
    JustifyContent,
    AlignItems,
    FlexGrow,
    FlexShrink,
}

impl PropertyId {
    /// Convert a CSS property name string to PropertyId
    /// Handles kebab-case property names (case-insensitive)
    pub fn from_name(name: &str) -> Option<PropertyId> {
        let normalized = name.to_ascii_lowercase();
        match normalized.as_str() {
            // Display & Positioning
            "display" => Some(PropertyId::Display),
            "position" => Some(PropertyId::Position),
            "float" => Some(PropertyId::Float),
            "clear" => Some(PropertyId::Clear),

            // Box Model - Size
            "width" => Some(PropertyId::Width),
            "height" => Some(PropertyId::Height),
            "min-width" => Some(PropertyId::MinWidth),
            "min-height" => Some(PropertyId::MinHeight),
            "max-width" => Some(PropertyId::MaxWidth),
            "max-height" => Some(PropertyId::MaxHeight),

            // Box Model - Margin
            "margin" => Some(PropertyId::Margin),
            "margin-top" => Some(PropertyId::MarginTop),
            "margin-right" => Some(PropertyId::MarginRight),
            "margin-bottom" => Some(PropertyId::MarginBottom),
            "margin-left" => Some(PropertyId::MarginLeft),

            // Box Model - Padding
            "padding" => Some(PropertyId::Padding),
            "padding-top" => Some(PropertyId::PaddingTop),
            "padding-right" => Some(PropertyId::PaddingRight),
            "padding-bottom" => Some(PropertyId::PaddingBottom),
            "padding-left" => Some(PropertyId::PaddingLeft),

            // Box Model - Border
            "border" => Some(PropertyId::Border),
            "border-top" => Some(PropertyId::BorderTop),
            "border-right" => Some(PropertyId::BorderRight),
            "border-bottom" => Some(PropertyId::BorderBottom),
            "border-left" => Some(PropertyId::BorderLeft),
            "border-width" => Some(PropertyId::BorderWidth),
            "border-style" => Some(PropertyId::BorderStyle),
            "border-color" => Some(PropertyId::BorderColor),

            // Color & Background
            "color" => Some(PropertyId::Color),
            "background-color" => Some(PropertyId::BackgroundColor),
            "background" => Some(PropertyId::Background),

            // Typography
            "font-size" => Some(PropertyId::FontSize),
            "font-weight" => Some(PropertyId::FontWeight),
            "font-family" => Some(PropertyId::FontFamily),
            "font-style" => Some(PropertyId::FontStyle),
            "text-align" => Some(PropertyId::TextAlign),
            "text-decoration" => Some(PropertyId::TextDecoration),
            "text-transform" => Some(PropertyId::TextTransform),
            "line-height" => Some(PropertyId::LineHeight),
            "letter-spacing" => Some(PropertyId::LetterSpacing),
            "word-spacing" => Some(PropertyId::WordSpacing),

            // Overflow & Visibility
            "overflow" => Some(PropertyId::Overflow),
            "overflow-x" => Some(PropertyId::OverflowX),
            "overflow-y" => Some(PropertyId::OverflowY),
            "visibility" => Some(PropertyId::Visibility),
            "opacity" => Some(PropertyId::Opacity),

            // Positioning
            "z-index" => Some(PropertyId::ZIndex),
            "top" => Some(PropertyId::Top),
            "right" => Some(PropertyId::Right),
            "bottom" => Some(PropertyId::Bottom),
            "left" => Some(PropertyId::Left),

            // Miscellaneous
            "cursor" => Some(PropertyId::Cursor),
            "list-style-type" => Some(PropertyId::ListStyleType),
            "box-sizing" => Some(PropertyId::BoxSizing),

            // Flexbox
            "flex-direction" => Some(PropertyId::FlexDirection),
            "flex-wrap" => Some(PropertyId::FlexWrap),
            "justify-content" => Some(PropertyId::JustifyContent),
            "align-items" => Some(PropertyId::AlignItems),
            "flex-grow" => Some(PropertyId::FlexGrow),
            "flex-shrink" => Some(PropertyId::FlexShrink),

            _ => None,
        }
    }

    /// Get the CSS property name for this PropertyId
    pub fn name(&self) -> &'static str {
        match self {
            // Display & Positioning
            PropertyId::Display => "display",
            PropertyId::Position => "position",
            PropertyId::Float => "float",
            PropertyId::Clear => "clear",

            // Box Model - Size
            PropertyId::Width => "width",
            PropertyId::Height => "height",
            PropertyId::MinWidth => "min-width",
            PropertyId::MinHeight => "min-height",
            PropertyId::MaxWidth => "max-width",
            PropertyId::MaxHeight => "max-height",

            // Box Model - Margin
            PropertyId::Margin => "margin",
            PropertyId::MarginTop => "margin-top",
            PropertyId::MarginRight => "margin-right",
            PropertyId::MarginBottom => "margin-bottom",
            PropertyId::MarginLeft => "margin-left",

            // Box Model - Padding
            PropertyId::Padding => "padding",
            PropertyId::PaddingTop => "padding-top",
            PropertyId::PaddingRight => "padding-right",
            PropertyId::PaddingBottom => "padding-bottom",
            PropertyId::PaddingLeft => "padding-left",

            // Box Model - Border
            PropertyId::Border => "border",
            PropertyId::BorderTop => "border-top",
            PropertyId::BorderRight => "border-right",
            PropertyId::BorderBottom => "border-bottom",
            PropertyId::BorderLeft => "border-left",
            PropertyId::BorderWidth => "border-width",
            PropertyId::BorderStyle => "border-style",
            PropertyId::BorderColor => "border-color",

            // Color & Background
            PropertyId::Color => "color",
            PropertyId::BackgroundColor => "background-color",
            PropertyId::Background => "background",

            // Typography
            PropertyId::FontSize => "font-size",
            PropertyId::FontWeight => "font-weight",
            PropertyId::FontFamily => "font-family",
            PropertyId::FontStyle => "font-style",
            PropertyId::TextAlign => "text-align",
            PropertyId::TextDecoration => "text-decoration",
            PropertyId::TextTransform => "text-transform",
            PropertyId::LineHeight => "line-height",
            PropertyId::LetterSpacing => "letter-spacing",
            PropertyId::WordSpacing => "word-spacing",

            // Overflow & Visibility
            PropertyId::Overflow => "overflow",
            PropertyId::OverflowX => "overflow-x",
            PropertyId::OverflowY => "overflow-y",
            PropertyId::Visibility => "visibility",
            PropertyId::Opacity => "opacity",

            // Positioning
            PropertyId::ZIndex => "z-index",
            PropertyId::Top => "top",
            PropertyId::Right => "right",
            PropertyId::Bottom => "bottom",
            PropertyId::Left => "left",

            // Miscellaneous
            PropertyId::Cursor => "cursor",
            PropertyId::ListStyleType => "list-style-type",
            PropertyId::BoxSizing => "box-sizing",

            // Flexbox
            PropertyId::FlexDirection => "flex-direction",
            PropertyId::FlexWrap => "flex-wrap",
            PropertyId::JustifyContent => "justify-content",
            PropertyId::AlignItems => "align-items",
            PropertyId::FlexGrow => "flex-grow",
            PropertyId::FlexShrink => "flex-shrink",
        }
    }

    /// Check if this property inherits by default
    ///
    /// Inheriting properties:
    /// - color, font-*, text-*, line-height
    /// - visibility, cursor, list-style-type
    /// - letter-spacing, word-spacing
    ///
    /// Most box model properties do NOT inherit.
    pub fn inherits(&self) -> bool {
        match self {
            // Typography properties inherit
            PropertyId::Color
            | PropertyId::FontSize
            | PropertyId::FontWeight
            | PropertyId::FontFamily
            | PropertyId::FontStyle
            | PropertyId::TextAlign
            | PropertyId::TextDecoration
            | PropertyId::TextTransform
            | PropertyId::LineHeight
            | PropertyId::LetterSpacing
            | PropertyId::WordSpacing => true,

            // Other inheriting properties
            PropertyId::Visibility | PropertyId::Cursor | PropertyId::ListStyleType => true,

            // Box model, layout, and positioning properties do NOT inherit
            _ => false,
        }
    }

    /// Get the CSS initial value for this property
    ///
    /// Note: Some of these are simplified. Full CSS spec has complex initial values
    /// for properties like 'border' that depend on other properties.
    pub fn initial_value(&self) -> CssValue {
        match self {
            // Display & Positioning
            PropertyId::Display => CssValue::Keyword("inline".to_string()),
            PropertyId::Position => CssValue::Keyword("static".to_string()),
            PropertyId::Float => CssValue::None,
            PropertyId::Clear => CssValue::None,

            // Box Model - Size
            PropertyId::Width | PropertyId::Height => CssValue::Auto,
            PropertyId::MinWidth | PropertyId::MinHeight => CssValue::Auto,
            PropertyId::MaxWidth | PropertyId::MaxHeight => CssValue::None,

            // Box Model - Margin (initial is 0)
            PropertyId::Margin
            | PropertyId::MarginTop
            | PropertyId::MarginRight
            | PropertyId::MarginBottom
            | PropertyId::MarginLeft => CssValue::Length(0.0, LengthUnit::Px),

            // Box Model - Padding (initial is 0)
            PropertyId::Padding
            | PropertyId::PaddingTop
            | PropertyId::PaddingRight
            | PropertyId::PaddingBottom
            | PropertyId::PaddingLeft => CssValue::Length(0.0, LengthUnit::Px),

            // Box Model - Border
            PropertyId::Border
            | PropertyId::BorderTop
            | PropertyId::BorderRight
            | PropertyId::BorderBottom
            | PropertyId::BorderLeft => CssValue::None, // Complex: width style color
            PropertyId::BorderWidth => CssValue::Keyword("medium".to_string()),
            PropertyId::BorderStyle => CssValue::None,
            PropertyId::BorderColor => CssValue::Keyword("currentcolor".to_string()),

            // Color & Background
            // Note: 'color' initial value depends on user agent, but commonly black
            PropertyId::Color => CssValue::Color(CssColor::Named("black".to_string())),
            PropertyId::BackgroundColor => CssValue::Keyword("transparent".to_string()),
            PropertyId::Background => CssValue::None, // Complex shorthand

            // Typography
            PropertyId::FontSize => CssValue::Keyword("medium".to_string()), // ~16px
            PropertyId::FontWeight => CssValue::Keyword("normal".to_string()),
            PropertyId::FontFamily => {
                // User agent dependent, typical default
                CssValue::String("serif".to_string())
            }
            PropertyId::FontStyle => CssValue::Keyword("normal".to_string()),
            PropertyId::TextAlign => CssValue::Keyword("start".to_string()),
            PropertyId::TextDecoration => CssValue::None,
            PropertyId::TextTransform => CssValue::None,
            PropertyId::LineHeight => CssValue::Keyword("normal".to_string()),
            PropertyId::LetterSpacing => CssValue::Keyword("normal".to_string()),
            PropertyId::WordSpacing => CssValue::Keyword("normal".to_string()),

            // Overflow & Visibility
            PropertyId::Overflow | PropertyId::OverflowX | PropertyId::OverflowY => {
                CssValue::Keyword("visible".to_string())
            }
            PropertyId::Visibility => CssValue::Keyword("visible".to_string()),
            PropertyId::Opacity => CssValue::Number(1.0),

            // Positioning
            PropertyId::ZIndex => CssValue::Auto,
            PropertyId::Top | PropertyId::Right | PropertyId::Bottom | PropertyId::Left => CssValue::Auto,

            // Miscellaneous
            PropertyId::Cursor => CssValue::Auto,
            PropertyId::ListStyleType => CssValue::Keyword("disc".to_string()),
            PropertyId::BoxSizing => CssValue::Keyword("content-box".to_string()),

            // Flexbox
            PropertyId::FlexDirection => CssValue::Keyword("row".to_string()),
            PropertyId::FlexWrap => CssValue::Keyword("nowrap".to_string()),
            PropertyId::JustifyContent => CssValue::Keyword("flex-start".to_string()),
            PropertyId::AlignItems => CssValue::Keyword("stretch".to_string()),
            PropertyId::FlexGrow => CssValue::Number(0.0),
            PropertyId::FlexShrink => CssValue::Number(1.0),
        }
    }
}

/// Expand CSS shorthand properties into longhand properties
///
/// Current implementation:
/// - margin: X → margin-top/right/bottom/left all set to X
/// - padding: X → padding-top/right/bottom/left all set to X
/// - border: value → border-width/style/color (simplified - just copies value for now)
///
/// Note: Full CSS shorthand parsing is complex (e.g., margin: 10px 20px with 2-4 values).
/// This implementation handles the simple 1-value case. More sophisticated parsing should
/// be added later when we integrate a proper CSS parser.
pub fn expand_shorthand(property: &str, value: &CssValue) -> Vec<(PropertyId, CssValue)> {
    let prop_id = match PropertyId::from_name(property) {
        Some(id) => id,
        None => return vec![],
    };

    match prop_id {
        PropertyId::Margin => vec![
            (PropertyId::MarginTop, value.clone()),
            (PropertyId::MarginRight, value.clone()),
            (PropertyId::MarginBottom, value.clone()),
            (PropertyId::MarginLeft, value.clone()),
        ],
        PropertyId::Padding => vec![
            (PropertyId::PaddingTop, value.clone()),
            (PropertyId::PaddingRight, value.clone()),
            (PropertyId::PaddingBottom, value.clone()),
            (PropertyId::PaddingLeft, value.clone()),
        ],
        // Border shorthand is complex (width style color), but for now just expand to all three
        PropertyId::Border => vec![
            (PropertyId::BorderWidth, value.clone()),
            (PropertyId::BorderStyle, value.clone()),
            (PropertyId::BorderColor, value.clone()),
        ],
        // Overflow shorthand sets both X and Y
        PropertyId::Overflow => vec![
            (PropertyId::OverflowX, value.clone()),
            (PropertyId::OverflowY, value.clone()),
        ],
        // Not a shorthand property, return as-is
        _ => vec![(prop_id, value.clone())],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_property_from_name_basic() {
        assert_eq!(PropertyId::from_name("display"), Some(PropertyId::Display));
        assert_eq!(PropertyId::from_name("color"), Some(PropertyId::Color));
        assert_eq!(PropertyId::from_name("font-size"), Some(PropertyId::FontSize));
    }

    #[test]
    fn test_property_from_name_kebab_case() {
        assert_eq!(PropertyId::from_name("margin-top"), Some(PropertyId::MarginTop));
        assert_eq!(PropertyId::from_name("padding-left"), Some(PropertyId::PaddingLeft));
        assert_eq!(PropertyId::from_name("border-color"), Some(PropertyId::BorderColor));
        assert_eq!(
            PropertyId::from_name("background-color"),
            Some(PropertyId::BackgroundColor)
        );
        assert_eq!(PropertyId::from_name("flex-direction"), Some(PropertyId::FlexDirection));
    }

    #[test]
    fn test_property_from_name_case_insensitive() {
        assert_eq!(PropertyId::from_name("DISPLAY"), Some(PropertyId::Display));
        assert_eq!(PropertyId::from_name("Color"), Some(PropertyId::Color));
        assert_eq!(PropertyId::from_name("Font-Size"), Some(PropertyId::FontSize));
        assert_eq!(PropertyId::from_name("MARGIN-TOP"), Some(PropertyId::MarginTop));
    }

    #[test]
    fn test_property_from_name_unknown() {
        assert_eq!(PropertyId::from_name("unknown-property"), None);
        assert_eq!(PropertyId::from_name("not-a-css-property"), None);
        assert_eq!(PropertyId::from_name(""), None);
    }

    #[test]
    fn test_property_name() {
        assert_eq!(PropertyId::Display.name(), "display");
        assert_eq!(PropertyId::MarginTop.name(), "margin-top");
        assert_eq!(PropertyId::FontSize.name(), "font-size");
        assert_eq!(PropertyId::BackgroundColor.name(), "background-color");
    }

    #[test]
    fn test_property_inherits() {
        // Inheriting properties
        assert!(PropertyId::Color.inherits());
        assert!(PropertyId::FontSize.inherits());
        assert!(PropertyId::FontWeight.inherits());
        assert!(PropertyId::FontFamily.inherits());
        assert!(PropertyId::FontStyle.inherits());
        assert!(PropertyId::TextAlign.inherits());
        assert!(PropertyId::TextDecoration.inherits());
        assert!(PropertyId::LineHeight.inherits());
        assert!(PropertyId::Visibility.inherits());
        assert!(PropertyId::Cursor.inherits());
        assert!(PropertyId::ListStyleType.inherits());
        assert!(PropertyId::LetterSpacing.inherits());
        assert!(PropertyId::WordSpacing.inherits());

        // Non-inheriting properties
        assert!(!PropertyId::Display.inherits());
        assert!(!PropertyId::Width.inherits());
        assert!(!PropertyId::Height.inherits());
        assert!(!PropertyId::Margin.inherits());
        assert!(!PropertyId::MarginTop.inherits());
        assert!(!PropertyId::Padding.inherits());
        assert!(!PropertyId::PaddingLeft.inherits());
        assert!(!PropertyId::Border.inherits());
        assert!(!PropertyId::BackgroundColor.inherits());
        assert!(!PropertyId::Position.inherits());
        assert!(!PropertyId::Float.inherits());
        assert!(!PropertyId::Opacity.inherits());
        assert!(!PropertyId::FlexDirection.inherits());
    }

    #[test]
    fn test_property_initial_values() {
        // Display & Positioning
        assert_eq!(
            PropertyId::Display.initial_value(),
            CssValue::Keyword("inline".to_string())
        );
        assert_eq!(
            PropertyId::Position.initial_value(),
            CssValue::Keyword("static".to_string())
        );
        assert_eq!(PropertyId::Float.initial_value(), CssValue::None);

        // Box Model - Size
        assert_eq!(PropertyId::Width.initial_value(), CssValue::Auto);
        assert_eq!(PropertyId::Height.initial_value(), CssValue::Auto);

        // Box Model - Margin/Padding
        assert_eq!(
            PropertyId::MarginTop.initial_value(),
            CssValue::Length(0.0, LengthUnit::Px)
        );
        assert_eq!(
            PropertyId::PaddingLeft.initial_value(),
            CssValue::Length(0.0, LengthUnit::Px)
        );

        // Typography
        assert_eq!(
            PropertyId::FontSize.initial_value(),
            CssValue::Keyword("medium".to_string())
        );
        assert_eq!(
            PropertyId::FontWeight.initial_value(),
            CssValue::Keyword("normal".to_string())
        );
        assert_eq!(
            PropertyId::TextAlign.initial_value(),
            CssValue::Keyword("start".to_string())
        );

        // Visibility & Opacity
        assert_eq!(
            PropertyId::Visibility.initial_value(),
            CssValue::Keyword("visible".to_string())
        );
        assert_eq!(PropertyId::Opacity.initial_value(), CssValue::Number(1.0));

        // Flexbox
        assert_eq!(
            PropertyId::FlexDirection.initial_value(),
            CssValue::Keyword("row".to_string())
        );
        assert_eq!(PropertyId::FlexGrow.initial_value(), CssValue::Number(0.0));
        assert_eq!(PropertyId::FlexShrink.initial_value(), CssValue::Number(1.0));
    }

    #[test]
    fn test_expand_shorthand_margin() {
        let value = CssValue::Length(10.0, LengthUnit::Px);
        let expanded = expand_shorthand("margin", &value);

        assert_eq!(expanded.len(), 4);
        assert!(expanded.contains(&(PropertyId::MarginTop, value.clone())));
        assert!(expanded.contains(&(PropertyId::MarginRight, value.clone())));
        assert!(expanded.contains(&(PropertyId::MarginBottom, value.clone())));
        assert!(expanded.contains(&(PropertyId::MarginLeft, value.clone())));
    }

    #[test]
    fn test_expand_shorthand_padding() {
        let value = CssValue::Length(20.0, LengthUnit::Px);
        let expanded = expand_shorthand("padding", &value);

        assert_eq!(expanded.len(), 4);
        assert!(expanded.contains(&(PropertyId::PaddingTop, value.clone())));
        assert!(expanded.contains(&(PropertyId::PaddingRight, value.clone())));
        assert!(expanded.contains(&(PropertyId::PaddingBottom, value.clone())));
        assert!(expanded.contains(&(PropertyId::PaddingLeft, value.clone())));
    }

    #[test]
    fn test_expand_shorthand_border() {
        let value = CssValue::Keyword("solid".to_string());
        let expanded = expand_shorthand("border", &value);

        assert_eq!(expanded.len(), 3);
        assert!(expanded.contains(&(PropertyId::BorderWidth, value.clone())));
        assert!(expanded.contains(&(PropertyId::BorderStyle, value.clone())));
        assert!(expanded.contains(&(PropertyId::BorderColor, value.clone())));
    }

    #[test]
    fn test_expand_shorthand_overflow() {
        let value = CssValue::Keyword("hidden".to_string());
        let expanded = expand_shorthand("overflow", &value);

        assert_eq!(expanded.len(), 2);
        assert!(expanded.contains(&(PropertyId::OverflowX, value.clone())));
        assert!(expanded.contains(&(PropertyId::OverflowY, value.clone())));
    }

    #[test]
    fn test_expand_shorthand_non_shorthand() {
        let value = CssValue::Keyword("block".to_string());
        let expanded = expand_shorthand("display", &value);

        assert_eq!(expanded.len(), 1);
        assert_eq!(expanded[0], (PropertyId::Display, value));
    }

    #[test]
    fn test_expand_shorthand_unknown_property() {
        let value = CssValue::Auto;
        let expanded = expand_shorthand("unknown-property", &value);

        assert_eq!(expanded.len(), 0);
    }

    #[test]
    fn test_property_id_roundtrip() {
        // Test that from_name and name are consistent
        let properties = vec![
            PropertyId::Display,
            PropertyId::MarginTop,
            PropertyId::FontSize,
            PropertyId::BackgroundColor,
            PropertyId::FlexDirection,
        ];

        for prop in properties {
            let name = prop.name();
            let parsed = PropertyId::from_name(name);
            assert_eq!(parsed, Some(prop));
        }
    }

    #[test]
    fn test_property_id_hash() {
        // Test that PropertyId can be used in HashMap
        let mut map = HashMap::new();
        map.insert(PropertyId::Color, CssValue::Color(CssColor::Named("red".to_string())));
        map.insert(PropertyId::FontSize, CssValue::Length(16.0, LengthUnit::Px));

        assert_eq!(
            map.get(&PropertyId::Color),
            Some(&CssValue::Color(CssColor::Named("red".to_string())))
        );
        assert_eq!(
            map.get(&PropertyId::FontSize),
            Some(&CssValue::Length(16.0, LengthUnit::Px))
        );
    }
}
