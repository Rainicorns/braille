//! Honest TDD tests for Anubis challenge support.
//!
//! These tests use the ACTUAL Anubis code patterns — no workarounds.
//! Tests are expected to FAIL until Braille implements the missing features.
//! Each failing test represents a concrete gap to fix.
//!
//! Based on white-box analysis of https://github.com/TecharoHQ/anubis

use braille_engine::Engine;

// =========================================================================
// CHALLENGE TYPE 1: MetaRefresh
//
// Simplest challenge. No JS execution needed.
// Server sends <meta http-equiv="refresh" content="DELAY; url=REDIRECT">
// or a Refresh HTTP header.
// =========================================================================

/// MetaRefresh: detect redirect URL from meta tag
#[test]
fn metarefresh_detect_meta_tag() {
    let html = r#"<!doctype html><html><head>
        <title>Making sure you're not a bot!</title>
    </head><body>
        <h1>Making sure you're not a bot!</h1>
        <div class="centered-div">
            <p id="status">Loading...</p>
            <meta http-equiv="refresh" content="2; url=/.within.website/x/cmd/anubis/api/pass-challenge?challenge=deadbeef01234567&amp;id=test-id-001&amp;redir=%2F">
        </div>
    </body></html>"#;

    let mut engine = Engine::new();
    engine.load_html(html);

    let refresh = engine.check_meta_refresh(Some("https://example.com"));
    assert!(refresh.is_some(), "should detect meta refresh tag");

    let mr = refresh.unwrap();
    assert_eq!(mr.delay_seconds, 2);
    let url = mr.url.expect("should have redirect URL");
    assert!(url.contains("pass-challenge"), "URL should contain pass-challenge: {}", url);
    assert!(url.contains("challenge=deadbeef01234567"), "URL should contain challenge data: {}", url);
    assert!(url.contains("id=test-id-001"), "URL should contain challenge ID: {}", url);
    // &amp; should be decoded by html5ever during parsing
    assert!(url.contains("&id="), "HTML entities should be decoded: {}", url);
}

/// MetaRefresh: when randomData[0] % 2 != 0, Anubis sends Refresh HTTP header instead.
/// Engine correctly returns None — CLI is responsible for checking the header.
#[test]
fn metarefresh_no_meta_tag_returns_none() {
    let html = r#"<!doctype html><html><head>
        <title>Making sure you're not a bot!</title>
    </head><body>
        <h1>Making sure you're not a bot!</h1>
        <p id="status">Loading...</p>
    </body></html>"#;

    let mut engine = Engine::new();
    engine.load_html(html);

    let refresh = engine.check_meta_refresh(None);
    assert!(refresh.is_none());
}

// =========================================================================
// CHALLENGE TYPE 2: Preact
//
// Server sends:
//   - <script id="preact_info" type="application/json"> with {redir, challenge, difficulty}
//   - Bundled Preact app as <script type="module"> (ES module!)
//
// The app:
//   1. Reads JSON via JSON.parse(document.getElementById('preact_info').textContent)
//   2. Computes SHA256(challenge) using @aws-crypto/sha256-js (Sha256 class)
//   3. Waits difficulty * 125ms
//   4. Redirects to redir + ?result=SHA256_HEX
//
// Server validates: SHA256(storedChallenge.randomData) == request.result
// =========================================================================

/// Preact: the actual Anubis page uses <script type="module">.
/// This tests whether Braille executes ES module scripts at all.
#[test]
fn preact_es_module_script_executes() {
    let html = r#"<!doctype html><html><head></head><body>
        <p id="result">not executed</p>
        <script type="module">
            document.getElementById('result').textContent = 'module executed';
        </script>
    </body></html>"#;

    let mut engine = Engine::new();
    engine.load_html(html);
    engine.settle();

    let result = engine.eval_js("document.getElementById('result').textContent").unwrap();
    assert_eq!(result, "module executed", "ES module scripts should execute");
}

/// Preact: ES module with import/export (the real Preact app does this)
#[test]
fn preact_es_module_import_export() {
    // Anubis bundles everything into one <script type="module">, but it uses
    // import syntax internally. This tests if the engine handles it.
    let html = r#"<!doctype html><html><head></head><body>
        <p id="result">not executed</p>
        <script type="module">
            // Simulates the pattern: const x = (() => { ... })(); export default x;
            const toHexString = (arr) => Array.from(arr).map(c => c.toString(16).padStart(2, '0')).join('');
            document.getElementById('result').textContent = toHexString(new Uint8Array([0xde, 0xad]));
        </script>
    </body></html>"#;

    let mut engine = Engine::new();
    engine.load_html(html);
    engine.settle();

    let result = engine.eval_js("document.getElementById('result').textContent").unwrap();
    assert_eq!(result, "dead", "ES module with const/arrow functions should work");
}

/// Preact: the actual solver reads challenge JSON and computes SHA-256.
/// This is the EXACT flow from app.tsx, using the real Anubis HTML structure.
#[test]
fn preact_full_solver_flow() {
    // This matches the actual Anubis preact challenge page structure.
    // The <script type="module"> contains the solver logic inlined
    // (in production it's bundled from app.tsx + preact + @aws-crypto/sha256-js)
    let html = r#"<!doctype html><html><head>
        <script id="preact_info" type="application/json">{"redir":"https://example.com/.within.website/x/cmd/anubis/api/pass-challenge?id=test-001&redir=%2F","challenge":"hello","difficulty":1,"loading_message":"Loading...","connection_security_message":"Please wait.","pensive_url":"/img/pensive.webp"}</script>
    </head><body>
        <div id="app"><p id="status">Loading...</p></div>
        <script type="module">
            // This is what the real Anubis Preact app does (simplified, no Preact rendering):
            // 1. Read challenge data
            var info = JSON.parse(document.getElementById('preact_info').textContent);

            // 2. Compute SHA-256 (real app uses @aws-crypto/sha256-js Sha256 class,
            //    but crypto.subtle.digest is equivalent)
            var encoder = new TextEncoder();
            var data = encoder.encode(info.challenge);
            crypto.subtle.digest('SHA-256', data).then(function(hashBuffer) {
                var hashArray = new Uint8Array(hashBuffer);
                var hex = '';
                for (var i = 0; i < hashArray.length; i++) {
                    hex += hashArray[i].toString(16).padStart(2, '0');
                }

                // 3. Construct redirect URL (real app uses: u(redir, {result: hash}))
                var url = new URL(info.redir);
                url.searchParams.set('result', hex);

                // 4. Redirect (real app sets window.location.href)
                document.getElementById('status').textContent = 'redirect:' + url.toString();
            });
        </script>
    </body></html>"#;

    let mut engine = Engine::new();
    engine.load_html(html);
    engine.settle();

    let status = engine.eval_js("document.getElementById('status').textContent").unwrap();

    // SHA-256("hello") = 2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824
    assert!(status.starts_with("redirect:"), "solver should complete and set redirect URL: {}", status);
    assert!(
        status.contains("result=2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"),
        "result should be SHA-256('hello'): {}", status
    );
}

