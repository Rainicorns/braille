//! CSS math functions: calc(), min(), max(), clamp()
//!
//! Recursive descent parser + evaluator for CSS math expressions.
//! Evaluation happens during computed-value resolution, same phase as length unit resolution.

/// A CSS calc expression node.
#[derive(Debug, Clone)]
pub enum CalcExpr {
    Num(f32),
    Length(f32, LengthUnit),
    Percent(f32),
    Add(Box<CalcExpr>, Box<CalcExpr>),
    Sub(Box<CalcExpr>, Box<CalcExpr>),
    Mul(Box<CalcExpr>, Box<CalcExpr>),
    Div(Box<CalcExpr>, Box<CalcExpr>),
    Min(Vec<CalcExpr>),
    Max(Vec<CalcExpr>),
    Clamp(Box<CalcExpr>, Box<CalcExpr>, Box<CalcExpr>),
}

#[derive(Debug, Clone, Copy)]
pub enum LengthUnit {
    Px,
    Em,
    Rem,
    Pt,
    Vh,
    Vw,
}

/// Context needed to resolve relative units during evaluation.
pub struct CalcContext {
    pub font_size: f32,
    pub parent_font_size: f32,
    pub root_font_size: f32,
    pub vw: f32,
    pub vh: f32,
}

/// Check if a CSS value string contains a math function that we should parse.
pub fn is_math_function(val: &str) -> bool {
    let v = val.trim().to_ascii_lowercase();
    v.starts_with("calc(")
        || v.starts_with("min(")
        || v.starts_with("max(")
        || v.starts_with("clamp(")
}

/// Parse and evaluate a CSS math function, returning the result in px.
pub fn eval_math_function(val: &str, ctx: &CalcContext) -> Option<f32> {
    let trimmed = val.trim();
    let expr = parse_math_expr(trimmed)?;
    Some(eval_expr(&expr, ctx))
}

/// Parse a top-level math function: calc(...), min(...), max(...), clamp(...)
fn parse_math_expr(input: &str) -> Option<CalcExpr> {
    let trimmed = input.trim();
    let lower = trimmed.to_ascii_lowercase();

    if let Some(inner) = strip_func(&lower, trimmed, "calc(") {
        parse_sum(inner.trim())
    } else if let Some(inner) = strip_func(&lower, trimmed, "min(") {
        let args = split_args(inner.trim())?;
        let exprs: Option<Vec<CalcExpr>> = args.iter().map(|a| parse_sum(a.trim())).collect();
        Some(CalcExpr::Min(exprs?))
    } else if let Some(inner) = strip_func(&lower, trimmed, "max(") {
        let args = split_args(inner.trim())?;
        let exprs: Option<Vec<CalcExpr>> = args.iter().map(|a| parse_sum(a.trim())).collect();
        Some(CalcExpr::Max(exprs?))
    } else if let Some(inner) = strip_func(&lower, trimmed, "clamp(") {
        let args = split_args(inner.trim())?;
        if args.len() != 3 {
            return None;
        }
        let min = parse_sum(args[0].trim())?;
        let val = parse_sum(args[1].trim())?;
        let max = parse_sum(args[2].trim())?;
        Some(CalcExpr::Clamp(Box::new(min), Box::new(val), Box::new(max)))
    } else {
        // Not a function, try as a bare value
        parse_value(trimmed)
    }
}

/// Strip a function prefix and its matching closing paren.
/// Uses the lowercase version for prefix matching but returns from the original string
/// to preserve case in values like unit suffixes.
fn strip_func<'a>(lower: &str, original: &'a str, prefix: &str) -> Option<&'a str> {
    if lower.starts_with(prefix) {
        let inner = &original[prefix.len()..];
        // Find the matching closing paren
        let mut depth = 1;
        for (i, c) in inner.char_indices() {
            match c {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(&inner[..i]);
                    }
                }
                _ => {}
            }
        }
        // No matching close paren — try using everything
        Some(inner.trim_end_matches(')'))
    } else {
        None
    }
}

