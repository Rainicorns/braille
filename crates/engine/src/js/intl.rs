//! Intl native formatting functions backed by chrono (dates) and manual formatting (numbers).
//! All output uses en-US locale.

use rquickjs::{Ctx, Function};

/// Register native Intl helper functions on the global object.
pub fn register_intl(ctx: &Ctx<'_>) {
    let g = ctx.globals();

    // __n_intlFormatDate(timestamp_ms: f64, options_json: String) -> String
    g.set(
        "__n_intlFormatDate",
        Function::new(ctx.clone(), |ts_ms: rquickjs::Value<'_>, options_json: String| -> String {
            let ms = ts_ms.as_float().or_else(|| ts_ms.as_int().map(|i| i as f64)).unwrap_or(0.0);
            format_date(ms, &options_json)
        })
        .unwrap(),
    )
    .unwrap();

    // __n_intlFormatNumber(value: f64, options_json: String) -> String
    g.set(
        "__n_intlFormatNumber",
        Function::new(ctx.clone(), |value: rquickjs::Value<'_>, options_json: String| -> String {
            let v = value.as_float().or_else(|| value.as_int().map(|i| i as f64)).unwrap_or(0.0);
            format_number(v, &options_json)
        })
        .unwrap(),
    )
    .unwrap();
}

fn format_date(ts_ms: f64, options_json: &str) -> String {
    use chrono::{DateTime, Utc};

    let secs = (ts_ms / 1000.0) as i64;
    let nanos = ((ts_ms % 1000.0) * 1_000_000.0) as u32;
    let dt = match DateTime::from_timestamp(secs, nanos) {
        Some(d) => d.with_timezone(&Utc),
        None => return String::from("Invalid Date"),
    };

    let opts: serde_json::Value = serde_json::from_str(options_json).unwrap_or_default();

    let has_any_date = opts.get("year").is_some()
        || opts.get("month").is_some()
        || opts.get("day").is_some()
        || opts.get("weekday").is_some();
    let has_any_time = opts.get("hour").is_some()
        || opts.get("minute").is_some()
        || opts.get("second").is_some();

    // If no options specified, use default: month/day/year
    if !has_any_date && !has_any_time {
        return dt.format("%-m/%-d/%Y").to_string();
    }

    let mut parts: Vec<String> = Vec::new();

    // Weekday
    if let Some(wd) = opts.get("weekday").and_then(|v| v.as_str()) {
        match wd {
            "long" => parts.push(dt.format("%A").to_string()),
            "short" => parts.push(dt.format("%a").to_string()),
            "narrow" => parts.push(dt.format("%A").to_string().chars().next().unwrap().to_string()),
            _ => {}
        }
    }

    // Date parts
    if has_any_date {
        let month = opts.get("month").and_then(|v| v.as_str()).unwrap_or("");
        let year = opts.get("year").and_then(|v| v.as_str());
        let day = opts.get("day").and_then(|v| v.as_str());

        let month_str = match month {
            "long" => Some(dt.format("%B").to_string()),
            "short" => Some(dt.format("%b").to_string()),
            "narrow" => Some(dt.format("%B").to_string().chars().next().unwrap().to_string()),
            "2-digit" => Some(dt.format("%m").to_string()),
            "numeric" => Some(dt.format("%-m").to_string()),
            _ => None,
        };

        let day_str = match day {
            Some("2-digit") => Some(dt.format("%d").to_string()),
            Some("numeric") => Some(dt.format("%-d").to_string()),
            _ => None,
        };

        let year_str = match year {
            Some("numeric") => Some(dt.format("%Y").to_string()),
            Some("2-digit") => Some(dt.format("%y").to_string()),
            _ => None,
        };

        // Assemble date portion in en-US order
        let mut date_parts: Vec<String> = Vec::new();
        if let Some(m) = month_str {
            date_parts.push(m);
        }
        if let Some(d) = day_str {
            date_parts.push(d);
        }
        if let Some(y) = year_str {
            // If we have month and day, add comma before year
            if date_parts.len() >= 2 {
                let last = date_parts.last_mut().unwrap();
                last.push(',');
            }
            date_parts.push(y);
        }
        if !date_parts.is_empty() {
            parts.push(date_parts.join(" "));
        }
    }

    // Time parts
    if has_any_time {
        let hour12 = opts.get("hour12").and_then(|v| v.as_bool()).unwrap_or(true);
        let has_hour = opts.get("hour").is_some();
        let has_minute = opts.get("minute").is_some();
        let has_second = opts.get("second").is_some();

        if hour12 {
            let mut time_parts: Vec<String> = Vec::new();
            if has_hour {
                time_parts.push(dt.format("%-I").to_string());
            }
            if has_minute {
                time_parts.push(dt.format("%M").to_string());
            }
            if has_second {
                time_parts.push(dt.format("%S").to_string());
            }
            let time_str = time_parts.join(":");
            let ampm = dt.format("%p").to_string();
            parts.push(format!("{} {}", time_str, ampm));
        } else {
            let mut time_parts: Vec<String> = Vec::new();
            if has_hour {
                time_parts.push(dt.format("%H").to_string());
            }
            if has_minute {
                time_parts.push(dt.format("%M").to_string());
            }
            if has_second {
                time_parts.push(dt.format("%S").to_string());
            }
            parts.push(time_parts.join(":"));
        }
    }

    if parts.is_empty() {
        dt.format("%-m/%-d/%Y").to_string()
    } else {
        parts.join(", ")
    }
}

