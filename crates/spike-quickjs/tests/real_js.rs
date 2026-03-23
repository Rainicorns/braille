//! Spike: can QuickJS evaluate the JS bundles that stack-overflow Boa?
//!
//! Run: cargo test -p spike-quickjs --test real_js -- --ignored --nocapture

use rquickjs::{CatchResultExt, Context, Runtime};

fn setup_stubs(ctx: &rquickjs::Ctx<'_>) {
    // Minimal DOM stubs so scripts don't immediately ReferenceError on globals.
    // We don't need real DOM — just enough to let the JS framework initialize.
    ctx.eval::<(), _>(
        r#"
        globalThis.window = globalThis;
        globalThis.self = globalThis;
        globalThis.document = {
            createElement: () => ({
                style: {},
                setAttribute: () => {},
                getAttribute: () => null,
                appendChild: () => {},
                removeChild: () => {},
                addEventListener: () => {},
                removeEventListener: () => {},
                classList: { add: () => {}, remove: () => {}, contains: () => false },
                querySelectorAll: () => [],
                querySelector: () => null,
                children: [],
                childNodes: [],
                parentNode: null,
                innerHTML: "",
                textContent: "",
                insertBefore: () => {},
                cloneNode: () => ({}),
                contains: () => false,
                getBoundingClientRect: () => ({ top: 0, left: 0, width: 0, height: 0, right: 0, bottom: 0 }),
                dataset: {},
                ownerDocument: null,
                nodeName: "DIV",
                nodeType: 1,
            }),
            createElementNS: () => ({
                style: {},
                setAttribute: () => {},
                getAttribute: () => null,
                appendChild: () => {},
                children: [],
                childNodes: [],
            }),
            createTextNode: (t) => ({ nodeType: 3, textContent: t, parentNode: null }),
            createComment: (t) => ({ nodeType: 8, textContent: t, parentNode: null }),
            createDocumentFragment: () => ({ nodeType: 11, childNodes: [], appendChild: () => {}, children: [] }),
            getElementById: () => null,
            getElementsByTagName: () => [],
            getElementsByClassName: () => [],
            querySelector: () => null,
            querySelectorAll: () => [],
            addEventListener: () => {},
            removeEventListener: () => {},
            head: { appendChild: () => {}, children: [], querySelectorAll: () => [] },
            body: { appendChild: () => {}, children: [], classList: { add: () => {}, remove: () => {} }, style: {}, setAttribute: () => {}, getAttribute: () => null },
            documentElement: { appendChild: () => {}, style: {}, setAttribute: () => {}, getAttribute: () => null, classList: { add: () => {}, remove: () => {} } },
            title: "",
            cookie: "",
            readyState: "complete",
            location: { href: "about:blank", protocol: "https:", hostname: "localhost", pathname: "/", search: "", hash: "", origin: "https://localhost" },
            defaultView: globalThis,
            createRange: () => ({ setStart: () => {}, setEnd: () => {}, commonAncestorContainer: null, collapsed: true }),
            createTreeWalker: () => ({ nextNode: () => null, currentNode: null }),
            implementation: { createHTMLDocument: () => globalThis.document },
        };
        globalThis.navigator = {
            userAgent: "Mozilla/5.0",
            language: "en-US",
            languages: ["en-US"],
            platform: "Linux",
            onLine: true,
            cookieEnabled: true,
            clipboard: { writeText: () => Promise.resolve() },
            mediaDevices: {},
            serviceWorker: { register: () => Promise.resolve() },
            sendBeacon: () => true,
        };
        globalThis.location = globalThis.document.location;
        globalThis.history = { pushState: () => {}, replaceState: () => {}, back: () => {}, forward: () => {}, state: null, length: 1 };
        globalThis.localStorage = { getItem: () => null, setItem: () => {}, removeItem: () => {}, clear: () => {}, key: () => null, length: 0 };
        globalThis.sessionStorage = globalThis.localStorage;
        globalThis.getComputedStyle = () => new Proxy({}, { get: () => "" });
        globalThis.matchMedia = () => ({ matches: false, addListener: () => {}, removeListener: () => {}, addEventListener: () => {}, removeEventListener: () => {} });
        globalThis.requestAnimationFrame = (cb) => setTimeout(cb, 0);
        globalThis.cancelAnimationFrame = () => {};
        globalThis.requestIdleCallback = (cb) => setTimeout(cb, 0);
        globalThis.cancelIdleCallback = () => {};
        globalThis.ResizeObserver = class { observe() {} unobserve() {} disconnect() {} };
        globalThis.IntersectionObserver = class { observe() {} unobserve() {} disconnect() {} };
        globalThis.MutationObserver = class { observe() {} disconnect() {} takeRecords() { return []; } };
        globalThis.Event = globalThis.Event || class { constructor(t, o) { this.type = t; this.bubbles = o?.bubbles || false; this.cancelable = o?.cancelable || false; this.defaultPrevented = false; this.target = null; this.currentTarget = null; } preventDefault() { this.defaultPrevented = true; } stopPropagation() {} stopImmediatePropagation() {} };
        globalThis.CustomEvent = class extends Event { constructor(t, o) { super(t, o); this.detail = o?.detail; } };
        globalThis.performance = { now: () => 0, mark: () => {}, measure: () => {}, getEntriesByType: () => [], getEntriesByName: () => [], timing: { navigationStart: 0 } };
        globalThis.fetch = () => Promise.resolve({ ok: true, status: 200, json: () => Promise.resolve({}), text: () => Promise.resolve("") });
        globalThis.XMLHttpRequest = class { open() {} send() {} setRequestHeader() {} addEventListener() {} };
        globalThis.AbortController = class { constructor() { this.signal = { aborted: false, addEventListener: () => {} }; } abort() {} };
        globalThis.URL = class { constructor(u) { this.href = u; this.origin = ""; this.pathname = "/"; this.search = ""; this.hash = ""; this.searchParams = { get: () => null, set: () => {}, has: () => false }; } toString() { return this.href; } };
        globalThis.URLSearchParams = class { constructor() {} get() { return null; } set() {} has() { return false; } toString() { return ""; } };
        globalThis.DOMParser = class { parseFromString() { return globalThis.document; } };
        globalThis.getSelection = () => ({ rangeCount: 0, removeAllRanges: () => {}, addRange: () => {} });
        globalThis.queueMicrotask = (cb) => Promise.resolve().then(cb);
        globalThis.structuredClone = (v) => JSON.parse(JSON.stringify(v));
        globalThis.TextEncoder = class { encode(s) { return new Uint8Array(0); } };
        globalThis.TextDecoder = class { decode() { return ""; } };
        globalThis.btoa = (s) => s;
        globalThis.atob = (s) => s;
        globalThis.crypto = { subtle: { digest: () => Promise.resolve(new ArrayBuffer(0)) }, getRandomValues: (a) => a };
        globalThis.HTMLElement = class {};
        globalThis.Element = class {};
        globalThis.Node = class {};
        globalThis.CSSStyleSheet = class { insertRule() {} deleteRule() {} get cssRules() { return []; } };
        globalThis.ShadowRoot = class {};
        globalThis.DocumentFragment = class {};
        globalThis.WeakRef = globalThis.WeakRef || class { constructor(t) { this._t = t; } deref() { return this._t; } };
        globalThis.FinalizationRegistry = globalThis.FinalizationRegistry || class { register() {} };
        globalThis.dataLayer = [];
        globalThis.ga = function() {};
        globalThis.gtag = function() {};
    "#,
    )
    .catch(ctx)
    .unwrap();
}