/// Split comma-separated arguments, respecting nested parentheses.
fn split_args(input: &str) -> Option<Vec<&str>> {
    let mut args = Vec::new();
    let mut depth = 0;
    let mut start = 0;

    for (i, c) in input.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                args.push(&input[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    args.push(&input[start..]);
    Some(args)
}

/// Parse a sum expression: product (('+' | '-') product)*
fn parse_sum(input: &str) -> Option<CalcExpr> {
    let input = input.trim();
    if input.is_empty() {
        return None;
    }

    let (mut result, mut remaining) = parse_product_segment(input)?;

    loop {
        let rest = remaining.trim();
        if rest.is_empty() {
            break;
        }

        // Look for + or - operator (must be preceded by whitespace which we already trimmed)
        if let Some(r) = rest.strip_prefix('+') {
            let (rhs, rest2) = parse_product_segment(r.trim())?;
            result = CalcExpr::Add(Box::new(result), Box::new(rhs));
            remaining = rest2;
        } else if let Some(r) = rest.strip_prefix('-') {
            let (rhs, rest2) = parse_product_segment(r.trim())?;
            result = CalcExpr::Sub(Box::new(result), Box::new(rhs));
            remaining = rest2;
        } else {
            break;
        }
    }

    Some(result)
}

/// Parse a product expression and return (expr, remaining_input)
fn parse_product_segment(input: &str) -> Option<(CalcExpr, &str)> {
    let input = input.trim();
    let (mut left, mut remaining) = parse_atom(input)?;

    loop {
        let r = remaining.trim();
        if r.is_empty() {
            break;
        }

        if let Some(rest) = r.strip_prefix('*') {
            let (rhs, rest2) = parse_atom(rest.trim())?;
            left = CalcExpr::Mul(Box::new(left), Box::new(rhs));
            remaining = rest2;
        } else if let Some(rest) = r.strip_prefix('/') {
            let (rhs, rest2) = parse_atom(rest.trim())?;
            left = CalcExpr::Div(Box::new(left), Box::new(rhs));
            remaining = rest2;
        } else {
            // Check if next char is + or - (these are sum-level operators)
            break;
        }
    }

    Some((left, remaining))
}

/// Parse an atom: number with unit, nested function, or parenthesized expression
fn parse_atom(input: &str) -> Option<(CalcExpr, &str)> {
    let input = input.trim();

    // Nested function call
    let lower = input.to_ascii_lowercase();
    for func in &["calc(", "min(", "max(", "clamp("] {
        if lower.starts_with(func) {
            // Find matching close paren
            let after_prefix = &input[func.len()..];
            let mut depth = 1;
            for (i, c) in after_prefix.char_indices() {
                match c {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            let full_func = &input[..func.len() + i + 1];
                            let expr = parse_math_expr(full_func)?;
                            return Some((expr, &input[func.len() + i + 1..]));
                        }
                    }
                    _ => {}
                }
            }
            return None;
        }
    }

    // Parenthesized expression
    if let Some(inner) = input.strip_prefix('(') {
        let mut depth = 1;
        for (i, c) in inner.char_indices() {
            match c {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        let expr = parse_sum(&inner[..i])?;
                        return Some((expr, &inner[i + 1..]));
                    }
                }
                _ => {}
            }
        }
        return None;
    }

    // Value: number with optional unit
    parse_value_with_rest(input)
}

/// Parse a value (number + optional unit) and return (expr, remaining_input)
fn parse_value_with_rest(input: &str) -> Option<(CalcExpr, &str)> {
    let input = input.trim();

    // Find the end of the numeric part (including negative sign and decimal)
    let mut end = 0;
    let bytes = input.as_bytes();

    // Optional sign
    if end < bytes.len() && (bytes[end] == b'-' || bytes[end] == b'+') {
        end += 1;
    }

    // Digits and decimal point
    let mut has_digit = false;
    while end < bytes.len() && (bytes[end].is_ascii_digit() || bytes[end] == b'.') {
        if bytes[end].is_ascii_digit() {
            has_digit = true;
        }
        end += 1;
    }

    if !has_digit {
        return None;
    }

    let num_str = &input[..end];
    let num: f32 = num_str.parse().ok()?;
    let after_num = &input[end..];

    // Check for unit suffix
    let lower_rest = after_num.to_ascii_lowercase();
    if lower_rest.starts_with("px") {
        Some((CalcExpr::Length(num, LengthUnit::Px), &after_num[2..]))
    } else if lower_rest.starts_with("rem") {
        Some((CalcExpr::Length(num, LengthUnit::Rem), &after_num[3..]))
    } else if lower_rest.starts_with("em") {
        Some((CalcExpr::Length(num, LengthUnit::Em), &after_num[2..]))
    } else if lower_rest.starts_with("pt") {
        Some((CalcExpr::Length(num, LengthUnit::Pt), &after_num[2..]))
    } else if lower_rest.starts_with("vh") {
        Some((CalcExpr::Length(num, LengthUnit::Vh), &after_num[2..]))
    } else if lower_rest.starts_with("vw") {
        Some((CalcExpr::Length(num, LengthUnit::Vw), &after_num[2..]))
    } else if lower_rest.starts_with('%') {
        Some((CalcExpr::Percent(num), &after_num[1..]))
    } else {
        Some((CalcExpr::Num(num), after_num))
    }
}

/// Parse a standalone value string (no remaining input expected)
fn parse_value(input: &str) -> Option<CalcExpr> {
    let (expr, rest) = parse_value_with_rest(input)?;
    if rest.trim().is_empty() {
        Some(expr)
    } else {
        // Has remaining — try parsing as a full sum expression
        parse_sum(input)
    }
}

