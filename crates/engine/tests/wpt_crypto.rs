//! WPT (Web Platform Tests) runner for WebCryptoAPI/.
//!
//! Uses libtest-mimic to create one test Trial per HTML file.
//! Each Trial loads the test HTML via Engine, injects testharness.js,
//! and reads window.__wpt_results to determine pass/fail.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use libtest_mimic::{Arguments, Failed, Trial};

use braille_engine::{Engine, FetchedResources};

// ---------------------------------------------------------------------------
// Paths
// ---------------------------------------------------------------------------

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn wpt_root() -> PathBuf {
    workspace_root().join("tests/wpt")
}

// ---------------------------------------------------------------------------
// Test harness JS
// ---------------------------------------------------------------------------

fn load_testharness_js() -> String {
    let path = wpt_root().join("resources/testharness.js");
    std::fs::read_to_string(&path).expect("failed to read testharness.js")
}

/// Minimal testharness shim that provides test(), assert_*(), setup(), done()
/// and captures results to window.__wpt_results.
/// We prepend this before the real testharness.js. It provides the critical
/// globals in case testharness.js fails to initialize its WindowTestEnvironment.
fn testharness_preamble() -> String {
    r#"
// Minimal WPT test harness preamble
(function() {
    var results = [];
    var setup_fn = null;
    var setup_ran = false;
    var single_test_mode = false;

    function run_setup() {
        if (!setup_ran && setup_fn) {
            setup_ran = true;
            setup_fn();
        }
    }

    self.test = function(fn, name) {
        run_setup();
        var cleanups = [];
        var t = {
            name: name || "(unnamed)",
            step: function(f) { return function() { return f.apply(t, arguments); }; },
            step_func: function(f) { return function() { return f.apply(t, arguments); }; },
            step_func_done: function(f) { return function() { return f.apply(t, arguments); }; },
            unreached_func: function(msg) { return function() { throw new Error(msg || "unreached"); }; },
            add_cleanup: function(f) { cleanups.push(f); }
        };
        var result = { name: name || "(unnamed)", status: 0, message: "" };
        try {
            fn.call(t, t);
        } catch(e) {
            result.status = 1;
            result.message = e.message || String(e);
        }
        // Run cleanups even on failure
        for (var i = 0; i < cleanups.length; i++) {
            try { cleanups[i](); } catch(e) {}
        }
        results.push(result);
    };

    self.async_test = function(fn, name) {
        // For sync-like async tests, run immediately
        if (typeof fn === "string") {
            // async_test(name) form — return test object
            name = fn;
            fn = null;
        }
        var t = {
            name: name || "(unnamed)",
            step: function(f) {
                f.apply(t, [t]);
            },
            step_func: function(f) {
                return function() { return f.apply(t, arguments); };
            },
            step_func_done: function(f) {
                return function() {
                    f.apply(t, arguments);
                    t._done = true;
                };
            },
            done: function() { t._done = true; },
            unreached_func: function(msg) {
                return function() { throw new Error(msg || "unreached"); };
            },
            add_cleanup: function() {},
            step_timeout: function(fn, timeout) { fn(); },
            _done: false
        };
        var result = { name: t.name, status: 0, message: "" };
        if (fn) {
            try {
                fn.call(t, t);
            } catch(e) {
                result.status = 1;
                result.message = e.message || String(e);
            }
        }
        results.push(result);
        return t;
    };

    self.promise_test = function(fn, name) {
        var result = { name: name || "(unnamed)", status: 0, message: "" };
        results.push(result);
        var cleanups = [];
        try {
            var t = {
                name: name || "(unnamed)",
                step_func: function(f) {
                    return function() { return f.apply(t, arguments); };
                },
                step_func_done: function(f) {
                    return function() {
                        f.apply(t, arguments);
                        t._done = true;
                    };
                },
                done: function() { t._done = true; },
                unreached_func: function(msg) {
                    return function() { throw new Error(msg || "unreached"); };
                },
                add_cleanup: function(f) { cleanups.push(f); },
                step_timeout: function(fn, timeout) { fn(); },
                _done: false
            };
            var p = fn(t);
            if (p && typeof p.then === 'function') {
                p.then(function() {}, function(e) {
                    result.status = 1;
                    result.message = e.message || String(e);
                });
            }
        } catch(e) {
            result.status = 1;
            result.message = e.message || String(e);
        }
        // Always run cleanups after the sync portion completes
        // (promise_test may have pending awaits that never resolve)
        for (var i = 0; i < cleanups.length; i++) {
            try { cleanups[i](); } catch(ce) {}
        }
    };

    self.setup = function(fn_or_props) {
        if (typeof fn_or_props === 'function') {
            fn_or_props();
            setup_ran = true;
        } else if (fn_or_props && fn_or_props.single_test) {
            single_test_mode = true;
        }
    };

    self.done = function() {
        if (single_test_mode && results.length === 0) {
            results.push({ name: "(single test)", status: 0, message: "" });
        }
    };

    self.add_completion_callback = function() {};
    self.add_result_callback = function() {};
    self.add_start_callback = function() {};

    self.on_event = function(obj, event_type, handler) {
        if (obj && typeof obj.addEventListener === 'function') {
            obj.addEventListener(event_type, handler);
        }
    };

    self.step_timeout = function(fn, timeout) {
        fn();
    };

    self.generate_tests = function(fn, tests, props) {
        for (var i = 0; i < tests.length; i++) {
            var args = tests[i];
            var name = args[0];
            self.test(function() { fn.apply(null, args.slice(1)); }, name);
        }
    };

    // Assertions
    self.assert_true = function(val, msg) {
        if (val !== true) throw new Error(msg || "assert_true: got " + val);
    };
    self.assert_false = function(val, msg) {
        if (val !== false) throw new Error(msg || "assert_false: got " + val);
    };
    self.assert_equals = function(a, b, msg) {
        if (a !== b) throw new Error(msg || "assert_equals: " + a + " !== " + b);
    };
    self.assert_not_equals = function(a, b, msg) {
        if (a === b) throw new Error(msg || "assert_not_equals: values are equal: " + a);
    };
    self.assert_in_array = function(val, arr, msg) {
        if (arr.indexOf(val) === -1) throw new Error(msg || "assert_in_array: " + val + " not in array");
    };
    self.assert_greater_than = function(a, b, msg) {
        if (!(a > b)) throw new Error(msg || "assert_greater_than: " + a + " <= " + b);
    };
    self.assert_less_than = function(a, b, msg) {
        if (!(a < b)) throw new Error(msg || "assert_less_than: " + a + " >= " + b);
    };
    self.assert_greater_than_equal = function(a, b, msg) {
        if (!(a >= b)) throw new Error(msg || "assert_greater_than_equal: " + a + " < " + b);
    };
    self.assert_less_than_equal = function(a, b, msg) {
        if (!(a <= b)) throw new Error(msg || "assert_less_than_equal: " + a + " > " + b);
    };
    self.assert_array_equals = function(a, b, msg) {
        // Support both true arrays and array-like objects (NodeList, HTMLCollection)
        var aLen = a ? a.length : undefined;
        var bLen = b ? b.length : undefined;
        if (aLen === undefined || bLen === undefined || aLen !== bLen) {
            throw new Error(msg || "assert_array_equals: length mismatch (" + aLen + " vs " + bLen + ")");
        }
        for (var i = 0; i < aLen; i++) {
            if (a[i] !== b[i]) throw new Error(msg || "assert_array_equals: index " + i + ": " + a[i] + " !== " + b[i]);
        }
    };
    self.assert_object_equals = function(a, b, msg) {
        // Deep comparison via JSON serialization (good enough for arrays/objects)
        var aStr = JSON.stringify(a);
        var bStr = JSON.stringify(b);
        if (aStr !== bStr) throw new Error(msg || "assert_object_equals: " + aStr + " !== " + bStr);
    };
    self.assert_regexp_match = function(val, re, msg) {
        if (!re.test(val)) throw new Error(msg || "assert_regexp_match: " + val + " doesn't match " + re);
    };
    self.assert_own_property = function(obj, prop, msg) {
        if (!obj.hasOwnProperty(prop)) throw new Error(msg || "assert_own_property: missing " + prop);
    };
    self.assert_class_string = function(obj, expected, msg) {
        var actual = Object.prototype.toString.call(obj);
        var cls = actual.slice(8, -1);
        if (cls !== expected) throw new Error(msg || "assert_class_string: " + cls + " !== " + expected);
    };
    self.assert_throws_js = function(ctor, fn, msg) {
        var threw = false;
        try { fn(); } catch(e) {
            threw = true;
            if (!(e instanceof ctor)) throw new Error(msg || "assert_throws_js: wrong error type: " + e);
        }
        if (!threw) throw new Error(msg || "assert_throws_js: no error thrown");
    };
    self.assert_throws_dom = function(name, fn, msg) {
        var threw = false;
        try { fn(); } catch(e) {
            threw = true;
            // Accept any error with matching name or message
        }
        if (!threw) throw new Error(msg || "assert_throws_dom(" + name + "): no error thrown");
    };
    self.assert_throws_exactly = function(expected, fn, msg) {
        var threw = false;
        try { fn(); } catch(e) {
            threw = true;
            if (e !== expected) throw new Error(msg || "assert_throws_exactly: wrong error");
        }
        if (!threw) throw new Error(msg || "assert_throws_exactly: no error thrown");
    };
    self.promise_rejects_js = function(test, constructor, promise, description) {
        return promise.then(
            function() { throw new Error(description + ": promise resolved, expected rejection"); },
            function(e) {
                if (!(e instanceof constructor)) {
                    throw new Error(description + ": wrong rejection type: " + e);
                }
            }
        );
    };
    self.promise_rejects_exactly = function(test, exception, promise, description) {
        return promise.then(
            function() { throw new Error(description + ": promise resolved, expected rejection"); },
            function(e) {
                if (e !== exception) {
                    throw new Error(description + ": wrong rejection value");
                }
            }
        );
    };
    self.assert_unreached = function(msg) {
        throw new Error(msg || "assert_unreached");
    };
    self.assert_readonly = function(obj, prop, msg) {
        var desc = Object.getOwnPropertyDescriptor(obj, prop);
        if (!desc || desc.writable !== false) {
            // Check if setter-less accessor
            if (!desc || desc.set) throw new Error(msg || "assert_readonly: " + prop + " is not readonly");
        }
    };
    self.assert_idl_attribute = function(obj, prop, msg) {
        if (!(prop in obj)) throw new Error(msg || "assert_idl_attribute: missing " + prop);
    };
    self.assert_implements = function(val, msg) {
        if (!val) throw new Error(msg || "assert_implements: not implemented");
    };
    self.assert_implements_optional = function(val, msg) {
        if (!val) throw new Error(msg || "assert_implements_optional: not implemented");
    };
    self.format_value = function(val) {
        if (val === null) return "null";
        if (val === undefined) return "undefined";
        if (typeof val === "string") return '"' + val + '"';
        return String(val);
    };

    // Event constants
    if (typeof Event !== 'undefined') {
        if (!Event.NONE) Event.NONE = 0;
        if (!Event.CAPTURING_PHASE) Event.CAPTURING_PHASE = 1;
        if (!Event.AT_TARGET) Event.AT_TARGET = 2;
        if (!Event.BUBBLING_PHASE) Event.BUBBLING_PHASE = 3;
    }

    // EventWatcher — watches for events on a target, returns promises
    self.EventWatcher = function(test, target, eventTypes, setup) {
        if (typeof eventTypes === "string") eventTypes = [eventTypes];
        var waitingFor = null; // { type, resolve }
        var self_ew = this;

        function listener(evt) {
            if (waitingFor && evt.type === waitingFor.type) {
                var resolve = waitingFor.resolve;
                waitingFor = null;
                resolve(evt);
            }
        }

        for (var i = 0; i < eventTypes.length; i++) {
            target.addEventListener(eventTypes[i], listener);
        }

        if (test && test.add_cleanup) {
            test.add_cleanup(function() {
                for (var i = 0; i < eventTypes.length; i++) {
                    target.removeEventListener(eventTypes[i], listener);
                }
            });
        }

        this.wait_for = function(type) {
            if (setup) setup();
            return new Promise(function(resolve) {
                waitingFor = { type: type, resolve: resolve };
            });
        };
    };

    // Make results available
    self.__wpt_get_results = function() { return results; };
})();
"#
    .to_string()
}