fn format_number(value: f64, options_json: &str) -> String {
    let opts: serde_json::Value = serde_json::from_str(options_json).unwrap_or_default();

    let style = opts.get("style").and_then(|v| v.as_str()).unwrap_or("decimal");

    match style {
        "percent" => {
            let pct = value * 100.0;
            let min_frac = opts
                .get("minimumFractionDigits")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;
            let formatted = format_decimal(pct, min_frac);
            format!("{}%", formatted)
        }
        "currency" => {
            let currency = opts
                .get("currency")
                .and_then(|v| v.as_str())
                .unwrap_or("USD");
            let symbol = match currency.to_uppercase().as_str() {
                "USD" => "$",
                "EUR" => "\u{20ac}",
                "GBP" => "\u{a3}",
                "JPY" => "\u{a5}",
                _ => currency,
            };
            let min_frac = opts
                .get("minimumFractionDigits")
                .and_then(|v| v.as_u64())
                .unwrap_or(2) as usize;
            let formatted = format_decimal(value.abs(), min_frac);
            let with_groups = add_grouping(&formatted);
            if value < 0.0 {
                format!("-{}{}", symbol, with_groups)
            } else {
                format!("{}{}", symbol, with_groups)
            }
        }
        _ => {
            // decimal
            let min_frac = opts
                .get("minimumFractionDigits")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;
            let max_frac = opts
                .get("maximumFractionDigits")
                .and_then(|v| v.as_u64())
                .unwrap_or(3) as usize;
            let use_grouping = opts
                .get("useGrouping")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);

            let formatted = format_decimal_range(value.abs(), min_frac, max_frac);
            let result = if use_grouping {
                add_grouping(&formatted)
            } else {
                formatted
            };
            if value < 0.0 {
                format!("-{}", result)
            } else {
                result
            }
        }
    }
}

/// Format a number with exactly `min_frac` decimal places.
fn format_decimal(value: f64, min_frac: usize) -> String {
    format!("{:.prec$}", value, prec = min_frac)
}

/// Format a number with at least `min_frac` and at most `max_frac` decimal places.
fn format_decimal_range(value: f64, min_frac: usize, max_frac: usize) -> String {
    let s = format!("{:.prec$}", value, prec = max_frac);
    if min_frac == max_frac {
        return s;
    }
    // Trim trailing zeros down to min_frac
    if let Some(dot_pos) = s.find('.') {
        let decimals = &s[dot_pos + 1..];
        let mut keep = decimals.len();
        while keep > min_frac && decimals.as_bytes()[keep - 1] == b'0' {
            keep -= 1;
        }
        if keep == 0 {
            s[..dot_pos].to_string()
        } else {
            format!("{}.{}", &s[..dot_pos], &decimals[..keep])
        }
    } else {
        s
    }
}

/// Add thousands grouping (en-US style commas).
fn add_grouping(s: &str) -> String {
    let (integer_part, decimal_part) = match s.find('.') {
        Some(pos) => (&s[..pos], Some(&s[pos..])),
        None => (s, None),
    };

    let digits: Vec<char> = integer_part.chars().collect();
    let mut result = String::new();
    for (i, ch) in digits.iter().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            result.push(',');
        }
        result.push(*ch);
    }
    if let Some(dec) = decimal_part {
        result.push_str(dec);
    }
    result
}