/// Preact: new URL() with relative path requires a valid base URL.
/// Anubis does: new URL(info.redir, window.location.href)
/// When Braille loads HTML without a URL, window.location.href is "about:blank".
#[test]
fn preact_url_resolution_with_base() {
    let html = r#"<html><body>
        <p id="result">pending</p>
        <script>
            var url = new URL('/.within.website/api/test?id=123&redir=%2F', window.location.href);
            document.getElementById('result').textContent = url.toString();
        </script>
    </body></html>"#;

    let mut engine = Engine::new();
    engine.load_html(html);

    let result = engine.eval_js("document.getElementById('result').textContent").unwrap();
    // With about:blank as base, relative URLs should still resolve somehow.
    // Real browsers resolve against the current page URL.
    assert!(result.contains("api/test"), "relative URL should resolve: {}", result);
    assert!(result.contains("id=123"), "query params should be preserved: {}", result);
}

// =========================================================================
// CHALLENGE TYPE 3: Proof-of-Work (fast/slow)
//
// Server sends HTML with:
//   - <script id="anubis_challenge" type="application/json"> with challenge data
//   - <script async type="module" src="/.../main.mjs"> (external ES module!)
//
// main.mjs:
//   1. Checks dependencies (Web Workers, Cookies)
//   2. Reads challenge from anubis_challenge JSON
//   3. Spawns N Web Workers (navigator.hardwareConcurrency / 2)
//   4. Each worker brute-forces: SHA256(randomData + nonce) until N leading zeros
//   5. Worker uses either crypto.subtle (secure context) or @aws-crypto/sha256-js
//   6. Main thread redirects to pass-challenge with {response: hash, nonce, elapsedTime}
//
// Server validates:
//   - SHA256(randomData + nonce) == response
//   - response has `difficulty` leading zero hex chars
// =========================================================================

/// PoW: the challenge page loads an external ES module via <script async type="module" src="...">
#[test]
fn pow_external_es_module_loads() {
    // Anubis PoW uses an external module script. This tests if the engine
    // even recognizes and attempts to load it.
    let html = r#"<!doctype html><html><head></head><body>
        <p id="status">Loading...</p>
        <script async type="module" src="/static/js/main.mjs"></script>
    </body></html>"#;

    let mut engine = Engine::new();
    let descriptors = engine.parse_and_collect_scripts(html);

    // Should recognize the external module script
    let has_module = descriptors.iter().any(|d| d.external_url().is_some());
    assert!(has_module, "should detect external module script descriptor");
}

/// PoW: the main.mjs requires Web Workers (new Worker(url))
#[test]
fn pow_web_workers_exist() {
    let html = "<html><body></body></html>";
    let mut engine = Engine::new();
    engine.load_html(html);

    // Anubis checks: window.Worker as a dependency
    let result = engine.eval_js("typeof Worker").unwrap();
    assert_eq!(result, "function", "Web Worker constructor should exist");
}

/// PoW: the main.mjs creates workers and posts messages
#[test]
fn pow_web_worker_basic_functionality() {
    let html = r#"<html><body>
        <p id="result">pending</p>
        <script>
            // Anubis creates workers like this:
            // var worker = new Worker(webWorkerURL);
            // worker.onmessage = (event) => { ... };
            // worker.postMessage({data, difficulty, nonce: 0, threads: 1});
            try {
                var worker = new Worker('data:text/javascript,postMessage("hello")');
                worker.onmessage = function(e) {
                    document.getElementById('result').textContent = 'got:' + e.data;
                };
            } catch(e) {
                document.getElementById('result').textContent = 'error:' + e.message;
            }
        </script>
    </body></html>"#;

    let mut engine = Engine::new();
    engine.load_html(html);
    engine.settle();

    let result = engine.eval_js("document.getElementById('result').textContent").unwrap();
    assert_eq!(result, "got:hello", "Web Worker should post message back to main thread");
}

/// PoW: full proof-of-work solver (inline, no workers — tests the math)
/// This is NOT how Anubis runs it (uses Workers), but verifies the hash computation works.
#[test]
fn pow_sha256_nonce_search_inline() {
    let html = r#"<html><body>
        <p id="result">pending</p>
        <script>
            // Replicate the PoW worker logic inline (Anubis uses Web Workers)
            var challenge = "testchallenge";
            var difficulty = 1;
            var encoder = new TextEncoder();

            async function solve() {
                for (var nonce = 0; nonce < 1000000; nonce++) {
                    var data = encoder.encode(challenge + nonce);
                    var hashBuffer = await crypto.subtle.digest('SHA-256', data);
                    var hashArray = new Uint8Array(hashBuffer);

                    // difficulty=1 means first hex char is '0' (first nibble is 0)
                    if (hashArray[0] >> 4 === 0) {
                        var hex = '';
                        for (var i = 0; i < hashArray.length; i++) {
                            hex += hashArray[i].toString(16).padStart(2, '0');
                        }
                        document.getElementById('result').textContent = 'nonce=' + nonce + ',hash=' + hex;
                        return;
                    }
                }
                document.getElementById('result').textContent = 'exhausted';
            }
            solve();
        </script>
    </body></html>"#;

    let mut engine = Engine::new();
    engine.load_html(html);
    engine.settle();

    let result = engine.eval_js("document.getElementById('result').textContent").unwrap();
    assert!(result.starts_with("nonce="), "should find a valid nonce: {}", result);

    // Verify the hash actually starts with '0'
    let hash = result.split(",hash=").nth(1).unwrap_or("");
    assert!(hash.starts_with("0"), "hash should have 1 leading zero: {}", hash);
}

// =========================================================================
// GENERAL: APIs that Anubis depends on
// =========================================================================

/// Anubis main.mjs checks window.Worker as a hard dependency
#[test]
fn anubis_dependency_check_worker() {
    let html = "<html><body></body></html>";
    let mut engine = Engine::new();
    engine.load_html(html);

    let result = engine.eval_js("!!window.Worker").unwrap();
    assert_eq!(result, "true", "window.Worker should be truthy (Anubis checks this)");
}