fn fetch_page_scripts(url: &str) -> (String, Vec<(String, String)>) {
    let client = reqwest::blocking::Client::builder()
        .user_agent("Mozilla/5.0 (compatible; Braille/0.1)")
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .unwrap();

    let resp = client.get(url).send().unwrap();
    let final_url = resp.url().to_string();
    let html = resp.text().unwrap();

    let mut scripts = Vec::new();
    let mut pos = 0;
    while let Some(start) = html[pos..].find("<script") {
        let tag_start = pos + start;
        let tag_end = match html[tag_start..].find('>') {
            Some(e) => tag_start + e + 1,
            None => break,
        };
        let tag = &html[tag_start..tag_end];

        // Skip non-JS scripts (json-ld, importmap, etc.)
        let is_non_js = tag.contains("application/json")
            || tag.contains("application/ld+json")
            || tag.contains("importmap");
        if is_non_js {
            pos = tag_end;
            continue;
        }

        if let Some(src_start) = tag.find("src=") {
            // External script
            let rest = &tag[src_start + 4..];
            let (quote, rest) = if rest.starts_with('"') {
                ('"', &rest[1..])
            } else if rest.starts_with('\'') {
                ('\'', &rest[1..])
            } else {
                pos = tag_end;
                continue;
            };
            if let Some(end) = rest.find(quote) {
                let src = &rest[..end];
                let resolved = if src.starts_with("http") {
                    src.to_string()
                } else if src.starts_with("//") {
                    format!("https:{src}")
                } else if src.starts_with('/') {
                    let origin = url::Url::parse(&final_url)
                        .map(|u: url::Url| {
                            format!("{}://{}", u.scheme(), u.host_str().unwrap_or(""))
                        })
                        .unwrap_or_default();
                    format!("{origin}{src}")
                } else {
                    src.to_string()
                };

                match client.get(&resolved).send() {
                    Ok(resp) if resp.status().is_success() => {
                        if let Ok(body) = resp.text() {
                            scripts.push((resolved, body));
                        }
                    }
                    _ => {}
                }
            }
        } else {
            // Inline script — extract content between <script...> and </script>
            let close_tag = "</script>";
            if let Some(close_pos) = html[tag_end..].find(close_tag) {
                let inline_code = &html[tag_end..tag_end + close_pos];
                let trimmed = inline_code.trim();
                if !trimmed.is_empty() {
                    scripts.push(("inline".to_string(), trimmed.to_string()));
                }
            }
        }

        pos = tag_end;
    }

    (html, scripts)
}

