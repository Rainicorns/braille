//! Grid-specific CSS parsing helpers for converting CSS grid properties to taffy types.

use taffy::prelude::*;
use taffy::MinMax;

/// Parse a grid-template-columns/rows value like "1fr 200px 1fr", "repeat(3, 1fr)", "minmax(100px, 1fr)"
pub fn parse_grid_template(val: &str) -> Vec<TrackSizingFunction> {
    let val = val.trim();
    if val.is_empty() || val == "none" || val == "auto" {
        return Vec::new();
    }

    let mut result = Vec::new();
    let mut remaining = val;

    while !remaining.is_empty() {
        remaining = remaining.trim_start();
        if remaining.is_empty() {
            break;
        }

        let lower = remaining.to_ascii_lowercase();

        // repeat(count, tracks...)
        if lower.starts_with("repeat(") {
            if let Some(end) = find_matching_paren(remaining, 7) {
                let inner = &remaining[7..end];
                if let Some(comma) = inner.find(',') {
                    let count_str = inner[..comma].trim();
                    let tracks_str = inner[comma + 1..].trim();
                    let count = if count_str == "auto-fill" || count_str == "auto-fit" {
                        GridTrackRepetition::AutoFill
                    } else {
                        let n: u16 = count_str.parse().unwrap_or(1);
                        GridTrackRepetition::Count(n)
                    };
                    let inner_tracks = parse_grid_template(tracks_str);
                    let non_repeated: Vec<NonRepeatedTrackSizingFunction> = inner_tracks
                        .into_iter()
                        .filter_map(|t| match t {
                            TrackSizingFunction::Single(nr) => Some(nr),
                            _ => None,
                        })
                        .collect();
                    if !non_repeated.is_empty() {
                        result.push(TrackSizingFunction::Repeat(count, non_repeated));
                    }
                }
                remaining = &remaining[end + 1..];
                continue;
            }
        }

        // minmax(min, max)
        if lower.starts_with("minmax(") {
            if let Some(end) = find_matching_paren(remaining, 7) {
                let inner = &remaining[7..end];
                if let Some(comma) = inner.find(',') {
                    let min_str = inner[..comma].trim();
                    let max_str = inner[comma + 1..].trim();
                    let min = parse_min_sizing(min_str);
                    let max = parse_max_sizing(max_str);
                    result.push(TrackSizingFunction::Single(MinMax { min, max }));
                }
                remaining = &remaining[end + 1..];
                continue;
            }
        }

        // Single value token
        let (token, rest) = next_token(remaining);
        if !token.is_empty() {
            let sizing = parse_single_track(token);
            result.push(TrackSizingFunction::Single(sizing));
        }
        remaining = rest;
    }

    result
}

/// Parse a grid placement value like "1", "span 2", "auto", "1 / 3"
pub fn parse_grid_placement(val: &str) -> GridPlacement {
    let val = val.trim();
    if val.is_empty() || val == "auto" {
        return GridPlacement::Auto;
    }

    let lower = val.to_ascii_lowercase();
    if let Some(rest) = lower.strip_prefix("span ") {
        let n: u16 = rest.trim().parse().unwrap_or(1);
        return GridPlacement::Span(n);
    }

    if let Ok(n) = val.parse::<i16>() {
        return GridPlacement::from_line_index(n);
    }

    GridPlacement::Auto
}

/// Parse grid-auto-flow value
pub fn parse_grid_auto_flow(val: &str) -> GridAutoFlow {
    match val.trim().to_ascii_lowercase().as_str() {
        "column" => GridAutoFlow::Column,
        "row dense" | "dense" => GridAutoFlow::RowDense,
        "column dense" => GridAutoFlow::ColumnDense,
        _ => GridAutoFlow::Row,
    }
}

/// Parse a gap value (e.g. "10px", "1em") into a LengthPercentage.
pub fn parse_gap_value(val: &str) -> LengthPercentage {
    let val = val.trim();
    if val == "normal" || val.is_empty() {
        return LengthPercentage::Length(0.0);
    }
    if val.ends_with('%') {
        let pct = val.trim_end_matches('%').trim().parse::<f32>().unwrap_or(0.0);
        LengthPercentage::Percent(pct / 100.0)
    } else {
        LengthPercentage::Length(parse_px_val(val))
    }
}

fn parse_px_val(s: &str) -> f32 {
    s.trim()
        .trim_end_matches("px")
        .trim()
        .parse::<f32>()
        .unwrap_or(0.0)
}

fn parse_min_sizing(val: &str) -> MinTrackSizingFunction {
    let lower = val.trim().to_ascii_lowercase();
    match lower.as_str() {
        "auto" => MinTrackSizingFunction::Auto,
        "min-content" => MinTrackSizingFunction::MinContent,
        "max-content" => MinTrackSizingFunction::MaxContent,
        _ => {
            if lower.ends_with('%') {
                let pct = lower.trim_end_matches('%').trim().parse::<f32>().unwrap_or(0.0);
                MinTrackSizingFunction::Fixed(LengthPercentage::Percent(pct / 100.0))
            } else {
                MinTrackSizingFunction::Fixed(LengthPercentage::Length(parse_px_val(&lower)))
            }
        }
    }
}

