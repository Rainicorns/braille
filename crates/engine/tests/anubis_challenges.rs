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
