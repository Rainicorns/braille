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
    for round in 0..max_rounds {
        if !engine.has_pending_fetches() {
            break;
        }
        let pending = engine.pending_fetches();
        for req in &pending {
            eprintln!("    [fetch round {round}] {} {} (body={})",
                req.method, &req.url[..req.url.len().min(100)],
                req.body.as_ref().map(|b| b.len()).unwrap_or(0));
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
                        status_text: status_text(status).to_string(),
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

    // 3. Fetch external scripts + import map URLs
    let mut fetched_scripts = std::collections::HashMap::new();
    let import_map_urls = Engine::import_map_urls(&descriptors);
    let all_urls: Vec<String> = descriptors
        .iter()
        .filter_map(|d| d.external_url().map(|s| s.to_string()))
        .chain(import_map_urls.into_iter())
        .collect();
    for src in &all_urls {
        let src = src.as_str();
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
        } else if let Ok(base) = url::Url::parse(&final_url) {
            base.join(src).map(|u| u.to_string()).unwrap_or_default()
        } else {
            continue;
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

    let fetched = FetchedResources::scripts_only(fetched_scripts);

    // 4. Set URL before scripts so routers see correct pathname
    engine.set_url(&final_url);

    // 5. Execute scripts (lossy — collect errors) and settle
    let errors = engine.execute_scripts_lossy(&descriptors, &fetched);

    // 6. Install error tracking right after runtime is up
    engine.eval_js(r#"
        window.__early_errors = [];
        window.addEventListener('error', function(e) {
            __early_errors.push('ERR: ' + (e.message || e) + (e.filename ? ' @ ' + e.filename : ''));
        });
        window.addEventListener('unhandledrejection', function(e) {
            var r = e && e.reason;
            __early_errors.push('REJ: ' + (r instanceof Error ? r.message + '\n' + (r.stack || '') : String(r)));
        });
    "#).ok();

    // 7. Interleave settle + fetch until quiescent
    for _round in 0..30 {
        engine.settle();
        let app_kids = engine.eval_js(
            "document.querySelector('.app-root') ? document.querySelector('.app-root').childNodes.length : -1"
        );
        let pending = engine.has_pending_fetches();
        let has_timers = engine.has_pending_timers();
        eprintln!("    [settle/fetch round {_round}] app-root children={:?}, pending_fetches={pending}, timers={has_timers}",
            app_kids);
        if !pending {
            break;
        }
        service_fetches(&client, &mut engine, 5);
    }

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

#[test]
#[ignore]
fn smoke_github() {
    let (mut engine, errors) = load_real_site("https://github.com");
    print_diagnostics("https://github.com", &mut engine, &errors);

    let snap = engine.snapshot(SnapMode::Text);
    assert!(!snap.is_empty(), "snapshot should be non-empty");
}

#[test]
#[ignore]
fn smoke_wikipedia() {
    let (mut engine, errors) = load_real_site("https://en.wikipedia.org/wiki/Main_Page");
    print_diagnostics("https://en.wikipedia.org/wiki/Main_Page", &mut engine, &errors);

    let snap = engine.snapshot(SnapMode::Text);
    assert!(!snap.is_empty(), "snapshot should be non-empty");
}

#[test]
#[ignore]
fn smoke_hackernews() {
    let (mut engine, errors) = load_real_site("https://news.ycombinator.com");
    print_diagnostics("https://news.ycombinator.com", &mut engine, &errors);

    let snap = engine.snapshot(SnapMode::Text);
    assert!(!snap.is_empty(), "snapshot should be non-empty");
}

#[test]
#[ignore]
fn smoke_stackoverflow() {
    let (mut engine, errors) = load_real_site("https://stackoverflow.com");
    print_diagnostics("https://stackoverflow.com", &mut engine, &errors);

    let snap = engine.snapshot(SnapMode::Text);
    assert!(!snap.is_empty(), "snapshot should be non-empty");
}

#[test]
#[ignore]
fn smoke_mdn() {
    let (mut engine, errors) = load_real_site("https://developer.mozilla.org/en-US/");
    print_diagnostics("https://developer.mozilla.org/en-US/", &mut engine, &errors);

    let snap = engine.snapshot(SnapMode::Text);
    assert!(!snap.is_empty(), "snapshot should be non-empty");
}

#[test]
#[ignore]
fn smoke_protonmail_signup() {
    let (mut engine, errors) = load_real_site("https://account.proton.me/signup");
    print_diagnostics("https://account.proton.me/signup", &mut engine, &errors);

    // Check dynamic script loading results
    let script_log = engine.eval_js("__braille_script_log.join('\\n')");
    eprintln!("  Dynamic script log:\n{}", script_log.as_deref().unwrap_or("ERR"));

    // Check early errors
    let early = engine.eval_js("__early_errors.length + ' errors:\\n' + __early_errors.join('\\n')");
    eprintln!("  Early errors: {}", early.as_deref().unwrap_or("ERR"));

    // Check timer errors
    let timer_errs = engine.eval_js("__braille_timer_errors.length + ' timer errors:\\n' + __braille_timer_errors.slice(0, 10).join('\\n---\\n')");
    eprintln!("  Timer errors: {}", timer_errs.as_deref().unwrap_or("ERR"));

    // Check missing APIs
    let api_check = engine.eval_js(r#"
        (function() {
            var el = document.createElement('div');
            var missing = [];
            var checks = [
                ['el.cloneNode', typeof el.cloneNode],
                ['el.replaceChild', typeof el.replaceChild],
                ['el.contains', typeof el.contains],
                ['el.compareDocumentPosition', typeof el.compareDocumentPosition],
                ['el.closest', typeof el.closest],
                ['el.matches', typeof el.matches],
                ['el.remove', typeof el.remove],
                ['el.getRootNode', typeof el.getRootNode],
                ['el.hasAttribute', typeof el.hasAttribute],
                ['el.dataset', typeof el.dataset],
                ['el.style.setProperty', el.style ? typeof el.style.setProperty : 'no style'],
                ['document.createComment', typeof document.createComment],
                ['document.createDocumentFragment', typeof document.createDocumentFragment],
                ['document.createRange', typeof document.createRange],
                ['window.getComputedStyle', typeof window.getComputedStyle],
            ];
            for (var i = 0; i < checks.length; i++) {
                if (checks[i][1] !== 'function' && checks[i][1] !== 'object') missing.push(checks[i][0] + '=' + checks[i][1]);
            }
            return missing.length ? 'MISSING: ' + missing.join(', ') : 'all OK';
        })()
    "#);
    eprintln!("  API check: {}", api_check.as_deref().unwrap_or("ERR"));

    // Deep fiber walk
    let fiber_walk = engine.eval_js(r#"
        (function() {
            var el = document.querySelector('.app-root');
            if (!el) return 'NO APP-ROOT';
            var fiberKey = null;
            var keys = Object.keys(el);
            for (var i = 0; i < keys.length; i++) {
                if (keys[i].indexOf('__reactContainer') === 0 || keys[i].indexOf('__reactFiber') === 0) {
                    fiberKey = keys[i]; break;
                }
            }
            if (!fiberKey) return 'NO FIBER KEY found in: ' + keys.join(',');
            var fiber = el[fiberKey];
            if (!fiber) return 'FIBER IS NULL';

            // Walk to root
            var node = fiber;
            while (node.return) node = node.return;
            var root = node.stateNode;

            var info = [];
            info.push('rootFiber.tag=' + node.tag);
            info.push('root.pendingLanes=' + root.pendingLanes);
            info.push('root.suspendedLanes=' + root.suspendedLanes);
            info.push('root.pingedLanes=' + root.pingedLanes);
            info.push('root.expiredLanes=' + root.expiredLanes);

            // Walk fiber tree from root.current
            var current = root.current;
            function walkFiber(f, depth) {
                if (!f || depth > 8) return;
                var indent = '  '.repeat(depth);
                var typeStr = typeof f.type === 'function' ? f.type.name || 'fn' :
                              typeof f.type === 'string' ? f.type : String(f.type);
                info.push(indent + 'tag=' + f.tag + ' type=' + typeStr +
                    (f.memoizedState ? ' state=yes' : '') +
                    (f.child ? ' hasChild' : ' noChild') +
                    (f.sibling ? ' hasSibling' : ''));
                walkFiber(f.child, depth + 1);
                walkFiber(f.sibling, depth);
            }
            walkFiber(current, 0);

            return info.join('\n');
        })()
    "#);
    eprintln!("  Fiber walk:\n{}", fiber_walk.as_deref().unwrap_or("ERR"));

    // Check body state immediately
    let body_check = engine.eval_js("document.body ? document.body.childNodes.length + ' children' : 'no body'");
    eprintln!("  body right after load: {:?}", body_check);
    let body_html = engine.eval_js("document.body ? document.body.innerHTML.substring(0, 300) : 'no body'");
    eprintln!("  body innerHTML: {:?}", body_html);

    // Check if script[src*=...] selectors work (pre.js uses this)
    let script_check = engine.eval_js("document.querySelectorAll('script').length");
    eprintln!("  script elements: {:?}", script_check);
    let script_src_check = engine.eval_js("document.querySelector('script[src]') ? document.querySelector('script[src]').getAttribute('src') : 'none'");
    eprintln!("  first script[src]: {:?}", script_src_check);

    // Check what pre.js module 61017 sees
    let pre_check = engine.eval_js(r#"
        (function() {
            var scripts = document.querySelectorAll('script[src]');
            var info = 'script[src] count: ' + scripts.length;
            for (var i = 0; i < scripts.length; i++) {
                info += '\n  ' + scripts[i].getAttribute('src');
            }
            return info;
        })()
    "#);
    eprintln!("  pre.js selector check: {:?}", pre_check);

    // Check what browser features pre.js detects
    // Module 61017 classifies as Unsupported=0 — find out why
    let feature_probes = [
        "typeof CSS !== 'undefined' && typeof CSS.supports === 'function'",
        "typeof globalThis !== 'undefined'",
        "typeof window.Proxy === 'function'",
        "typeof Symbol === 'function'",
        "typeof Map === 'function'",
        "typeof Set === 'function'",
        "typeof Promise === 'function'",
        "typeof fetch === 'function'",
        "typeof Array.from === 'function'",
        "typeof Object.assign === 'function'",
        "'noModule' in document.createElement('script')",
        "typeof window.AbortController === 'function'",
        "typeof window.Intl === 'object'",
        "typeof window.ResizeObserver === 'function'",
        "typeof window.IntersectionObserver === 'function'",
        "typeof window.crypto === 'object'",
        "typeof window.crypto.subtle === 'object'",
    ];
    eprintln!("\n  Browser feature probes:");
    for probe in &feature_probes {
        let result = engine.eval_js(probe);
        eprintln!("    {probe} = {:?}", result);
    }

    // The webpack entry modules are 46868, 31109, 33759 in public-index.js
    // Module 31109 and 33759 are likely the app bootstrap
    // The entry function calls e.O(0,[6775],...,5) — deferred until chunk 6775 ready
    // Then immediately calls t(46868), t(31109), t(33759)
    // If any of these modules use async imports, they'd resolve via promises
    // Let's check if there are any webpack async module promises pending
    let async_check = engine.eval_js(r#"
        (function() {
            var info = [];
            info.push('SENTRY_RELEASE: ' + JSON.stringify(window.SENTRY_RELEASE || self.SENTRY_RELEASE));
            info.push('protonSupportedBrowser: ' + window.protonSupportedBrowser);
            return info.join('\n');
        })()
    "#);
    eprintln!("  async module check: {:?}", async_check);

    // Install unhandled rejection tracker and settle more
    engine.eval_js(r#"
        var __rejections = [];
        window.addEventListener('unhandledrejection', function(e) {
            var r = e && e.reason;
            __rejections.push(r instanceof Error ? r.message + '\n' + (r.stack || '') : String(r));
        });
    "#).ok();

    // Check React fiber state on app-root
    let react_state = engine.eval_js(r#"
        (function() {
            var el = document.querySelector('.app-root');
            if (!el) return 'NO APP-ROOT';
            var info = [];
            // Check for React internal properties
            var keys = Object.keys(el);
            for (var i = 0; i < keys.length; i++) {
                if (keys[i].indexOf('react') >= 0 || keys[i].indexOf('__react') >= 0 || keys[i].indexOf('_react') >= 0) {
                    info.push(keys[i] + ': ' + typeof el[keys[i]]);
                }
            }
            // Check all own property names (including symbols)
            info.push('ownKeys: ' + keys.slice(0, 20).join(','));
            info.push('__nid: ' + el.__nid);
            // Check if createRoot worked
            info.push('_reactRoot: ' + typeof el._reactRootContainer);
            return info.join('\n');
        })()
    "#);
    eprintln!("  React state on app-root: {:?}", react_state);

    // Check dynamic scripts in head (not links)
    let head_scripts = engine.eval_js(r#"
        (function() {
            var all = document.querySelectorAll('head *');
            var scripts = [], links = [];
            for (var i = 0; i < all.length; i++) {
                var tag = all[i].tagName;
                if (tag === 'SCRIPT') {
                    scripts.push(all[i].getAttribute('src') || 'inline');
                } else if (tag === 'LINK') {
                    var rel = all[i].getAttribute('rel') || '';
                    var href = all[i].getAttribute('href') || '';
                    if (href.indexOf('chunk') >= 0 || rel === 'prefetch')
                        links.push(rel + ': ' + href.substring(0, 80));
                }
            }
            return 'scripts in head: ' + scripts.length + '\n' + scripts.join('\n') +
                   '\nprefetch links: ' + links.length + '\n' + links.slice(0, 15).join('\n');
        })()
    "#);
    eprintln!("  Dynamic head elements:\n{}", head_scripts.unwrap_or_default());

    // Check if the React fiber root has any work
    let fiber_probe = engine.eval_js(r#"
        (function() {
            var el = document.querySelector('.app-root');
            if (!el) return 'NO ELEMENT';
            var info = [];
            var keys = Object.keys(el);
            for (var i = 0; i < keys.length; i++) {
                if (keys[i].indexOf('__reactContainer') === 0) {
                    var fiber = el[keys[i]];
                    if (fiber) {
                        info.push('fiber.tag: ' + fiber.tag);
                        info.push('fiber.type: ' + (fiber.type && fiber.type.name || typeof fiber.type));
                        info.push('fiber.child: ' + (fiber.child ? 'yes(tag=' + fiber.child.tag + ')' : 'null'));
                        info.push('fiber.memoizedState: ' + (fiber.memoizedState ? 'yes' : 'null'));
                        if (fiber.child) {
                            var c = fiber.child;
                            info.push('child.type: ' + (c.type && c.type.name || typeof c.type));
                            info.push('child.child: ' + (c.child ? 'yes(tag=' + c.child.tag + ')' : 'null'));
                        }
                        // Walk up to find the root fiber
                        var node = fiber;
                        while (node.return) node = node.return;
                        info.push('root.tag: ' + node.tag);
                        var root = node.stateNode;
                        if (root) {
                            info.push('root.current.tag: ' + (root.current ? root.current.tag : 'none'));
                            info.push('root.pendingLanes: ' + root.pendingLanes);
                            info.push('root.callbackNode: ' + (root.callbackNode ? 'yes' : 'null'));
                        }
                    }
                }
            }
            return info.join('\n') || 'no fiber found';
        })()
    "#);
    eprintln!("  Fiber probe:\n{}", fiber_probe.unwrap_or_default());

    // Deep fiber inspection
    let fiber_deep = engine.eval_js(r#"
        (function() {
            var el = document.querySelector('.app-root');
            if (!el) return 'NO ELEMENT';
            var key;
            var keys = Object.keys(el);
            for (var i = 0; i < keys.length; i++) {
                if (keys[i].indexOf('__reactContainer') === 0) { key = keys[i]; break; }
            }
            if (!key) return 'no react key';
            var fiber = el[key];
            if (!fiber) return 'no fiber';
            var info = [];
            // Walk memoizedState chain (hooks)
            var ms = fiber.memoizedState;
            var hookIdx = 0;
            while (ms && hookIdx < 10) {
                var val = ms.memoizedState;
                if (val && typeof val === 'object' && val !== null) {
                    info.push('hook[' + hookIdx + ']: ' + JSON.stringify(val).substring(0, 200));
                } else {
                    info.push('hook[' + hookIdx + ']: ' + String(val));
                }
                ms = ms.next;
                hookIdx++;
            }
            // Check the element returned
            info.push('pendingProps: ' + JSON.stringify(fiber.pendingProps).substring(0, 200));
            info.push('memoizedProps: ' + JSON.stringify(fiber.memoizedProps).substring(0, 200));
            return info.join('\n');
        })()
    "#);
    eprintln!("  Fiber deep:\n{}", fiber_deep.unwrap_or_default());

    // Check ALL elements in head (not just script/link)
    let all_head = engine.eval_js(r#"
        (function() {
            var head = document.querySelector('head');
            if (!head) return 'no head';
            var all = head.childNodes;
            var scripts = [];
            for (var i = 0; i < all.length; i++) {
                var tag = all[i].tagName || '#text';
                if (tag === 'SCRIPT') {
                    scripts.push('SCRIPT src=' + (all[i].getAttribute('src') || 'inline') +
                        ' data-webpack=' + (all[i].getAttribute('data-webpack') || 'none'));
                }
            }
            return 'total head children: ' + all.length + ', scripts: ' + scripts.length + '\n' + scripts.join('\n');
        })()
    "#);
    eprintln!("  Head scripts: {:?}", all_head);

    eprintln!("\n  Deep settle/fetch cycles:");
    let client2 = reqwest::blocking::Client::builder()
        .user_agent("Mozilla/5.0 (compatible; Braille/0.1)")
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .unwrap();
    let mut sentry_bodies: Vec<String> = Vec::new();
    for round in 0..15 {
        engine.settle();
        let pf = engine.has_pending_fetches();
        let app_children = engine.eval_js("var r = document.querySelector('.app-root'); r ? r.childNodes.length : 0");
        let console_lines = engine.drain_console();
        let pending = if pf { engine.pending_fetches() } else { vec![] };
        // Capture sentry POST bodies
        for req in &pending {
            if req.url.contains("sentry") {
                if let Some(body) = &req.body {
                    sentry_bodies.push(body[..body.len().min(500)].to_string());
                }
            }
        }
        let non_version: Vec<_> = pending.iter().filter(|r| !r.url.contains("version.json")).collect();
        if !non_version.is_empty() || app_children.as_deref() != Ok("0") || !console_lines.is_empty() || round < 3 {
            eprintln!("    [round {round}] app-root children={:?}, total_fetches={}, non-version:",
                app_children, pending.len());
            for req in &non_version {
                eprintln!("      {} {}", req.method, &req.url[..req.url.len().min(150)]);
            }
            for line in console_lines.iter().take(5) {
                eprintln!("      console: {}", &line[..line.len().min(200)]);
            }
        }
        if pf {
            service_fetches(&client2, &mut engine, 1);
        }
        if app_children.as_deref() != Ok("0") {
            break;
        }
    }
    eprintln!("  Sentry error reports: {}", sentry_bodies.len());
    for (i, body) in sentry_bodies.iter().enumerate().take(5) {
        eprintln!("    [sentry {i}] {body}");
    }

    let rejections = engine.eval_js("JSON.stringify(__rejections.slice(0, 20))");
    eprintln!("  Unhandled rejections: {:?}", rejections);
    let app_root = engine.eval_js("var r = document.querySelector('.app-root'); r ? 'children=' + r.childNodes.length : 'MISSING'");
    eprintln!("  app-root: {:?}", app_root);

    // Check what fetch() does when called
    let fetch_test = engine.eval_js(r#"
        var __ft_result = 'not called';
        fetch('https://account.proton.me/api/core/v4/features').then(function(r) {
            __ft_result = 'resolved: ' + r.status;
        }).catch(function(e) {
            __ft_result = 'error: ' + e.message;
        });
        'fetch queued, pending=' + (typeof __braille_fetch_setup)
    "#);
    eprintln!("  Direct fetch test: {:?}", fetch_test);
    eprintln!("  has_pending_fetches after direct fetch: {}", engine.has_pending_fetches());

    // Check console errors
    let console_out = engine.drain_console();
    eprintln!("  Console: {} lines", console_out.len());
    for (i, line) in console_out.iter().enumerate().take(20) {
        eprintln!("    [{i}] {}", &line[..line.len().min(200)]);
    }
}

#[test]
#[ignore]
fn smoke_protonmail_signup_debug() {
    let client = reqwest::blocking::Client::builder()
        .user_agent("Mozilla/5.0 (compatible; Braille/0.1)")
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .unwrap();

    let resp = client.get("https://account.proton.me/signup").send().unwrap();
    let final_url = resp.url().to_string();
    let html = resp.text().unwrap();

    eprintln!("\n  Final URL: {final_url}");
    eprintln!("  HTML length: {} bytes", html.len());
    eprintln!("  HTML (first 2000 chars):");
    for line in html.lines().take(50) {
        eprintln!("    {line}");
    }

    let mut engine = Engine::new();
    let descriptors = engine.parse_and_collect_scripts(&html);
    eprintln!("\n  Script descriptors: {}", descriptors.len());
    for (i, d) in descriptors.iter().enumerate() {
        let url = d.external_url().map(|s| s.to_string()).unwrap_or_else(|| {
            if d.is_module() { "inline module".to_string() } else { "inline".to_string() }
        });
        eprintln!("    [{i}] {url}");
    }

    // Fetch scripts
    let import_map_urls = Engine::import_map_urls(&descriptors);
    let all_urls: Vec<String> = descriptors
        .iter()
        .filter_map(|d| d.external_url().map(|s| s.to_string()))
        .chain(import_map_urls.into_iter())
        .collect();
    let mut fetched_scripts = std::collections::HashMap::new();
    for src in &all_urls {
        let resolved = if src.starts_with("http://") || src.starts_with("https://") {
            src.to_string()
        } else if src.starts_with("//") {
            format!("https:{src}")
        } else if src.starts_with('/') {
            if let Ok(base) = url::Url::parse(&final_url) {
                format!("{}://{}{}", base.scheme(), base.host_str().unwrap_or(""), src)
            } else { continue; }
        } else if let Ok(base) = url::Url::parse(&final_url) {
            base.join(src).map(|u| u.to_string()).unwrap_or_default()
        } else { continue; };

        match client.get(&resolved).send() {
            Ok(resp) if resp.status().is_success() => {
                let body = resp.text().unwrap_or_default();
                eprintln!("  [fetched] {src} ({} bytes)", body.len());
                fetched_scripts.insert(src.to_string(), body);
            }
            Ok(resp) => eprintln!("  [script] {src} → HTTP {}", resp.status()),
            Err(e) => eprintln!("  [script] {src} → error: {e}"),
        }
    }

    let fetched = FetchedResources::scripts_only(fetched_scripts.clone());
    // Set URL before script execution so routers see correct pathname
    engine.set_url(&final_url);
    let errors = engine.execute_scripts_lossy(&descriptors, &fetched);
    engine.settle();
    service_fetches(&client, &mut engine, 10);

    // Check right after main flow
    let main_body = engine.eval_js("document.body ? document.body.childNodes.length : 'no body'");
    let main_app = engine.eval_js("document.querySelector('.app-root') ? 'FOUND' : 'MISSING'");
    let main_app_children = engine.eval_js("var r = document.querySelector('.app-root'); r ? r.childNodes.length : 'none'");
    eprintln!("  [main] right after flow: body={:?}, app-root={:?}, app-root children={:?}", main_body, main_app, main_app_children);

    // Check chunk structure — first/last chars of each script
    for src in &all_urls {
        if let Some(body) = fetched_scripts.get(src.as_str()) {
            let name = src.split('/').last().unwrap_or(src);
            eprintln!("\n  {name} ({} bytes):", body.len());
            eprintln!("    first 300: {}", &body[..body.len().min(300)]);
            let start = body.len().saturating_sub(300);
            eprintln!("    last 300: {}", &body[start..]);
        }
    }

    // Check if React was even loaded as a webpack module
    let react_check = engine.eval_js(r#"
        (function() {
            var info = [];
            // Check if __webpack_require__ is defined in some scope
            try { info.push('wpr: ' + typeof __webpack_require__); } catch(e) { info.push('wpr error: ' + e.message); }
            // Check if createRoot exists anywhere
            try {
                var chunk = webpackChunkproton_account;
                info.push('chunk count: ' + chunk.length);
                // Dump chunk structure
                for (var i = 0; i < Math.min(chunk.length, 3); i++) {
                    if (Array.isArray(chunk[i])) {
                        info.push('chunk[' + i + ']: ids=' + JSON.stringify(chunk[i][0]) + ' modules=' + (chunk[i][1] ? Object.keys(chunk[i][1]).length + ' modules' : 'none'));
                    } else {
                        info.push('chunk[' + i + ']: type=' + typeof chunk[i]);
                    }
                }
            } catch(e) { info.push('chunk error: ' + e.message); }
            return info.join('\n');
        })()
    "#);
    eprintln!("  [main] React check:\n{}", react_check.unwrap_or_default());

    // Extra settle rounds to catch MessageChannel-deferred work
    for round in 0..5 {
        engine.settle();
        let app_children = engine.eval_js("var r = document.querySelector('.app-root'); r ? r.childNodes.length : 0");
        eprintln!("  [main] settle round {round}: app-root children = {:?}", app_children);
        if app_children.as_deref() != Ok("0") { break; }
    }

    // Check DOMContentLoaded listeners
    let dcl = engine.eval_js("JSON.stringify(Object.keys(document.__listeners || {}))");
    eprintln!("  [main] document listeners: {:?}", dcl);

    // Check pending timers
    let timer_check = engine.eval_js("typeof __braille_timer_count === 'function' ? __braille_timer_count() : 'no timer fn'");
    eprintln!("  [main] timer count: {:?}", timer_check);

    // Check console
    let console_main = engine.drain_console();
    eprintln!("  [main] console: {} lines", console_main.len());
    for (i, line) in console_main.iter().enumerate().take(20) {
        eprintln!("    [{i}] {line}");
    }

    let main_compact = engine.snapshot(SnapMode::Compact);
    eprintln!("  [main] compact snapshot ({} chars):", main_compact.len());
    for line in main_compact.lines().take(20) {
        eprintln!("    {line}");
    }
    let main_text = engine.snapshot(SnapMode::Text);
    eprintln!("  [main] text snapshot ({} chars): {:?}", main_text.len(), &main_text[..main_text.len().min(300)]);

    eprintln!("\n  JS errors: {}", errors.len());
    for (i, err) in errors.iter().enumerate().take(20) {
        let truncated = if err.len() > 300 { &err[..300] } else { err };
        eprintln!("    [{i}] {truncated}");
    }

    // Check console
    let console = engine.drain_console();
    eprintln!("\n  Console output: {} lines", console.len());
    for (i, line) in console.iter().enumerate().take(30) {
        let truncated = if line.len() > 200 { &line[..200] } else { line };
        eprintln!("    [{i}] {truncated}");
    }

    let compact = engine.snapshot(SnapMode::Compact);
    eprintln!("\n  Compact snapshot ({} chars):", compact.len());
    for line in compact.lines().take(40) {
        eprintln!("    {line}");
    }

    // Check what's in app-root
    let app_root_check = engine.eval_js(
        "var r = document.querySelector('.app-root'); r ? 'found, children=' + r.childNodes.length + ', innerHTML=' + r.innerHTML.substring(0, 200) : 'NOT FOUND'"
    );
    eprintln!("\n  app-root: {:?}", app_root_check);

    // Check if DOMContentLoaded listeners exist
    let dcl_check = engine.eval_js(
        "typeof document.__listeners === 'object' ? JSON.stringify(Object.keys(document.__listeners)) : 'no __listeners'"
    );
    eprintln!("  doc listeners: {:?}", dcl_check);

    // Try firing DOMContentLoaded manually
    engine.eval_js(
        "document.dispatchEvent(new Event('DOMContentLoaded', {bubbles: true}));"
    ).ok();
    engine.settle();
    service_fetches(&client, &mut engine, 10);

    // Also fire window load
    engine.eval_js(
        "window.dispatchEvent(new Event('load'));"
    ).ok();
    engine.settle();
    service_fetches(&client, &mut engine, 10);

    let app_root_after = engine.eval_js(
        "var r = document.querySelector('.app-root'); r ? 'found, children=' + r.childNodes.length + ', innerHTML=' + r.innerHTML.substring(0, 500) : 'NOT FOUND'"
    );
    eprintln!("  app-root after DOMContentLoaded+load: {:?}", app_root_after);

    let compact2 = engine.snapshot(SnapMode::Compact);
    eprintln!("\n  Compact snapshot after events ({} chars):", compact2.len());
    for line in compact2.lines().take(40) {
        eprintln!("    {line}");
    }

    // Check console again
    let console2 = engine.drain_console();
    eprintln!("\n  Console after events: {} lines", console2.len());
    for (i, line) in console2.iter().enumerate().take(30) {
        let truncated = if line.len() > 200 { &line[..200] } else { line };
        eprintln!("    [{i}] {truncated}");
    }

    // Check readyState — test on a fresh engine to isolate
    let mut fresh = Engine::new();
    fresh.load_html("<html><body></body></html>");
    let fresh_rs = fresh.eval_js("document.readyState");
    eprintln!("  fresh engine readyState: {:?}", fresh_rs);

    let rs = engine.eval_js("document.readyState");
    eprintln!("  readyState: {:?}", rs);
    let rs2 = engine.eval_js("'readyState' in document");
    eprintln!("  'readyState' in document: {:?}", rs2);

    // Check location parsing works at all
    let loc_test = engine.eval_js("location.href = 'https://example.com/foo?bar=1#baz'; location.pathname + '|' + location.hostname + '|' + location.search + '|' + location.hash");
    eprintln!("  location parse test: {:?}", loc_test);
    // Reset URL
    engine.set_url(&final_url);
    let loc_after_set = engine.eval_js("location.href + '|' + location.pathname");
    eprintln!("  after set_url: {:?}", loc_after_set);

    // Deep probe: check what globals the bundle set up
    let probes = [
        "typeof React",
        "typeof ReactDOM",
        "typeof __webpack_require__",
        "typeof self",
        "typeof globalThis.webpackChunkproton_account",
        "typeof process",
        "typeof __webpack_modules__",
        "typeof window.__REDUX_DEVTOOLS_EXTENSION__",
        // Check if the bundle set any global error handler
        "typeof window.onerror",
        // Check document.readyState
        "document.readyState",
        // Check if there's a pending promise or async init
        "typeof Promise",
        // Check crypto
        "typeof crypto",
        "typeof crypto.subtle",
        "typeof crypto.getRandomValues",
        // TextEncoder/TextDecoder
        "typeof TextEncoder",
        "typeof TextDecoder",
        // Check structuredClone
        "typeof structuredClone",
        // Check if window.location.pathname is correct
        "window.location.pathname",
        "window.location.href",
    ];
    eprintln!("\n  Global probes:");
    for probe in &probes {
        let result = engine.eval_js(probe);
        eprintln!("    {probe} = {:?}", result);
    }

    // Try wrapping console.error to catch silent failures
    engine.eval_js(r#"
        var __errors = [];
        var __origError = console.error;
        console.error = function() {
            var msg = Array.prototype.slice.call(arguments).map(function(a) {
                return typeof a === 'object' ? JSON.stringify(a) : String(a);
            }).join(' ');
            __errors.push(msg);
            if (__origError) __origError.apply(console, arguments);
        };
        var __origWarn = console.warn;
        console.warn = function() {
            var msg = Array.prototype.slice.call(arguments).map(function(a) {
                return typeof a === 'object' ? JSON.stringify(a) : String(a);
            }).join(' ');
            __errors.push('[warn] ' + msg);
        };
    "#).ok();

    // Re-run the scripts with error trapping
    // Actually, let's just check what window.onerror catches
    engine.eval_js(r#"
        window.onerror = function(msg, url, line, col, error) {
            __errors.push('onerror: ' + msg + ' at ' + url + ':' + line + ':' + col);
        };
        window.addEventListener('unhandledrejection', function(e) {
            __errors.push('unhandled rejection: ' + String(e.reason));
        });
    "#).ok();

    // Fire DOMContentLoaded again with error trapping
    engine.eval_js("document.dispatchEvent(new Event('DOMContentLoaded', {bubbles: true}));").ok();
    engine.settle();
    service_fetches(&client, &mut engine, 5);
    engine.eval_js("window.dispatchEvent(new Event('load'));").ok();
    engine.settle();
    service_fetches(&client, &mut engine, 5);

    let trapped_errors = engine.eval_js("JSON.stringify(__errors.slice(0, 20))");
    eprintln!("\n  Trapped errors: {:?}", trapped_errors);

    // Check pending fetches
    let has_fetches = engine.has_pending_fetches();
    eprintln!("  has_pending_fetches: {has_fetches}");
    if has_fetches {
        let pending = engine.pending_fetches();
        eprintln!("  pending fetch count: {}", pending.len());
        for (i, req) in pending.iter().enumerate().take(10) {
            eprintln!("    [{i}] {} {}", req.method, req.url);
        }
        // Don't resolve them yet, just report
    }

    // Test: does a simple HTML with body children parse correctly?
    let mut test_engine = Engine::new();
    test_engine.load_html(r#"<html><body><div class="app-root"></div><div class="loader"></div><script>/* test */</script></body></html>"#);
    let test_snap = test_engine.eval_js("document.querySelector('.app-root') ? 'FOUND' : 'MISSING'");
    eprintln!("  simple parse test: {:?}", test_snap);

    // Test: parse_and_collect_scripts then check tree BEFORE running scripts
    let mut test_before = Engine::new();
    let descs_before = test_before.parse_and_collect_scripts(&html);
    eprintln!("  before scripts: {} descriptors", descs_before.len());
    // Create runtime manually to check tree state
    test_before.load_html(&html); // just to get a runtime
    let before_body = test_before.eval_js("document.body ? document.body.childNodes.length : 'no body'");
    eprintln!("  before scripts body childNodes: {:?}", before_body);

    // Full run with step-by-step checking
    {
        let mut test_full = Engine::new();
        test_full.parse_and_collect_scripts(&html);
        test_full.set_url(&final_url);
        let f = FetchedResources::scripts_only(fetched_scripts.clone());
        let errs = test_full.execute_scripts_lossy(&descriptors, &f);
        eprintln!("  [full] after execute_scripts_lossy: body = {:?}, errors = {}",
            test_full.eval_js("document.body ? document.body.childNodes.length : 'no'"),
            errs.len());
        test_full.settle();
        eprintln!("  [full] after settle: body = {:?}",
            test_full.eval_js("document.body ? document.body.childNodes.length : 'no'"));
        service_fetches(&client, &mut test_full, 10);
        eprintln!("  [full] after service_fetches: body = {:?}",
            test_full.eval_js("document.body ? document.body.childNodes.length : 'no'"));
        eprintln!("  [full] app-root: {:?}",
            test_full.eval_js("document.querySelector('.app-root') ? 'FOUND' : 'MISSING'"));
    }

    // Run each script individually on the same runtime to find which clears body
    for (i, desc) in descriptors.iter().enumerate() {
        let mut test_step = Engine::new();
        test_step.parse_and_collect_scripts(&html);
        test_step.set_url(&final_url);
        let partial_descs: Vec<_> = descriptors[..=i].to_vec();
        let f = FetchedResources::scripts_only(fetched_scripts.clone());
        test_step.execute_scripts_lossy(&partial_descs, &f);
        let body_count = test_step.eval_js("document.body ? document.body.childNodes.length : 'no body'");
        let url = desc.external_url().unwrap_or("inline");
        eprintln!("  after scripts [0..={i}] ({url}): body childNodes = {:?}", body_count);
    }

    // Now: parse + execute WITHOUT the actual scripts (empty fetched)
    let mut test_no_scripts = Engine::new();
    let descs_ns = test_no_scripts.parse_and_collect_scripts(&html);
    let empty_fetched = FetchedResources::scripts_only(std::collections::HashMap::new());
    test_no_scripts.execute_scripts_lossy(&descs_ns, &empty_fetched);
    let ns_body = test_no_scripts.eval_js("document.body ? document.body.childNodes.length : 'no body'");
    eprintln!("  no-scripts execute body childNodes: {:?}", ns_body);
    let ns_app = test_no_scripts.eval_js("document.querySelector('.app-root') ? 'FOUND' : 'MISSING'");
    eprintln!("  no-scripts app-root: {:?}", ns_app);

    // Test: parse actual ProtonMail HTML directly (no scripts)
    let mut test_actual = Engine::new();
    test_actual.load_html(&html);
    let actual_body = test_actual.eval_js("document.body ? document.body.childNodes.length : 'no body'");
    eprintln!("  actual HTML load_html body childNodes: {:?}", actual_body);
    let actual_app = test_actual.eval_js("document.querySelector('.app-root') ? 'FOUND' : 'MISSING'");
    eprintln!("  actual HTML load_html app-root: {:?}", actual_app);

    // Test with ProtonMail-like structure
    let mut test_proton = Engine::new();
    let proton_html = r#"<!doctype html><html lang="en-US"><head><meta charset="utf-8"></head><body><div class="app-root"></div><div class="app-root-loader"></div><noscript class="app-noscript">JS required</noscript><script defer="defer" src="/runtime.js"></script><script defer="defer" src="/main.js"></script></body></html>"#;
    let proton_descs = test_proton.parse_and_collect_scripts(proton_html);
    eprintln!("  proton-like descs: {}", proton_descs.len());
    let fetched_proton = FetchedResources::scripts_only(std::collections::HashMap::new());
    test_proton.execute_scripts_lossy(&proton_descs, &fetched_proton);
    let proton_body = test_proton.eval_js("document.body ? document.body.childNodes.length : 'no body'");
    eprintln!("  proton-like body childNodes: {:?}", proton_body);
    let proton_app = test_proton.eval_js("document.querySelector('.app-root') ? 'FOUND' : 'MISSING'");
    eprintln!("  proton-like app-root: {:?}", proton_app);

    // Test with parse_and_collect_scripts
    let mut test2 = Engine::new();
    let descs = test2.parse_and_collect_scripts(r#"<html><body><div class="app-root"></div><script>/*x*/</script></body></html>"#);
    let fetched2 = FetchedResources::scripts_only(std::collections::HashMap::new());
    test2.execute_scripts_lossy(&descs, &fetched2);
    let test2_snap = test2.eval_js("document.querySelector('.app-root') ? 'FOUND' : 'MISSING'");
    eprintln!("  parse_and_collect + execute_scripts test: {:?}", test2_snap);
    let test2_body = test2.eval_js("document.body ? document.body.childNodes.length : 'no body'");
    eprintln!("  test2 body childNodes: {:?}", test2_body);

    // Check native tree via JS bridge
    let native_check = engine.eval_js(r#"
        var bodyId = __n_getBodyId();
        var children = bodyId >= 0 ? __n_getAllChildIds(bodyId) : [];
        var info = 'bodyId=' + bodyId + ' children=[' + children.join(',') + ']';
        for (var i = 0; i < children.length; i++) {
            info += '\n  child ' + children[i] + ' type=' + __n_getNodeType(children[i]) + ' tag=' + __n_getTagName(children[i]);
        }
        // Also check total nodes by probing node 0 (document)
        var docChildren = __n_getAllChildIds(0);
        info += '\ndoc children=[' + docChildren.join(',') + ']';
        for (var j = 0; j < docChildren.length; j++) {
            info += '\n  doc child ' + docChildren[j] + ' type=' + __n_getNodeType(docChildren[j]) + ' tag=' + __n_getTagName(docChildren[j]);
            var gc = __n_getAllChildIds(docChildren[j]);
            for (var k = 0; k < gc.length; k++) {
                info += '\n    grandchild ' + gc[k] + ' type=' + __n_getNodeType(gc[k]) + ' tag=' + __n_getTagName(gc[k]);
            }
        }
        info
    "#);
    eprintln!("\n  Native tree: {:?}", native_check);

    // Direct tree check
    let body_nid = engine.eval_js("document.body ? document.body.__nid : 'no body'");
    eprintln!("  body.__nid: {:?}", body_nid);
    let body_tag = engine.eval_js("document.body ? document.body.tagName : 'none'");
    eprintln!("  body.tagName: {:?}", body_tag);
    let app_root_by_class = engine.eval_js("document.querySelectorAll('div').length");
    eprintln!("  all divs: {:?}", app_root_by_class);
    let html_el = engine.eval_js("document.documentElement ? document.documentElement.tagName : 'none'");
    eprintln!("  documentElement: {:?}", html_el);
    let tree_children = engine.eval_js("document.body ? __n_getAllChildIds(document.body.__nid).length : 'no body'");
    eprintln!("  body native children: {:?}", tree_children);

    // Check basic DOM access
    let dom_probe = engine.eval_js("document.querySelector('.app-root') ? 'found' : 'NOT FOUND'");
    eprintln!("  querySelector('.app-root'): {:?}", dom_probe);
    let dom_probe2 = engine.eval_js("document.querySelector('div') ? document.querySelector('div').className : 'no div'");
    eprintln!("  first div className: {:?}", dom_probe2);
    let dom_probe3 = engine.eval_js("document.body ? document.body.childNodes.length : 'no body'");
    eprintln!("  body childNodes: {:?}", dom_probe3);
    let dom_probe4 = engine.eval_js("document.body ? document.body.innerHTML.substring(0, 200) : 'no body'");
    eprintln!("  body innerHTML: {:?}", dom_probe4);

    // Check if the app has any error boundaries or caught errors
    let error_probe = engine.eval_js(r#"
        (function() {
            // Check if there are any unresolved promises
            var info = [];
            // Check window.__PROTON__
            info.push('__PROTON__: ' + typeof window.__PROTON__);
            // Check if createRoot was ever called
            info.push('_reactRootContainer: ' + typeof document.querySelector('.app-root')._reactRootContainer);
            // Check if ReactDOM is in any webpack module
            try {
                var chunk = webpackChunkproton_account;
                info.push('chunks: ' + chunk.length);
                // Try to look at first chunk structure
                if (chunk[0]) {
                    info.push('chunk0 type: ' + typeof chunk[0]);
                    if (Array.isArray(chunk[0])) {
                        info.push('chunk0 len: ' + chunk[0].length);
                        info.push('chunk0[0] type: ' + typeof chunk[0][0]);
                        info.push('chunk0[1] type: ' + typeof chunk[0][1]);
                    }
                }
            } catch(e) { info.push('error: ' + e.message); }
            return info.join('\n');
        })()
    "#);
    eprintln!("\n  Error probe: {:?}", error_probe);

    // Deep webpack probe
    let wp_probes = [
        "typeof webpackChunkproton_account",
        "Array.isArray(webpackChunkproton_account)",
        "webpackChunkproton_account.length",
        "typeof webpackChunkproton_account.push",
        // Check if it's been patched (webpack runtime replaces .push)
        "webpackChunkproton_account.push === Array.prototype.push",
        "typeof webpackChunkproton_account.push.toString().substring(0, 100)",
    ];
    eprintln!("\n  Webpack probes:");
    for probe in &wp_probes {
        let result = engine.eval_js(probe);
        eprintln!("    {probe} = {:?}", result);
    }
}
