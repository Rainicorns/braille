use reqwest::blocking::Client;

/// Response from a network fetch.
pub struct FetchResponse {
    pub body: String,
    pub url: String,
    pub status: u16,
    pub content_type: Option<String>,
}

/// HTTP client with per-session cookie jar, redirect following, and URL resolution.
pub struct NetworkClient {
    client: Client,
    base_url: Option<String>,
}

impl Default for NetworkClient {
    fn default() -> Self {
        Self::new()
    }
}

impl NetworkClient {
    /// Create a new NetworkClient. Redirects and cookies are handled manually
    /// so that Set-Cookie headers from intermediate 3xx responses are not lost.
    pub fn new() -> Self {
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .user_agent("Mozilla/5.0 (compatible; Braille/0.1)")
            .build()
            .expect("failed to build reqwest client");
        NetworkClient { client, base_url: None }
    }

    /// Get a clone of the underlying reqwest client (for parallel fetching).
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Fetch a URL. Follows redirects manually to preserve Set-Cookie headers.
    /// Updates base_url to the final URL after redirects.
    pub fn fetch(&mut self, url: &str) -> Result<FetchResponse, String> {
        let resolved = self.resolve_url(url);
        let mut current_url = url::Url::parse(&resolved)
            .map_err(|e| format!("invalid URL {resolved}: {e}"))?;

        for _ in 0..10 {
            let response = self
                .client
                .get(current_url.as_str())
                .send()
                .map_err(|e| format!("fetch failed: {e}"))?;

            let status = response.status().as_u16();
            if (300..400).contains(&status) {
                if let Some(location) = response.headers().get("location") {
                    let loc = location.to_str().map_err(|e| format!("invalid Location header: {e}"))?;
                    let mut next = current_url.join(loc)
                        .map_err(|e| format!("invalid redirect URL {loc}: {e}"))?;
                    if current_url.scheme() == "https" && next.scheme() == "http" {
                        let _ = next.set_scheme("https");
                    }
                    current_url = next;
                    continue;
                }
            }

            let final_url = current_url.to_string();
            let content_type = response
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());

            self.set_base_url(&final_url);

            let body = response.text().map_err(|e| format!("failed to read body: {e}"))?;

            return Ok(FetchResponse {
                body,
                url: final_url,
                status,
                content_type,
            });
        }

        Err(format!("too many redirects for {resolved}"))
    }

    /// Resolve a possibly-relative URL against the current base_url.
    ///
    /// Rules:
    /// - Starts with "http://" or "https://" -> absolute, use as-is
    /// - Starts with "//" -> protocol-relative, prepend scheme from base_url
    /// - Starts with "/" -> root-relative, prepend scheme+host from base_url
    /// - Starts with "#" -> fragment, append to base_url (stripped of existing fragment)
    /// - Otherwise -> relative path, join with base_url's directory
    pub fn resolve_url(&self, url: &str) -> String {
        // Absolute URL -- use as-is
        if url.starts_with("http://") || url.starts_with("https://") {
            return url.to_string();
        }

        let base = match &self.base_url {
            Some(b) => b.as_str(),
            None => return url.to_string(),
        };

        // Protocol-relative: //example.com/path
        if url.starts_with("//") {
            let scheme = extract_scheme(base);
            return format!("{scheme}:{url}");
        }

        // Root-relative: /path/to/thing
        if url.starts_with('/') {
            let origin = extract_origin(base);
            return format!("{origin}{url}");
        }

        // Fragment: #section
        if url.starts_with('#') {
            let base_no_fragment = strip_fragment(base);
            return format!("{base_no_fragment}{url}");
        }

        // Relative path: join with base directory
        let dir = extract_directory(base);
        format!("{dir}{url}")
    }

    /// Fetch a URL with custom method, headers, and body.
    /// Follows redirects manually. 301/302/303 switch to GET and drop body.
    pub fn fetch_with_options(
        &mut self,
        url: &str,
        method: &str,
        headers: &[(String, String)],
        body: Option<&str>,
    ) -> Result<FetchResponse, String> {
        let resolved = self.resolve_url(url);
        let mut current_url = url::Url::parse(&resolved)
            .map_err(|e| format!("invalid URL {resolved}: {e}"))?;
        let mut current_method = method.to_uppercase();
        let mut current_body: Option<String> = body.map(|s| s.to_string());

        for _ in 0..10 {
            let mut builder = match current_method.as_str() {
                "POST" => self.client.post(current_url.as_str()),
                "PUT" => self.client.put(current_url.as_str()),
                "DELETE" => self.client.delete(current_url.as_str()),
                "PATCH" => self.client.patch(current_url.as_str()),
                "HEAD" => self.client.head(current_url.as_str()),
                _ => self.client.get(current_url.as_str()),
            };

            for (name, value) in headers {
                builder = builder.header(name.as_str(), value.as_str());
            }

            if let Some(ref body_str) = current_body {
                builder = builder.body(body_str.clone());
            }

            let response = builder.send().map_err(|e| format!("fetch failed: {e}"))?;

            let status = response.status().as_u16();
            if (300..400).contains(&status) {
                if let Some(location) = response.headers().get("location") {
                    let loc = location.to_str().map_err(|e| format!("invalid Location header: {e}"))?;
                    let mut next = current_url.join(loc)
                        .map_err(|e| format!("invalid redirect URL {loc}: {e}"))?;
                    if current_url.scheme() == "https" && next.scheme() == "http" {
                        let _ = next.set_scheme("https");
                    }
                    current_url = next;
                    if (301..=303).contains(&status) {
                        current_method = "GET".to_string();
                        current_body = None;
                    }
                    continue;
                }
            }

            let final_url = current_url.to_string();
            let content_type = response
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());

            let body = response.text().map_err(|e| format!("failed to read body: {e}"))?;

            return Ok(FetchResponse {
                body,
                url: final_url,
                status,
                content_type,
            });
        }

        Err(format!("too many redirects for {resolved}"))
    }

    /// Update the base URL (typically after navigation).
    pub fn set_base_url(&mut self, url: &str) {
        self.base_url = Some(url.to_string());
    }
}

