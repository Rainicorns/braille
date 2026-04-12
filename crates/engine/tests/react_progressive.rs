//! React 18 progressive test suite.
//!
//! Builds and serves a real React 18 app via `vite preview`, navigates to it
//! using the same fetch path as the CLI, and verifies that React's rendering,
//! event delegation, and state updates work end-to-end.

use std::process::{Child, Command, Stdio};
use std::time::Duration;

use braille_engine::navigation::FetchProvider;
use braille_engine::Engine;
use braille_wire::{
    FetchOutcome, FetchRequest, FetchResponseData, FetchResult, SnapMode,
};

const REACT_APP_DIR: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/react-counter");
const PREVIEW_PORT: u16 = 4173;

/// A real HTTP fetch provider using reqwest — same path as the CLI.
struct HttpFetcher {
    client: reqwest::blocking::Client,
    base_url: String,
}

impl HttpFetcher {
    fn new(base_url: &str) -> Self {
        Self {
            client: reqwest::blocking::ClientBuilder::new()
                .redirect(reqwest::redirect::Policy::limited(10))
                .timeout(Duration::from_secs(10))
                .build()
                .expect("failed to build reqwest client"),
            base_url: base_url.to_string(),
        }
    }

    /// Resolve a possibly-relative URL against the base, same as the CLI does.
    fn resolve_url(&self, raw: &str) -> String {
        if raw.starts_with("http://") || raw.starts_with("https://") {
            return raw.to_string();
        }
        if raw.starts_with('/') {
            if let Ok(parsed) = url::Url::parse(&self.base_url) {
                let host = parsed.host_str().unwrap_or("localhost");
                let origin = match parsed.port() {
                    Some(port) => format!("{}://{}:{}", parsed.scheme(), host, port),
                    None => format!("{}://{}", parsed.scheme(), host),
                };
                return format!("{origin}{raw}");
            }
        }
        if let Ok(base) = url::Url::parse(&self.base_url) {
            if let Ok(resolved) = base.join(raw) {
                return resolved.to_string();
            }
        }
        raw.to_string()
    }
}

impl FetchProvider for HttpFetcher {
    fn fetch_batch(&mut self, requests: Vec<FetchRequest>) -> Vec<FetchResult> {
        requests
            .into_iter()
            .map(|req| {
                let resolved = self.resolve_url(&req.url);
                eprintln!("[http-fetcher] {} -> {}", req.url, resolved);
                let outcome = match self.client.get(&resolved).send() {
                    Ok(resp) => {
                        let status = resp.status().as_u16();
                        let status_text =
                            resp.status().canonical_reason().unwrap_or("OK").to_string();
                        let headers: Vec<(String, String)> = resp
                            .headers()
                            .iter()
                            .filter_map(|(name, value)| {
                                value.to_str().ok().map(|v| (name.to_string(), v.to_string()))
                            })
                            .collect();
                        let final_url = resp.url().to_string();
                        let body = resp.text().unwrap_or_default();
                        FetchOutcome::Ok(FetchResponseData {
                            status,
                            status_text,
                            headers,
                            body,
                            url: final_url,
                            redirect_chain: vec![],
                        })
                    }
                    Err(e) => FetchOutcome::Err(format!("fetch failed: {e}")),
                };
                FetchResult {
                    id: req.id,
                    outcome,
                }
            })
            .collect()
    }
}

/// Build the React app and start `vite preview`. Returns the child process.
fn start_preview_server() -> Child {
    let build_status = Command::new("npm")
        .args(["run", "build"])
        .current_dir(REACT_APP_DIR)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .status()
        .expect("failed to run npm run build");
    assert!(build_status.success(), "npm run build failed");

    let mut child = Command::new("npm")
        .args(["run", "preview", "--", "--port", &PREVIEW_PORT.to_string()])
        .current_dir(REACT_APP_DIR)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to start vite preview");

    let client = reqwest::blocking::Client::new();
    let check_url = format!("http://localhost:{PREVIEW_PORT}");
    for _ in 0..50 {
        if client.get(&check_url).send().is_ok() {
            return child;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    child.wait().ok();
    panic!("vite preview did not start within 5 seconds");
}

#[test]
fn react_real_app() {
    let mut server = start_preview_server();
    let base_url = format!("http://localhost:{PREVIEW_PORT}");

    // --- Test 1: Initial render ---
    eprintln!("\n=== TEST: Initial Render ===");
    {
        let mut engine = Engine::new();
        let mut fetcher = HttpFetcher::new(&base_url);
        let snap = engine
            .navigate(&base_url, &mut fetcher, SnapMode::Accessibility)
            .expect("navigate failed");
        eprintln!("{snap}");

        assert!(snap.contains("Count:"), "should contain 'Count:': {snap}");
        assert!(snap.contains("Increment"), "should contain 'Increment': {snap}");
        assert!(snap.contains("Decrement"), "should contain 'Decrement': {snap}");
        assert!(snap.contains("Count: 0"), "initial count should be 0: {snap}");
    }

    // --- Test 2: Click increment ---
    eprintln!("\n=== TEST: Click Increment ===");
    {
        let mut engine = Engine::new();
        let mut fetcher = HttpFetcher::new(&base_url);
        engine
            .navigate(&base_url, &mut fetcher, SnapMode::Accessibility)
            .expect("navigate failed");

        let action = engine.handle_click("#increment");
        eprintln!("click action: {action:?}");
        engine.settle();

        let snap = engine.snapshot(SnapMode::Accessibility);
        eprintln!("{snap}");

        assert!(
            snap.contains("Count: 1"),
            "count should be 1 after increment: {snap}"
        );
    }

    // --- Test 3: Multiple clicks ---
    eprintln!("\n=== TEST: Multiple Clicks ===");
    {
        let mut engine = Engine::new();
        let mut fetcher = HttpFetcher::new(&base_url);
        engine
            .navigate(&base_url, &mut fetcher, SnapMode::Accessibility)
            .expect("navigate failed");

        for i in 1..=3 {
            engine.handle_click("#increment");
            engine.settle();
            let snap = engine.snapshot(SnapMode::Accessibility);
            eprintln!("after click {i}: {snap}");
        }

        let snap = engine.snapshot(SnapMode::Accessibility);
        assert!(
            snap.contains("Count: 3"),
            "count should be 3 after 3 clicks: {snap}"
        );
    }

    // --- Test 4: Decrement ---
    eprintln!("\n=== TEST: Decrement ===");
    {
        let mut engine = Engine::new();
        let mut fetcher = HttpFetcher::new(&base_url);
        engine
            .navigate(&base_url, &mut fetcher, SnapMode::Accessibility)
            .expect("navigate failed");

        engine.handle_click("#decrement");
        engine.settle();

        let snap = engine.snapshot(SnapMode::Accessibility);
        eprintln!("{snap}");

        assert!(
            snap.contains("Count: -1"),
            "count should be -1 after decrement: {snap}"
        );
    }

    server.kill().ok();
    server.wait().ok();
}
