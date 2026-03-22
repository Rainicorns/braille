//! Debug test: fetch real ProtonMail page and scripts, load them in the engine,
//! and interrogate the JS state to find what's failing.
//!
//! Run with: cargo test -p braille-engine --test proton_debug -- --nocapture --ignored

use std::collections::HashMap;
use braille_engine::{Engine, FetchedResources, ScriptDescriptor};

fn fetch_url(url: &str) -> Option<String> {
    let output = std::process::Command::new("curl")
        .args(["-sL", "--max-time", "15", url])
        .output()
        .ok()?;
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        None
    }
}

fn resolve_url(base: &str, src: &str) -> String {
    if src.starts_with("http://") || src.starts_with("https://") {
        return src.to_string();
    }
    if src.starts_with("//") {
        return format!("https:{src}");
    }
    if src.starts_with('/') {
        let origin = if let Some(idx) = base[8..].find('/') {
            &base[..8 + idx]
        } else {
            base
        };
        return format!("{origin}{src}");
    }
    format!("{base}/{src}")
}

#[test]
#[ignore] // requires network access
fn proton_login_debug() {
    let base_url = "https://account.proton.me";
    let page_url = format!("{base_url}/login");

    eprintln!("[1] Fetching page HTML...");
    let html = fetch_url(&page_url).expect("failed to fetch proton login page");
    eprintln!("    HTML length: {} bytes", html.len());

    let mut engine = Engine::new();
    let descriptors = engine.parse_and_collect_scripts(&html);

    eprintln!("[2] Found {} script descriptors:", descriptors.len());
    for (i, desc) in descriptors.iter().enumerate() {
        match desc {
            ScriptDescriptor::Inline(t) => eprintln!("    [{i}] Inline ({} bytes)", t.len()),
            ScriptDescriptor::External(u) => eprintln!("    [{i}] External: {u}"),
            ScriptDescriptor::InlineModule(t) => eprintln!("    [{i}] InlineModule ({} bytes)", t.len()),
            ScriptDescriptor::ExternalModule(u) => eprintln!("    [{i}] ExternalModule: {u}"),
            ScriptDescriptor::ImportMap(t) => eprintln!("    [{i}] ImportMap ({} bytes)", t.len()),
        }
    }

    // Fetch all external scripts
    eprintln!("[3] Fetching external scripts...");
    let mut scripts: HashMap<String, String> = HashMap::new();
    for desc in &descriptors {
        let url = match desc {
            ScriptDescriptor::External(u) | ScriptDescriptor::ExternalModule(u) => u,
            _ => continue,
        };
        let resolved = resolve_url(base_url, url);
        eprintln!("    Fetching: {resolved}");
        match fetch_url(&resolved) {
            Some(content) => {
                eprintln!("      -> {} bytes", content.len());
                scripts.insert(url.clone(), content);
            }
            None => {
                eprintln!("      -> FAILED");
            }
        }
    }

    engine.set_url(&page_url);

    eprintln!("[4] Executing scripts...");
    let errors = engine.execute_scripts_lossy(
        &descriptors,
        &FetchedResources::scripts_only(scripts),
    );
    eprintln!("    JS errors during initial execution: {}", errors.len());
    for (i, err) in errors.iter().enumerate() {
        eprintln!("    ERROR[{i}]: {}", &err[..err.len().min(300)]);
    }

    // Wrap ALL global native functions that take numeric args to trap the f64 error
    engine.eval_js(r#"
        self.__f64_trap_log = [];
        function wrapNative(name) {
            var orig = globalThis[name];
            if (typeof orig !== 'function') return;
            globalThis[name] = function() {
                for (var i = 0; i < arguments.length; i++) {
                    if (arguments[i] === undefined) {
                        __f64_trap_log.push(name + ' arg[' + i + ']=undefined');
                    }
                }
                try {
                    return orig.apply(this, arguments);
                } catch(e) {
                    __f64_trap_log.push(name + ' threw: ' + e.message);
                    throw e;
                }
            };
        }
        // Wrap all __braille_ and __n_ functions
        var globals = Object.getOwnPropertyNames(globalThis);
        for (var i = 0; i < globals.length; i++) {
            var n = globals[i];
            if ((n.indexOf('__braille_') === 0 || n.indexOf('__n_') === 0) && typeof globalThis[n] === 'function') {
                wrapNative(n);
            }
        }
    "#).unwrap();

    // Install error tracking
    engine.eval_js(r#"
        self.__proton_errors = [];
        self.__origConsoleError = console.error;
        console.error = function() {
            var msg = Array.prototype.slice.call(arguments).map(function(a) {
                if (a instanceof Error) return a.message + '\n' + (a.stack || '');
                return String(a);
            }).join(' ');
            __proton_errors.push(msg);
            __origConsoleError.apply(console, arguments);
        };
    "#).unwrap();

    // Settle
    engine.settle();

    eprintln!("[5] After initial settle...");
    if let Ok(errs) = engine.eval_js("JSON.stringify(__proton_errors.slice(0, 5))") {
        eprintln!("    console.error calls: {}", &errs[..errs.len().min(500)]);
    }
    if let Ok(te) = engine.eval_js("JSON.stringify(__braille_timer_errors.slice(0, 5))") {
        if te != "[]" { eprintln!("    timer errors: {}", &te[..te.len().min(500)]); }
    }

    // Resolve pending fetches (dynamic chunks)
    eprintln!("[6] Resolving pending fetches...");
    let pending = engine.pending_fetches();
    eprintln!("    {} pending fetches", pending.len());
    for pf in &pending {
        let short = pf.url.rsplit('/').next().unwrap_or(&pf.url);
        let body = fetch_url(&pf.url).unwrap_or_default();
        let ct = if pf.url.ends_with(".js") { "application/javascript" }
            else if pf.url.ends_with(".svg") { "image/svg+xml" }
            else if pf.url.ends_with(".json") { "application/json" }
            else { "text/plain" };
        eprintln!("    {} ({} bytes)", short, body.len());
        engine.resolve_fetch(pf.id, &braille_wire::FetchResponseData {
            url: pf.url.clone(),
            status: if body.is_empty() { 404 } else { 200 },
            status_text: "OK".to_string(),
            headers: vec![("content-type".to_string(), ct.to_string())],
            body,
        });
    }

    engine.settle();

    eprintln!("[7] After chunk resolution...");
    if let Ok(errs) = engine.eval_js("JSON.stringify(__proton_errors)") {
        eprintln!("    console.error calls: {}", &errs[..errs.len().min(1000)]);
    }
    if let Ok(te) = engine.eval_js("JSON.stringify(__braille_timer_errors)") {
        if te != "[]" { eprintln!("    timer errors: {}", &te[..te.len().min(1000)]); }
    }

    // Check for more pending fetches (API calls from the app)
    let more = engine.pending_fetches();
    eprintln!("    {} additional pending fetches", more.len());
    for pf in more.iter().take(10) {
        let short = &pf.url[..pf.url.len().min(100)];
        eprintln!("      {} {}", pf.method, short);
    }

    // Check DOM state
    if let Ok(r) = engine.eval_js("document.querySelectorAll('div').length") {
        eprintln!("    div count: {r}");
    }
    if let Ok(r) = engine.eval_js("(function() { var r = document.querySelector('.app-root'); return r ? 'children=' + r.children.length + ' text=' + r.textContent.substring(0, 200) : 'NOT FOUND'; })()") {
        eprintln!("    app-root: {}", &r[..r.len().min(300)]);
    }

    // Find the exact code causing "not a function"
    // The error is at E (eval_script:2320:60894) — that's inside PublicAppInteractive chunk
    // Let me extract the code around offset 60894 in that chunk
    eprintln!("\n[8] Finding error location...");
    if let Ok(r) = engine.eval_js(r#"
(function() {
    // The chunk scripts are evaled as strings. We can't easily get the source.
    // But we can look at the error more carefully by wrapping the failing path.
    // The error says "not a function" at E() — this is likely a DOM API call.
    // Let's check what common DOM methods might be missing or broken.
    var missing = [];
    var el = document.createElement('div');

    // Check common APIs that might be called as functions
    var checks = {
        'el.getAnimations': typeof el.getAnimations,
        'el.animate': typeof el.animate,
        'el.attachShadow': typeof el.attachShadow,
        'el.replaceChildren': typeof el.replaceChildren,
        'el.append': typeof el.append,
        'el.prepend': typeof el.prepend,
        'el.after': typeof el.after,
        'el.before': typeof el.before,
        'el.replaceWith': typeof el.replaceWith,
        'el.toggleAttribute': typeof el.toggleAttribute,
        'el.getAttributeNode': typeof el.getAttributeNode,
        'el.setAttributeNS': typeof el.setAttributeNS,
        'el.getAttributeNS': typeof el.getAttributeNS,
        'el.removeAttributeNS': typeof el.removeAttributeNS,
        'el.hasAttributeNS': typeof el.hasAttributeNS,
        'document.createTreeWalker': typeof document.createTreeWalker,
        'document.createNodeIterator': typeof document.createNodeIterator,
        'document.importNode': typeof document.importNode,
        'document.adoptNode': typeof document.adoptNode,
        'document.createEvent': typeof document.createEvent,
        'document.dispatchEvent': typeof document.dispatchEvent,
        'window.dispatchEvent': typeof window.dispatchEvent,
        'window.getComputedStyle': typeof window.getComputedStyle,
        'el.getElementsByTagName': typeof el.getElementsByTagName,
        'el.getElementsByClassName': typeof el.getElementsByClassName,
        'el.insertAdjacentHTML': typeof el.insertAdjacentHTML,
        'el.insertAdjacentElement': typeof el.insertAdjacentElement,
        'Node.prototype.isConnected': typeof Object.getOwnPropertyDescriptor(Object.getPrototypeOf(el), 'isConnected'),
    };

    for (var k in checks) {
        if (checks[k] !== 'function' && checks[k] !== 'undefined') {
            // Skip properties that are supposed to be non-function
        }
        if (checks[k] === 'undefined') {
            missing.push(k);
        }
    }
    return 'Missing methods: ' + missing.join(', ');
})()
"#) {
        eprintln!("    {r}");
    }

    // Try to instrument the failing function E to see what it's trying to call
    if let Ok(r) = engine.eval_js(r#"
(function() {
    // Wrap Function.prototype.call and apply to catch "not a function" errors
    var notFnCalls = [];
    var origCall = Function.prototype.call;
    // Can't easily wrap — too invasive. Instead, let's look at the timer error more carefully.

    // The stack trace includes line/column references into eval_script:2320
    // 2320 is the PublicAppInteractive chunk (857KB)
    // offset 60894 in that chunk — let's find what function E is

    // Check if there's a global E that shouldn't be
    if (typeof E === 'function') return 'Global E exists: ' + String(E).substring(0, 200);
    if (typeof E === 'undefined') return 'No global E';
    return 'E is: ' + typeof E;
})()
"#) {
        eprintln!("    {r}");
    }

    // The most likely cause: a DOM method that returns undefined instead of a function
    // React calls various DOM methods. Let's check what React-specific things are missing.
    if let Ok(r) = engine.eval_js(r#"
(function() {
    var el = document.createElement('div');
    document.body.appendChild(el);
    var missing = [];

    // React checks
    if (typeof el.append !== 'function') missing.push('Element.append');
    if (typeof el.prepend !== 'function') missing.push('Element.prepend');
    if (typeof el.replaceChildren !== 'function') missing.push('Element.replaceChildren');
    if (typeof el.after !== 'function') missing.push('Element.after');
    if (typeof el.before !== 'function') missing.push('Element.before');
    if (typeof el.replaceWith !== 'function') missing.push('Element.replaceWith');
    if (typeof el.toggleAttribute !== 'function') missing.push('Element.toggleAttribute');

    // React DOM internals
    if (typeof el.setAttributeNS !== 'function') missing.push('Element.setAttributeNS');
    if (typeof el.removeAttributeNS !== 'function') missing.push('Element.removeAttributeNS');

    // Document methods
    if (typeof document.createTreeWalker !== 'function') missing.push('document.createTreeWalker');
    if (typeof document.importNode !== 'function') missing.push('document.importNode');
    if (typeof document.dispatchEvent !== 'function') missing.push('document.dispatchEvent');

    // Window
    if (typeof window.dispatchEvent !== 'function') missing.push('window.dispatchEvent');

    // DOMTokenList (classList)
    var cl = el.classList;
    if (cl) {
        if (typeof cl.add !== 'function') missing.push('classList.add');
        if (typeof cl.remove !== 'function') missing.push('classList.remove');
        if (typeof cl.contains !== 'function') missing.push('classList.contains');
        if (typeof cl.forEach !== 'function') missing.push('classList.forEach');
    } else {
        missing.push('classList itself');
    }

    // NodeList
    var nl = document.querySelectorAll('div');
    if (typeof nl.forEach !== 'function') missing.push('NodeList.forEach');
    if (typeof nl.item !== 'function') missing.push('NodeList.item');
    if (typeof nl.entries !== 'function') missing.push('NodeList.entries');

    // Array-like methods on NodeList/HTMLCollection
    if (typeof nl.indexOf !== 'function') missing.push('NodeList.indexOf (array method)');

    document.body.removeChild(el);
    return 'Missing (' + missing.length + '): ' + missing.join(', ');
})()
"#) {
        eprintln!("    {r}");
    }

    // Resolve more pending fetches (API calls)
    eprintln!("\n[9] Resolving API fetches...");
    let api_fetches = engine.pending_fetches();
    for pf in &api_fetches {
        let short = &pf.url[..pf.url.len().min(120)];
        eprintln!("    {} {}", pf.method, short);
        // Resolve with realistic responses
        let (status, body) = if pf.url.contains("/api/") {
            // API calls — return empty/error responses
            (200, r#"{"Code":1000}"#.to_string())
        } else {
            (404, String::new())
        };
        engine.resolve_fetch(pf.id, &braille_wire::FetchResponseData {
            url: pf.url.clone(),
            status,
            status_text: "OK".to_string(),
            headers: vec![("content-type".to_string(), "application/json".to_string())],
            body,
        });
    }
    engine.settle();

    // Check for more
    let more2 = engine.pending_fetches();
    eprintln!("    {} more pending after API resolution", more2.len());
    for pf in more2.iter().take(5) {
        eprintln!("      {} {}", pf.method, &pf.url[..pf.url.len().min(120)]);
    }

    if let Ok(errs) = engine.eval_js("JSON.stringify(__proton_errors.slice(-3))") {
        eprintln!("    latest errors: {}", &errs[..errs.len().min(500)]);
    }

    // Resolve MORE pending fetches iteratively
    for wave in 0..5 {
        let pf = engine.pending_fetches();
        if pf.is_empty() { break; }
        eprintln!("\n[wave {wave}] {} pending", pf.len());
        for p in &pf {
            eprintln!("    {} {}", p.method, &p.url[..p.url.len().min(120)]);
            let body = if p.url.contains("/api/") {
                // Return realistic API responses
                if p.url.contains("/core/v4/settings") || p.url.contains("/core/v4/features") {
                    r#"{"Code":1000,"Features":[]}"#.to_string()
                } else if p.url.contains("sentry") {
                    String::new()
                } else {
                    r#"{"Code":1000}"#.to_string()
                }
            } else {
                fetch_url(&p.url).unwrap_or_default()
            };
            engine.resolve_fetch(p.id, &braille_wire::FetchResponseData {
                url: p.url.clone(),
                status: 200,
                status_text: "OK".to_string(),
                headers: vec![("content-type".to_string(), "application/json".to_string())],
                body,
            });
        }
        engine.settle();
    }

    if let Ok(trap) = engine.eval_js("JSON.stringify(__f64_trap_log)") {
        if trap != "[]" { eprintln!("\n    f64 trap log: {trap}"); }
    }
    if let Ok(errs) = engine.eval_js("JSON.stringify(__proton_errors.slice(-2))") {
        eprintln!("\n    final errors: {}", &errs[..errs.len().min(500)]);
    }
    if let Ok(te) = engine.eval_js("JSON.stringify(__braille_timer_errors.slice(-2))") {
        if te != "[]" { eprintln!("    final timer errors: {}", &te[..te.len().min(500)]); }
    }

    let snapshot = engine.snapshot(braille_wire::SnapMode::Compact);
    eprintln!("\n[SNAPSHOT] {}", &snapshot[..snapshot.len().min(800)]);
}
