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

use crate::css::calc::{self, CalcContext};
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
    Contents,
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ComputedLength {
    Px(f32),
    Percent(f32), // 0.0–1.0 (50% stored as 0.5)
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
        ComputedColor {
            r: 0,
            g: 0,
            b: 0,
            a: 1.0,
        }
    }

    pub fn transparent() -> Self {
        ComputedColor {
            r: 0,
            g: 0,
            b: 0,
            a: 0.0,
        }
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
    pub margin_top: ComputedLength,
    pub margin_right: ComputedLength,
    pub margin_bottom: ComputedLength,
    pub margin_left: ComputedLength,
    pub padding_top: ComputedLength,
    pub padding_right: ComputedLength,
    pub padding_bottom: ComputedLength,
    pub padding_left: ComputedLength,
    pub width: Option<ComputedLength>,
    pub height: Option<ComputedLength>,
    pub position: Position,
    pub top: Option<ComputedLength>,
    pub right: Option<ComputedLength>,
    pub bottom: Option<ComputedLength>,
    pub left: Option<ComputedLength>,
    pub opacity: f32,
    pub overflow: Overflow,
    pub scroll_snap_type: String,
    pub scroll_snap_align: String,
    pub custom_properties: HashMap<String, String>,
    // Grid properties (stored as raw strings, parsed at layout time)
    pub grid_template_columns: String,
    pub grid_template_rows: String,
    pub grid_column_start: String,
    pub grid_column_end: String,
    pub grid_row_start: String,
    pub grid_row_end: String,
    pub row_gap: String,
    pub column_gap: String,
    pub grid_auto_flow: String,
    pub grid_auto_columns: String,
    pub grid_auto_rows: String,
    pub align_content: String,
    pub justify_items: String,
    pub align_self: String,
    pub justify_self: String,
    // Min/max size
    pub min_width: Option<ComputedLength>,
    pub min_height: Option<ComputedLength>,
    pub max_width: Option<ComputedLength>,
    pub max_height: Option<ComputedLength>,
}

/// Root default font size used for `rem` units.
const ROOT_FONT_SIZE: f32 = 16.0;

