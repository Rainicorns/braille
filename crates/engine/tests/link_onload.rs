//! Tests for <link> element onload behavior.
//!
//! Webpack 5 loads CSS chunks via <link rel="stylesheet"> elements and waits
//! for their onload event to resolve promises. If onload never fires, the
//! app never bootstraps.

use std::collections::HashMap;
use braille_engine::{Engine, FetchedResources};

/// Basic: appending a <link rel="stylesheet"> fires onload
#[test]
fn link_stylesheet_fires_onload() {
    let html = r#"<!DOCTYPE html>
<html><head></head><body>
<div id="result">waiting</div>
<script src="/test.js"></script>
</body></html>"#;

    let test_js = r#"
var link = document.createElement('link');
link.rel = 'stylesheet';
link.href = '/style.css';
link.onload = function(e) {
    document.getElementById('result').textContent = 'loaded:' + e.type;
};
link.onerror = function(e) {
    document.getElementById('result').textContent = 'error:' + e.type;
};
document.head.appendChild(link);
"#;

    let mut engine = Engine::new();
    let mut scripts = HashMap::new();
    scripts.insert("/test.js".to_string(), test_js.to_string());
    let descriptors = engine.parse_and_collect_scripts(html);
    let errors = engine.execute_scripts_lossy(&descriptors, &FetchedResources::scripts_only(scripts));
    assert!(errors.is_empty(), "no JS errors: {errors:?}");

    // onload fires via setTimeout(0), so we need to settle
    engine.settle();

    let result = engine.eval_js("document.getElementById('result').textContent").unwrap();
    assert_eq!(result, "loaded:load", "link onload should fire with type='load'");
}

/// Link onload fires even with settle_no_advance (setTimeout(0) is already due)
#[test]
fn link_onload_fires_with_settle_no_advance() {
    let html = r#"<!DOCTYPE html>
<html><head></head><body>
<div id="result">waiting</div>
<script src="/test.js"></script>
</body></html>"#;

    let test_js = r#"
var link = document.createElement('link');
link.rel = 'stylesheet';
link.href = '/style.css';
link.onload = function() {
    document.getElementById('result').textContent = 'loaded';
};
document.head.appendChild(link);
"#;

    let mut engine = Engine::new();
    let mut scripts = HashMap::new();
    scripts.insert("/test.js".to_string(), test_js.to_string());
    let descriptors = engine.parse_and_collect_scripts(html);
    let errors = engine.execute_scripts_lossy(&descriptors, &FetchedResources::scripts_only(scripts));
    assert!(errors.is_empty(), "no JS errors: {errors:?}");

    // Use settle_no_advance — setTimeout(0) should still fire since it's already due
    engine.settle_no_advance();

    let result = engine.eval_js("document.getElementById('result').textContent").unwrap();
    assert_eq!(result, "loaded", "link onload should fire even with settle_no_advance");
}

/// Link onload resolves a Promise (webpack CSS chunk pattern)
#[test]
fn link_onload_resolves_promise() {
    let html = r#"<!DOCTYPE html>
<html><head></head><body>
<div id="result">waiting</div>
<script src="/test.js"></script>
</body></html>"#;

    let test_js = r#"
var cssLoaded = new Promise(function(resolve, reject) {
    var link = document.createElement('link');
    link.rel = 'stylesheet';
    link.href = '/style.css';
    link.onload = link.onerror = function(e) {
        link.onload = link.onerror = null;
        if (e.type === 'load') resolve();
        else reject(new Error('CSS load failed'));
    };
    document.head.appendChild(link);
});

cssLoaded.then(function() {
    document.getElementById('result').textContent = 'css-resolved';
}).catch(function(e) {
    document.getElementById('result').textContent = 'css-rejected:' + e.message;
});
"#;

    let mut engine = Engine::new();
    let mut scripts = HashMap::new();
    scripts.insert("/test.js".to_string(), test_js.to_string());
    let descriptors = engine.parse_and_collect_scripts(html);
    let errors = engine.execute_scripts_lossy(&descriptors, &FetchedResources::scripts_only(scripts));
    assert!(errors.is_empty(), "no JS errors: {errors:?}");

    engine.settle();

    let result = engine.eval_js("document.getElementById('result').textContent").unwrap();
    assert_eq!(result, "css-resolved", "CSS loading promise should resolve via link onload");
}