/// Anubis main.mjs checks navigator.cookieEnabled as a hard dependency
#[test]
fn anubis_dependency_check_cookies() {
    let html = "<html><body></body></html>";
    let mut engine = Engine::new();
    engine.load_html(html);

    let result = engine.eval_js("!!navigator.cookieEnabled").unwrap();
    assert_eq!(result, "true", "navigator.cookieEnabled should be true (Anubis checks this)");
}

/// Anubis uses navigator.hardwareConcurrency to determine worker thread count
#[test]
fn anubis_dependency_hardware_concurrency() {
    let html = "<html><body></body></html>";
    let mut engine = Engine::new();
    engine.load_html(html);

    let result = engine.eval_js("typeof navigator.hardwareConcurrency").unwrap();
    assert_eq!(result, "number", "navigator.hardwareConcurrency should be a number");

    let value = engine.eval_js("navigator.hardwareConcurrency > 0").unwrap();
    assert_eq!(value, "true", "should be positive");
}

/// Anubis uses window.isSecureContext to choose between webcrypto and purejs workers
#[test]
fn anubis_dependency_secure_context() {
    let html = "<html><body></body></html>";
    let mut engine = Engine::new();
    engine.load_html(html);

    let result = engine.eval_js("typeof window.isSecureContext").unwrap();
    assert_eq!(result, "boolean", "window.isSecureContext should be a boolean");
}

/// Anubis uses document.documentElement.lang for localization
#[test]
fn anubis_dependency_document_lang() {
    let html = r#"<!doctype html><html lang="en"><body></body></html>"#;
    let mut engine = Engine::new();
    engine.load_html(html);

    let result = engine.eval_js("document.documentElement.lang").unwrap();
    assert_eq!(result, "en");
}

// =========================================================================
// CHALLENGE TYPE 3 — INTEGRATION: Full PoW flow with external module + Worker
//
// This mirrors the ACTUAL Anubis PoW path from white-box analysis of
// /tmp/anubis/web/js/{main.ts, algorithms/fast.ts, worker/sha256-webcrypto.ts}
//
// The real flow:
//   1. HTML has <script id="anubis_challenge" type="application/json">
//   2. <script async type="module" src="/static/js/main.mjs"> loads
//   3. main.mjs reads challenge JSON, checks deps, spawns Worker
//   4. Worker brute-forces SHA256(randomData + nonce) for leading zeros
//   5. Worker postMessages result back
//   6. main.mjs redirects to pass-challenge URL
//
// We provide the external scripts via FetchedResources (same as WPT tests).
// The JS is plain ES5 (transpiled from the TypeScript source) to match what
// esbuild would produce.
// =========================================================================

/// PoW integration: full Anubis PoW flow with external module + worker
#[test]
fn pow_full_flow_external_module_and_worker() {
    use braille_engine::FetchedResources;
    use std::collections::HashMap;

    // --- The HTML: actual Anubis structure ---
    let html = r#"<!doctype html><html lang="en"><head>
        <script id="anubis_version" type="application/json">"v1.0.0-test"</script>
        <script id="anubis_challenge" type="application/json">{
            "rules":{"algorithm":"fast","difficulty":1},
            "challenge":{
                "id":"test-challenge-001",
                "randomData":"testchallenge",
                "difficulty":1
            }
        }</script>
        <script id="anubis_base_prefix" type="application/json">""</script>
    </head><body>
        <h1 id="title">Making sure you're not a bot!</h1>
        <div class="centered-div">
            <p id="status">Loading...</p>
            <div id="progress" style="display:none"><div class="bar-inner"></div></div>
        </div>
        <script async type="module" src="/static/js/main.mjs"></script>
    </body></html>"#;

    // --- main.mjs: transpiled from main.ts + fast.ts ---
    // Reads challenge JSON, spawns worker, handles result, sets redirect.
    // Simplified: skips i18n fetch (not relevant to the challenge flow).
    let main_mjs = r#"
        // Helpers from main.ts
        var u = function(url, params) {
            var result = new URL(url, window.location.href);
            var keys = Object.keys(params || {});
            for (var i = 0; i < keys.length; i++) {
                result.searchParams.set(keys[i], params[keys[i]]);
            }
            return result.toString();
        };

        var j = function(id) {
            var elem = document.getElementById(id);
            if (elem === null) return null;
            return JSON.parse(elem.textContent);
        };

        // Dependency checks (from main.ts lines 102-113)
        var status = document.getElementById('status');
        if (!window.Worker) { status.textContent = 'error:no-workers'; }
        if (!navigator.cookieEnabled) { status.textContent = 'error:no-cookies'; }

        var challengeData = j('anubis_challenge');
        var basePrefix = j('anubis_base_prefix') || '';
        var challenge = challengeData.challenge;
        var rules = challengeData.rules;

        // Worker URL (from fast.ts line 39) — use webcrypto variant
        var workerURL = basePrefix + '/.within.website/x/cmd/anubis/static/js/worker/sha256-webcrypto.mjs';

        // Spawn worker (from fast.ts lines 68-93)
        var worker = new Worker(workerURL);
        worker.onmessage = function(event) {
            if (typeof event.data === 'number') {
                // Progress update — ignore for test
                return;
            }
            // Got result (from main.ts lines 263-272)
            var hash = event.data.hash;
            var nonce = event.data.nonce;
            var redir = window.location.href;
            var redirectURL = u(basePrefix + '/.within.website/x/cmd/anubis/api/pass-challenge', {
                id: challenge.id,
                response: hash,
                nonce: nonce,
                redir: redir,
                elapsedTime: 1
            });
            status.textContent = 'redirect:' + redirectURL;
        };
        worker.onerror = function(event) {
            status.textContent = 'error:worker-failed:' + String(event.message || event);
        };

        // Post challenge to worker (from fast.ts lines 85-89)
        worker.postMessage({
            data: challenge.randomData,
            difficulty: rules.difficulty,
            nonce: 0,
            threads: 1
        });
    "#;

    // --- Worker script: transpiled from sha256-webcrypto.ts ---
    let worker_mjs = r#"
        var encoder = new TextEncoder();

        addEventListener('message', async function(e) {
            var data = e.data.data;
            var difficulty = e.data.difficulty;
            var threads = e.data.threads;
            var nonce = e.data.nonce;
            var isMainThread = nonce === 0;
            var iterations = 0;

            var requiredZeroBytes = Math.floor(difficulty / 2);
            var isDifficultyOdd = difficulty % 2 !== 0;

            for (;;) {
                var hashBuffer = await crypto.subtle.digest('SHA-256', encoder.encode(data + nonce));
                var hashArray = new Uint8Array(hashBuffer);

                var isValid = true;
                for (var i = 0; i < requiredZeroBytes; i++) {
                    if (hashArray[i] !== 0) { isValid = false; break; }
                }
                if (isValid && isDifficultyOdd) {
                    if (hashArray[requiredZeroBytes] >> 4 !== 0) { isValid = false; }
                }

                if (isValid) {
                    var hex = '';
                    for (var i = 0; i < hashArray.length; i++) {
                        hex += hashArray[i].toString(16).padStart(2, '0');
                    }
                    postMessage({ hash: hex, data: data, difficulty: difficulty, nonce: nonce });
                    return;
                }

                nonce += threads;
                iterations++;

                if (isMainThread && (iterations & 1023) === 0) {
                    postMessage(nonce);
                }
            }
        });
    "#;

    let mut scripts = HashMap::new();
    scripts.insert("/static/js/main.mjs".to_string(), main_mjs.to_string());
    scripts.insert(
        "/.within.website/x/cmd/anubis/static/js/worker/sha256-webcrypto.mjs".to_string(),
        worker_mjs.to_string(),
    );
    let resources = FetchedResources {
        scripts,
        iframes: HashMap::new(),
    };

    let mut engine = Engine::new();
    let errors = engine.load_html_with_resources_lossy(html, &resources);
    engine.settle();

    // Debug
    let loc = engine.eval_js("location.origin").unwrap();
    eprintln!("location.origin: {}", loc);
    let resolved = engine.eval_js("location.origin + '/.within.website/x/cmd/anubis/static/js/worker/sha256-webcrypto.mjs'").unwrap();
    eprintln!("Resolved worker URL: {}", resolved);

    let status = engine.eval_js("document.getElementById('status').textContent").unwrap();

    // If we got JS errors, include them in the failure message
    let err_ctx = if errors.is_empty() {
        String::new()
    } else {
        format!(" (JS errors: {:?})", errors.iter().map(|e| &e[..e.len().min(200)]).collect::<Vec<_>>())
    };

    assert!(
        status.starts_with("redirect:"),
        "PoW solver should complete and set redirect URL, got: {}{}",
        status, err_ctx
    );
    assert!(
        status.contains("id=test-challenge-001"),
        "redirect should include challenge ID: {}",
        status
    );
    // Extract the response hash from the URL
    let response_param = status.split("response=").nth(1).unwrap_or("");
    let response_hash = response_param.split('&').next().unwrap_or("");
    assert!(
        response_hash.starts_with("0"),
        "response hash should start with 0 (difficulty=1): {}",
        status
    );
    assert!(
        status.contains("nonce="),
        "redirect should include nonce: {}",
        status
    );
}

