//! CSS @media query evaluation.
//!
//! Parses and evaluates media queries against viewport dimensions and
//! environment settings appropriate for a text browser.

/// Evaluate a media query string against the given viewport dimensions.
///
/// Returns `true` if the media query matches (rules should be included).
pub fn evaluate_media_query(query: &str, viewport_w: f32, viewport_h: f32) -> bool {
    let query = query.trim();
    if query.is_empty() || query == "all" {
        return true;
    }

    // Handle comma-separated media queries (OR logic)
    if query.contains(',') {
        return query.split(',').any(|q| evaluate_single_query(q.trim(), viewport_w, viewport_h));
    }

    evaluate_single_query(query, viewport_w, viewport_h)
}

fn evaluate_single_query(query: &str, viewport_w: f32, viewport_h: f32) -> bool {
    let query = query.trim();

    // Handle "not" prefix
    if let Some(rest) = query.strip_prefix("not ") {
        return !evaluate_single_query(rest.trim(), viewport_w, viewport_h);
    }

    // Handle "only" prefix (same as without it for our purposes)
    let query = query.strip_prefix("only ").unwrap_or(query);

    // Split by "and"
    let conditions: Vec<&str> = query.split(" and ").map(|s| s.trim()).collect();

    for condition in conditions {
        if !evaluate_condition(condition, viewport_w, viewport_h) {
            return false;
        }
    }
    true
}

fn evaluate_condition(condition: &str, viewport_w: f32, viewport_h: f32) -> bool {
    let condition = condition.trim();

    // Media types
    match condition {
        "all" | "screen" => return true,
        "print" | "speech" | "tty" | "tv" | "projection" | "handheld" | "braille" | "embossed" | "aural" => {
            return false;
        }
        _ => {}
    }

    // Parenthesized feature queries
    let inner = if let Some(inner) = condition.strip_prefix('(').and_then(|s| s.strip_suffix(')')) {
        inner.trim()
    } else {
        // If it starts with a media type followed by conditions, the type alone
        if condition == "screen" || condition == "all" {
            return true;
        }
        return true; // Unknown conditions default to true (permissive)
    };

    // Parse feature: value
    if let Some((feature, value)) = inner.split_once(':') {
        let feature = feature.trim().to_ascii_lowercase();
        let value = value.trim();

        match feature.as_str() {
            "min-width" => {
                if let Some(px) = parse_px_value(value) {
                    return viewport_w >= px;
                }
            }
            "max-width" => {
                if let Some(px) = parse_px_value(value) {
                    return viewport_w <= px;
                }
            }
            "min-height" => {
                if let Some(px) = parse_px_value(value) {
                    return viewport_h >= px;
                }
            }
            "max-height" => {
                if let Some(px) = parse_px_value(value) {
                    return viewport_h <= px;
                }
            }
            "prefers-color-scheme" => {
                return value.trim() == "light"; // Text browser defaults to light
            }
            "prefers-reduced-motion" => {
                return value.trim() == "reduce"; // Text browser always prefers reduced motion
            }
            "pointer" => {
                return value.trim() == "none"; // Text browser has no pointer
            }
            "hover" => {
                return value.trim() == "none"; // Text browser can't hover
            }
            "color" => {
                // Text browser has no color display, but we say yes for compatibility
                return true;
            }
            "min-device-width" | "min-device-height" => {
                // Treat like viewport
                if let Some(px) = parse_px_value(value) {
                    return viewport_w >= px;
                }
            }
            "max-device-width" | "max-device-height" => {
                if let Some(px) = parse_px_value(value) {
                    return viewport_w <= px;
                }
            }
            "orientation" => match value.trim() {
                "landscape" => return viewport_w >= viewport_h,
                "portrait" => return viewport_h > viewport_w,
                _ => return true,
            },
            "display-mode" => {
                return value.trim() == "browser";
            }
            _ => {
                // Unknown features default to true (permissive)
                return true;
            }
        }
    } else {
        // Boolean features (no colon)
        let feature = inner.to_ascii_lowercase();
        match feature.as_str() {
            "color" | "grid" => return true,
            "hover" | "pointer" => return false,
            _ => return true,
        }
    }

    true // Default permissive
}

fn parse_px_value(val: &str) -> Option<f32> {
    let val = val.trim().to_ascii_lowercase();
    if let Some(num) = val.strip_suffix("px") {
        num.trim().parse::<f32>().ok()
    } else if let Some(num) = val.strip_suffix("em") {
        // 1em = 16px for media queries
        num.trim().parse::<f32>().ok().map(|n| n * 16.0)
    } else if let Some(num) = val.strip_suffix("rem") {
        num.trim().parse::<f32>().ok().map(|n| n * 16.0)
    } else {
        val.parse::<f32>().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_media_types() {
        assert!(evaluate_media_query("all", 1280.0, 800.0));
        assert!(evaluate_media_query("screen", 1280.0, 800.0));
        assert!(!evaluate_media_query("print", 1280.0, 800.0));
        assert!(evaluate_media_query("", 1280.0, 800.0));
    }

    #[test]
    fn min_width() {
        assert!(evaluate_media_query("(min-width: 768px)", 1280.0, 800.0));
        assert!(!evaluate_media_query("(min-width: 1400px)", 1280.0, 800.0));
    }

    #[test]
    fn max_width() {
        assert!(evaluate_media_query("(max-width: 1400px)", 1280.0, 800.0));
        assert!(!evaluate_media_query("(max-width: 768px)", 1280.0, 800.0));
    }

    #[test]
    fn combined_and() {
        assert!(evaluate_media_query("screen and (min-width: 768px) and (max-width: 1400px)", 1280.0, 800.0));
        assert!(!evaluate_media_query("screen and (min-width: 768px) and (max-width: 1000px)", 1280.0, 800.0));
    }

    #[test]
    fn not_query() {
        assert!(evaluate_media_query("not print", 1280.0, 800.0));
        assert!(!evaluate_media_query("not screen", 1280.0, 800.0));
    }

    #[test]
    fn comma_separated() {
        assert!(evaluate_media_query("print, screen", 1280.0, 800.0));
        assert!(evaluate_media_query("print, (min-width: 768px)", 1280.0, 800.0));
    }

    #[test]
    fn prefers_color_scheme() {
        assert!(evaluate_media_query("(prefers-color-scheme: light)", 1280.0, 800.0));
        assert!(!evaluate_media_query("(prefers-color-scheme: dark)", 1280.0, 800.0));
    }

    #[test]
    fn prefers_reduced_motion() {
        assert!(evaluate_media_query("(prefers-reduced-motion: reduce)", 1280.0, 800.0));
    }
}