fn parse_max_sizing(val: &str) -> MaxTrackSizingFunction {
    let lower = val.trim().to_ascii_lowercase();
    match lower.as_str() {
        "auto" => MaxTrackSizingFunction::Auto,
        "min-content" => MaxTrackSizingFunction::MinContent,
        "max-content" => MaxTrackSizingFunction::MaxContent,
        _ => {
            if lower.ends_with("fr") {
                let fr = lower.trim_end_matches("fr").trim().parse::<f32>().unwrap_or(1.0);
                MaxTrackSizingFunction::Fraction(fr)
            } else if lower.ends_with('%') {
                let pct = lower.trim_end_matches('%').trim().parse::<f32>().unwrap_or(0.0);
                MaxTrackSizingFunction::Fixed(LengthPercentage::Percent(pct / 100.0))
            } else {
                MaxTrackSizingFunction::Fixed(LengthPercentage::Length(parse_px_val(&lower)))
            }
        }
    }
}

fn parse_single_track(val: &str) -> NonRepeatedTrackSizingFunction {
    let lower = val.trim().to_ascii_lowercase();
    match lower.as_str() {
        "auto" => NonRepeatedTrackSizingFunction::AUTO,
        "min-content" => NonRepeatedTrackSizingFunction::MIN_CONTENT,
        "max-content" => NonRepeatedTrackSizingFunction::MAX_CONTENT,
        _ => {
            if lower.ends_with("fr") {
                let fr = lower.trim_end_matches("fr").trim().parse::<f32>().unwrap_or(1.0);
                MinMax {
                    min: MinTrackSizingFunction::Auto,
                    max: MaxTrackSizingFunction::Fraction(fr),
                }
            } else if lower.ends_with('%') {
                let pct = lower.trim_end_matches('%').trim().parse::<f32>().unwrap_or(0.0);
                let lp = LengthPercentage::Percent(pct / 100.0);
                MinMax {
                    min: MinTrackSizingFunction::Fixed(lp),
                    max: MaxTrackSizingFunction::Fixed(lp),
                }
            } else {
                let px = parse_px_val(&lower);
                let lp = LengthPercentage::Length(px);
                MinMax {
                    min: MinTrackSizingFunction::Fixed(lp),
                    max: MaxTrackSizingFunction::Fixed(lp),
                }
            }
        }
    }
}

/// Find the closing paren starting from `start` offset (past the opening paren).
fn find_matching_paren(s: &str, start: usize) -> Option<usize> {
    let mut depth = 1;
    for (i, c) in s[start..].char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(start + i);
                }
            }
            _ => {}
        }
    }
    None
}

/// Extract the next whitespace-delimited token (respecting parens).
fn next_token(s: &str) -> (&str, &str) {
    let s = s.trim_start();
    if s.is_empty() {
        return ("", "");
    }
    let mut depth = 0;
    for (i, c) in s.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            ' ' | '\t' | '\n' if depth == 0 => {
                return (&s[..i], &s[i..]);
            }
            _ => {}
        }
    }
    (s, "")
}

pub fn parse_align_content(val: Option<&String>) -> Option<AlignContent> {
    match val.map(|s| s.as_str()) {
        Some("start") | Some("flex-start") => Some(AlignContent::Start),
        Some("end") | Some("flex-end") => Some(AlignContent::End),
        Some("center") => Some(AlignContent::Center),
        Some("stretch") => Some(AlignContent::Stretch),
        Some("space-between") => Some(AlignContent::SpaceBetween),
        Some("space-around") => Some(AlignContent::SpaceAround),
        Some("space-evenly") => Some(AlignContent::SpaceEvenly),
        _ => None,
    }
}

pub fn parse_align_self_val(val: Option<&String>) -> Option<AlignSelf> {
    match val.map(|s| s.as_str()) {
        Some("start") | Some("flex-start") => Some(AlignSelf::Start),
        Some("end") | Some("flex-end") => Some(AlignSelf::End),
        Some("center") => Some(AlignSelf::Center),
        Some("stretch") => Some(AlignSelf::Stretch),
        Some("baseline") => Some(AlignSelf::Baseline),
        _ => None,
    }
}

pub fn parse_justify_items_val(val: Option<&String>) -> Option<JustifyItems> {
    match val.map(|s| s.as_str()) {
        Some("start") => Some(JustifyItems::Start),
        Some("end") => Some(JustifyItems::End),
        Some("center") => Some(JustifyItems::Center),
        Some("stretch") => Some(JustifyItems::Stretch),
        Some("baseline") => Some(JustifyItems::Baseline),
        _ => None,
    }
}

pub fn parse_justify_self_val(val: Option<&String>) -> Option<JustifySelf> {
    match val.map(|s| s.as_str()) {
        Some("start") => Some(JustifySelf::Start),
        Some("end") => Some(JustifySelf::End),
        Some("center") => Some(JustifySelf::Center),
        Some("stretch") => Some(JustifySelf::Stretch),
        Some("baseline") => Some(JustifySelf::Baseline),
        _ => None,
    }
}