// =========================================================================
// END-TO-END: MetaRefresh challenge → follow redirect → arrive at destination
//
// This reproduces the ACTUAL live site flow:
//   1. GET / → Anubis returns challenge HTML with meta refresh
//   2. Engine loads HTML, detects meta refresh tag
//   3. Navigation layer follows the redirect URL
//   4. Server validates challenge, sets cookie, redirects to final page
//
// In unit tests we can't hit a real server, but we CAN verify that the
// engine correctly extracts the redirect URL from the challenge HTML —
// which is what the binary's fetch_and_load_inner does at line 419-428.
// =========================================================================

/// End-to-end: simulate the FULL binary goto flow for both metarefresh variants.
/// The binary does: check_refresh_header(headers) || engine.check_meta_refresh()
/// This test exercises BOTH paths with real Anubis HTML/headers.
#[test]
fn metarefresh_full_goto_flow_both_variants() {
    use braille_engine::check_refresh_header;

    let base_url = "https://anubis.techaro.lol/";

    // --- Variant 1: Refresh HTTP header (no meta tag) ---
    // This is what the live site returns when randomData[0] % 2 != 0
    let headers_v1: Vec<(String, String)> = vec![
        ("content-type".into(), "text/html; charset=utf-8".into()),
        ("refresh".into(), "2; url=/.within.website/x/cmd/anubis/api/pass-challenge?challenge=e923bdb21a302af7&id=test-001&redir=%2F".into()),
        ("set-cookie".into(), "techaro.lol-anubis-cookie-verification=test-001; Path=/".into()),
    ];
    let html_v1 = r#"<!doctype html><html lang="en"><head>
        <title>Making sure you're not a bot!</title>
        <script id="anubis_challenge" type="application/json">{"rules":{"algorithm":"metarefresh","difficulty":1},"challenge":{"id":"test-001","randomData":"e923bdb21a302af7"}}</script>
    </head><body>
        <h1>Making sure you're not a bot!</h1>
        <p id="status">Loading...</p>
    </body></html>"#;

    let mut engine_v1 = Engine::new();
    engine_v1.load_html(html_v1);

    // Binary flow: check header first, then meta tag
    let refresh_v1 = check_refresh_header(&headers_v1, Some(base_url))
        .or_else(|| engine_v1.check_meta_refresh(Some(base_url)));

    assert!(refresh_v1.is_some(), "Variant 1 (Refresh header): should find redirect");
    let url_v1 = refresh_v1.unwrap().url.expect("should have URL");
    assert!(url_v1.contains("pass-challenge"), "should redirect to pass-challenge: {}", url_v1);
    assert!(url_v1.starts_with("https://anubis.techaro.lol/"), "should be absolute: {}", url_v1);

    // --- Variant 2: meta tag (no Refresh header) ---
    // This is what the live site returns when randomData[0] % 2 == 0
    let headers_v2: Vec<(String, String)> = vec![
        ("content-type".into(), "text/html; charset=utf-8".into()),
    ];
    let html_v2 = r#"<!doctype html><html lang="en"><head>
        <title>Making sure you're not a bot!</title>
    </head><body>
        <h1>Making sure you're not a bot!</h1>
        <div class="centered-div">
            <p id="status">Loading...</p>
            <meta http-equiv="refresh" content="2; url=/.within.website/x/cmd/anubis/api/pass-challenge?challenge=b48a6ef3981a6b2f&amp;id=test-002&amp;redir=%2F">
        </div>
    </body></html>"#;

    let mut engine_v2 = Engine::new();
    engine_v2.load_html(html_v2);

    let refresh_v2 = check_refresh_header(&headers_v2, Some(base_url))
        .or_else(|| engine_v2.check_meta_refresh(Some(base_url)));

    assert!(refresh_v2.is_some(), "Variant 2 (meta tag): should find redirect");
    let url_v2 = refresh_v2.unwrap().url.expect("should have URL");
    assert!(url_v2.contains("pass-challenge"), "should redirect to pass-challenge: {}", url_v2);
    assert!(url_v2.starts_with("https://anubis.techaro.lol/"), "should be absolute: {}", url_v2);
}