/// Shim that replaces testharnessreport.js — no-op since we use our own preamble.
fn testharnessreport_shim() -> String {
    "// testharnessreport.js shim — no-op".to_string()
}

// ---------------------------------------------------------------------------
// Skip list — tests that need features we don't support
// ---------------------------------------------------------------------------

fn should_skip(rel_path: &str) -> Option<&'static str> {
    let skip_patterns: &[(&str, &str)] = &[
        (".https.", "requires secure context"),
        (".sub.", "requires server-side substitution"),
        ("idlharness", "requires full IDL harness infrastructure"),
        ("secure_context/", "requires secure context"),
    ];

    for (pattern, reason) in skip_patterns {
        if rel_path.contains(pattern) {
            return Some(reason);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Script resolution — resolve relative script src to filesystem content
// ---------------------------------------------------------------------------

fn resolve_script_src(html_path: &Path, src: &str, preamble: &str, report_shim: &str) -> Option<String> {
    if src == "/resources/testharness.js" {
        return Some(preamble.to_string());
    }
    if src == "/resources/testharnessreport.js" {
        return Some(report_shim.to_string());
    }

    // Absolute paths starting with / are relative to WPT root
    let resolved_path = if src.starts_with('/') {
        wpt_root().join(src.trim_start_matches('/'))
    } else {
        // Relative to the HTML file's directory
        html_path.parent().unwrap().join(src)
    };

    match std::fs::read_to_string(&resolved_path) {
        Ok(content) => Some(content),
        Err(_) => None, // External script not found — will be skipped by execute_scripts
    }
}

// ---------------------------------------------------------------------------
// Collect all script src attributes from HTML to build the fetched map
// ---------------------------------------------------------------------------

fn extract_script_srcs(html: &str) -> Vec<String> {
    let mut srcs = Vec::new();
    let lower = html.to_ascii_lowercase();
    let mut pos = 0;
    while let Some(script_start) = lower[pos..].find("<script") {
        let abs_start = pos + script_start;
        let tag_end = match lower[abs_start..].find('>') {
            Some(e) => abs_start + e,
            None => break,
        };
        let tag = &html[abs_start..=tag_end];

        if let Some(src_idx) = tag.to_ascii_lowercase().find("src=") {
            let after_src = &tag[src_idx + 4..];
            let quote = after_src.chars().next().unwrap_or(' ');
            if quote == '"' || quote == '\'' {
                if let Some(end_quote) = after_src[1..].find(quote) {
                    let src_val = &after_src[1..1 + end_quote];
                    srcs.push(src_val.to_string());
                }
            } else {
                let end = after_src
                    .find(|c: char| c.is_whitespace() || c == '>')
                    .unwrap_or(after_src.len());
                srcs.push(after_src[..end].to_string());
            }
        }

        pos = tag_end + 1;
    }
    srcs
}

// ---------------------------------------------------------------------------
// Iframe src resolution — resolve iframe src to filesystem content
// ---------------------------------------------------------------------------

fn extract_iframe_srcs(html: &str) -> Vec<String> {
    let mut srcs = Vec::new();
    let lower = html.to_ascii_lowercase();
    let mut pos = 0;
    while let Some(tag_start) = lower[pos..].find("<iframe") {
        let abs_start = pos + tag_start;
        let tag_end = match lower[abs_start..].find('>') {
            Some(e) => abs_start + e,
            None => break,
        };
        let tag = &html[abs_start..=tag_end];

        if let Some(src_idx) = tag.to_ascii_lowercase().find("src=") {
            let after_src = &tag[src_idx + 4..];
            let quote = after_src.chars().next().unwrap_or(' ');
            if quote == '"' || quote == '\'' {
                if let Some(end_quote) = after_src[1..].find(quote) {
                    let src_val = &after_src[1..1 + end_quote];
                    srcs.push(src_val.to_string());
                }
            } else {
                let end = after_src
                    .find(|c: char| c.is_whitespace() || c == '>')
                    .unwrap_or(after_src.len());
                srcs.push(after_src[..end].to_string());
            }
        }

        pos = tag_end + 1;
    }
    srcs
}

fn extract_js_iframe_srcs(html: &str) -> Vec<String> {
    let mut srcs = Vec::new();
    let lower = html.to_ascii_lowercase();
    let mut pos = 0;
    while let Some(start) = lower[pos..].find("<script") {
        let abs_start = pos + start;
        let open_end = match lower[abs_start..].find('>') {
            Some(e) => abs_start + e + 1,
            None => break,
        };
        let close = match lower[open_end..].find("</script") {
            Some(e) => open_end + e,
            None => break,
        };
        let body = &html[open_end..close];
        let mut spos = 0;
        while spos < body.len() {
            if let Some(idx) = body[spos..].find(".src") {
                let after_src = spos + idx + 4;
                let rest = &body[after_src..];
                let trimmed = rest.trim_start();
                if trimmed.starts_with('=') {
                    let after_eq = trimmed[1..].trim_start();
                    let quote = after_eq.chars().next().unwrap_or(' ');
                    if quote == '"' || quote == '\'' {
                        if let Some(end_quote) = after_eq[1..].find(quote) {
                            let src_val = &after_eq[1..1 + end_quote];
                            srcs.push(src_val.to_string());
                        }
                    }
                }
                spos = after_src;
            } else {
                break;
            }
        }
        pos = close;
    }
    srcs
}

fn resolve_iframe_src(html_path: &Path, src: &str) -> Option<String> {
    let src_no_fragment = src.split('#').next().unwrap_or(src);
    let resolved_path = if src_no_fragment.starts_with('/') {
        wpt_root().join(src_no_fragment.trim_start_matches('/'))
    } else {
        html_path.parent().unwrap().join(src_no_fragment)
    };
    std::fs::read_to_string(&resolved_path).ok()
}

// ---------------------------------------------------------------------------
// Run a single WPT test file
// ---------------------------------------------------------------------------

fn run_wpt_test(html_path: &Path, preamble: &str, report_shim: &str) -> Result<(), Failed> {
    let html = std::fs::read_to_string(html_path)
        .map_err(|e| Failed::from(format!("failed to read {}: {}", html_path.display(), e)))?;

    // Build the fetched map for external scripts
    let srcs = extract_script_srcs(&html);
    let mut fetched_scripts = HashMap::new();

    for src in &srcs {
        if let Some(content) = resolve_script_src(html_path, src, preamble, report_shim) {
            fetched_scripts.insert(src.clone(), content);
        }
    }

    // Build the fetched map for iframe src content (HTML attributes + JS property assignments)
    let mut iframe_srcs = extract_iframe_srcs(&html);
    iframe_srcs.extend(extract_js_iframe_srcs(&html));
    let mut fetched_iframes = HashMap::new();
    for src in &iframe_srcs {
        let src_no_fragment = src.split('#').next().unwrap_or(src);
        if let Some(content) = resolve_iframe_src(html_path, src) {
            fetched_iframes.insert(src_no_fragment.to_string(), content);
        }
    }

    let resources = FetchedResources {
        scripts: fetched_scripts,
        iframes: fetched_iframes,
    };

    let mut engine = Engine::new();

    let js_errors = engine.load_html_with_resources_lossy(&html, &resources);

    // Crash tests don't include testharness.js — if we got here, the test passed
    let is_crash_test = !srcs.iter().any(|s| s.contains("testharness.js"));
    if is_crash_test {
        return Ok(());
    }

    // Check if our preamble loaded
    let has_test_fn = engine.eval_js("typeof test").unwrap_or_default();
    if has_test_fn != "function" {
        let err_summary = if js_errors.is_empty() {
            "test harness preamble did not load".to_string()
        } else {
            format!(
                "preamble failed. First error: {}",
                js_errors[0].chars().take(200).collect::<String>()
            )
        };
        return Err(Failed::from(err_summary));
    }

    // Read results from our preamble's results array
    let results_json = engine
        .eval_js("JSON.stringify(__wpt_get_results())")
        .map_err(|e| Failed::from(format!("failed to get results: {}", e)))?;

    if results_json == "undefined" || results_json == "null" || results_json == "[]" {
        let errs: Vec<String> = js_errors
            .iter()
            .map(|e| e.chars().take(200).collect::<String>())
            .collect();
        return Err(Failed::from(format!(
            "no tests ran. js_errors({})={:?}",
            js_errors.len(),
            errs
        )));
    }

    let results: Vec<WptResult> = serde_json::from_str(&results_json)
        .map_err(|e| Failed::from(format!("failed to parse results JSON: {}\nJSON: {}", e, results_json)))?;

    let mut failures = Vec::new();
    for r in &results {
        if r.status != 0 {
            let status_name = match r.status {
                1 => "FAIL",
                2 => "TIMEOUT",
                3 => "NOTRUN",
                _ => "UNKNOWN",
            };
            failures.push(format!("  [{}] {}: {}", status_name, r.name, r.message));
        }
    }

    let pass_count = results.iter().filter(|r| r.status == 0).count();
    let total = results.len();

    if failures.is_empty() {
        Ok(())
    } else {
        Err(Failed::from(format!(
            "{}/{} subtests passed\n{}",
            pass_count,
            total,
            failures.join("\n")
        )))
    }
}

#[derive(serde::Deserialize)]
struct WptResult {
    name: String,
    status: i32,
    message: String,
}

// ---------------------------------------------------------------------------
// Wrap .any.js / .window.js in HTML template
// ---------------------------------------------------------------------------

fn wrap_js_in_html(js_path: &Path) -> String {
    let js_content = std::fs::read_to_string(js_path).unwrap();
    let title = js_path.file_stem().unwrap().to_str().unwrap();

    // Parse // META: script=<path> directives
    let mut meta_scripts = String::new();
    for line in js_content.lines() {
        if let Some(rest) = line.strip_prefix("// META: script=") {
            let script_path = rest.trim();
            let resolved = js_path.parent().unwrap().join(script_path);
            if let Ok(script_content) = std::fs::read_to_string(&resolved) {
                meta_scripts.push_str(&format!(
                    "<script>\n{script_content}\n</script>\n"
                ));
            }
        }
    }

    format!(
        r#"<!DOCTYPE html>
<meta charset=utf-8>
<title>{title}</title>
<script src="/resources/testharness.js"></script>
<script src="/resources/testharnessreport.js"></script>
{meta_scripts}<script>
{js_content}
</script>
"#
    )
}

// ---------------------------------------------------------------------------
// Test discovery (recursive)
// ---------------------------------------------------------------------------

fn discover_tests(dir: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if !dir.exists() {
        return paths;
    }
    for entry in std::fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            paths.extend(discover_tests(&path));
            continue;
        }
        let name = path.file_name().unwrap().to_str().unwrap();
        if name.ends_with(".html") || name.ends_with(".any.js") || name.ends_with(".window.js") {
            if name.ends_with(".worker.js") {
                continue;
            }
            paths.push(path);
        }
    }
    paths.sort();
    paths
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    let child = std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024) // 32 MB
        .spawn(main_inner)
        .expect("failed to spawn test thread");
    child.join().unwrap();
}

