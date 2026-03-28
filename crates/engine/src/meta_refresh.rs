use crate::dom::node::NodeData;

use super::Engine;

/// Parsed result of a `<meta http-equiv="refresh">` tag.
#[derive(Debug, Clone, PartialEq)]
pub struct MetaRefresh {
    /// Delay in seconds before the redirect/refresh.
    pub delay_seconds: u32,
    /// The target URL, or None if the page should refresh itself.
    pub url: Option<String>,
}

impl Engine {
    /// Check the DOM for `<meta http-equiv="refresh">` tags and return the
    /// parsed redirect information if one is found.
    ///
    /// The `content` attribute format is either:
    /// - `SECONDS; url=URL` — redirect to URL after SECONDS
    /// - `SECONDS` — refresh the current page after SECONDS
    ///
    /// If a URL is present and relative, it is resolved against `base_url`.
    /// If `base_url` is None, relative URLs are returned as-is.
    pub fn check_meta_refresh(&self, base_url: Option<&str>) -> Option<MetaRefresh> {
        let tree = self.tree.borrow();
        let metas = tree.get_elements_by_tag_name("meta");
        for meta_id in metas {
            let node = tree.get_node(meta_id);
            if let NodeData::Element { attributes, .. } = &node.data {
                let is_refresh = attributes.iter().any(|a| {
                    a.local_name.eq_ignore_ascii_case("http-equiv")
                        && a.value.eq_ignore_ascii_case("refresh")
                });
                if !is_refresh {
                    continue;
                }
                let content = attributes
                    .iter()
                    .find(|a| a.local_name.eq_ignore_ascii_case("content"))
                    .map(|a| a.value.as_str());
                if let Some(content) = content {
                    return Some(parse_meta_refresh_content(content, base_url));
                }
            }
        }
        None
    }
}

/// Check HTTP response headers for a `Refresh` header.
/// This is the HTTP header equivalent of `<meta http-equiv="refresh">`.
/// Anubis sends this when `randomData[0] % 2 != 0`.
pub fn check_refresh_header(headers: &[(String, String)], base_url: Option<&str>) -> Option<MetaRefresh> {
    for (name, value) in headers {
        if name.eq_ignore_ascii_case("refresh") {
            return Some(parse_meta_refresh_content(value.trim(), base_url));
        }
    }
    None
}

/// Parse the `content` attribute value of a meta refresh tag.
///
/// Handles formats like:
/// - `"5"` — refresh same page after 5 seconds
/// - `"2; url=/path"` — redirect to /path after 2 seconds
/// - `"0;url=https://example.com"` — immediate redirect (space around ; is optional)
/// - `"2; URL=/path"` — case-insensitive "url=" prefix
pub(crate) fn parse_meta_refresh_content(content: &str, base_url: Option<&str>) -> MetaRefresh {
    let content = content.trim();

    // Split on ';' or ',' (both are valid separators per the spec)
    let (delay_str, rest) = match content.find([';', ',']) {
        Some(pos) => (&content[..pos], Some(content[pos + 1..].trim())),
        None => (content, None),
    };

    let delay_seconds = delay_str.trim().parse::<u32>().unwrap_or(0);

    let url = rest.and_then(|rest| {
        // Strip optional "url=" prefix (case-insensitive)
        let rest_lower = rest.to_ascii_lowercase();
        let url_str = if rest_lower.starts_with("url=") {
            rest[4..].trim()
        } else if rest_lower.starts_with("url =") {
            rest[5..].trim()
        } else {
            // No url= prefix but there's content after the semicolon — treat as URL anyway
            rest
        };

        // Strip surrounding quotes if present
        let url_str = url_str
            .strip_prefix('\'')
            .and_then(|s| s.strip_suffix('\''))
            .or_else(|| url_str.strip_prefix('"').and_then(|s| s.strip_suffix('"')))
            .unwrap_or(url_str);

        if url_str.is_empty() {
            return None;
        }

        // Resolve relative URLs against base_url
        if let Some(base) = base_url {
            if let Ok(base_parsed) = url::Url::parse(base) {
                if let Ok(resolved) = base_parsed.join(url_str) {
                    return Some(resolved.to_string());
                }
            }
        }

        Some(url_str.to_string())
    });

    MetaRefresh {
        delay_seconds,
        url,
    }
}
