//! Diagnostic smoke tests — load real production websites, print what renders.
//!
//! These are `#[ignore]`d so they don't run in normal CI. Run manually:
//!   cargo test -p braille-cli --test real_sites -- --ignored --nocapture

use braille_engine::{Engine, FetchedResources};
use braille_wire::{FetchResponseData, SnapMode};

// ---------------------------------------------------------------------------
// Fetch servicing helper (same pattern as fetch_integration.rs)
// ---------------------------------------------------------------------------

fn service_fetches(client: &reqwest::blocking::Client, engine: &mut Engine, max_rounds: usize) {
    for _ in 0..max_rounds {
        if !engine.has_pending_fetches() {
            break;
        }
        let pending = engine.pending_fetches();
        for req in pending {
            let result = match req.method.as_str() {
                "POST" => {
                    let mut builder = client.post(&req.url);
                    for (k, v) in &req.headers {
                        builder = builder.header(k.as_str(), v.as_str());
                    }
                    if let Some(body) = &req.body {
                        builder = builder.body(body.clone());
                    }
                    builder.send()
                }
                _ => client.get(&req.url).send(),
            };

            match result {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let ct = resp
                        .headers()
                        .get("content-type")
                        .and_then(|v| v.to_str().ok())
                        .map(|s| s.to_string());
                    let final_url = resp.url().to_string();
                    let body = resp.text().unwrap_or_default();

                    let headers = ct
                        .map(|c| vec![("content-type".to_string(), c)])
                        .unwrap_or_default();

                    let response_data = FetchResponseData {
                        status,
                        status_text: status_text(status).to_string(),
                        headers,
                        body,
                        url: final_url,
                    };
                    engine.resolve_fetch(req.id, &response_data);
                }
                Err(e) => {
                    engine.reject_fetch(req.id, &format!("{e}"));
                }
            }
        }
        engine.settle();
    }
}

fn status_text(code: u16) -> &'static str {
    match code {
        200 => "OK",
        301 => "Moved Permanently",
        302 => "Found",
        304 => "Not Modified",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "",
    }
}

// ---------------------------------------------------------------------------
// Helper: fetch page HTML + external scripts, load into engine
// ---------------------------------------------------------------------------

fn load_real_site(url: &str) -> (Engine, Vec<String>) {
    let client = reqwest::blocking::Client::builder()
        .user_agent("Mozilla/5.0 (compatible; Braille/0.1; +https://github.com/nicksrandall/braille)")
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .expect("build reqwest client");

    // 1. Fetch main HTML
    let resp = client.get(url).send().expect("fetch main page");
    let final_url = resp.url().to_string();
    let html = resp.text().expect("read response body");

    // 2. Parse and collect script descriptors
    let mut engine = Engine::new();
    let descriptors = engine.parse_and_collect_scripts(&html);

    // 3. Fetch external scripts
    let mut fetched_scripts = std::collections::HashMap::new();
    for desc in &descriptors {
        if let Some(src) = desc.external_url() {
            // Resolve relative URLs against the page URL
            let resolved = if src.starts_with("http://") || src.starts_with("https://") {
                src.to_string()
            } else if src.starts_with("//") {
                format!("https:{src}")
            } else if src.starts_with('/') {
                // Absolute path — resolve against origin
                if let Ok(base) = url::Url::parse(&final_url) {
                    format!("{}://{}{}", base.scheme(), base.host_str().unwrap_or(""), src)
                } else {
                    continue;
                }
            } else {
                if let Ok(base) = url::Url::parse(&final_url) {
                    base.join(src).map(|u| u.to_string()).unwrap_or_default()
                } else {
                    continue;
                }
            };

            match client.get(&resolved).send() {
                Ok(resp) if resp.status().is_success() => {
                    if let Ok(body) = resp.text() {
                        fetched_scripts.insert(src.to_string(), body);
                    }
                }
                Ok(resp) => {
                    eprintln!("  [script] {src} → HTTP {}", resp.status());
                }
                Err(e) => {
                    eprintln!("  [script] {src} → error: {e}");
                }
            }
        }
    }

    let fetched = FetchedResources::scripts_only(fetched_scripts);

    // 4. Execute scripts (lossy — collect errors)
    let errors = engine.execute_scripts_lossy(&descriptors, &fetched);

    // 5. Set URL and settle
    engine.set_url(&final_url);
    engine.settle();

    // 6. Service pending fetches
    service_fetches(&client, &mut engine, 10);

    (engine, errors)
}

fn print_diagnostics(site: &str, engine: &mut Engine, errors: &[String]) {
    let sep = "=".repeat(72);
    eprintln!("\n{sep}");
    eprintln!("  SITE: {site}");
    eprintln!("{sep}");
    eprintln!("  JS errors: {}", errors.len());
    for (i, err) in errors.iter().enumerate().take(10) {
        let truncated = if err.len() > 200 { &err[..200] } else { err };
        eprintln!("    [{i}] {truncated}");
    }
    if errors.len() > 10 {
        eprintln!("    ... and {} more", errors.len() - 10);
    }

    let accessibility = engine.snapshot(SnapMode::Accessibility);
    let text = engine.snapshot(SnapMode::Text);
    eprintln!("\n  Accessibility snapshot ({} chars):", accessibility.len());
    // Print first 40 lines
    for (i, line) in accessibility.lines().enumerate().take(40) {
        eprintln!("    {line}");
        if i == 39 {
            eprintln!("    ...");
        }
    }

    eprintln!("\n  Text snapshot ({} chars):", text.len());
    for (i, line) in text.lines().enumerate().take(20) {
        eprintln!("    {line}");
        if i == 19 {
            eprintln!("    ...");
        }
    }
    eprintln!();
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn smoke_example_com() {
    let (mut engine, errors) = load_real_site("https://example.com");
    print_diagnostics("https://example.com", &mut engine, &errors);

    let snap = engine.snapshot(SnapMode::Text);
    assert!(!snap.is_empty(), "snapshot should be non-empty");
}

#[test]
#[ignore]
fn smoke_react_dev() {
    let (mut engine, errors) = load_real_site("https://react.dev");
    print_diagnostics("https://react.dev", &mut engine, &errors);

    let snap = engine.snapshot(SnapMode::Text);
    assert!(!snap.is_empty(), "snapshot should be non-empty");
}

#[test]
#[ignore]
fn smoke_google() {
    let (mut engine, errors) = load_real_site("https://www.google.com");
    print_diagnostics("https://www.google.com", &mut engine, &errors);

    let snap = engine.snapshot(SnapMode::Text);
    assert!(!snap.is_empty(), "snapshot should be non-empty");
}