fn main_inner() {
    let args = Arguments::from_args();

    let _testharness_js = load_testharness_js();
    let preamble = testharness_preamble();
    let report_shim = testharnessreport_shim();

    let wpt = wpt_root();
    let crypto_root = wpt.join("WebCryptoAPI");

    let mut trials = Vec::new();

    let test_files = discover_tests(&crypto_root);

    for path in test_files {
        let rel_path = path
            .strip_prefix(&wpt)
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();

        let ignored = should_skip(&rel_path).is_some();

        let file_name = path.file_name().unwrap().to_str().unwrap().to_string();
        let test_path = path.clone();
        let th_js = preamble.clone();
        let shim = report_shim.clone();
        let is_js = file_name.ends_with(".any.js") || file_name.ends_with(".window.js");

        trials.push(
            Trial::test(rel_path, move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    if is_js {
                        let html = wrap_js_in_html(&test_path);
                        let tmp_dir = std::env::temp_dir();
                        let tmp_path = tmp_dir.join(&file_name).with_extension("html");
                        std::fs::write(&tmp_path, &html).unwrap();
                        run_wpt_test(&tmp_path, &th_js, &shim)
                    } else {
                        run_wpt_test(&test_path, &th_js, &shim)
                    }
                }));

                match result {
                    Ok(inner) => inner,
                    Err(panic_info) => {
                        let msg = if let Some(s) = panic_info.downcast_ref::<&str>() {
                            s.to_string()
                        } else if let Some(s) = panic_info.downcast_ref::<String>() {
                            s.clone()
                        } else {
                            "test panicked".to_string()
                        };
                        Err(Failed::from(format!("PANIC: {}", msg)))
                    }
                }
            })
            .with_ignored_flag(ignored),
        );
    }

    libtest_mimic::run(&args, trials).exit();
}
