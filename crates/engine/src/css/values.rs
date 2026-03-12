use std::fmt;

/// CSS length units
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LengthUnit {
    Px,
    Em,
    Rem,
    Pt,
    Vh,
    Vw,
    Percent,
}

impl fmt::Display for LengthUnit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LengthUnit::Px => write!(f, "px"),
            LengthUnit::Em => write!(f, "em"),
            LengthUnit::Rem => write!(f, "rem"),
            LengthUnit::Pt => write!(f, "pt"),
            LengthUnit::Vh => write!(f, "vh"),
            LengthUnit::Vw => write!(f, "vw"),
            LengthUnit::Percent => write!(f, "%"),
        }
    }
}

/// CSS color representation
#[derive(Debug, Clone, PartialEq)]
pub enum CssColor {
    Named(String),
    Rgb(u8, u8, u8),
    Rgba(u8, u8, u8, f32),
    Hex(String),
}

impl fmt::Display for CssColor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CssColor::Named(name) => write!(f, "{}", name),
            CssColor::Rgb(r, g, b) => write!(f, "rgb({}, {}, {})", r, g, b),
            CssColor::Rgba(r, g, b, a) => write!(f, "rgba({}, {}, {}, {})", r, g, b, a),
            CssColor::Hex(hex) => write!(f, "{}", hex),
        }
    }
}

/// CSS value types
#[derive(Debug, Clone, PartialEq)]
pub enum CssValue {
    /// CSS keyword like "block", "none", "inherit", "initial", "bold"
    Keyword(String),
    /// Length with unit (e.g., 16px, 1.5em, 2rem)
    Length(f32, LengthUnit),
    /// Percentage value (e.g., 50%)
    Percentage(f32),
    /// Color value
    Color(CssColor),
    /// Unitless number (e.g., line-height: 1.5)
    Number(f32),
    /// Quoted string (e.g., font-family: "Arial")
    String(String),
    /// The `auto` keyword
    Auto,
    /// The `none` keyword (distinct from "not set")
    None,
}

impl fmt::Display for CssValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CssValue::Keyword(k) => write!(f, "{}", k),
            CssValue::Length(val, unit) => {
                if *unit == LengthUnit::Percent {
                    write!(f, "{}%", val)
                } else {
                    write!(f, "{}{}", val, unit)
                }
            }
            CssValue::Percentage(val) => write!(f, "{}%", val),
            CssValue::Color(color) => write!(f, "{}", color),
            CssValue::Number(n) => write!(f, "{}", n),
            CssValue::String(s) => write!(f, "\"{}\"", s),
            CssValue::Auto => write!(f, "auto"),
            CssValue::None => write!(f, "none"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_length_unit_display() {
        assert_eq!(format!("{}", LengthUnit::Px), "px");
        assert_eq!(format!("{}", LengthUnit::Em), "em");
        assert_eq!(format!("{}", LengthUnit::Rem), "rem");
        assert_eq!(format!("{}", LengthUnit::Pt), "pt");
        assert_eq!(format!("{}", LengthUnit::Vh), "vh");
        assert_eq!(format!("{}", LengthUnit::Vw), "vw");
        assert_eq!(format!("{}", LengthUnit::Percent), "%");
    }

    #[test]
    fn test_css_color_display() {
        assert_eq!(format!("{}", CssColor::Named("red".to_string())), "red");
        assert_eq!(format!("{}", CssColor::Rgb(255, 0, 0)), "rgb(255, 0, 0)");
        assert_eq!(format!("{}", CssColor::Rgba(255, 0, 0, 0.5)), "rgba(255, 0, 0, 0.5)");
        assert_eq!(format!("{}", CssColor::Hex("#ff0000".to_string())), "#ff0000");
    }

    #[test]
    fn test_css_value_display() {
        assert_eq!(format!("{}", CssValue::Keyword("block".to_string())), "block");
        assert_eq!(format!("{}", CssValue::Length(16.0, LengthUnit::Px)), "16px");
        assert_eq!(format!("{}", CssValue::Length(1.5, LengthUnit::Em)), "1.5em");
        assert_eq!(format!("{}", CssValue::Length(50.0, LengthUnit::Percent)), "50%");
        assert_eq!(format!("{}", CssValue::Percentage(50.0)), "50%");
        assert_eq!(format!("{}", CssValue::Color(CssColor::Named("blue".to_string()))), "blue");
        assert_eq!(format!("{}", CssValue::Number(1.5)), "1.5");
        assert_eq!(format!("{}", CssValue::String("Arial".to_string())), "\"Arial\"");
        assert_eq!(format!("{}", CssValue::Auto), "auto");
        assert_eq!(format!("{}", CssValue::None), "none");
    }

    #[test]
    fn test_css_value_construction() {
        let length = CssValue::Length(16.0, LengthUnit::Px);
        assert_eq!(length, CssValue::Length(16.0, LengthUnit::Px));

        let color = CssValue::Color(CssColor::Rgb(255, 0, 0));
        assert_eq!(color, CssValue::Color(CssColor::Rgb(255, 0, 0)));

        let keyword = CssValue::Keyword("inherit".to_string());
        assert_eq!(keyword, CssValue::Keyword("inherit".to_string()));
    }

    #[test]
    fn test_length_unit_equality() {
        assert_eq!(LengthUnit::Px, LengthUnit::Px);
        assert_ne!(LengthUnit::Px, LengthUnit::Em);
    }

    #[test]
    fn test_css_color_equality() {
        assert_eq!(CssColor::Named("red".to_string()), CssColor::Named("red".to_string()));
        assert_ne!(CssColor::Named("red".to_string()), CssColor::Named("blue".to_string()));
        assert_eq!(CssColor::Rgb(255, 0, 0), CssColor::Rgb(255, 0, 0));
        assert_ne!(CssColor::Rgb(255, 0, 0), CssColor::Rgb(0, 255, 0));
    }
}