/// Promise.all with CSS link + dynamic script fetch (webpack entry pattern)
#[test]
fn css_and_js_chunk_loading_both_resolve() {
    let html = r#"<!DOCTYPE html>
<html><head></head><body>
<div id="result">waiting</div>
<script src="/boot.js"></script>
</body></html>"#;

    let boot_js = r#"
// Simulate webpack's chunk loading: load CSS + JS in parallel
var cssPromise = new Promise(function(resolve, reject) {
    var link = document.createElement('link');
    link.rel = 'stylesheet';
    link.href = '/chunk.css';
    link.onload = link.onerror = function(e) {
        link.onload = link.onerror = null;
        if (e.type === 'load') resolve();
        else reject(new Error('CSS failed'));
    };
    document.head.appendChild(link);
});

var jsPromise = new Promise(function(resolve, reject) {
    var script = document.createElement('script');
    script.src = '/chunk.js';
    // webpack checks if module registered, not script onload
    // but for this test, use a global flag
    script.onload = function() {
        if (typeof CHUNK_LOADED !== 'undefined') resolve();
        else reject(new Error('chunk did not register'));
    };
    script.onerror = function() { reject(new Error('script error')); };
    document.head.appendChild(script);
});

Promise.all([cssPromise, jsPromise]).then(function() {
    document.getElementById('result').textContent = 'both-loaded:' + CHUNK_LOADED;
}).catch(function(e) {
    document.getElementById('result').textContent = 'error:' + e.message;
});
"#;

    let chunk_js = "self.CHUNK_LOADED = 'hello';";

    let mut engine = Engine::new();
    let mut scripts = HashMap::new();
    scripts.insert("/boot.js".to_string(), boot_js.to_string());
    let descriptors = engine.parse_and_collect_scripts(html);
    let errors = engine.execute_scripts_lossy(&descriptors, &FetchedResources::scripts_only(scripts));
    assert!(errors.is_empty(), "no JS errors: {errors:?}");

    // CSS link onload fires via setTimeout(0), script fetch is pending
    assert!(engine.has_pending_fetches(), "should have pending fetch for chunk.js");

    let pending = engine.pending_fetches();
    assert_eq!(pending.len(), 1);
    assert!(pending[0].url.contains("chunk.js"), "pending fetch should be chunk.js, got: {}", pending[0].url);

    // Resolve the JS chunk fetch
    engine.resolve_fetch(pending[0].id, &braille_wire::FetchResponseData {
        url: pending[0].url.clone(),
        status: 200,
        status_text: "OK".to_string(),
        headers: vec![("content-type".to_string(), "application/javascript".to_string())],
        body: chunk_js.to_string(),
        redirect_chain: vec![],
    });

    // Settle to fire CSS onload timer + JS onload + Promise.all resolution
    engine.settle();

    let result = engine.eval_js("document.getElementById('result').textContent").unwrap();
    assert_eq!(result, "both-loaded:hello", "Promise.all(css, js) should resolve");
}

