use std::collections::{HashMap, HashSet};

use braille_wire::{FetchOutcome, FetchRequest, FetchResponseData, FetchResult, SnapMode};

use crate::{check_refresh_header, Engine, FetchedResources, ScriptDescriptor};

/// Trait for providing network fetches to the engine.
/// The binary implements this via IPC; tests implement it with canned responses.
pub trait FetchProvider {
    fn fetch_batch(&mut self, requests: Vec<FetchRequest>) -> Vec<FetchResult>;
}

/// Response from fetching a page (the initial HTML document).
struct PageResponse {
    url: String,
    headers: Vec<(String, String)>,
    body: String,
}

impl Engine {
    /// Navigate to a URL, fetching the page and all scripts, settling the runtime,
    /// following meta-refresh redirects, and returning a snapshot.
    pub fn navigate(
        &mut self,
        url: &str,
        fetcher: &mut impl FetchProvider,
        snap_mode: SnapMode,
    ) -> Result<String, String> {
        self.navigate_inner(url, fetcher, snap_mode, 0)
    }

    fn navigate_inner(
        &mut self,
        url: &str,
        fetcher: &mut impl FetchProvider,
        snap_mode: SnapMode,
        redirect_depth: u32,
    ) -> Result<String, String> {
        if redirect_depth > 5 {
            return Err("too many meta-refresh redirects".into());
        }

        eprintln!("[navigate] depth={redirect_depth} url={}", &url[..url.len().min(120)]);

        // 1. Fetch the page (with cookies)
        let page = self.fetch_page(url, fetcher)?;
        eprintln!("[navigate] fetched url={} headers={}", &page.url[..page.url.len().min(120)], page.headers.len());
        for (k, v) in &page.headers {
            eprintln!("[navigate]   {k}: {}", &v[..v.len().min(200)]);
        }

        // 2. Inject response cookies
        self.inject_response_cookies(&page.url, &page.headers);

        // 3. Parse HTML, collect script descriptors
        let descriptors = self.parse_and_collect_scripts(&page.body);
        eprintln!("[navigate] descriptors={}", descriptors.len());
        for (i, d) in descriptors.iter().enumerate() {
            eprintln!("[navigate]   desc[{i}]={d:?}");
        }
        eprintln!("[navigate] body_len={} body_start={}", page.body.len(), &page.body[..page.body.len().min(500)].replace('\n', "\\n"));

        // 4. Fetch external scripts + import map URLs
        let fetched = self.fetch_scripts(&descriptors, fetcher);
        eprintln!("[navigate] fetched {} scripts", fetched.len());

        // 5. Set URL, execute scripts
        self.set_url(&page.url);
        let errors = self.execute_scripts_lossy(&descriptors, &FetchedResources::scripts_only(fetched));
        if !errors.is_empty() {
            eprintln!("[navigate] JS errors: {}", errors.len());
            for e in &errors {
                eprintln!("[navigate]   {}", &e[..e.len().min(200)]);
            }
        }

        // 6. Interleaved settle + dynamic fetch loop
        self.settle_with_fetches(fetcher);

        // 6b. Full settle with time advancement so rAF-driven frameworks
        //     (Preact, React, etc.) can complete their render/effect chains.
        self.settle();

        // 7. Check if JS set location.href (takes priority over meta-refresh)
        if let Some(nav_url) = self.take_pending_navigation() {
            eprintln!("[navigate] JS location.href redirect to {}", &nav_url[..nav_url.len().min(120)]);
            return self.navigate_inner(&nav_url, fetcher, snap_mode, redirect_depth + 1);
        }

        // 8. Check meta refresh (HTTP header + meta tag)
        let refresh = check_refresh_header(&page.headers, Some(&page.url))
            .or_else(|| self.check_meta_refresh(Some(&page.url)));
        eprintln!("[navigate] refresh={:?}", refresh);
        if let Some(mr) = refresh {
            if let Some(redirect_url) = mr.url {
                eprintln!("[navigate] following redirect to {}", &redirect_url[..redirect_url.len().min(120)]);
                return self.navigate_inner(&redirect_url, fetcher, snap_mode, redirect_depth + 1);
            }
        }

        // 8. Return snapshot
        Ok(self.snapshot(snap_mode))
    }

    /// Fetch a single page URL via the FetchProvider, attaching cookies.
    fn fetch_page(
        &mut self,
        url: &str,
        fetcher: &mut impl FetchProvider,
    ) -> Result<PageResponse, String> {
        let mut page_headers = vec![];
        let cookie_value = self.get_cookies_for_url(url);
        if !cookie_value.is_empty() {
            page_headers.push(("Cookie".into(), cookie_value));
        }
        let request = FetchRequest {
            id: 0,
            url: url.to_string(),
            method: "GET".into(),
            headers: page_headers,
            body: None,
        };
        let results = fetcher.fetch_batch(vec![request]);
        let result = results
            .into_iter()
            .next()
            .ok_or_else(|| "no fetch result received".to_string())?;

        match result.outcome {
            FetchOutcome::Ok(data) => Ok(PageResponse {
                url: data.url,
                headers: data.headers,
                body: data.body,
            }),
            FetchOutcome::Err(e) => Err(format!("fetch failed: {e}")),
        }
    }

