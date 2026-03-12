//! Computed style resolution for the Braille text browser.
//!
//! This module resolves cascaded CSS values (from the cascade algorithm) into
//! fully computed styles. All lengths become px, all colors are resolved, and
//! no `inherit`/`initial`/`unset` keywords remain.
//!
//! ## Resolution algorithm
//!
//! 1. Start with `ComputedStyle::initial()` (spec-defined defaults).
//! 2. For each property in the cascaded values:
//!    - `"inherit"` -> copy from parent (or initial if no parent)
//!    - `"initial"` -> use initial value
//!    - `"unset"` -> if inherited property, inherit; otherwise initial
//!    - Otherwise, parse the raw value string into the correct type
//! 3. For inherited properties NOT in cascaded: inherit from parent.
//! 4. For non-inherited properties NOT in cascaded: keep initial.

use std::collections::HashMap;

use crate::css::properties::PropertyId;

// ---------------------------------------------------------------------------
// CascadedEntry -- defined locally here.
// cascade.rs (Agent C-2A) defines its own CascadedEntry. Once both modules
// compile together, one should re-export the other to avoid duplication.
// ---------------------------------------------------------------------------

/// A single cascaded value produced by the cascade algorithm.
#[derive(Debug, Clone)]
pub struct CascadedEntry {
    /// The raw CSS value string (e.g. "red", "16px", "bold").
    pub value: String,
    /// Whether the declaration was marked `!important`.
    pub important: bool,
}