fn eval_with_quickjs(scripts: &[(String, String)]) -> (usize, usize, Vec<String>) {
    let rt = Runtime::new().unwrap();
    // Set memory limit to 256MB and stack size to 64MB
    rt.set_memory_limit(256 * 1024 * 1024);
    rt.set_max_stack_size(64 * 1024 * 1024);

    let ctx = Context::full(&rt).unwrap();

    let mut successes = 0;
    let mut failures = 0;
    let mut errors = Vec::new();

    ctx.with(|ctx| {
        setup_stubs(&ctx);

        for (url, code) in scripts {
            let short_url = if url.len() > 80 {
                format!("...{}", &url[url.len() - 60..])
            } else {
                url.clone()
            };

            match ctx.eval::<(), _>(code.as_str()).catch(&ctx) {
                Ok(()) => {
                    eprintln!("  ✓ {short_url} ({} bytes)", code.len());
                    successes += 1;
                }
                Err(e) => {
                    let msg = format!("{e:?}");
                    let short_msg = if msg.len() > 120 {
                        format!("{}...", &msg[..120])
                    } else {
                        msg.clone()
                    };
                    eprintln!("  ✗ {short_url} ({} bytes): {short_msg}", code.len());
                    errors.push(format!("{short_url}: {short_msg}"));
                    failures += 1;
                }
            }
        }
    });

    (successes, failures, errors)
}

#[test]
#[ignore]
fn quickjs_github() {
    eprintln!("\n=== GitHub.com ===");
    let (_html, scripts) = fetch_page_scripts("https://github.com");
    eprintln!("Fetched {} scripts", scripts.len());

    let total_bytes: usize = scripts.iter().map(|(_, s)| s.len()).sum();
    eprintln!("Total JS: {} bytes ({:.1} KB)", total_bytes, total_bytes as f64 / 1024.0);

    let (ok, fail, _errors) = eval_with_quickjs(&scripts);
    eprintln!("\nResult: {ok} succeeded, {fail} failed out of {} scripts", scripts.len());

    // The point: QuickJS should NOT stack overflow. Errors from missing DOM APIs are fine.
    // If we get here at all, the spike succeeded.
    eprintln!("✓ QuickJS did not stack overflow!");
}

#[test]
#[ignore]
fn quickjs_react_dev() {
    eprintln!("\n=== react.dev ===");
    let (_html, scripts) = fetch_page_scripts("https://react.dev");
    eprintln!("Fetched {} scripts", scripts.len());

    let total_bytes: usize = scripts.iter().map(|(_, s)| s.len()).sum();
    eprintln!("Total JS: {} bytes ({:.1} KB)", total_bytes, total_bytes as f64 / 1024.0);

    let (ok, fail, _errors) = eval_with_quickjs(&scripts);
    eprintln!("\nResult: {ok} succeeded, {fail} failed out of {} scripts", scripts.len());
    eprintln!("✓ QuickJS did not stack overflow!");
}

#[test]
#[ignore]
fn quickjs_google() {
    eprintln!("\n=== Google.com ===");
    let (_html, scripts) = fetch_page_scripts("https://www.google.com");
    eprintln!("Fetched {} scripts", scripts.len());

    let total_bytes: usize = scripts.iter().map(|(_, s)| s.len()).sum();
    eprintln!("Total JS: {} bytes ({:.1} KB)", total_bytes, total_bytes as f64 / 1024.0);

    let (ok, fail, _errors) = eval_with_quickjs(&scripts);
    eprintln!("\nResult: {ok} succeeded, {fail} failed out of {} scripts", scripts.len());
    eprintln!("✓ QuickJS did not stack overflow!");
}

#[test]
#[ignore]
fn quickjs_protonmail_signup() {
    eprintln!("\n=== ProtonMail Signup ===");
    let (_html, scripts) = fetch_page_scripts("https://account.proton.me/signup");
    eprintln!("Fetched {} scripts", scripts.len());

    let total_bytes: usize = scripts.iter().map(|(_, s)| s.len()).sum();
    eprintln!("Total JS: {} bytes ({:.1} KB)", total_bytes, total_bytes as f64 / 1024.0);

    let (ok, fail, _errors) = eval_with_quickjs(&scripts);
    eprintln!("\nResult: {ok} succeeded, {fail} failed out of {} scripts", scripts.len());
    eprintln!("✓ QuickJS did not stack overflow!");
}