/// Viewport dimensions used for `vh` and `vw` units.
/// Must match window.innerWidth / innerHeight in dom_stubs.rs.
const VIEWPORT_WIDTH: f32 = 1280.0;
const VIEWPORT_HEIGHT: f32 = 800.0;

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
            margin_top: ComputedLength::Px(0.0),
            margin_right: ComputedLength::Px(0.0),
            margin_bottom: ComputedLength::Px(0.0),
            margin_left: ComputedLength::Px(0.0),
            padding_top: ComputedLength::Px(0.0),
            padding_right: ComputedLength::Px(0.0),
            padding_bottom: ComputedLength::Px(0.0),
            padding_left: ComputedLength::Px(0.0),
            width: None,
            height: None,
            position: Position::Static,
            top: None,
            right: None,
            bottom: None,
            left: None,
            opacity: 1.0,
            overflow: Overflow::Visible,
            scroll_snap_type: "none".to_string(),
            scroll_snap_align: "none".to_string(),
            custom_properties: HashMap::new(),
            grid_template_columns: String::new(),
            grid_template_rows: String::new(),
            grid_column_start: String::new(),
            grid_column_end: String::new(),
            grid_row_start: String::new(),
            grid_row_end: String::new(),
            row_gap: String::new(),
            column_gap: String::new(),
            grid_auto_flow: String::new(),
            grid_auto_columns: String::new(),
            grid_auto_rows: String::new(),
            align_content: String::new(),
            justify_items: String::new(),
            align_self: String::new(),
            justify_self: String::new(),
            min_width: None,
            min_height: None,
            max_width: None,
            max_height: None,
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
        "contents" => Display::Contents,
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
    if let Some(hex) = trimmed.strip_prefix('#') {
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

    // calc()/min()/max()/clamp() — delegate to calc module
    if calc::is_math_function(&trimmed) {
        let ctx = CalcContext {
            font_size: parent_font_size,
            parent_font_size,
            root_font_size: ROOT_FONT_SIZE,
            vw: VIEWPORT_WIDTH,
            vh: VIEWPORT_HEIGHT,
        };
        if let Some(result) = calc::eval_math_function(&trimmed, &ctx) {
            return result;
        }
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

    if let Some(num) = trimmed.strip_suffix("vh") {
        let factor = num.trim().parse::<f32>().unwrap_or(0.0);
        return factor / 100.0 * VIEWPORT_HEIGHT;
    }

    if let Some(num) = trimmed.strip_suffix("vw") {
        let factor = num.trim().parse::<f32>().unwrap_or(0.0);
        return factor / 100.0 * VIEWPORT_WIDTH;
    }

    if let Some(num) = trimmed.strip_suffix('%') {
        let pct = num.trim().parse::<f32>().unwrap_or(0.0);
        return pct / 100.0 * parent_font_size;
    }

    // Bare number
    trimmed.parse::<f32>().unwrap_or(0.0)
}

/// Substitute `var(--name)` and `var(--name, fallback)` in a CSS value string.
/// Handles nested var() references.
fn substitute_var(value: &str, custom_props: &HashMap<String, String>) -> String {
    let mut result = value.to_string();
    // Iterate up to 10 times to handle nested var() references
    for _ in 0..10 {
        if !result.contains("var(") {
            break;
        }
        let mut new_result = String::with_capacity(result.len());
        let mut i = 0;
        let bytes = result.as_bytes();
        while i < bytes.len() {
            if i + 4 <= bytes.len() && &result[i..i + 4] == "var(" {
                // Find matching close paren
                let start = i + 4;
                let mut depth = 1;
                let mut end = start;
                while end < bytes.len() {
                    if bytes[end] == b'(' {
                        depth += 1;
                    } else if bytes[end] == b')' {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    end += 1;
                }
                let inner = &result[start..end];
                // Split on first comma for fallback
                let (name, fallback) = if let Some(comma_pos) = find_top_level_comma(inner) {
                    (inner[..comma_pos].trim(), Some(inner[comma_pos + 1..].trim()))
                } else {
                    (inner.trim(), None)
                };
                // Look up the custom property
                if let Some(val) = custom_props.get(name) {
                    new_result.push_str(val.trim());
                } else if let Some(fb) = fallback {
                    new_result.push_str(fb);
                }
                // Skip past the closing paren
                i = if end < bytes.len() { end + 1 } else { end };
            } else {
                new_result.push(result.as_bytes()[i] as char);
                i += 1;
            }
        }
        if new_result == result {
            break;
        }
        result = new_result;
    }
    result
}

/// Find the first comma at top level (not inside nested parens).
fn find_top_level_comma(s: &str) -> Option<usize> {
    let mut depth = 0;
    for (i, c) in s.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => return Some(i),
            _ => {}
        }
    }
    None
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

/// Parse a CSS length that may be a percentage of the containing block.
/// For layout properties (width, height, margin, padding, inset), `%` means
/// "percentage of containing block" and must be resolved by the layout engine,
/// NOT during CSS computation.
fn parse_length_or_percent(val: &str, font_size: f32) -> ComputedLength {
    let trimmed = val.trim().to_ascii_lowercase();
    if let Some(num) = trimmed.strip_suffix('%') {
        let pct = num.trim().parse::<f32>().unwrap_or(0.0);
        ComputedLength::Percent(pct / 100.0)
    } else {
        ComputedLength::Px(parse_length(val, font_size))
    }
}

fn parse_optional_length_or_percent(val: &str, font_size: f32) -> Option<ComputedLength> {
    let trimmed = val.trim().to_ascii_lowercase();
    if trimmed == "auto" {
        None
    } else {
        Some(parse_length_or_percent(val, font_size))
    }
}

/// Parse an opacity value (0.0 to 1.0).
fn parse_opacity(val: &str) -> f32 {
    val.trim().parse::<f32>().unwrap_or(1.0).clamp(0.0, 1.0)
}

// ---------------------------------------------------------------------------
// Inheritance helpers
// ---------------------------------------------------------------------------

/// Returns true if the given CSS property name is inherited by default.
fn is_inherited_property(property: &str) -> bool {
    // CSS custom properties always inherit
    if property.starts_with("--") {
        return true;
    }
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
pub fn resolve_style(cascaded: &HashMap<String, CascadedEntry>, parent_style: Option<&ComputedStyle>) -> ComputedStyle {
    let initial = ComputedStyle::initial();
    let parent = parent_style.unwrap_or(&initial);

    // Start from initial values.
    let mut style = ComputedStyle::initial();

    // Phase 0: Collect custom properties (--*) and inherit from parent.
    // Custom properties always inherit.
    if parent_style.is_some() {
        style.custom_properties = parent.custom_properties.clone();
    }
    for (property, entry) in cascaded.iter() {
        if property.starts_with("--") {
            let val = entry.value.trim();
            if val == "initial" {
                style.custom_properties.remove(property.as_str());
            } else if val == "inherit" || val == "unset" {
                // Already inherited from parent in the clone above
            } else {
                style.custom_properties.insert(property.clone(), val.to_string());
            }
        }
    }

    // Phase 1: For inherited properties NOT in the cascaded map, inherit from parent.
    // We do this first so that explicit cascaded values can override in Phase 2.
    for &prop in INHERITED_PROPERTIES {
        if !cascaded.contains_key(prop) && parent_style.is_some() {
            inherit_property(&mut style, prop, parent);
            // If no parent, keep initial (already set).
        }
    }

    // Determine the parent's font-size for em/% resolution on this element.
    let parent_font_size = parent.font_size;

    // Phase 2: font-size must be resolved first because other properties (em, line-height)
    // depend on the element's own font-size.
    if let Some(entry) = cascaded.get("font-size") {
        let val = substitute_var(entry.value.trim(), &style.custom_properties);
        let val = val.trim();
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
        // Skip font-size (already handled) and custom properties (handled in Phase 0).
        if property == "font-size" || property.starts_with("--") {
            continue;
        }

        // Substitute var() references before processing
        let raw_val = entry.value.trim();
        let val = substitute_var(raw_val, &style.custom_properties);
        let val = val.trim();

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
fn apply_parsed_value(style: &mut ComputedStyle, property: &str, val: &str, parent_font_size: f32, own_font_size: f32) {
    match property {
        "display" => style.display = parse_display(val),
        "visibility" => style.visibility = parse_visibility(val),
        "color" => style.color = parse_color(val),
        "background-color" => style.background_color = parse_color(val),
        "font-weight" => style.font_weight = parse_font_weight(val),
        "font-style" => style.font_style = parse_font_style(val),
        "font-family" => {
            style.font_family = val.trim().trim_matches('"').trim_matches('\'').to_string();
        }
        "line-height" => style.line_height = parse_line_height(val, own_font_size, parent_font_size),
        "text-align" => style.text_align = parse_text_align(val),
        "text-decoration" => style.text_decoration = parse_text_decoration(val),
        "margin-top" => style.margin_top = parse_length_or_percent(val, own_font_size),
        "margin-right" => style.margin_right = parse_length_or_percent(val, own_font_size),
        "margin-bottom" => style.margin_bottom = parse_length_or_percent(val, own_font_size),
        "margin-left" => style.margin_left = parse_length_or_percent(val, own_font_size),
        "padding-top" => style.padding_top = parse_length_or_percent(val, own_font_size),
        "padding-right" => style.padding_right = parse_length_or_percent(val, own_font_size),
        "padding-bottom" => style.padding_bottom = parse_length_or_percent(val, own_font_size),
        "padding-left" => style.padding_left = parse_length_or_percent(val, own_font_size),
        "width" => style.width = parse_optional_length_or_percent(val, own_font_size),
        "height" => style.height = parse_optional_length_or_percent(val, own_font_size),
        "position" => style.position = parse_position(val),
        "top" => style.top = parse_optional_length_or_percent(val, own_font_size),
        "right" => style.right = parse_optional_length_or_percent(val, own_font_size),
        "bottom" => style.bottom = parse_optional_length_or_percent(val, own_font_size),
        "left" => style.left = parse_optional_length_or_percent(val, own_font_size),
        "opacity" => style.opacity = parse_opacity(val),
        "overflow" => style.overflow = parse_overflow(val),
        "scroll-snap-type" => style.scroll_snap_type = val.trim().to_ascii_lowercase(),
        "scroll-snap-align" => style.scroll_snap_align = val.trim().to_ascii_lowercase(),
        // Min/max size
        "min-width" => style.min_width = parse_optional_length_or_percent(val, own_font_size),
        "min-height" => style.min_height = parse_optional_length_or_percent(val, own_font_size),
        "max-width" => style.max_width = parse_optional_length_or_percent(val, own_font_size),
        "max-height" => style.max_height = parse_optional_length_or_percent(val, own_font_size),
        // Grid properties (stored as raw strings)
        "grid-template-columns" => style.grid_template_columns = val.trim().to_string(),
        "grid-template-rows" => style.grid_template_rows = val.trim().to_string(),
        "grid-column-start" => style.grid_column_start = val.trim().to_string(),
        "grid-column-end" => style.grid_column_end = val.trim().to_string(),
        "grid-row-start" => style.grid_row_start = val.trim().to_string(),
        "grid-row-end" => style.grid_row_end = val.trim().to_string(),
        "grid-column" => {
            // Shorthand: grid-column: start / end
            let parts: Vec<&str> = val.splitn(2, '/').collect();
            style.grid_column_start = parts[0].trim().to_string();
            if parts.len() > 1 {
                style.grid_column_end = parts[1].trim().to_string();
            }
        }
        "grid-row" => {
            // Shorthand: grid-row: start / end
            let parts: Vec<&str> = val.splitn(2, '/').collect();
            style.grid_row_start = parts[0].trim().to_string();
            if parts.len() > 1 {
                style.grid_row_end = parts[1].trim().to_string();
            }
        }
        "row-gap" => style.row_gap = val.trim().to_string(),
        "column-gap" => style.column_gap = val.trim().to_string(),
        "gap" => {
            // Shorthand: gap sets both row-gap and column-gap
            let parts: Vec<&str> = val.split_whitespace().collect();
            style.row_gap = parts[0].to_string();
            style.column_gap = if parts.len() > 1 { parts[1].to_string() } else { parts[0].to_string() };
        }
        "grid-auto-flow" => style.grid_auto_flow = val.trim().to_string(),
        "grid-auto-columns" => style.grid_auto_columns = val.trim().to_string(),
        "grid-auto-rows" => style.grid_auto_rows = val.trim().to_string(),
        "align-content" => style.align_content = val.trim().to_string(),
        "justify-items" => style.justify_items = val.trim().to_string(),
        "align-self" => style.align_self = val.trim().to_string(),
        "justify-self" => style.justify_self = val.trim().to_string(),
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
        "scroll-snap-type" => style.scroll_snap_type = parent.scroll_snap_type.clone(),
        "scroll-snap-align" => style.scroll_snap_align = parent.scroll_snap_align.clone(),
        "min-width" => style.min_width = parent.min_width,
        "min-height" => style.min_height = parent.min_height,
        "max-width" => style.max_width = parent.max_width,
        "max-height" => style.max_height = parent.max_height,
        "grid-template-columns" => style.grid_template_columns = parent.grid_template_columns.clone(),
        "grid-template-rows" => style.grid_template_rows = parent.grid_template_rows.clone(),
        "grid-column-start" => style.grid_column_start = parent.grid_column_start.clone(),
        "grid-column-end" => style.grid_column_end = parent.grid_column_end.clone(),
        "grid-row-start" => style.grid_row_start = parent.grid_row_start.clone(),
        "grid-row-end" => style.grid_row_end = parent.grid_row_end.clone(),
        "row-gap" => style.row_gap = parent.row_gap.clone(),
        "column-gap" => style.column_gap = parent.column_gap.clone(),
        "grid-auto-flow" => style.grid_auto_flow = parent.grid_auto_flow.clone(),
        "grid-auto-columns" => style.grid_auto_columns = parent.grid_auto_columns.clone(),
        "grid-auto-rows" => style.grid_auto_rows = parent.grid_auto_rows.clone(),
        "align-content" => style.align_content = parent.align_content.clone(),
        "justify-items" => style.justify_items = parent.justify_items.clone(),
        "align-self" => style.align_self = parent.align_self.clone(),
        "justify-self" => style.justify_self = parent.justify_self.clone(),
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
        pairs.iter().map(|(k, v)| (k.to_string(), make_entry(v))).collect()
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
        assert_eq!(style.margin_top, ComputedLength::Px(0.0));
        assert_eq!(style.margin_right, ComputedLength::Px(0.0));
        assert_eq!(style.margin_bottom, ComputedLength::Px(0.0));
        assert_eq!(style.margin_left, ComputedLength::Px(0.0));
        assert_eq!(style.padding_top, ComputedLength::Px(0.0));
        assert_eq!(style.padding_right, ComputedLength::Px(0.0));
        assert_eq!(style.padding_bottom, ComputedLength::Px(0.0));
        assert_eq!(style.padding_left, ComputedLength::Px(0.0));
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
        parent.margin_top = ComputedLength::Px(20.0);

        let cascaded = empty_cascaded();
        let style = resolve_style(&cascaded, Some(&parent));

        // margin-top is NOT inherited, so child should keep initial (0.0)
        assert_eq!(style.margin_top, ComputedLength::Px(0.0));
    }

    // --- 4. `inherit` keyword forces inheritance ---

    #[test]
    fn test_inherit_keyword_forces_inheritance() {
        let mut parent = ComputedStyle::initial();
        parent.margin_top = ComputedLength::Px(42.0);

        // margin-top is non-inherited; using `inherit` should force it
        let cascaded = cascaded_with(&[("margin-top", "inherit")]);
        let style = resolve_style(&cascaded, Some(&parent));

        assert_eq!(style.margin_top, ComputedLength::Px(42.0));
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
        parent.margin_top = ComputedLength::Px(50.0);

        let cascaded = cascaded_with(&[("margin-top", "unset")]);
        let style = resolve_style(&cascaded, Some(&parent));

        // margin-top is NOT inherited, so `unset` behaves like `initial`
        assert_eq!(style.margin_top, ComputedLength::Px(0.0));
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
        assert_eq!(parse_color("rgb(128, 64, 32)"), ComputedColor::new(128, 64, 32, 1.0));
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
        let cascaded = cascaded_with(&[("font-size", "20px"), ("padding-top", "2em")]);
        let style = resolve_style(&cascaded, None);

        // own font-size is 20px, so 2em = 40px
        assert_eq!(style.padding_top, ComputedLength::Px(40.0));
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
        assert_eq!(style.width, Some(ComputedLength::Px(200.0)));
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
        assert_eq!(style.margin_top, ComputedLength::Px(10.0));
        assert_eq!(style.padding_left, ComputedLength::Px(5.0));
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
        let cascaded = cascaded_with(&[("font-size", "20px"), ("line-height", "1.5")]);
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