/// MetaRefresh end-to-end: load real Anubis challenge HTML, extract redirect URL
/// This uses the ACTUAL HTML structure captured from the live site.
#[test]
fn metarefresh_end_to_end_from_captured_html() {
    // This is the actual HTML from https://anubis.techaro.lol/ (captured)
    // Algorithm: metarefresh, difficulty: 1
    // The meta refresh tag is inside the page body (Anubis places it there
    // when randomData[0] % 2 == 0)
    let html = r#"<!doctype html><html lang="en"><head>
        <title>Making sure you&#39;re not a bot!</title>
        <script id="anubis_version" type="application/json">"v1.25.0-test"</script>
        <script id="anubis_challenge" type="application/json">{"rules":{"algorithm":"metarefresh","difficulty":1},"challenge":{"id":"test-meta-001","method":"metarefresh","randomData":"b48a6ef3981a6b2fcf4b3cbf5479e9d6","difficulty":1}}</script>
    </head><body>
        <h1 id="title">Making sure you're not a bot!</h1>
        <div class="centered-div">
            <p id="status">Loading...</p>
            <meta http-equiv="refresh" content="2; url=/.within.website/x/cmd/anubis/api/pass-challenge?challenge=b48a6ef3981a6b2fcf4b3cbf5479e9d6&amp;id=test-meta-001&amp;redir=%2F">
        </div>
    </body></html>"#;

    let mut engine = Engine::new();
    engine.load_html(html);

    // This is what the binary does: check_meta_refresh with the page URL as base
    let refresh = engine.check_meta_refresh(Some("https://anubis.techaro.lol/"));
    assert!(refresh.is_some(), "should detect meta refresh from Anubis challenge page");

    let mr = refresh.unwrap();
    let url = mr.url.expect("meta refresh should have a URL");

    // The URL should be absolute (resolved against the base)
    assert!(
        url.starts_with("https://anubis.techaro.lol/"),
        "redirect URL should be resolved to absolute: {}",
        url
    );
    assert!(
        url.contains("pass-challenge"),
        "redirect should go to pass-challenge endpoint: {}",
        url
    );
    assert!(
        url.contains("challenge=b48a6ef3981a6b2fcf4b3cbf5479e9d6"),
        "redirect should include challenge data: {}",
        url
    );
    assert!(
        url.contains("&id=test-meta-001"),
        "HTML entities should be decoded and id param present: {}",
        url
    );
    assert!(
        url.contains("&redir=%2F"),
        "redirect should include redir param: {}",
        url
    );
}

/// MetaRefresh: when the challenge uses Refresh HTTP header instead of meta tag,
/// the engine returns None but the binary checks the header separately.
/// This tests that the engine doesn't false-positive on pages WITHOUT the meta tag.
#[test]
fn metarefresh_no_meta_tag_header_only_variant() {
    // Same Anubis structure but WITHOUT the meta refresh tag
    // (randomData[0] % 2 != 0 → server sends Refresh HTTP header instead)
    let html = r#"<!doctype html><html lang="en"><head>
        <title>Making sure you&#39;re not a bot!</title>
        <script id="anubis_challenge" type="application/json">{"rules":{"algorithm":"metarefresh","difficulty":1},"challenge":{"id":"test-meta-002","method":"metarefresh","randomData":"37ede6f3e523453c","difficulty":1}}</script>
    </head><body>
        <h1>Making sure you're not a bot!</h1>
        <p id="status">Loading...</p>
    </body></html>"#;

    let mut engine = Engine::new();
    engine.load_html(html);

    let refresh = engine.check_meta_refresh(Some("https://anubis.techaro.lol/"));
    assert!(
        refresh.is_none(),
        "should NOT detect meta refresh when tag is absent (header-only variant)"
    );
}

/// End-to-end: simulate the binary's goto flow for Anubis metarefresh.
/// The binary fetches the page, gets HTML + headers, loads into engine,
/// checks for Refresh header OR meta tag, and follows the redirect.
/// This test uses the real captured response from the live site.
#[test]
fn metarefresh_end_to_end_with_refresh_header() {
    // Simulate: the live site returns a Refresh HTTP header (no meta tag in body)
    let headers: Vec<(String, String)> = vec![
        ("content-type".into(), "text/html; charset=utf-8".into()),
        ("refresh".into(), "2; url=/.within.website/x/cmd/anubis/api/pass-challenge?challenge=e923bdb21a302af7&id=test-hdr-001&redir=%2F".into()),
    ];
    let html = r#"<!doctype html><html lang="en"><head>
        <title>Making sure you're not a bot!</title>
        <script id="anubis_challenge" type="application/json">{"rules":{"algorithm":"metarefresh","difficulty":1},"challenge":{"id":"test-hdr-001","randomData":"e923bdb21a302af7","difficulty":1}}</script>
    </head><body>
        <h1>Making sure you're not a bot!</h1>
        <p id="status">Loading...</p>
    </body></html>"#;

    let base_url = "https://anubis.techaro.lol/";

    let mut engine = Engine::new();
    engine.load_html(html);

    // Step 1: check meta tag — should be None (header-only variant)
    let meta_refresh = engine.check_meta_refresh(Some(base_url));
    assert!(meta_refresh.is_none(), "no meta tag in this variant");

    // Step 2: check Refresh header — this is what the binary does
    // The binary calls check_refresh_header() which is internal to the binary,
    // so we replicate the logic here.
    let refresh_header = headers.iter().find(|(k, _)| k.eq_ignore_ascii_case("refresh"));
    assert!(refresh_header.is_some(), "Refresh header should be present");

    let (_, value) = refresh_header.unwrap();
    // Parse: "2; url=/.within.website/..."
    let semicolon = value.find(';').expect("should have semicolon");
    let delay: u32 = value[..semicolon].trim().parse().unwrap();
    assert_eq!(delay, 2);

    let url_part = value[semicolon + 1..].trim();
    assert!(url_part.to_lowercase().starts_with("url="), "should start with url=: {}", url_part);
    let relative_url = &url_part[4..];

    // Resolve relative URL against base
    let base = url::Url::parse(base_url).unwrap();
    let resolved = base.join(relative_url).unwrap().to_string();

    assert!(
        resolved.starts_with("https://anubis.techaro.lol/.within.website/"),
        "should resolve to absolute URL: {}",
        resolved
    );
    assert!(
        resolved.contains("pass-challenge"),
        "should point to pass-challenge: {}",
        resolved
    );
    assert!(
        resolved.contains("id=test-hdr-001"),
        "should include challenge id: {}",
        resolved
    );
}