// ---------------------------------------------------------------------------
// Enum types for computed style fields
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Display {
    Block,
    Inline,
    InlineBlock,
    Flex,
    Grid,
    None,
    Table,
    TableRow,
    TableCell,
    ListItem,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Visibility {
    Visible,
    Hidden,
    Collapse,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Position {
    Static,
    Relative,
    Absolute,
    Fixed,
    Sticky,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextAlign {
    Left,
    Right,
    Center,
    Justify,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextDecoration {
    None,
    Underline,
    Overline,
    LineThrough,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FontStyle {
    Normal,
    Italic,
    Oblique,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Overflow {
    Visible,
    Hidden,
    Scroll,
    Auto,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ComputedColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: f32,
}

impl ComputedColor {
    pub fn new(r: u8, g: u8, b: u8, a: f32) -> Self {
        ComputedColor { r, g, b, a }
    }

    pub fn black() -> Self {
        ComputedColor { r: 0, g: 0, b: 0, a: 1.0 }
    }

    pub fn transparent() -> Self {
        ComputedColor { r: 0, g: 0, b: 0, a: 0.0 }
    }
}

// ---------------------------------------------------------------------------
// ComputedStyle
// ---------------------------------------------------------------------------

/// Fully resolved computed style for an element.
///
/// All lengths are in px, all colors are resolved, and no `inherit` / `initial` /
/// `unset` keywords remain.
#[derive(Debug, Clone)]
pub struct ComputedStyle {
    pub display: Display,
    pub visibility: Visibility,
    pub color: ComputedColor,
    pub background_color: ComputedColor,
    pub font_size: f32,
    pub font_weight: u16,
    pub font_style: FontStyle,
    pub font_family: String,
    pub line_height: f32,
    pub text_align: TextAlign,
    pub text_decoration: TextDecoration,
    pub margin_top: f32,
    pub margin_right: f32,
    pub margin_bottom: f32,
    pub margin_left: f32,
    pub padding_top: f32,
    pub padding_right: f32,
    pub padding_bottom: f32,
    pub padding_left: f32,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub position: Position,
    pub opacity: f32,
    pub overflow: Overflow,
}

/// Root default font size used for `rem` units.
const ROOT_FONT_SIZE: f32 = 16.0;

impl ComputedStyle {
    /// Returns the spec-defined initial computed style (used for the root element
    /// or when no cascade/inheritance applies).
    pub fn initial() -> Self {
        ComputedStyle {
            display: Display::Inline,
            visibility: Visibility::Visible,
            color: ComputedColor::black(),
            background_color: ComputedColor::transparent(),
            font_size: 16.0,
            font_weight: 400,
            font_style: FontStyle::Normal,
            font_family: "serif".to_string(),
            line_height: 19.2, // 1.2 * 16
            text_align: TextAlign::Left,
            text_decoration: TextDecoration::None,
            margin_top: 0.0,
            margin_right: 0.0,
            margin_bottom: 0.0,
            margin_left: 0.0,
            padding_top: 0.0,
            padding_right: 0.0,
            padding_bottom: 0.0,
            padding_left: 0.0,
            width: None,
            height: None,
            position: Position::Static,
            opacity: 1.0,
            overflow: Overflow::Visible,
        }
    }
}

// ---------------------------------------------------------------------------
// Value-parsing helpers
// ---------------------------------------------------------------------------

fn parse_display(val: &str) -> Display {
    match val.trim().to_ascii_lowercase().as_str() {
        "block" => Display::Block,
        "inline" => Display::Inline,
        "inline-block" => Display::InlineBlock,
        "flex" => Display::Flex,
        "grid" => Display::Grid,
        "none" => Display::None,
        "table" => Display::Table,
        "table-row" => Display::TableRow,
        "table-cell" => Display::TableCell,
        "list-item" => Display::ListItem,
        _ => Display::Inline, // fallback to initial
    }
}

fn parse_visibility(val: &str) -> Visibility {
    match val.trim().to_ascii_lowercase().as_str() {
        "visible" => Visibility::Visible,
        "hidden" => Visibility::Hidden,
        "collapse" => Visibility::Collapse,
        _ => Visibility::Visible,
    }
}

fn parse_position(val: &str) -> Position {
    match val.trim().to_ascii_lowercase().as_str() {
        "static" => Position::Static,
        "relative" => Position::Relative,
        "absolute" => Position::Absolute,
        "fixed" => Position::Fixed,
        "sticky" => Position::Sticky,
        _ => Position::Static,
    }
}

fn parse_text_align(val: &str) -> TextAlign {
    match val.trim().to_ascii_lowercase().as_str() {
        "left" | "start" => TextAlign::Left,
        "right" | "end" => TextAlign::Right,
        "center" => TextAlign::Center,
        "justify" => TextAlign::Justify,
        _ => TextAlign::Left,
    }
}

fn parse_text_decoration(val: &str) -> TextDecoration {
    match val.trim().to_ascii_lowercase().as_str() {
        "none" => TextDecoration::None,
        "underline" => TextDecoration::Underline,
        "overline" => TextDecoration::Overline,
        "line-through" => TextDecoration::LineThrough,
        _ => TextDecoration::None,
    }
}

fn parse_font_style(val: &str) -> FontStyle {
    match val.trim().to_ascii_lowercase().as_str() {
        "normal" => FontStyle::Normal,
        "italic" => FontStyle::Italic,
        "oblique" => FontStyle::Oblique,
        _ => FontStyle::Normal,
    }
}

fn parse_overflow(val: &str) -> Overflow {
    match val.trim().to_ascii_lowercase().as_str() {
        "visible" => Overflow::Visible,
        "hidden" => Overflow::Hidden,
        "scroll" => Overflow::Scroll,
        "auto" => Overflow::Auto,
        _ => Overflow::Visible,
    }
}

/// Parse a CSS color string into a `ComputedColor`.
///
/// Supports:
/// - Named colors (black, white, red, green, blue, yellow, cyan, magenta, gray/grey, orange, transparent)
/// - `rgb(r, g, b)` / `rgba(r, g, b, a)` functional notation
/// - Hex colors: `#rgb`, `#rrggbb`, `#rrggbbaa`
fn parse_color(val: &str) -> ComputedColor {
    let trimmed = val.trim().to_ascii_lowercase();

    // Named colors
    match trimmed.as_str() {
        "black" => return ComputedColor::new(0, 0, 0, 1.0),
        "white" => return ComputedColor::new(255, 255, 255, 1.0),
        "red" => return ComputedColor::new(255, 0, 0, 1.0),
        "green" => return ComputedColor::new(0, 128, 0, 1.0),
        "blue" => return ComputedColor::new(0, 0, 255, 1.0),
        "yellow" => return ComputedColor::new(255, 255, 0, 1.0),
        "cyan" | "aqua" => return ComputedColor::new(0, 255, 255, 1.0),
        "magenta" | "fuchsia" => return ComputedColor::new(255, 0, 255, 1.0),
        "gray" | "grey" => return ComputedColor::new(128, 128, 128, 1.0),
        "orange" => return ComputedColor::new(255, 165, 0, 1.0),
        "transparent" => return ComputedColor::transparent(),
        "currentcolor" => {
            // currentColor should inherit from color property; fallback to black
            return ComputedColor::black();
        }
        _ => {}
    }

    // rgba(r, g, b, a)
    if trimmed.starts_with("rgba(") && trimmed.ends_with(')') {
        let inner = &trimmed[5..trimmed.len() - 1];
        let parts: Vec<&str> = inner.split(',').collect();
        if parts.len() == 4 {
            let r = parts[0].trim().parse::<u8>().unwrap_or(0);
            let g = parts[1].trim().parse::<u8>().unwrap_or(0);
            let b = parts[2].trim().parse::<u8>().unwrap_or(0);
            let a = parts[3].trim().parse::<f32>().unwrap_or(1.0);
            return ComputedColor::new(r, g, b, a);
        }
    }

    // rgb(r, g, b)
    if trimmed.starts_with("rgb(") && trimmed.ends_with(')') {
        let inner = &trimmed[4..trimmed.len() - 1];
        let parts: Vec<&str> = inner.split(',').collect();
        if parts.len() == 3 {
            let r = parts[0].trim().parse::<u8>().unwrap_or(0);
            let g = parts[1].trim().parse::<u8>().unwrap_or(0);
            let b = parts[2].trim().parse::<u8>().unwrap_or(0);
            return ComputedColor::new(r, g, b, 1.0);
        }
    }

    // Hex colors
    if trimmed.starts_with('#') {
        let hex = &trimmed[1..];
        match hex.len() {
            // #rgb -> expand each digit: r -> rr, etc.
            3 => {
                let r = u8::from_str_radix(&hex[0..1], 16).unwrap_or(0);
                let g = u8::from_str_radix(&hex[1..2], 16).unwrap_or(0);
                let b = u8::from_str_radix(&hex[2..3], 16).unwrap_or(0);
                return ComputedColor::new(r * 17, g * 17, b * 17, 1.0);
            }
            // #rrggbb
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
                let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
                let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
                return ComputedColor::new(r, g, b, 1.0);
            }
            // #rrggbbaa
            8 => {
                let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
                let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
                let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
                let a = u8::from_str_radix(&hex[6..8], 16).unwrap_or(255);
                return ComputedColor::new(r, g, b, a as f32 / 255.0);
            }
            _ => {}
        }
    }

    // Fallback to black
    ComputedColor::black()
}

/// Parse a CSS length string into px.
///
/// Supports:
/// - `px` (absolute)
/// - `em` (relative to `parent_font_size`)
/// - `rem` (relative to root, i.e. 16px)
/// - `pt` (1pt = 4/3 px)
/// - `%` (percentage of `parent_font_size`)
/// - bare numbers (treated as px)
fn parse_length(val: &str, parent_font_size: f32) -> f32 {
    let trimmed = val.trim().to_ascii_lowercase();

    if trimmed == "0" {
        return 0.0;
    }

    if let Some(num) = trimmed.strip_suffix("px") {
        return num.trim().parse::<f32>().unwrap_or(0.0);
    }

    // Check rem BEFORE em, since "rem" also ends with "em"
    if let Some(num) = trimmed.strip_suffix("rem") {
        let factor = num.trim().parse::<f32>().unwrap_or(0.0);
        return factor * ROOT_FONT_SIZE;
    }

    if let Some(num) = trimmed.strip_suffix("em") {
        let factor = num.trim().parse::<f32>().unwrap_or(0.0);
        return factor * parent_font_size;
    }

    if let Some(num) = trimmed.strip_suffix("pt") {
        let pt = num.trim().parse::<f32>().unwrap_or(0.0);
        return pt * (4.0 / 3.0); // 1pt ≈ 1.333px
    }

    if let Some(num) = trimmed.strip_suffix('%') {
        let pct = num.trim().parse::<f32>().unwrap_or(0.0);
        return pct / 100.0 * parent_font_size;
    }

    // Bare number
    trimmed.parse::<f32>().unwrap_or(0.0)
}

/// Parse a font-size value, handling keywords like "medium", "small", "large", etc.
fn parse_font_size(val: &str, parent_font_size: f32) -> f32 {
    let trimmed = val.trim().to_ascii_lowercase();
    match trimmed.as_str() {
        "xx-small" => 9.0,
        "x-small" => 10.0,
        "small" => 13.0,
        "medium" => 16.0,
        "large" => 18.0,
        "x-large" => 24.0,
        "xx-large" => 32.0,
        "smaller" => parent_font_size * 0.833,
        "larger" => parent_font_size * 1.2,
        _ => parse_length(val, parent_font_size),
    }
}

/// Parse a font-weight value.
///
/// Handles keywords ("normal" -> 400, "bold" -> 700, "lighter", "bolder")
/// and numeric values (100-900).
fn parse_font_weight(val: &str) -> u16 {
    let trimmed = val.trim().to_ascii_lowercase();
    match trimmed.as_str() {
        "normal" => 400,
        "bold" => 700,
        "lighter" => 100, // simplified: should depend on parent
        "bolder" => 700,  // simplified: should depend on parent
        _ => trimmed.parse::<u16>().unwrap_or(400).clamp(100, 900),
    }
}

/// Parse a line-height value.
///
/// Handles "normal" (1.2 * font-size), unitless numbers (multiplier),
/// and length/percentage values.
fn parse_line_height(val: &str, font_size: f32, parent_font_size: f32) -> f32 {
    let trimmed = val.trim().to_ascii_lowercase();
    match trimmed.as_str() {
        "normal" => font_size * 1.2,
        _ => {
            // Try unitless number first (multiplier of own font-size)
            if let Ok(factor) = trimmed.parse::<f32>() {
                return factor * font_size;
            }
            // Otherwise parse as length
            parse_length(val, parent_font_size)
        }
    }
}

/// Parse an optional length (returns None for "auto").
fn parse_optional_length(val: &str, parent_font_size: f32) -> Option<f32> {
    let trimmed = val.trim().to_ascii_lowercase();
    if trimmed == "auto" {
        None
    } else {
        Some(parse_length(val, parent_font_size))
    }
}

/// Parse an opacity value (0.0 to 1.0).
fn parse_opacity(val: &str) -> f32 {
    val.trim()
        .parse::<f32>()
        .unwrap_or(1.0)
        .clamp(0.0, 1.0)
}

// ---------------------------------------------------------------------------
// Inheritance helpers
// ---------------------------------------------------------------------------

/// Returns true if the given CSS property name is inherited by default.
fn is_inherited_property(property: &str) -> bool {
    match PropertyId::from_name(property) {
        Some(pid) => pid.inherits(),
        None => {
            // Fallback for property names not in PropertyId
            matches!(
                property,
                "color"
                    | "font-size"
                    | "font-weight"
                    | "font-family"
                    | "font-style"
                    | "line-height"
                    | "text-align"
                    | "text-decoration"
                    | "text-transform"
                    | "visibility"
                    | "list-style-type"
                    | "letter-spacing"
                    | "word-spacing"
                    | "cursor"
            )
        }
    }
}

/// Copy the value of a single inherited property from a parent style.
fn inherit_property(style: &mut ComputedStyle, property: &str, parent: &ComputedStyle) {
    match property {
        "color" => style.color = parent.color.clone(),
        "font-size" => style.font_size = parent.font_size,
        "font-weight" => style.font_weight = parent.font_weight,
        "font-family" => style.font_family = parent.font_family.clone(),
        "font-style" => style.font_style = parent.font_style,
        "line-height" => style.line_height = parent.line_height,
        "text-align" => style.text_align = parent.text_align,
        "text-decoration" => style.text_decoration = parent.text_decoration,
        "visibility" => style.visibility = parent.visibility,
        _ => {}
    }
}

/// All inherited property names that we track in ComputedStyle.
const INHERITED_PROPERTIES: &[&str] = &[
    "color",
    "font-size",
    "font-weight",
    "font-family",
    "font-style",
    "line-height",
    "text-align",
    "text-decoration",
    "visibility",
];

// ---------------------------------------------------------------------------
// Core resolution entry point
// ---------------------------------------------------------------------------

/// Resolve cascaded values into a fully computed style.
///
/// `cascaded` maps CSS property names (e.g. `"color"`, `"font-size"`) to their
/// cascaded values from the cascade algorithm.
///
/// `parent_style` is the computed style of the parent element. Pass `None` for
/// the root element.
pub fn resolve_style(
    cascaded: &HashMap<String, CascadedEntry>,
    parent_style: Option<&ComputedStyle>,
) -> ComputedStyle {
    let initial = ComputedStyle::initial();
    let parent = parent_style.unwrap_or(&initial);

    // Start from initial values.
    let mut style = ComputedStyle::initial();

    // Phase 1: For inherited properties NOT in the cascaded map, inherit from parent.
    // We do this first so that explicit cascaded values can override in Phase 2.
    for &prop in INHERITED_PROPERTIES {
        if !cascaded.contains_key(prop) {
            if parent_style.is_some() {
                inherit_property(&mut style, prop, parent);
            }
            // If no parent, keep initial (already set).
        }
    }

    // Determine the parent's font-size for em/% resolution on this element.
    let parent_font_size = parent.font_size;

    // Phase 2: font-size must be resolved first because other properties (em, line-height)
    // depend on the element's own font-size.
    if let Some(entry) = cascaded.get("font-size") {
        let val = entry.value.trim();
        match val {
            "inherit" => style.font_size = parent.font_size,
            "initial" => style.font_size = initial.font_size,
            "unset" => {
                // font-size is inherited
                style.font_size = parent.font_size;
            }
            _ => style.font_size = parse_font_size(val, parent_font_size),
        }
    }

    // The element's own font-size, now resolved.
    let own_font_size = style.font_size;

    // Phase 3: Resolve all other cascaded properties.
    for (property, entry) in cascaded.iter() {
        // Skip font-size (already handled).
        if property == "font-size" {
            continue;
        }

        let val = entry.value.trim();

        // Handle CSS-wide keywords.
        if val == "inherit" {
            if parent_style.is_some() {
                apply_inherited_value(&mut style, property, parent);
            } else {
                apply_initial_value(&mut style, property, &initial);
            }
            continue;
        }
        if val == "initial" {
            apply_initial_value(&mut style, property, &initial);
            continue;
        }
        if val == "unset" {
            if is_inherited_property(property) {
                if parent_style.is_some() {
                    apply_inherited_value(&mut style, property, parent);
                }
                // else keep initial
            } else {
                apply_initial_value(&mut style, property, &initial);
            }
            continue;
        }

        // Parse the actual value.
        apply_parsed_value(&mut style, property, val, parent_font_size, own_font_size);
    }

    // Recompute line-height if it was not explicitly set but we have a parent.
    // The inherited line-height was set in Phase 1. We keep it as-is because
    // CSS spec says the *computed* value of line-height inherits, not the factor.
    if !cascaded.contains_key("line-height") && parent_style.is_some() {
        style.line_height = parent.line_height;
    }

    style
}

/// Apply a parsed (non-keyword) value to a computed style.
fn apply_parsed_value(
    style: &mut ComputedStyle,
    property: &str,
    val: &str,
    parent_font_size: f32,
    own_font_size: f32,
) {
    match property {
        "display" => style.display = parse_display(val),
        "visibility" => style.visibility = parse_visibility(val),
        "color" => style.color = parse_color(val),
        "background-color" => style.background_color = parse_color(val),
        "font-weight" => style.font_weight = parse_font_weight(val),
        "font-style" => style.font_style = parse_font_style(val),
        "font-family" => {
            style.font_family = val
                .trim()
                .trim_matches('"')
                .trim_matches('\'')
                .to_string();
        }
        "line-height" => style.line_height = parse_line_height(val, own_font_size, parent_font_size),
        "text-align" => style.text_align = parse_text_align(val),
        "text-decoration" => style.text_decoration = parse_text_decoration(val),
        "margin-top" => style.margin_top = parse_length(val, own_font_size),
        "margin-right" => style.margin_right = parse_length(val, own_font_size),
        "margin-bottom" => style.margin_bottom = parse_length(val, own_font_size),
        "margin-left" => style.margin_left = parse_length(val, own_font_size),
        "padding-top" => style.padding_top = parse_length(val, own_font_size),
        "padding-right" => style.padding_right = parse_length(val, own_font_size),
        "padding-bottom" => style.padding_bottom = parse_length(val, own_font_size),
        "padding-left" => style.padding_left = parse_length(val, own_font_size),
        "width" => style.width = parse_optional_length(val, own_font_size),
        "height" => style.height = parse_optional_length(val, own_font_size),
        "position" => style.position = parse_position(val),
        "opacity" => style.opacity = parse_opacity(val),
        "overflow" => style.overflow = parse_overflow(val),
        _ => {
            // Unknown properties are silently ignored.
        }
    }
}

/// Copy a single property from parent to style (for `inherit` keyword).
fn apply_inherited_value(style: &mut ComputedStyle, property: &str, parent: &ComputedStyle) {
    match property {
        "display" => style.display = parent.display,
        "visibility" => style.visibility = parent.visibility,
        "color" => style.color = parent.color.clone(),
        "background-color" => style.background_color = parent.background_color.clone(),
        "font-weight" => style.font_weight = parent.font_weight,
        "font-style" => style.font_style = parent.font_style,
        "font-family" => style.font_family = parent.font_family.clone(),
        "line-height" => style.line_height = parent.line_height,
        "text-align" => style.text_align = parent.text_align,
        "text-decoration" => style.text_decoration = parent.text_decoration,
        "margin-top" => style.margin_top = parent.margin_top,
        "margin-right" => style.margin_right = parent.margin_right,
        "margin-bottom" => style.margin_bottom = parent.margin_bottom,
        "margin-left" => style.margin_left = parent.margin_left,
        "padding-top" => style.padding_top = parent.padding_top,
        "padding-right" => style.padding_right = parent.padding_right,
        "padding-bottom" => style.padding_bottom = parent.padding_bottom,
        "padding-left" => style.padding_left = parent.padding_left,
        "width" => style.width = parent.width,
        "height" => style.height = parent.height,
        "position" => style.position = parent.position,
        "opacity" => style.opacity = parent.opacity,
        "overflow" => style.overflow = parent.overflow,
        _ => {}
    }
}

/// Reset a single property to its initial value.
fn apply_initial_value(style: &mut ComputedStyle, property: &str, initial: &ComputedStyle) {
    apply_inherited_value(style, property, initial);
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_cascaded() -> HashMap<String, CascadedEntry> {
        HashMap::new()
    }

    fn make_entry(value: &str) -> CascadedEntry {
        CascadedEntry {
            value: value.to_string(),
            important: false,
        }
    }

    fn make_important_entry(value: &str) -> CascadedEntry {
        CascadedEntry {
            value: value.to_string(),
            important: true,
        }
    }

    fn cascaded_with(pairs: &[(&str, &str)]) -> HashMap<String, CascadedEntry> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), make_entry(v)))
            .collect()
    }

    // --- 1. Initial values ---

    #[test]
    fn test_initial_values_are_correct() {
        let style = ComputedStyle::initial();

        assert_eq!(style.display, Display::Inline);
        assert_eq!(style.visibility, Visibility::Visible);
        assert_eq!(style.color, ComputedColor::black());
        assert_eq!(style.background_color, ComputedColor::transparent());
        assert_eq!(style.font_size, 16.0);
        assert_eq!(style.font_weight, 400);
        assert_eq!(style.font_style, FontStyle::Normal);
        assert_eq!(style.font_family, "serif");
        assert!((style.line_height - 19.2).abs() < 0.01);
        assert_eq!(style.text_align, TextAlign::Left);
        assert_eq!(style.text_decoration, TextDecoration::None);
        assert_eq!(style.margin_top, 0.0);
        assert_eq!(style.margin_right, 0.0);
        assert_eq!(style.margin_bottom, 0.0);
        assert_eq!(style.margin_left, 0.0);
        assert_eq!(style.padding_top, 0.0);
        assert_eq!(style.padding_right, 0.0);
        assert_eq!(style.padding_bottom, 0.0);
        assert_eq!(style.padding_left, 0.0);
        assert_eq!(style.width, None);
        assert_eq!(style.height, None);
        assert_eq!(style.position, Position::Static);
        assert_eq!(style.opacity, 1.0);
        assert_eq!(style.overflow, Overflow::Visible);
    }

    // --- 2. Inherited property passes from parent ---

    #[test]
    fn test_inherited_property_color_from_parent() {
        let mut parent = ComputedStyle::initial();
        parent.color = ComputedColor::new(255, 0, 0, 1.0); // red

        let cascaded = empty_cascaded();
        let style = resolve_style(&cascaded, Some(&parent));

        // color is inherited, so child should get red
        assert_eq!(style.color, ComputedColor::new(255, 0, 0, 1.0));
    }

    // --- 3. Non-inherited property uses initial when not set ---

    #[test]
    fn test_non_inherited_margin_uses_initial() {
        let mut parent = ComputedStyle::initial();
        parent.margin_top = 20.0;

        let cascaded = empty_cascaded();
        let style = resolve_style(&cascaded, Some(&parent));

        // margin-top is NOT inherited, so child should keep initial (0.0)
        assert_eq!(style.margin_top, 0.0);
    }

    // --- 4. `inherit` keyword forces inheritance ---

    #[test]
    fn test_inherit_keyword_forces_inheritance() {
        let mut parent = ComputedStyle::initial();
        parent.margin_top = 42.0;

        // margin-top is non-inherited; using `inherit` should force it
        let cascaded = cascaded_with(&[("margin-top", "inherit")]);
        let style = resolve_style(&cascaded, Some(&parent));

        assert_eq!(style.margin_top, 42.0);
    }

    // --- 5. `initial` keyword forces initial value ---

    #[test]
    fn test_initial_keyword_forces_initial() {
        let mut parent = ComputedStyle::initial();
        parent.color = ComputedColor::new(255, 0, 0, 1.0);

        // color is inherited by default; using `initial` should override to black
        let cascaded = cascaded_with(&[("color", "initial")]);
        let style = resolve_style(&cascaded, Some(&parent));

        assert_eq!(style.color, ComputedColor::black());
    }

    // --- 6. em units ---

    #[test]
    fn test_em_units_relative_to_parent_font_size() {
        let mut parent = ComputedStyle::initial();
        parent.font_size = 16.0;

        let cascaded = cascaded_with(&[("font-size", "2em")]);
        let style = resolve_style(&cascaded, Some(&parent));

        assert!((style.font_size - 32.0).abs() < 0.01);
    }

    // --- 7. rem units ---

    #[test]
    fn test_rem_units() {
        let mut parent = ComputedStyle::initial();
        parent.font_size = 20.0; // parent is not root default

        let cascaded = cascaded_with(&[("font-size", "2rem")]);
        let style = resolve_style(&cascaded, Some(&parent));

        // rem is always relative to root (16px), not parent
        assert!((style.font_size - 32.0).abs() < 0.01);
    }

    // --- 8. Percentage on font-size ---

    #[test]
    fn test_percentage_on_font_size() {
        let mut parent = ComputedStyle::initial();
        parent.font_size = 16.0;

        let cascaded = cascaded_with(&[("font-size", "150%")]);
        let style = resolve_style(&cascaded, Some(&parent));

        assert!((style.font_size - 24.0).abs() < 0.01);
    }

    // --- 9. `unset` on inherited property -> inherits ---

    #[test]
    fn test_unset_on_inherited_property() {
        let mut parent = ComputedStyle::initial();
        parent.color = ComputedColor::new(0, 128, 0, 1.0); // green

        let cascaded = cascaded_with(&[("color", "unset")]);
        let style = resolve_style(&cascaded, Some(&parent));

        // color is inherited, so `unset` behaves like `inherit`
        assert_eq!(style.color, ComputedColor::new(0, 128, 0, 1.0));
    }

    // --- 10. `unset` on non-inherited property -> initial ---

    #[test]
    fn test_unset_on_non_inherited_property() {
        let mut parent = ComputedStyle::initial();
        parent.margin_top = 50.0;

        let cascaded = cascaded_with(&[("margin-top", "unset")]);
        let style = resolve_style(&cascaded, Some(&parent));

        // margin-top is NOT inherited, so `unset` behaves like `initial`
        assert_eq!(style.margin_top, 0.0);
    }

    // --- 11. display: none ---

    #[test]
    fn test_display_none() {
        let cascaded = cascaded_with(&[("display", "none")]);
        let style = resolve_style(&cascaded, None);

        assert_eq!(style.display, Display::None);
    }

    // --- 12. visibility: hidden ---

    #[test]
    fn test_visibility_hidden() {
        let cascaded = cascaded_with(&[("visibility", "hidden")]);
        let style = resolve_style(&cascaded, None);

        assert_eq!(style.visibility, Visibility::Hidden);
    }

    // --- 13. Named colors ---

    #[test]
    fn test_named_colors_resolve_correctly() {
        assert_eq!(parse_color("black"), ComputedColor::new(0, 0, 0, 1.0));
        assert_eq!(parse_color("white"), ComputedColor::new(255, 255, 255, 1.0));
        assert_eq!(parse_color("red"), ComputedColor::new(255, 0, 0, 1.0));
        assert_eq!(parse_color("green"), ComputedColor::new(0, 128, 0, 1.0));
        assert_eq!(parse_color("blue"), ComputedColor::new(0, 0, 255, 1.0));
        assert_eq!(parse_color("yellow"), ComputedColor::new(255, 255, 0, 1.0));
        assert_eq!(parse_color("cyan"), ComputedColor::new(0, 255, 255, 1.0));
        assert_eq!(parse_color("magenta"), ComputedColor::new(255, 0, 255, 1.0));
        assert_eq!(parse_color("gray"), ComputedColor::new(128, 128, 128, 1.0));
        assert_eq!(parse_color("grey"), ComputedColor::new(128, 128, 128, 1.0));
        assert_eq!(parse_color("orange"), ComputedColor::new(255, 165, 0, 1.0));
        assert_eq!(parse_color("transparent"), ComputedColor::transparent());
    }

    // --- 14. font-weight "bold" -> 700 ---

    #[test]
    fn test_font_weight_bold() {
        let cascaded = cascaded_with(&[("font-weight", "bold")]);
        let style = resolve_style(&cascaded, None);

        assert_eq!(style.font_weight, 700);
    }

    // --- Additional tests ---

    #[test]
    fn test_font_weight_normal() {
        assert_eq!(parse_font_weight("normal"), 400);
    }

    #[test]
    fn test_font_weight_numeric() {
        assert_eq!(parse_font_weight("300"), 300);
        assert_eq!(parse_font_weight("600"), 600);
        assert_eq!(parse_font_weight("900"), 900);
    }

    #[test]
    fn test_hex_color_parsing() {
        assert_eq!(parse_color("#ff0000"), ComputedColor::new(255, 0, 0, 1.0));
        assert_eq!(parse_color("#00ff00"), ComputedColor::new(0, 255, 0, 1.0));
        assert_eq!(parse_color("#0000ff"), ComputedColor::new(0, 0, 255, 1.0));
    }

    #[test]
    fn test_short_hex_color_parsing() {
        // #f00 -> #ff0000
        assert_eq!(parse_color("#f00"), ComputedColor::new(255, 0, 0, 1.0));
    }

    #[test]
    fn test_rgb_functional_color() {
        assert_eq!(
            parse_color("rgb(128, 64, 32)"),
            ComputedColor::new(128, 64, 32, 1.0)
        );
    }

    #[test]
    fn test_rgba_functional_color() {
        assert_eq!(
            parse_color("rgba(128, 64, 32, 0.5)"),
            ComputedColor::new(128, 64, 32, 0.5)
        );
    }

    #[test]
    fn test_display_block() {
        let cascaded = cascaded_with(&[("display", "block")]);
        let style = resolve_style(&cascaded, None);
        assert_eq!(style.display, Display::Block);
    }

    #[test]
    fn test_display_flex() {
        let cascaded = cascaded_with(&[("display", "flex")]);
        let style = resolve_style(&cascaded, None);
        assert_eq!(style.display, Display::Flex);
    }

    #[test]
    fn test_position_absolute() {
        let cascaded = cascaded_with(&[("position", "absolute")]);
        let style = resolve_style(&cascaded, None);
        assert_eq!(style.position, Position::Absolute);
    }

    #[test]
    fn test_padding_em_uses_own_font_size() {
        // When padding uses em, it is relative to the element's own font-size.
        let cascaded = cascaded_with(&[
            ("font-size", "20px"),
            ("padding-top", "2em"),
        ]);
        let style = resolve_style(&cascaded, None);

        // own font-size is 20px, so 2em = 40px
        assert!((style.padding_top - 40.0).abs() < 0.01);
    }

    #[test]
    fn test_width_auto() {
        let cascaded = cascaded_with(&[("width", "auto")]);
        let style = resolve_style(&cascaded, None);
        assert_eq!(style.width, None);
    }

    #[test]
    fn test_width_px() {
        let cascaded = cascaded_with(&[("width", "200px")]);
        let style = resolve_style(&cascaded, None);
        assert_eq!(style.width, Some(200.0));
    }

    #[test]
    fn test_opacity() {
        let cascaded = cascaded_with(&[("opacity", "0.5")]);
        let style = resolve_style(&cascaded, None);
        assert!((style.opacity - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_overflow_hidden() {
        let cascaded = cascaded_with(&[("overflow", "hidden")]);
        let style = resolve_style(&cascaded, None);
        assert_eq!(style.overflow, Overflow::Hidden);
    }

    #[test]
    fn test_multiple_properties_together() {
        let cascaded = cascaded_with(&[
            ("display", "block"),
            ("color", "blue"),
            ("font-size", "20px"),
            ("margin-top", "10px"),
            ("padding-left", "5px"),
        ]);
        let style = resolve_style(&cascaded, None);

        assert_eq!(style.display, Display::Block);
        assert_eq!(style.color, ComputedColor::new(0, 0, 255, 1.0));
        assert_eq!(style.font_size, 20.0);
        assert_eq!(style.margin_top, 10.0);
        assert_eq!(style.padding_left, 5.0);
    }

    #[test]
    fn test_root_element_no_parent() {
        // With no parent, inherited properties should use initial values.
        let cascaded = empty_cascaded();
        let style = resolve_style(&cascaded, None);

        assert_eq!(style.color, ComputedColor::black());
        assert_eq!(style.font_size, 16.0);
        assert_eq!(style.font_weight, 400);
    }

    #[test]
    fn test_font_family_quoted() {
        let cascaded = cascaded_with(&[("font-family", "\"Helvetica Neue\"")]);
        let style = resolve_style(&cascaded, None);
        assert_eq!(style.font_family, "Helvetica Neue");
    }

    #[test]
    fn test_text_align_center() {
        let cascaded = cascaded_with(&[("text-align", "center")]);
        let style = resolve_style(&cascaded, None);
        assert_eq!(style.text_align, TextAlign::Center);
    }

    #[test]
    fn test_text_decoration_underline() {
        let cascaded = cascaded_with(&[("text-decoration", "underline")]);
        let style = resolve_style(&cascaded, None);
        assert_eq!(style.text_decoration, TextDecoration::Underline);
    }

    #[test]
    fn test_font_style_italic() {
        let cascaded = cascaded_with(&[("font-style", "italic")]);
        let style = resolve_style(&cascaded, None);
        assert_eq!(style.font_style, FontStyle::Italic);
    }

    #[test]
    fn test_inherited_font_size_cascades() {
        // Parent has font-size 20px, child sets nothing -> inherits 20px
        let mut parent = ComputedStyle::initial();
        parent.font_size = 20.0;

        let cascaded = empty_cascaded();
        let style = resolve_style(&cascaded, Some(&parent));

        assert_eq!(style.font_size, 20.0);
    }

    #[test]
    fn test_inherit_keyword_on_non_inherited_background() {
        let mut parent = ComputedStyle::initial();
        parent.background_color = ComputedColor::new(255, 0, 0, 1.0);

        let cascaded = cascaded_with(&[("background-color", "inherit")]);
        let style = resolve_style(&cascaded, Some(&parent));

        assert_eq!(style.background_color, ComputedColor::new(255, 0, 0, 1.0));
    }

    #[test]
    fn test_line_height_unitless_multiplier() {
        let cascaded = cascaded_with(&[
            ("font-size", "20px"),
            ("line-height", "1.5"),
        ]);
        let style = resolve_style(&cascaded, None);

        // 1.5 * 20px = 30px
        assert!((style.line_height - 30.0).abs() < 0.01);
    }

    #[test]
    fn test_line_height_px() {
        let cascaded = cascaded_with(&[("line-height", "24px")]);
        let style = resolve_style(&cascaded, None);

        assert!((style.line_height - 24.0).abs() < 0.01);
    }

    #[test]
    fn test_pt_units() {
        // 12pt = 16px
        let result = parse_length("12pt", 16.0);
        assert!((result - 16.0).abs() < 0.01);
    }

    #[test]
    fn test_font_size_keyword_medium() {
        assert!((parse_font_size("medium", 16.0) - 16.0).abs() < 0.01);
    }

    #[test]
    fn test_font_size_keyword_large() {
        assert!((parse_font_size("large", 16.0) - 18.0).abs() < 0.01);
    }

    #[test]
    fn test_cascaded_entry_construction() {
        let entry = make_entry("red");
        assert_eq!(entry.value, "red");
        assert!(!entry.important);
    }

    #[test]
    fn test_cascaded_entry_important_construction() {
        let entry = make_important_entry("red");
        assert_eq!(entry.value, "red");
        assert!(entry.important);
    }
}