/// Webpack-realistic: CSS link created during initial script, JS chunks fetched later,
/// entry fires only after both CSS and JS are ready
#[test]
fn webpack_css_js_entry_fires_after_both_ready() {
    let html = r#"<!DOCTYPE html>
<html><head></head><body>
<div class="root">Loading...</div>
<script src="/runtime.js"></script>
<script src="/entry.js"></script>
</body></html>"#;

    let runtime_js = r#"
(function() {
    var modules = {};
    var installed = {};
    var cssInstalled = {};
    var deferred = [];

    function require(id) {
        if (installed[id]) return installed[id].exports;
        var m = installed[id] = { exports: {} };
        modules[id](m, m.exports, require);
        return m.exports;
    }

    // JS chunk loader
    require.loadJs = function(chunkId) {
        return new Promise(function(resolve, reject) {
            var script = document.createElement('script');
            script.src = '/chunks/' + chunkId + '.js';
            script.onload = function() { resolve(); };
            script.onerror = function() { reject(new Error('JS chunk failed')); };
            document.head.appendChild(script);
        });
    };

    // CSS chunk loader
    require.loadCss = function(chunkId) {
        if (cssInstalled[chunkId] === 0) return Promise.resolve();
        return new Promise(function(resolve, reject) {
            var link = document.createElement('link');
            link.rel = 'stylesheet';
            link.href = '/chunks/' + chunkId + '.css';
            link.onload = link.onerror = function(e) {
                link.onload = link.onerror = null;
                cssInstalled[chunkId] = 0;
                if (e.type === 'load') resolve();
                else reject(new Error('CSS failed'));
            };
            document.head.appendChild(link);
        });
    };

    // Load both JS and CSS for a chunk
    require.e = function(chunkId) {
        return Promise.all([require.loadJs(chunkId), require.loadCss(chunkId)]);
    };

    // Module registration (called by chunk scripts)
    var jsonp = self.webpackChunk = self.webpackChunk || [];
    jsonp.push = function(data) {
        var mods = data[1];
        for (var id in mods) modules[id] = mods[id];
    };
    for (var i = 0; i < jsonp.length; i++) jsonp.push(jsonp[i]);

    self.__r = require;
})();
"#;

    let entry_js = r#"
__r.e('ui').then(function() {
    var UI = __r('ui-render');
    UI.render();
}).catch(function(e) {
    document.querySelector('.root').textContent = 'ENTRY ERROR: ' + e.message;
});
"#;

    let chunk_ui_js = r#"
(self.webpackChunk = self.webpackChunk || []).push([['ui'], {
    'ui-render': function(module) {
        module.exports = {
            render: function() {
                var root = document.querySelector('.root');
                root.textContent = '';
                var h = document.createElement('h1');
                h.textContent = 'Sign In';
                root.appendChild(h);
                var btn = document.createElement('button');
                btn.textContent = 'Submit';
                root.appendChild(btn);
            }
        };
    }
}]);
"#;

    let mut engine = Engine::new();
    let mut scripts = HashMap::new();
    scripts.insert("/runtime.js".to_string(), runtime_js.to_string());
    scripts.insert("/entry.js".to_string(), entry_js.to_string());
    let descriptors = engine.parse_and_collect_scripts(html);
    let errors = engine.execute_scripts_lossy(&descriptors, &FetchedResources::scripts_only(scripts));
    assert!(errors.is_empty(), "no JS errors: {errors:?}");

    // After initial execution: JS chunk fetch pending, CSS link setTimeout(0) pending
    assert!(engine.has_pending_fetches(), "should have pending JS chunk fetch");
    let pending = engine.pending_fetches();
    assert_eq!(pending.len(), 1, "exactly 1 pending fetch");
    assert!(pending[0].url.contains("/chunks/ui.js"), "fetch should be for ui chunk");

    // Resolve JS chunk
    engine.resolve_fetch(pending[0].id, &braille_wire::FetchResponseData {
        url: pending[0].url.clone(),
        status: 200,
        status_text: "OK".to_string(),
        headers: vec![],
        body: chunk_ui_js.to_string(),
        redirect_chain: vec![],
    });

    // Settle — fires CSS onload timer, resolves both promises, entry runs
    engine.settle();

    let snapshot = engine.snapshot(braille_wire::SnapMode::Compact);
    assert!(!snapshot.contains("Loading"), "loader should be gone, got: {snapshot}");
    assert!(!snapshot.contains("ENTRY ERROR"), "entry should not error, got: {snapshot}");
    assert!(snapshot.contains("Sign In"), "should show Sign In, got: {snapshot}");
    assert!(snapshot.contains("Submit"), "should show Submit button, got: {snapshot}");
}