/// Normal page should not be detected as Anubis
#[test]
fn non_anubis_page_not_detected() {
    let html = r#"<!doctype html><html><head><title>Normal Page</title></head>
        <body><h1>Hello World</h1></body></html>"#;

    let mut engine = Engine::new();
    engine.load_html(html);

    let is_anubis = engine.eval_js(
        "document.getElementById('anubis_challenge') !== null ? 'true' : 'false'"
    ).unwrap();
    assert_eq!(is_anubis, "false");

    let refresh = engine.check_meta_refresh(None);
    assert!(refresh.is_none());
}

// =========================================================================
// END-TO-END: Full navigation flow tests using Engine::navigate() + MockFetcher
//
// These exercise the COMPLETE navigation loop that was previously only in
// the binary: fetch page → parse → fetch scripts → execute → settle →
// check meta refresh → follow redirect → snapshot.
// =========================================================================

use braille_engine::MockFetcher;
use braille_wire::SnapMode;

/// E2E navigate: MetaRefresh via meta tag → follow redirect → arrive at real page
#[test]
fn navigate_metarefresh_meta_tag_e2e() {
    let challenge_html = r#"<!doctype html><html lang="en"><head>
        <title>Making sure you're not a bot!</title>
    </head><body>
        <h1>Making sure you're not a bot!</h1>
        <div class="centered-div">
            <p id="status">Loading...</p>
            <meta http-equiv="refresh" content="2; url=/.within.website/x/cmd/anubis/api/pass-challenge?challenge=abc123&amp;id=test-001&amp;redir=%2F">
        </div>
    </body></html>"#;

    let real_page_html = r#"<!doctype html><html><head><title>Real Page</title></head>
        <body><h1>Welcome to the real page</h1><p>You passed the challenge.</p></body></html>"#;

    let mut fetcher = MockFetcher::new();
    fetcher.add_html("https://example.com/", challenge_html);
    fetcher.add_html(
        "https://example.com/.within.website/x/cmd/anubis/api/pass-challenge?challenge=abc123&id=test-001&redir=%2F",
        real_page_html,
    );

    let mut engine = Engine::new();
    let snapshot = engine.navigate("https://example.com/", &mut fetcher, SnapMode::Text).unwrap();
    assert!(snapshot.contains("Welcome to the real page"), "should arrive at real page: {}", snapshot);
}

/// E2E navigate: MetaRefresh via Refresh HTTP header → follow redirect → arrive at real page
#[test]
fn navigate_metarefresh_http_header_e2e() {
    let challenge_html = r#"<!doctype html><html lang="en"><head>
        <title>Making sure you're not a bot!</title>
    </head><body>
        <h1>Making sure you're not a bot!</h1>
        <p id="status">Loading...</p>
    </body></html>"#;

    let real_page_html = r#"<!doctype html><html><head><title>Real Page</title></head>
        <body><h1>Welcome to the real page</h1></body></html>"#;

    let mut fetcher = MockFetcher::new();
    fetcher.add_with_headers(
        "https://example.com/",
        challenge_html,
        vec![
            ("content-type".into(), "text/html; charset=utf-8".into()),
            ("refresh".into(), "2; url=/.within.website/x/cmd/anubis/api/pass-challenge?challenge=deadbeef&id=hdr-001&redir=%2F".into()),
        ],
    );
    fetcher.add_html(
        "https://example.com/.within.website/x/cmd/anubis/api/pass-challenge?challenge=deadbeef&id=hdr-001&redir=%2F",
        real_page_html,
    );

    let mut engine = Engine::new();
    let snapshot = engine.navigate("https://example.com/", &mut fetcher, SnapMode::Text).unwrap();
    assert!(snapshot.contains("Welcome to the real page"), "should arrive at real page: {}", snapshot);
}

/// E2E navigate: too many redirects should error
#[test]
fn navigate_too_many_redirects() {
    let redirect_html = r#"<!doctype html><html><head>
        <meta http-equiv="refresh" content="0; url=/loop">
    </head><body>Redirecting...</body></html>"#;

    let mut fetcher = MockFetcher::new();
    // Every URL returns a redirect to /loop
    fetcher.add_html("https://example.com/start", redirect_html);
    for i in 0..10 {
        let _ = i;
        fetcher.add_html("https://example.com/loop", redirect_html);
    }

    let mut engine = Engine::new();
    let result = engine.navigate("https://example.com/start", &mut fetcher, SnapMode::Text);
    assert!(result.is_err(), "should error on too many redirects");
    assert!(result.unwrap_err().contains("too many"), "error should mention too many redirects");
}

/// E2E navigate: PoW challenge with external module + worker scripts
#[test]
fn navigate_pow_with_external_scripts_e2e() {
    let challenge_html = r#"<!doctype html><html lang="en"><head>
        <script id="anubis_challenge" type="application/json">{
            "rules":{"algorithm":"fast","difficulty":1},
            "challenge":{"id":"pow-001","randomData":"testchallenge","difficulty":1}
        }</script>
    </head><body>
        <h1>Making sure you're not a bot!</h1>
        <p id="status">Loading...</p>
        <script async type="module" src="/static/js/main.mjs"></script>
    </body></html>"#;

    // Simplified main.mjs that does inline PoW (no workers for this test)
    let main_mjs = r#"
        var info = JSON.parse(document.getElementById('anubis_challenge').textContent);
        var challenge = info.challenge;
        var status = document.getElementById('status');

        var encoder = new TextEncoder();
        async function solve() {
            for (var nonce = 0; nonce < 1000000; nonce++) {
                var data = encoder.encode(challenge.randomData + nonce);
                var hashBuffer = await crypto.subtle.digest('SHA-256', data);
                var hashArray = new Uint8Array(hashBuffer);
                if (hashArray[0] >> 4 === 0) {
                    var hex = '';
                    for (var i = 0; i < hashArray.length; i++) {
                        hex += hashArray[i].toString(16).padStart(2, '0');
                    }
                    status.textContent = 'solved:nonce=' + nonce + ',hash=' + hex;
                    return;
                }
            }
        }
        solve();
    "#;

    let mut fetcher = MockFetcher::new();
    fetcher.add_html("https://example.com/", challenge_html);
    fetcher.add(
        "/static/js/main.mjs",
        braille_wire::FetchResponseData {
            status: 200,
            status_text: "OK".into(),
            headers: vec![("content-type".into(), "application/javascript".into())],
            body: main_mjs.into(),
            url: "/static/js/main.mjs".into(),
            redirect_chain: vec![],
        },
    );

    let mut engine = Engine::new();
    let _snapshot = engine.navigate("https://example.com/", &mut fetcher, SnapMode::Text).unwrap();

    // The page should show the solved result
    let status = engine.eval_js("document.getElementById('status').textContent").unwrap();
    assert!(status.starts_with("solved:nonce="), "PoW should solve: {}", status);
}

