//! Wikipedia browsing test — load Wikipedia, search, click through results.
//!
//! Run with: cargo test -p braille-cli --test wikipedia_test -- --ignored --nocapture

use braille_engine::{Engine, FetchedResources};
use braille_wire::{FetchResponseData, SnapMode};
use std::collections::HashMap;

fn service_fetches(client: &reqwest::blocking::Client, engine: &mut Engine, max_rounds: usize) {
    for round in 0..max_rounds {
        if !engine.has_pending_fetches() {
            break;
        }
        let pending = engine.pending_fetches();
        for req in &pending {
            eprintln!(
                "    [fetch round {round}] {} {} (body={})",
                req.method,
                &req.url[..req.url.len().min(100)],
                req.body.as_ref().map(|b| b.len()).unwrap_or(0)
            );
        }
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
                        status_text: match status {
                            200 => "OK",
                            301 => "Moved Permanently",
                            302 => "Found",
                            304 => "Not Modified",
                            404 => "Not Found",
                            _ => "",
                        }
                        .to_string(),
                        headers,
                        body,
                        url: final_url,
                        redirect_chain: vec![],
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

fn load_real_site(url: &str) -> (Engine, reqwest::blocking::Client) {
    let client = reqwest::blocking::Client::builder()
        .user_agent("Mozilla/5.0 (compatible; Braille/0.1)")
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .expect("build reqwest client");

    let resp = client.get(url).send().expect("fetch main page");
    let final_url = resp.url().to_string();
    let html = resp.text().expect("read response body");

    let mut engine = Engine::new();
    let descriptors = engine.parse_and_collect_scripts(&html);

    let import_map_urls = Engine::import_map_urls(&descriptors);
    let all_urls: Vec<String> = descriptors
        .iter()
        .filter_map(|d| d.external_url().map(|s| s.to_string()))
        .chain(import_map_urls.into_iter())
        .collect();

    let mut fetched_scripts = HashMap::new();
    for src in &all_urls {
        let resolved = if src.starts_with("http://") || src.starts_with("https://") {
            src.to_string()
        } else if src.starts_with("//") {
            format!("https:{src}")
        } else if src.starts_with('/') {
            let base = url::Url::parse(&final_url).unwrap();
            format!("{}://{}{}", base.scheme(), base.host_str().unwrap_or(""), src)
        } else {
            let base = url::Url::parse(&final_url).unwrap();
            base.join(src).map(|u| u.to_string()).unwrap_or_default()
        };

        eprintln!("  [script] {}", &resolved[..resolved.len().min(100)]);
        match client.get(&resolved).send() {
            Ok(r) => {
                let code = r.text().unwrap_or_default();
                fetched_scripts.insert(src.clone(), code);
            }
            Err(e) => {
                eprintln!("    FAILED: {e}");
            }
        }
    }

    let resources = FetchedResources {
        scripts: fetched_scripts,
        iframes: HashMap::new(),
    };
    engine.execute_scripts_lossy(&descriptors, &resources);
    engine.settle();

    service_fetches(&client, &mut engine, 3);

    (engine, client)
}

#[test]
#[ignore]
fn wikipedia_homepage() {
    eprintln!("\n=== Loading Wikipedia homepage ===");
    let (mut engine, _client) = load_real_site("https://en.wikipedia.org/wiki/Main_Page");

    eprintln!("\n--- Compact view ---");
    let compact = engine.snapshot(SnapMode::Compact);
    eprintln!("{}", &compact[..compact.len().min(3000)]);

    eprintln!("\n--- Headings view ---");
    let headings = engine.snapshot(SnapMode::Headings);
    eprintln!("{headings}");

    eprintln!("\n--- Links view (first 2000 chars) ---");
    let links = engine.snapshot(SnapMode::Links);
    eprintln!("{}", &links[..links.len().min(2000)]);

    eprintln!("\n--- Interactive view ---");
    let interactive = engine.snapshot(SnapMode::Interactive);
    eprintln!("{}", &interactive[..interactive.len().min(2000)]);

    // Basic sanity: should contain Wikipedia-ish content
    assert!(
        compact.contains("Wikipedia") || compact.contains("encyclopedia"),
        "Homepage should mention Wikipedia: {}",
        &compact[..compact.len().min(500)]
    );
}

#[test]
#[ignore]
fn wikipedia_article() {
    eprintln!("\n=== Loading Wikipedia article: Rust (programming language) ===");
    let (mut engine, _client) =
        load_real_site("https://en.wikipedia.org/wiki/Rust_(programming_language)");

    eprintln!("\n--- Compact view (first 3000 chars) ---");
    let compact = engine.snapshot(SnapMode::Compact);
    eprintln!("{}", &compact[..compact.len().min(3000)]);

    eprintln!("\n--- Headings view ---");
    let headings = engine.snapshot(SnapMode::Headings);
    eprintln!("{headings}");

    eprintln!("\n--- Text view (first 2000 chars) ---");
    let text = engine.snapshot(SnapMode::Text);
    eprintln!("{}", &text[..text.len().min(2000)]);

    // Should contain Rust-related content
    assert!(
        compact.contains("Rust") || compact.contains("programming"),
        "Article should mention Rust: {}",
        &compact[..compact.len().min(500)]
    );
}

#[test]
#[ignore]
fn wikipedia_search_flow() {
    eprintln!("\n=== Wikipedia search flow ===");
    let (mut engine, _client) = load_real_site("https://en.wikipedia.org/wiki/Main_Page");

    // Show what interactive elements are available
    eprintln!("\n--- Interactive view (looking for search) ---");
    let interactive = engine.snapshot(SnapMode::Interactive);
    eprintln!("{interactive}");

    // Try to find and use the search box
    eprintln!("\n--- Forms view ---");
    let forms = engine.snapshot(SnapMode::Forms);
    eprintln!("{forms}");

    // Type into search
    eprintln!("\n--- Typing 'artificial intelligence' into search ---");
    let _ = engine.handle_type("#searchInput", "artificial intelligence");

    eprintln!("\n--- After typing, forms view ---");
    let forms_after = engine.snapshot(SnapMode::Forms);
    eprintln!("{forms_after}");
}