    /// Fetch all external scripts and import map URLs referenced by descriptors.
    fn fetch_scripts(
        &mut self,
        descriptors: &[ScriptDescriptor],
        fetcher: &mut impl FetchProvider,
    ) -> HashMap<String, String> {
        let import_map_urls = Self::import_map_urls(descriptors);
        let mut requests: Vec<FetchRequest> = Vec::new();
        let mut next_id = 1u64;

        for desc in descriptors {
            if let Some(src_url) = desc.external_url() {
                requests.push(FetchRequest {
                    id: next_id,
                    url: src_url.to_string(),
                    method: "GET".into(),
                    headers: vec![],
                    body: None,
                });
                next_id += 1;
            }
        }
        for url in &import_map_urls {
            requests.push(FetchRequest {
                id: next_id,
                url: url.clone(),
                method: "GET".into(),
                headers: vec![],
                body: None,
            });
            next_id += 1;
        }

        let mut fetched = HashMap::new();
        if !requests.is_empty() {
            let id_to_url: HashMap<u64, String> =
                requests.iter().map(|r| (r.id, r.url.clone())).collect();
            for r in &requests {
                eprintln!("[navigate] script request id={} url={}", r.id, &r.url[..r.url.len().min(120)]);
            }
            let results = fetcher.fetch_batch(requests);
            for result in &results {
                let url = id_to_url.get(&result.id).map(|s| s.as_str()).unwrap_or("?");
                match &result.outcome {
                    FetchOutcome::Ok(data) => {
                        eprintln!("[navigate] script result id={} url={} body_len={}", result.id, &url[..url.len().min(80)], data.body.len());
                    }
                    FetchOutcome::Err(e) => {
                        eprintln!("[navigate] script FAILED id={} url={} err={}", result.id, &url[..url.len().min(80)], &e[..e.len().min(200)]);
                    }
                }
            }
            for result in results {
                if let (Some(url), FetchOutcome::Ok(data)) =
                    (id_to_url.get(&result.id), &result.outcome)
                {
                    fetched.insert(url.clone(), data.body.clone());
                }
            }
        }
        fetched
    }

    /// Interleave settle + fetch loops until quiescent.
    /// Uses settle_no_advance to avoid firing interval timers repeatedly.
    pub fn settle_with_fetches(&mut self, fetcher: &mut impl FetchProvider) {
        for _ in 0..30 {
            self.settle_no_advance();
            if !self.has_pending_fetches() {
                break;
            }
            self.resolve_pending_fetches_via(fetcher);
        }
        self.settle_no_advance();
    }

    /// Service all pending fetch requests from the JS runtime via the FetchProvider.
    fn resolve_pending_fetches_via(&mut self, fetcher: &mut impl FetchProvider) {
        let mut seen_urls: HashSet<String> = HashSet::new();

        for _ in 0..50 {
            if !self.has_pending_fetches() {
                break;
            }
            let pending = self.pending_fetches();

            let mut batch = Vec::new();
            let mut has_new = false;
            for req in pending {
                let key = format!("{} {}", req.method, req.url);
                let is_new = seen_urls.insert(key);
                if is_new {
                    has_new = true;
                }

                // Attach cookies if no Cookie header set by JS
                let mut headers = req.headers;
                let has_cookie_header =
                    headers.iter().any(|(h, _)| h.eq_ignore_ascii_case("cookie"));
                if !has_cookie_header {
                    let cookie_value = self.get_cookies_for_url(&req.url);
                    if !cookie_value.is_empty() {
                        headers.push(("Cookie".into(), cookie_value));
                    }
                }
                batch.push(FetchRequest {
                    id: req.id,
                    url: req.url,
                    method: req.method,
                    headers,
                    body: req.body,
                });
            }

            let results = fetcher.fetch_batch(batch);
            for result in results {
                match result.outcome {
                    FetchOutcome::Ok(data) => {
                        self.inject_response_cookies(&data.url, &data.headers);
                        self.resolve_fetch(result.id, &data);
                    }
                    FetchOutcome::Err(e) => {
                        self.reject_fetch(result.id, &e);
                    }
                }
            }

            self.settle_no_advance();

            if !has_new {
                break;
            }
        }
    }
}

/// A mock fetch provider for tests. Maps URLs to canned responses.
#[derive(Default)]
pub struct MockFetcher {
    responses: HashMap<String, FetchResponseData>,
}

impl MockFetcher {
    pub fn new() -> Self {
        Self {
            responses: HashMap::new(),
        }
    }

    /// Add a response for a URL.
    pub fn add(&mut self, url: &str, response: FetchResponseData) {
        self.responses.insert(url.to_string(), response);
    }

    /// Convenience: add a simple 200 OK HTML response.
    pub fn add_html(&mut self, url: &str, body: &str) {
        self.add(
            url,
            FetchResponseData {
                status: 200,
                status_text: "OK".into(),
                headers: vec![("content-type".into(), "text/html".into())],
                body: body.into(),
                url: url.to_string(),
            },
        );
    }

    /// Add a response with custom headers (e.g. Refresh header).
    pub fn add_with_headers(
        &mut self,
        url: &str,
        body: &str,
        headers: Vec<(String, String)>,
    ) {
        self.add(
            url,
            FetchResponseData {
                status: 200,
                status_text: "OK".into(),
                headers,
                body: body.into(),
                url: url.to_string(),
            },
        );
    }
}

impl FetchProvider for MockFetcher {
    fn fetch_batch(&mut self, requests: Vec<FetchRequest>) -> Vec<FetchResult> {
        requests
            .into_iter()
            .map(|r| {
                let outcome = match self.responses.get(&r.url) {
                    Some(data) => FetchOutcome::Ok(data.clone()),
                    None => FetchOutcome::Err(format!("MockFetcher: no response for {}", r.url)),
                };
                FetchResult {
                    id: r.id,
                    outcome,
                }
            })
            .collect()
    }
}