/// E2E navigate: simple page with no redirects
#[test]
fn navigate_simple_page() {
    let html = r#"<!doctype html><html><head><title>Hello</title></head>
        <body><h1>Hello World</h1><p>This is a test page.</p></body></html>"#;

    let mut fetcher = MockFetcher::new();
    fetcher.add_html("https://example.com/", html);

    let mut engine = Engine::new();
    let snapshot = engine.navigate("https://example.com/", &mut fetcher, SnapMode::Text).unwrap();
    assert!(snapshot.contains("Hello World"), "should render page: {}", snapshot);
}

// =========================================================================
// FAITHFUL LIVE SITE REPRODUCTION
//
// This test models the EXACT HTTP exchange observed on the live site via curl.
// It uses the real HTML, real headers, and real cookie flow.
//
// Live flow (from curl -sD - "https://anubis.techaro.lol/"):
//   1. GET / → 200, Refresh header, Set-Cookie (verification + clear auth), challenge HTML
//   2. GET /pass-challenge?... (with verification cookie) → server validates,
//      sets auth cookie, 302 → /
//   3. GET / (with auth cookie) → 200, real page
//
// In the real CLI, reqwest follows the 302 in step 2, so the engine sees
// the FINAL response from step 3 as the result of fetching the pass-challenge URL.
// The MockFetcher models this by returning the real page directly for the
// pass-challenge URL (simulating what the engine sees after reqwest follows 302).
//
// If this test PASSES but the live site FAILS, the bug is in the CLI's
// HTTP/cookie layer, not in the engine's navigation logic.
// If this test FAILS, the bug is in the engine.
// =========================================================================

/// Faithful reproduction: Anubis PoW challenge end-to-end via navigate().
///
/// From white-box analysis of Anubis source + daemon log from live site:
///   - Server returns PoW challenge (algorithm "fast"), NOT metarefresh
///   - HTML has <script async type="module" src="main.mjs"> (from proofofwork.templ)
///   - No Refresh header, no <meta http-equiv="refresh">
///   - main.mjs reads challenge JSON, spawns Worker, Worker brute-forces SHA256
///   - On success, main.mjs does window.location.replace(pass-challenge-url)
///   - pass-challenge validates, sets JWT cookie, 302 → real page
///
/// This test uses the actual Anubis HTML structure and JS logic.
/// The Worker script is provided via FetchedResources (same as populate_worker_scripts).
#[test]
fn navigate_anubis_pow_e2e() {
    // --- HTML: from Anubis base template + proofofwork.templ ---
    // The proofofwork template adds main.mjs; base template has JSON data blocks
    let challenge_html = r#"<!doctype html><html lang="en"><head>
        <title>Making sure you&#39;re not a bot!</title>
        <meta name="robots" content="noindex,nofollow">
        <script id="anubis_version" type="application/json">"v1.25.0-test"</script>
        <script id="anubis_challenge" type="application/json">{"rules":{"algorithm":"fast","difficulty":1},"challenge":{"id":"pow-e2e-001","randomData":"testchallenge","difficulty":1}}</script>
        <script id="anubis_base_prefix" type="application/json">""</script>
        <script id="anubis_public_url" type="application/json">""</script>
    </head><body>
        <script type="ignore"><a href="/honeypot/fake">Don't click me</a></script>
        <main>
        <h1 id="title">Making sure you're not a bot!</h1>
        <div class="centered-div">
            <img id="image" style="width:100%;max-width:256px;" src="/img/pensive.webp"/>
            <p id="status">Loading...</p>
            <script async type="module" src="/.within.website/x/cmd/anubis/static/js/main.mjs?cacheBuster=v1.25.0-test"></script>
            <div id="progress" role="progressbar"><div class="bar-inner"></div></div>
        </div>
        </main>
    </body></html>"#;

    // --- main.mjs: simplified from real Anubis main.mjs (fast algorithm path) ---
    // Real main.mjs: loads translations, checks deps, spawns Worker, redirects on success
    // Simplified: skip i18n fetch, inline the PoW instead of Worker (tests the solve + redirect)
    let main_mjs = r#"
        var j = function(id) {
            var elem = document.getElementById(id);
            if (elem === null) return null;
            return JSON.parse(elem.textContent);
        };
        var u = function(url, params) {
            var result = new URL(url, window.location.href);
            var keys = Object.keys(params || {});
            for (var i = 0; i < keys.length; i++) {
                result.searchParams.set(keys[i], params[keys[i]]);
            }
            return result.toString();
        };

        var status = document.getElementById('status');
        var challengeData = j('anubis_challenge');
        var basePrefix = j('anubis_base_prefix') || '';
        var challenge = challengeData.challenge;
        var rules = challengeData.rules;

        // Inline PoW solver (real main.mjs uses Workers)
        var encoder = new TextEncoder();
        async function solve() {
            for (var nonce = 0; nonce < 10000000; nonce++) {
                var data = encoder.encode(challenge.randomData + nonce);
                var hashBuffer = await crypto.subtle.digest('SHA-256', data);
                var hashArray = new Uint8Array(hashBuffer);

                // Check leading zero hex chars
                var requiredZeroBytes = Math.floor(rules.difficulty / 2);
                var isDifficultyOdd = rules.difficulty % 2 !== 0;
                var isValid = true;
                for (var i = 0; i < requiredZeroBytes; i++) {
                    if (hashArray[i] !== 0) { isValid = false; break; }
                }
                if (isValid && isDifficultyOdd) {
                    if (hashArray[requiredZeroBytes] >> 4 !== 0) { isValid = false; }
                }

                if (isValid) {
                    var hex = '';
                    for (var i = 0; i < hashArray.length; i++) {
                        hex += hashArray[i].toString(16).padStart(2, '0');
                    }
                    // Real main.mjs does: window.location.replace(redirectURL)
                    var redir = window.location.href;
                    var redirectURL = u(basePrefix + '/.within.website/x/cmd/anubis/api/pass-challenge', {
                        id: challenge.id,
                        response: hex,
                        nonce: nonce,
                        redir: redir,
                        elapsedTime: 1
                    });
                    status.textContent = 'redirect:' + redirectURL;
                    window.location.replace(redirectURL);
                    return;
                }
            }
            status.textContent = 'exhausted';
        }
        solve();
    "#;

    let mut fetcher = MockFetcher::new();
    fetcher.add_with_headers(
        "https://anubis.example.com/",
        challenge_html,
        vec![
            ("content-type".into(), "text/html; charset=utf-8".into()),
            ("cache-control".into(), "no-store".into()),
            ("set-cookie".into(), "test-anubis-cookie-verification=pow-e2e-001; Path=/; Secure".into()),
        ],
    );
    fetcher.add(
        "/.within.website/x/cmd/anubis/static/js/main.mjs?cacheBuster=v1.25.0-test",
        braille_wire::FetchResponseData {
            status: 200,
            status_text: "OK".into(),
            headers: vec![("content-type".into(), "application/javascript".into())],
            body: main_mjs.into(),
            url: "https://anubis.example.com/.within.website/x/cmd/anubis/static/js/main.mjs?cacheBuster=v1.25.0-test".into(),
            redirect_chain: vec![],
        },
    );
    let mut engine = Engine::new();
    let _snapshot = engine.navigate(
        "https://anubis.example.com/",
        &mut fetcher,
        SnapMode::Text,
    ).unwrap();

    // Check that the solver ran and computed a redirect
    let status = engine.eval_js("document.getElementById('status').textContent").unwrap();
    assert!(
        status.starts_with("redirect:"),
        "PoW solver should complete. Got status: {}",
        status
    );
    assert!(
        status.contains("id=pow-e2e-001"),
        "redirect should include challenge ID: {}",
        status
    );
    assert!(
        status.contains("response=0"),
        "response hash should start with 0 (difficulty=1): {}",
        status
    );

    // TODO: window.location.replace() is not yet intercepted by navigate().
    // Once it is, the engine should follow the redirect to pass-challenge,
    // which returns 302 → real page. For now we verify the solver works.
}

