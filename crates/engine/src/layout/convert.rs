use crate::dom::node::Node;
use taffy::prelude::*;

/// Convert a ComputedStyle (via the HashMap on the node) into a taffy::Style.
pub fn to_taffy_style(node: &Node) -> Style {
    let cs = match &node.computed_style {
        Some(cs) => cs,
        None => return Style::default(),
    };

    let display = match cs.get("display").map(|s| s.as_str()) {
        Some("none") => taffy::Display::None,
        Some("flex") => taffy::Display::Flex,
        Some("grid") => taffy::Display::Grid,
        // Taffy has no inline flow — approximate everything else as block
        _ => taffy::Display::Block,
    };

    let position = match cs.get("position").map(|s| s.as_str()) {
        Some("absolute") | Some("fixed") => taffy::Position::Absolute,
        _ => taffy::Position::Relative,
    };

    let mut style = Style {
        display,
        position,
        margin: Rect {
            top: parse_length_auto(cs.get("margin-top")),
            right: parse_length_auto(cs.get("margin-right")),
            bottom: parse_length_auto(cs.get("margin-bottom")),
            left: parse_length_auto(cs.get("margin-left")),
        },
        padding: Rect {
            top: parse_length_dimension(cs.get("padding-top")),
            right: parse_length_dimension(cs.get("padding-right")),
            bottom: parse_length_dimension(cs.get("padding-bottom")),
            left: parse_length_dimension(cs.get("padding-left")),
        },
        size: Size {
            width: parse_dimension(cs.get("width")),
            height: parse_dimension(cs.get("height")),
        },
        min_size: Size {
            width: parse_dimension(cs.get("min-width")),
            height: parse_dimension(cs.get("min-height")),
        },
        max_size: Size {
            width: parse_dimension_none(cs.get("max-width")),
            height: parse_dimension_none(cs.get("max-height")),
        },
        overflow: taffy::Point {
            x: parse_taffy_overflow(cs.get("overflow-x")),
            y: parse_taffy_overflow(cs.get("overflow-y")),
        },
        ..Style::default()
    };

    // Inset properties for positioned elements (left, right, top, bottom)
    style.inset = Rect {
        left: parse_length_auto(cs.get("left")),
        right: parse_length_auto(cs.get("right")),
        top: parse_length_auto(cs.get("top")),
        bottom: parse_length_auto(cs.get("bottom")),
    };

    // Flex/grid properties from the raw computed style map
    if display == taffy::Display::Flex {
        style.flex_direction = match cs.get("flex-direction").map(|s| s.as_str()) {
            Some("row-reverse") => FlexDirection::RowReverse,
            Some("column") => FlexDirection::Column,
            Some("column-reverse") => FlexDirection::ColumnReverse,
            _ => FlexDirection::Row,
        };
        style.flex_wrap = match cs.get("flex-wrap").map(|s| s.as_str()) {
            Some("wrap") => FlexWrap::Wrap,
            Some("wrap-reverse") => FlexWrap::WrapReverse,
            _ => FlexWrap::NoWrap,
        };
        style.justify_content = parse_justify_content(cs.get("justify-content"));
        style.align_items = parse_align_items(cs.get("align-items"));
        style.align_content = parse_align_content(cs.get("align-content"));
        if let Some(gap_val) = cs.get("gap") {
            let g = parse_px_value(gap_val);
            style.gap = Size {
                width: LengthPercentage::Length(g),
                height: LengthPercentage::Length(g),
            };
        }
    }

    // flex-grow/shrink/basis on child items
    if let Some(v) = cs.get("flex-grow") {
        style.flex_grow = v.parse::<f32>().unwrap_or(0.0);
    }
    if let Some(v) = cs.get("flex-shrink") {
        style.flex_shrink = v.parse::<f32>().unwrap_or(1.0);
    }
    style.flex_basis = parse_dimension(cs.get("flex-basis"));

    style
}

/// Measure text for a leaf text node. Monospace approximation:
/// char_width = font_size * 0.6, height = lines * line_height.
pub fn measure_text(text: &str, font_size: f32, line_height: f32, available_width: f32) -> (f32, f32) {
    if text.is_empty() || text.trim().is_empty() {
        return (0.0, 0.0);
    }
    let char_width = font_size * 0.6;
    let text_width = text.len() as f32 * char_width;

    if available_width <= 0.0 || text_width <= available_width {
        (text_width, line_height)
    } else {
        let chars_per_line = (available_width / char_width).floor().max(1.0);
        let lines = (text.len() as f32 / chars_per_line).ceil();
        (available_width, lines * line_height)
    }
}