/// Extract the scheme (e.g. "https") from a URL.
fn extract_scheme(url: &str) -> &str {
    match url.find("://") {
        Some(i) => &url[..i],
        None => "https",
    }
}

/// Extract the origin (scheme + host + port) from a URL.
/// e.g. "https://example.com:8080/path" -> "https://example.com:8080"
fn extract_origin(url: &str) -> &str {
    match url.find("://") {
        Some(i) => {
            let after_scheme = &url[i + 3..];
            match after_scheme.find('/') {
                Some(j) => &url[..i + 3 + j],
                None => url,
            }
        }
        None => url,
    }
}

/// Strip the fragment (everything from '#' onward) from a URL.
fn strip_fragment(url: &str) -> &str {
    match url.find('#') {
        Some(i) => &url[..i],
        None => url,
    }
}

/// Extract the "directory" part of a URL (everything up to and including the last '/').
/// e.g. "https://example.com/a/b/page.html" -> "https://example.com/a/b/"
/// e.g. "https://example.com/a/b/" -> "https://example.com/a/b/"
/// e.g. "https://example.com" -> "https://example.com/"
fn extract_directory(url: &str) -> String {
    let after_scheme = match url.find("://") {
        Some(i) => i + 3,
        None => 0,
    };
    let path_start = match url[after_scheme..].find('/') {
        Some(j) => after_scheme + j,
        None => return format!("{url}/"),
    };
    match url.rfind('/') {
        Some(i) if i >= path_start => format!("{}/", &url[..i]),
        _ => format!("{url}/"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- URL resolution: absolute URLs --

    #[test]
    fn resolve_absolute_http() {
        let client = NetworkClient::new();
        assert_eq!(client.resolve_url("http://example.com/page"), "http://example.com/page");
    }

    #[test]
    fn resolve_absolute_https() {
        let client = NetworkClient::new();
        assert_eq!(
            client.resolve_url("https://example.com/page"),
            "https://example.com/page"
        );
    }

    #[test]
    fn resolve_absolute_ignores_base_url() {
        let mut client = NetworkClient::new();
        client.set_base_url("https://base.com/old");
        assert_eq!(client.resolve_url("https://other.com/new"), "https://other.com/new");
    }

    // -- URL resolution: protocol-relative --

    #[test]
    fn resolve_protocol_relative_https() {
        let mut client = NetworkClient::new();
        client.set_base_url("https://example.com/page");
        assert_eq!(
            client.resolve_url("//cdn.example.com/script.js"),
            "https://cdn.example.com/script.js"
        );
    }

    #[test]
    fn resolve_protocol_relative_http() {
        let mut client = NetworkClient::new();
        client.set_base_url("http://example.com/page");
        assert_eq!(
            client.resolve_url("//cdn.example.com/script.js"),
            "http://cdn.example.com/script.js"
        );
    }

    // -- URL resolution: root-relative --

    #[test]
    fn resolve_root_relative() {
        let mut client = NetworkClient::new();
        client.set_base_url("https://example.com/a/b/c");
        assert_eq!(client.resolve_url("/other/path"), "https://example.com/other/path");
    }

    #[test]
    fn resolve_root_relative_with_port() {
        let mut client = NetworkClient::new();
        client.set_base_url("https://example.com:8080/a/b/c");
        assert_eq!(client.resolve_url("/other/path"), "https://example.com:8080/other/path");
    }

    // -- URL resolution: fragment --

    #[test]
    fn resolve_fragment() {
        let mut client = NetworkClient::new();
        client.set_base_url("https://example.com/page");
        assert_eq!(client.resolve_url("#section"), "https://example.com/page#section");
    }

    #[test]
    fn resolve_fragment_replaces_existing() {
        let mut client = NetworkClient::new();
        client.set_base_url("https://example.com/page#old");
        assert_eq!(client.resolve_url("#new"), "https://example.com/page#new");
    }

    // -- URL resolution: relative path --

    #[test]
    fn resolve_relative_path() {
        let mut client = NetworkClient::new();
        client.set_base_url("https://example.com/a/b/page.html");
        assert_eq!(client.resolve_url("other.html"), "https://example.com/a/b/other.html");
    }

    #[test]
    fn resolve_relative_path_from_directory() {
        let mut client = NetworkClient::new();
        client.set_base_url("https://example.com/a/b/");
        assert_eq!(client.resolve_url("other.html"), "https://example.com/a/b/other.html");
    }

    #[test]
    fn resolve_relative_no_base_url() {
        let client = NetworkClient::new();
        assert_eq!(client.resolve_url("page.html"), "page.html");
    }

    #[test]
    fn resolve_relative_base_no_path() {
        let mut client = NetworkClient::new();
        client.set_base_url("https://example.com");
        assert_eq!(client.resolve_url("page.html"), "https://example.com/page.html");
    }

    // -- base_url management --

    #[test]
    fn set_base_url_updates_state() {
        let mut client = NetworkClient::new();
        assert!(client.base_url.is_none());

        client.set_base_url("https://example.com/first");
        assert_eq!(client.base_url, Some("https://example.com/first".to_string()));

        client.set_base_url("https://example.com/second");
        assert_eq!(client.base_url, Some("https://example.com/second".to_string()));
    }

    // -- Helper function tests --

    #[test]
    fn test_extract_scheme() {
        assert_eq!(extract_scheme("https://example.com"), "https");
        assert_eq!(extract_scheme("http://example.com"), "http");
        assert_eq!(extract_scheme("noscheme"), "https");
    }

    #[test]
    fn test_extract_origin() {
        assert_eq!(extract_origin("https://example.com/path"), "https://example.com");
        assert_eq!(
            extract_origin("https://example.com:8080/path"),
            "https://example.com:8080"
        );
        assert_eq!(extract_origin("https://example.com"), "https://example.com");
    }

    #[test]
    fn test_strip_fragment() {
        assert_eq!(
            strip_fragment("https://example.com/page#frag"),
            "https://example.com/page"
        );
        assert_eq!(strip_fragment("https://example.com/page"), "https://example.com/page");
    }

    #[test]
    fn test_extract_directory() {
        assert_eq!(
            extract_directory("https://example.com/a/b/page.html"),
            "https://example.com/a/b/"
        );
        assert_eq!(
            extract_directory("https://example.com/a/b/"),
            "https://example.com/a/b/"
        );
        assert_eq!(extract_directory("https://example.com"), "https://example.com/");
    }

    // -- FetchResponse construction --

    #[test]
    fn fetch_response_construction() {
        let resp = FetchResponse {
            body: "<html></html>".to_string(),
            url: "https://example.com".to_string(),
            status: 200,
            content_type: Some("text/html; charset=utf-8".to_string()),
        };
        assert_eq!(resp.body, "<html></html>");
        assert_eq!(resp.url, "https://example.com");
        assert_eq!(resp.status, 200);
        assert_eq!(resp.content_type, Some("text/html; charset=utf-8".to_string()));
    }

    #[test]
    fn fetch_response_no_content_type() {
        let resp = FetchResponse {
            body: "data".to_string(),
            url: "https://example.com/api".to_string(),
            status: 404,
            content_type: None,
        };
        assert_eq!(resp.status, 404);
        assert!(resp.content_type.is_none());
    }
}
