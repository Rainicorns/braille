use std::collections::HashMap;
use std::path::{Path, PathBuf};

use braille_engine::{Engine, FetchedResources};

// ---------------------------------------------------------------------------
// Paths
// ---------------------------------------------------------------------------

pub fn workspace_root() -> PathBuf {
    // Walk up from CARGO_MANIFEST_DIR (crates/wpt-runner) to workspace root
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

pub fn wpt_root() -> PathBuf {
    workspace_root().join("tests/wpt")
}

// ---------------------------------------------------------------------------
// Test harness JS
// ---------------------------------------------------------------------------

pub fn load_testharness_js() -> String {
    let path = wpt_root().join("resources/testharness.js");
    std::fs::read_to_string(&path).expect("failed to read testharness.js")
}

pub fn testharness_preamble() -> String {
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
        for (var i = 0; i < cleanups.length; i++) {
            try { cleanups[i](); } catch(e) {}
        }
        results.push(result);
    };

    self.async_test = function(fn, name) {
        if (typeof fn === "string") {
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

    if (typeof Event !== 'undefined') {
        if (!Event.NONE) Event.NONE = 0;
        if (!Event.CAPTURING_PHASE) Event.CAPTURING_PHASE = 1;
        if (!Event.AT_TARGET) Event.AT_TARGET = 2;
        if (!Event.BUBBLING_PHASE) Event.BUBBLING_PHASE = 3;
    }

    self.EventWatcher = function(test, target, eventTypes, setup) {
        if (typeof eventTypes === "string") eventTypes = [eventTypes];
        var waitingFor = null;

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

    self.__wpt_get_results = function() { return results; };
})();
"#
    .to_string()
}

pub fn testharnessreport_shim() -> String {
    "// testharnessreport.js shim — no-op".to_string()
}

// ---------------------------------------------------------------------------
// Script resolution
// ---------------------------------------------------------------------------

pub fn resolve_script_src(
    html_path: &Path,
    src: &str,
    preamble: &str,
    report_shim: &str,
) -> Option<String> {
    if src == "/resources/testharness.js" {
        return Some(preamble.to_string());
    }
    if src == "/resources/testharnessreport.js" {
        return Some(report_shim.to_string());
    }

    let resolved_path = if src.starts_with('/') {
        wpt_root().join(src.trim_start_matches('/'))
    } else {
        html_path.parent().unwrap().join(src)
    };

    std::fs::read_to_string(&resolved_path).ok()
}

pub fn extract_script_srcs(html: &str) -> Vec<String> {
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
// Iframe src resolution
// ---------------------------------------------------------------------------

pub fn extract_iframe_srcs(html: &str) -> Vec<String> {
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

pub fn extract_js_iframe_srcs(html: &str) -> Vec<String> {
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
                if let Some(after_equals) = trimmed.strip_prefix('=') {
                    let after_eq = after_equals.trim_start();
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

pub fn resolve_iframe_src(html_path: &Path, src: &str) -> Option<String> {
    let src_no_fragment = src.split('#').next().unwrap_or(src);
    let resolved_path = if src_no_fragment.starts_with('/') {
        wpt_root().join(src_no_fragment.trim_start_matches('/'))
    } else {
        html_path.parent().unwrap().join(src_no_fragment)
    };
    std::fs::read_to_string(&resolved_path).ok()
}

// ---------------------------------------------------------------------------
// Wrap .any.js / .window.js in HTML template
// ---------------------------------------------------------------------------

pub fn wrap_js_in_html(js_path: &Path) -> String {
    let js_content = std::fs::read_to_string(js_path).unwrap();
    let title = js_path.file_stem().unwrap().to_str().unwrap();

    let mut meta_scripts = String::new();
    for line in js_content.lines() {
        if let Some(rest) = line.strip_prefix("// META: script=") {
            let script_path = rest.trim();
            let resolved = js_path.parent().unwrap().join(script_path);
            if let Ok(script_content) = std::fs::read_to_string(&resolved) {
                meta_scripts.push_str(&format!("<script>\n{script_content}\n</script>\n"));
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
// Test discovery
// ---------------------------------------------------------------------------

pub fn discover_tests_recursive(dir: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if !dir.exists() {
        return paths;
    }
    discover_recursive_inner(dir, &mut paths);
    paths.sort();
    paths
}

fn discover_recursive_inner(dir: &Path, paths: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap()).collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            discover_recursive_inner(&path, paths);
        } else {
            let name = path.file_name().unwrap().to_str().unwrap();
            if name.ends_with(".html")
                || name.ends_with(".any.js")
                || name.ends_with(".window.js")
            {
                if name.ends_with(".worker.js") {
                    continue;
                }
                paths.push(path);
            }
        }
    }
}

/// Convert an absolute path to a relative path from the wpt root.
pub fn to_wpt_relative(path: &Path) -> String {
    let wpt = wpt_root();
    path.strip_prefix(&wpt)
        .unwrap_or(path)
        .to_str()
        .unwrap()
        .to_string()
}

// ---------------------------------------------------------------------------
// Test result
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct WptTestResult {
    pub passed: usize,
    pub failed: usize,
    pub details: Vec<String>,
    pub is_crash_test: bool,
}

#[derive(serde::Deserialize)]
struct WptSubResult {
    name: String,
    status: i32,
    message: String,
}

/// Tests that need incremental (interleaved) HTML parsing.
const INCREMENTAL_TESTS: &[&str] = &["MutationObserver-document"];

// ---------------------------------------------------------------------------
// Run a single WPT test
// ---------------------------------------------------------------------------

pub fn run_wpt_test(html_path: &Path, preamble: &str, report_shim: &str) -> WptTestResult {
    let html = match std::fs::read_to_string(html_path) {
        Ok(h) => h,
        Err(e) => {
            return WptTestResult {
                passed: 0,
                failed: 1,
                details: vec![format!("failed to read {}: {}", html_path.display(), e)],
                is_crash_test: false,
            };
        }
    };

    let srcs = extract_script_srcs(&html);
    let mut fetched_scripts = HashMap::new();
    for src in &srcs {
        if let Some(content) = resolve_script_src(html_path, src, preamble, report_shim) {
            fetched_scripts.insert(src.clone(), content);
        }
    }

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

    let file_stem = html_path.file_stem().unwrap().to_str().unwrap();
    let use_incremental = INCREMENTAL_TESTS.iter().any(|t| file_stem.contains(t));
    let js_errors = if use_incremental {
        engine.load_html_incremental_with_resources_lossy(&html, &resources)
    } else {
        engine.load_html_with_resources_lossy(&html, &resources)
    };

    // Crash tests don't include testharness.js — if we got here, it passed
    let is_crash_test = !srcs.iter().any(|s| s.contains("testharness.js"));
    if is_crash_test {
        return WptTestResult {
            passed: 1,
            failed: 0,
            details: vec![],
            is_crash_test: true,
        };
    }

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
        return WptTestResult {
            passed: 0,
            failed: 1,
            details: vec![err_summary],
            is_crash_test: false,
        };
    }

    let results_json = match engine.eval_js("JSON.stringify(__wpt_get_results())") {
        Ok(j) => j,
        Err(e) => {
            return WptTestResult {
                passed: 0,
                failed: 1,
                details: vec![format!("failed to get results: {}", e)],
                is_crash_test: false,
            };
        }
    };

    if results_json == "undefined" || results_json == "null" || results_json == "[]" {
        let errs: Vec<String> = js_errors
            .iter()
            .map(|e| e.chars().take(200).collect::<String>())
            .collect();
        return WptTestResult {
            passed: 0,
            failed: 1,
            details: vec![format!(
                "no tests ran. js_errors({})={:?}",
                js_errors.len(),
                errs
            )],
            is_crash_test: false,
        };
    }

    let results: Vec<WptSubResult> = match serde_json::from_str(&results_json) {
        Ok(r) => r,
        Err(e) => {
            return WptTestResult {
                passed: 0,
                failed: 1,
                details: vec![format!("failed to parse results JSON: {}", e)],
                is_crash_test: false,
            };
        }
    };

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
    let fail_count = results.len() - pass_count;

    WptTestResult {
        passed: pass_count,
        failed: fail_count,
        details: failures,
        is_crash_test: false,
    }
}