fn parse_px_value(s: &str) -> f32 {
    s.trim()
        .trim_end_matches("px")
        .trim()
        .parse::<f32>()
        .unwrap_or(0.0)
}

fn parse_length_auto(val: Option<&String>) -> LengthPercentageAuto {
    match val.map(|s| s.as_str()) {
        Some("auto") | None => LengthPercentageAuto::Auto,
        Some(s) if s.ends_with('%') => {
            let pct = s.trim_end_matches('%').trim().parse::<f32>().unwrap_or(0.0);
            LengthPercentageAuto::Percent(pct / 100.0)
        }
        Some(s) => LengthPercentageAuto::Length(parse_px_value(s)),
    }
}

fn parse_length_dimension(val: Option<&String>) -> LengthPercentage {
    match val {
        None => LengthPercentage::Length(0.0),
        Some(s) if s.ends_with('%') => {
            let pct = s.trim_end_matches('%').trim().parse::<f32>().unwrap_or(0.0);
            LengthPercentage::Percent(pct / 100.0)
        }
        Some(s) => LengthPercentage::Length(parse_px_value(s)),
    }
}

fn parse_dimension(val: Option<&String>) -> Dimension {
    match val.map(|s| s.as_str()) {
        Some("auto") | None => Dimension::Auto,
        Some(s) if s.ends_with('%') => {
            let pct = s.trim_end_matches('%').trim().parse::<f32>().unwrap_or(0.0);
            Dimension::Percent(pct / 100.0)
        }
        Some(s) => {
            let v = parse_px_value(s);
            if v == 0.0 && !s.starts_with('0') {
                Dimension::Auto
            } else {
                Dimension::Length(v)
            }
        }
    }
}

fn parse_justify_content(val: Option<&String>) -> Option<JustifyContent> {
    match val.map(|s| s.as_str()) {
        Some("flex-start") | Some("start") => Some(JustifyContent::Start),
        Some("flex-end") | Some("end") => Some(JustifyContent::End),
        Some("center") => Some(JustifyContent::Center),
        Some("space-between") => Some(JustifyContent::SpaceBetween),
        Some("space-around") => Some(JustifyContent::SpaceAround),
        Some("space-evenly") => Some(JustifyContent::SpaceEvenly),
        _ => None,
    }
}

fn parse_align_items(val: Option<&String>) -> Option<AlignItems> {
    match val.map(|s| s.as_str()) {
        Some("flex-start") | Some("start") => Some(AlignItems::Start),
        Some("flex-end") | Some("end") => Some(AlignItems::End),
        Some("center") => Some(AlignItems::Center),
        Some("stretch") => Some(AlignItems::Stretch),
        Some("baseline") => Some(AlignItems::Baseline),
        _ => None,
    }
}

fn parse_align_content(val: Option<&String>) -> Option<AlignContent> {
    match val.map(|s| s.as_str()) {
        Some("flex-start") | Some("start") => Some(AlignContent::Start),
        Some("flex-end") | Some("end") => Some(AlignContent::End),
        Some("center") => Some(AlignContent::Center),
        Some("stretch") => Some(AlignContent::Stretch),
        Some("space-between") => Some(AlignContent::SpaceBetween),
        Some("space-around") => Some(AlignContent::SpaceAround),
        Some("space-evenly") => Some(AlignContent::SpaceEvenly),
        _ => None,
    }
}

/// Parse a dimension where "none" means Auto (for max-width/max-height).
fn parse_dimension_none(val: Option<&String>) -> Dimension {
    match val.map(|s| s.as_str()) {
        Some("none") | None => Dimension::Auto,
        Some("auto") => Dimension::Auto,
        Some(s) if s.ends_with('%') => {
            let pct = s.trim_end_matches('%').trim().parse::<f32>().unwrap_or(0.0);
            Dimension::Percent(pct / 100.0)
        }
        Some(s) => {
            let v = parse_px_value(s);
            Dimension::Length(v)
        }
    }
}

fn parse_taffy_overflow(val: Option<&String>) -> taffy::Overflow {
    match val.map(|s| s.as_str()) {
        Some("hidden") => taffy::Overflow::Hidden,
        Some("scroll") => taffy::Overflow::Scroll,
        Some("auto") => taffy::Overflow::Scroll,
        Some("clip") => taffy::Overflow::Clip,
        _ => taffy::Overflow::Visible,
    }
}