/// Faithful reproduction: exact Anubis metarefresh flow from live site.
/// Models the real HTTP response headers and HTML captured via curl.
#[test]
fn navigate_anubis_live_metarefresh_faithful() {
    // Exact challenge HTML from curl (trimmed CSS/meta but structurally identical)
    let challenge_html = r#"<!doctype html><html lang="en"><head>
        <title>Making sure you&#39;re not a bot!</title>
        <meta name="robots" content="noindex,nofollow">
        <script id="anubis_version" type="application/json">"v1.25.0-30-g3acf8ee"</script>
        <script id="anubis_challenge" type="application/json">{"rules":{"algorithm":"metarefresh","difficulty":1},"challenge":{"issuedAt":"2026-03-28T15:14:40.036144729Z","metadata":{},"id":"019d3502-f724-721e-9ed8-f801c9ee1b3d","method":"metarefresh","randomData":"5901bf72a34159b0","policyRuleHash":"ac980f49c4d35fab","difficulty":1,"spent":false}}</script>
        <script id="anubis_base_prefix" type="application/json">""</script>
        <script id="anubis_public_url" type="application/json">""</script>
    </head><body>
        <h1 id="title">Making sure you're not a bot!</h1>
        <div class="centered-div">
            <p id="status">Loading...</p>
            <div id="progress" style="display:none"><div class="bar-inner"></div></div>
        </div>
        <p>Please wait a moment while we ensure the security of your connection.</p>
        <footer>Protected by Anubis</footer>
        <script async type="module" src="/.within.website/x/cmd/anubis/static/js/main.mjs"></script>
    </body></html>"#;

    // Exact headers from curl response (the important ones)
    let challenge_headers = vec![
        ("content-type".into(), "text/html; charset=utf-8".into()),
        ("cache-control".into(), "no-store".into()),
        // This is the key: Refresh header with the pass-challenge redirect
        ("refresh".into(), "2; url=/.within.website/x/cmd/anubis/api/pass-challenge?challenge=5901bf72a34159b0&id=019d3502-f724-721e-9ed8-f801c9ee1b3d&redir=%2F".into()),
        // Server clears any existing auth cookie
        ("set-cookie".into(), "techaro.lol-anubis-auth=; Path=/; Expires=Thu, 01 Jan 1970 00:00:00 GMT; Max-Age=0; Secure; SameSite=None".into()),
        // Server sets verification cookie (must be sent back with pass-challenge)
        ("set-cookie".into(), "techaro.lol-anubis-cookie-verification=019d3502-f724-721e-9ed8-f801c9ee1b3d; Path=/; Secure; SameSite=None".into()),
    ];

    // The real page you'd see after passing the challenge
    let real_page_html = r#"<!doctype html><html lang="en"><head>
        <title>Techaro - Welcome</title>
    </head><body>
        <h1>Welcome to Techaro</h1>
        <p>You have passed the Anubis challenge.</p>
    </body></html>"#;

    // The pass-challenge URL (after resolving the relative URL against the base)
    let pass_challenge_url = "https://anubis.techaro.lol/.within.website/x/cmd/anubis/api/pass-challenge?challenge=5901bf72a34159b0&id=019d3502-f724-721e-9ed8-f801c9ee1b3d&redir=%2F";

    // In the real CLI, reqwest follows the 302 from pass-challenge back to /.
    // The engine sees the FINAL response. Model that here: the pass-challenge
    // URL returns the real page (as if reqwest already followed the 302).
    let pass_challenge_headers = vec![
        ("content-type".into(), "text/html; charset=utf-8".into()),
        ("set-cookie".into(), "techaro.lol-anubis-auth=valid-token; Path=/; Secure; SameSite=None".into()),
    ];

    let mut fetcher = MockFetcher::new();
    fetcher.add_with_headers(
        "https://anubis.techaro.lol/",
        challenge_html,
        challenge_headers,
    );
    fetcher.add_with_headers(
        pass_challenge_url,
        real_page_html,
        pass_challenge_headers,
    );
    // main.mjs fetch will fail (404) — that's fine, metarefresh doesn't need JS
    // The engine should follow the Refresh header BEFORE the JS solver completes

    let mut engine = Engine::new();
    let snapshot = engine.navigate(
        "https://anubis.techaro.lol/",
        &mut fetcher,
        SnapMode::Text,
    ).unwrap();

    assert!(
        snapshot.contains("Welcome to Techaro"),
        "Engine should follow Refresh header redirect and arrive at real page.\n\
         Got: {}",
        snapshot
    );
    assert!(
        !snapshot.contains("Loading..."),
        "Should NOT still be on the challenge page.\n\
         Got: {}",
        snapshot
    );
}