/// Verify that settle_no_advance followed by fetch resolution still fires CSS onload
/// (the daemon's actual flow: settle_no_advance → resolve fetches → settle_no_advance)
#[test]
fn settle_no_advance_then_fetch_then_settle_fires_entry() {
    let html = r#"<!DOCTYPE html>
<html><head></head><body>
<div id="out">init</div>
<script src="/boot.js"></script>
</body></html>"#;

    let boot_js = r#"
var cssReady = false, jsReady = false;

// CSS load
var link = document.createElement('link');
link.rel = 'stylesheet';
link.href = '/app.css';
link.onload = link.onerror = function(e) {
    link.onload = link.onerror = null;
    cssReady = true;
    maybeRender();
};
document.head.appendChild(link);

// JS chunk load
var script = document.createElement('script');
script.src = '/app-chunk.js';
script.onload = function() {
    jsReady = true;
    maybeRender();
};
document.head.appendChild(script);

function maybeRender() {
    if (cssReady && jsReady && typeof APP_CHUNK !== 'undefined') {
        document.getElementById('out').textContent = 'rendered:' + APP_CHUNK;
    }
}
"#;

    let chunk_js = "self.APP_CHUNK = 'success';";

    let mut engine = Engine::new();
    let mut scripts = HashMap::new();
    scripts.insert("/boot.js".to_string(), boot_js.to_string());
    let descriptors = engine.parse_and_collect_scripts(html);
    let errors = engine.execute_scripts_lossy(&descriptors, &FetchedResources::scripts_only(scripts));
    assert!(errors.is_empty(), "no JS errors: {errors:?}");

    // Mimic the daemon's exact flow:
    // 1. settle_no_advance (fires setTimeout(0) for CSS onload)
    engine.settle_no_advance();

    // CSS onload should have fired by now
    let css_ready = engine.eval_js("cssReady").unwrap();
    assert_eq!(css_ready, "true", "CSS onload should fire during settle_no_advance");

    // 2. resolve pending fetches (JS chunk)
    assert!(engine.has_pending_fetches(), "should have pending JS fetch");
    let pending = engine.pending_fetches();
    assert_eq!(pending.len(), 1);
    engine.resolve_fetch(pending[0].id, &braille_wire::FetchResponseData {
        url: pending[0].url.clone(),
        status: 200,
        status_text: "OK".to_string(),
        headers: vec![],
        body: chunk_js.to_string(),
        redirect_chain: vec![],
    });

    // 3. settle_no_advance again (fires script onload, runs maybeRender)
    engine.settle_no_advance();

    let js_ready = engine.eval_js("jsReady").unwrap();
    assert_eq!(js_ready, "true", "JS onload should fire after fetch resolution");

    let result = engine.eval_js("document.getElementById('out').textContent").unwrap();
    assert_eq!(result, "rendered:success", "app should render after both CSS and JS ready");
}

/// Multiple CSS links (webpack loads multiple CSS chunks in parallel)
#[test]
fn multiple_css_links_all_fire_onload() {
    let html = r#"<!DOCTYPE html>
<html><head></head><body>
<div id="out">0</div>
<script src="/test.js"></script>
</body></html>"#;

    let test_js = r#"
var count = 0;
var expected = 5;

for (var i = 0; i < expected; i++) {
    (function(idx) {
        var link = document.createElement('link');
        link.rel = 'stylesheet';
        link.href = '/css/chunk' + idx + '.css';
        link.onload = function() {
            count++;
            document.getElementById('out').textContent = String(count);
        };
        document.head.appendChild(link);
    })(i);
}
"#;

    let mut engine = Engine::new();
    let mut scripts = HashMap::new();
    scripts.insert("/test.js".to_string(), test_js.to_string());
    let descriptors = engine.parse_and_collect_scripts(html);
    let errors = engine.execute_scripts_lossy(&descriptors, &FetchedResources::scripts_only(scripts));
    assert!(errors.is_empty(), "no JS errors: {errors:?}");

    engine.settle();

    let result = engine.eval_js("document.getElementById('out').textContent").unwrap();
    assert_eq!(result, "5", "all 5 CSS link onloads should fire");
}

/// Link with rel="prefetch" also fires onload (webpack prefetch)
#[test]
fn link_prefetch_fires_onload() {
    let html = r#"<!DOCTYPE html>
<html><head></head><body>
<div id="out">waiting</div>
<script src="/test.js"></script>
</body></html>"#;

    let test_js = r#"
var link = document.createElement('link');
link.rel = 'prefetch';
link.as = 'script';
link.href = '/prefetch-chunk.js';
link.onload = function() {
    document.getElementById('out').textContent = 'prefetched';
};
document.head.appendChild(link);
"#;

    let mut engine = Engine::new();
    let mut scripts = HashMap::new();
    scripts.insert("/test.js".to_string(), test_js.to_string());
    let descriptors = engine.parse_and_collect_scripts(html);
    let errors = engine.execute_scripts_lossy(&descriptors, &FetchedResources::scripts_only(scripts));
    assert!(errors.is_empty(), "no JS errors: {errors:?}");

    engine.settle();

    let result = engine.eval_js("document.getElementById('out').textContent").unwrap();
    assert_eq!(result, "prefetched", "prefetch link onload should fire");
}
