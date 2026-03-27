//! HTTP cookie jar for document.cookie ↔ HTTP header synchronization.
//!
//! Bridges JS `document.cookie` with the CLI's HTTP cookie handling:
//! - Server Set-Cookie headers → available in JS via document.cookie
//! - JS document.cookie writes → included in outgoing HTTP requests
//! - HttpOnly cookies hidden from JS but sent in HTTP requests

use crate::js::runtime::JsRuntime;
use crate::Engine;

/// A single cookie stored in the engine's HTTP-level cookie jar.
#[derive(Debug, Clone)]
pub struct StoredCookie {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    pub http_only: bool,
    pub secure: bool,
    /// Expiry as milliseconds since epoch, or None for session cookies.
    pub expires_ms: Option<f64>,
}

impl Engine {
    /// Parse Set-Cookie headers from an HTTP response and store them in the
    /// engine's cookie jar. Non-HttpOnly cookies are also injected into the JS
    /// `document.cookie` jar so scripts can read them.
    pub fn inject_response_cookies(&mut self, page_url: &str, headers: &[(String, String)]) {
        let parsed_url = url::Url::parse(page_url).ok();
        let default_domain = parsed_url
            .as_ref()
            .and_then(|u| u.host_str())
            .unwrap_or("")
            .to_string();
        let default_path = parsed_url
            .as_ref()
            .map(|u| {
                let p = u.path();
                match p.rfind('/') {
                    Some(i) if i > 0 => &p[..i],
                    _ => "/",
                }
            })
            .unwrap_or("/")
            .to_string();

        for (name, value) in headers {
            if name.eq_ignore_ascii_case("set-cookie") {
                if let Some(cookie) = parse_set_cookie(value, &default_domain, &default_path) {
                    // Remove any existing cookie with same name+domain+path
                    self.http_cookie_jar.retain(|c| {
                        !(c.name == cookie.name && c.domain == cookie.domain && c.path == cookie.path)
                    });
                    self.http_cookie_jar.push(cookie);
                }
            }
        }

        self.sync_cookies_to_js();
    }

    /// Sync non-HttpOnly cookies from the Rust jar into the JS `document.cookie`.
    pub(crate) fn sync_cookies_to_js(&mut self) {
        if let Some(runtime) = &mut self.runtime {
            sync_cookies_to_runtime(&self.http_cookie_jar, runtime);
            self.cookies_pending_js_sync = false;
        } else {
            self.cookies_pending_js_sync = true;
        }
    }

    /// Get all cookies (including HttpOnly) that should be sent with an HTTP
    /// request to the given URL. Returns the value for a `Cookie` header.
    pub fn get_cookies_for_url(&mut self, request_url: &str) -> String {
        let parsed = match url::Url::parse(request_url) {
            Ok(u) => u,
            Err(_) => return String::new(),
        };
        let req_domain = parsed.host_str().unwrap_or("");
        let req_path = parsed.path();
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as f64;

        // Also collect cookies that were set via JS
        let js_cookie_string = if let Some(runtime) = &mut self.runtime {
            runtime.eval_to_string("document.cookie").unwrap_or_default()
        } else {
            String::new()
        };

        let mut parts: Vec<String> = Vec::new();
        let mut seen_names: std::collections::HashSet<String> = std::collections::HashSet::new();

        // HTTP jar cookies (including HttpOnly)
        for cookie in &self.http_cookie_jar {
            if let Some(exp) = cookie.expires_ms {
                if exp < now_ms {
                    continue;
                }
            }
            if !domain_matches(req_domain, &cookie.domain) {
                continue;
            }
            if !req_path.starts_with(&cookie.path) {
                continue;
            }

            parts.push(format!("{}={}", cookie.name, cookie.value));
            seen_names.insert(cookie.name.clone());
        }

        // JS-set cookies not already in the HTTP jar
        if !js_cookie_string.is_empty() {
            for pair in js_cookie_string.split("; ") {
                if let Some(eq_pos) = pair.find('=') {
                    let name = &pair[..eq_pos];
                    if !seen_names.contains(name) {
                        parts.push(pair.to_string());
                        seen_names.insert(name.to_string());
                    }
                }
            }
        }

        parts.join("; ")
    }
}

/// Sync non-HttpOnly cookies from the Rust jar into the JS `_cookieJar`.
fn sync_cookies_to_runtime(http_cookie_jar: &[StoredCookie], runtime: &mut JsRuntime) {
    for cookie in http_cookie_jar {
        if cookie.http_only {
            continue;
        }
        let escaped_name = cookie.name.replace('\\', "\\\\").replace('"', "\\\"");
        let escaped_value = cookie.value.replace('\\', "\\\\").replace('"', "\\\"");
        let js = format!("document.cookie = \"{}={}\"", escaped_name, escaped_value);
        let _ = runtime.eval(&js);
    }
}

/// Parse a single Set-Cookie header value into a StoredCookie.
fn parse_set_cookie(header: &str, default_domain: &str, default_path: &str) -> Option<StoredCookie> {
    let parts: Vec<&str> = header.split(';').collect();
    let first = parts.first()?.trim();
    let eq_pos = first.find('=')?;
    let name = first[..eq_pos].trim().to_string();
    let value = first[eq_pos + 1..].trim().to_string();

    if name.is_empty() {
        return None;
    }

    let mut domain = default_domain.to_string();
    let mut path = default_path.to_string();
    let mut http_only = false;
    let mut secure = false;
    let mut expires_ms: Option<f64> = None;

    for part in &parts[1..] {
        let part = part.trim();
        let lower = part.to_ascii_lowercase();

        if lower == "httponly" {
            http_only = true;
        } else if lower == "secure" {
            secure = true;
        } else if lower.starts_with("domain=") {
            domain = part.trim()[7..].trim().trim_start_matches('.').to_string();
        } else if lower.starts_with("path=") {
            path = part.trim()[5..].trim().to_string();
        } else if lower.starts_with("max-age=") {
            if let Ok(secs) = part.trim()[8..].trim().parse::<f64>() {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as f64;
                expires_ms = Some(now + secs * 1000.0);
            }
        } else if lower.starts_with("expires=") {
            let date_str = part.trim()[8..].trim();
            if let Ok(dt) = chrono::DateTime::parse_from_rfc2822(date_str) {
                expires_ms = Some(dt.timestamp_millis() as f64);
            }
        }
    }

    Some(StoredCookie {
        name,
        value,
        domain,
        path,
        http_only,
        secure,
        expires_ms,
    })
}

/// Check if a request domain matches a cookie domain.
fn domain_matches(request_domain: &str, cookie_domain: &str) -> bool {
    if request_domain == cookie_domain {
        return true;
    }
    request_domain.ends_with(&format!(".{}", cookie_domain))
}