/// Evaluate a CalcExpr to a pixel value given the context.
fn eval_expr(expr: &CalcExpr, ctx: &CalcContext) -> f32 {
    match expr {
        CalcExpr::Num(n) => *n,
        CalcExpr::Length(n, unit) => match unit {
            LengthUnit::Px => *n,
            LengthUnit::Em => *n * ctx.font_size,
            LengthUnit::Rem => *n * ctx.root_font_size,
            LengthUnit::Pt => *n * (4.0 / 3.0),
            LengthUnit::Vh => *n / 100.0 * ctx.vh,
            LengthUnit::Vw => *n / 100.0 * ctx.vw,
        },
        CalcExpr::Percent(n) => *n / 100.0 * ctx.parent_font_size,
        CalcExpr::Add(a, b) => eval_expr(a, ctx) + eval_expr(b, ctx),
        CalcExpr::Sub(a, b) => eval_expr(a, ctx) - eval_expr(b, ctx),
        CalcExpr::Mul(a, b) => eval_expr(a, ctx) * eval_expr(b, ctx),
        CalcExpr::Div(a, b) => {
            let divisor = eval_expr(b, ctx);
            if divisor == 0.0 {
                0.0
            } else {
                eval_expr(a, ctx) / divisor
            }
        }
        CalcExpr::Min(args) => args
            .iter()
            .map(|a| eval_expr(a, ctx))
            .fold(f32::INFINITY, f32::min),
        CalcExpr::Max(args) => args
            .iter()
            .map(|a| eval_expr(a, ctx))
            .fold(f32::NEG_INFINITY, f32::max),
        CalcExpr::Clamp(min, val, max) => {
            let min_v = eval_expr(min, ctx);
            let val_v = eval_expr(val, ctx);
            let max_v = eval_expr(max, ctx);
            val_v.clamp(min_v, max_v)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_ctx() -> CalcContext {
        CalcContext {
            font_size: 16.0,
            parent_font_size: 16.0,
            root_font_size: 16.0,
            vw: 1280.0,
            vh: 800.0,
        }
    }

    #[test]
    fn test_calc_simple_px() {
        let ctx = default_ctx();
        let result = eval_math_function("calc(100px + 50px)", &ctx).unwrap();
        assert!((result - 150.0).abs() < 0.01);
    }

    #[test]
    fn test_calc_mixed_units() {
        let ctx = default_ctx();
        let result = eval_math_function("calc(100px + 2em)", &ctx).unwrap();
        assert!((result - 132.0).abs() < 0.01);
    }

    #[test]
    fn test_calc_multiplication() {
        let ctx = default_ctx();
        let result = eval_math_function("calc(10px * 3)", &ctx).unwrap();
        assert!((result - 30.0).abs() < 0.01);
    }

    #[test]
    fn test_calc_division() {
        let ctx = default_ctx();
        let result = eval_math_function("calc(100px / 4)", &ctx).unwrap();
        assert!((result - 25.0).abs() < 0.01);
    }

    #[test]
    fn test_calc_subtraction() {
        let ctx = default_ctx();
        let result = eval_math_function("calc(100px - 30px)", &ctx).unwrap();
        assert!((result - 70.0).abs() < 0.01);
    }

    #[test]
    fn test_min_function() {
        let ctx = default_ctx();
        let result = eval_math_function("min(100px, 200px, 50px)", &ctx).unwrap();
        assert!((result - 50.0).abs() < 0.01);
    }

    #[test]
    fn test_max_function() {
        let ctx = default_ctx();
        let result = eval_math_function("max(100px, 200px, 50px)", &ctx).unwrap();
        assert!((result - 200.0).abs() < 0.01);
    }

    #[test]
    fn test_clamp_function() {
        let ctx = default_ctx();
        // clamp(min, preferred, max) — preferred is within range
        let result = eval_math_function("clamp(10px, 50px, 100px)", &ctx).unwrap();
        assert!((result - 50.0).abs() < 0.01);

        // preferred below min
        let result = eval_math_function("clamp(10px, 5px, 100px)", &ctx).unwrap();
        assert!((result - 10.0).abs() < 0.01);

        // preferred above max
        let result = eval_math_function("clamp(10px, 200px, 100px)", &ctx).unwrap();
        assert!((result - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_calc_with_rem() {
        let ctx = default_ctx();
        let result = eval_math_function("calc(1rem + 10px)", &ctx).unwrap();
        assert!((result - 26.0).abs() < 0.01);
    }

    #[test]
    fn test_calc_viewport_units() {
        let ctx = default_ctx();
        let result = eval_math_function("calc(50vw - 100px)", &ctx).unwrap();
        assert!((result - 540.0).abs() < 0.01); // 640 - 100
    }

    #[test]
    fn test_is_math_function() {
        assert!(is_math_function("calc(100px + 50px)"));
        assert!(is_math_function("min(100px, 50px)"));
        assert!(is_math_function("max(100px, 50px)"));
        assert!(is_math_function("clamp(10px, 50px, 100px)"));
        assert!(is_math_function("  CALC(100px) "));
        assert!(!is_math_function("100px"));
        assert!(!is_math_function("red"));
    }
}
