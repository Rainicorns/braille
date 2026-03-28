//! Test webpack-style dynamic chunk loading patterns.
//!
//! Reproduces the ProtonMail architecture:
//! - HTML has preloaded chunk <script> tags + runtime + entry
//! - Chunks execute before runtime, push to plain array
//! - Runtime installs custom push, processes pre-pushed chunks
//! - Entry calls __webpack_require__.e() which resolves immediately for preloaded chunks
//! - App renders and makes API fetches

use std::collections::HashMap;
use braille_engine::{Engine, FetchedResources};

/// Pattern 1: Chunks in HTML as <script src> before runtime (ProtonMail pattern)
#[test]
fn preloaded_chunks_before_runtime() {
    let html = r#"<!DOCTYPE html>
<html><head><title>Test</title></head>
<body>
<div class="app-root"><div class="loader">Loading</div></div>
<script src="/chunk.react.js"></script>
<script src="/chunk.app.js"></script>
<script src="/runtime.js"></script>
<script src="/entry.js"></script>
</body></html>"#;

    let chunk_react = r#"
(self.webpackChunkapp = self.webpackChunkapp || []).push([['react'], {
    'react': function(module, exports) {
        module.exports = {
            createElement: function(tag, props) {
                var el = document.createElement(tag);
                if (props) for (var k in props) {
                    if (k === 'textContent') el.textContent = props[k];
                    else el.setAttribute(k, String(props[k]));
                }
                for (var i = 2; i < arguments.length; i++) {
                    if (arguments[i] == null) continue;
                    if (typeof arguments[i] === 'string') el.appendChild(document.createTextNode(arguments[i]));
                    else el.appendChild(arguments[i]);
                }
                return el;
            },
            render: function(el, container) {
                while (container.firstChild) container.removeChild(container.firstChild);
                container.appendChild(el);
            }
        };
    }
}]);
"#;

    let chunk_app = r#"
(self.webpackChunkapp = self.webpackChunkapp || []).push([['app'], {
    'app': function(module, exports, require) {
        module.exports = {
            render: function(React) {
                var root = document.querySelector('.app-root');
                if (!root) return;
                var page = React.createElement('div', {},
                    React.createElement('h1', {textContent: 'Sign In'}),
                    React.createElement('input', {id: 'email', type: 'email'}),
                    React.createElement('button', {id: 'submit', textContent: 'Log in'})
                );
                React.render(page, root);
            }
        };
    }
}]);
"#;

    let runtime = r#"
(function() {
    var installedModules = {};
    var installedChunks = {};
    var webpackModules = {};

    function __webpack_require__(moduleId) {
        if (installedModules[moduleId]) return installedModules[moduleId].exports;
        var module = installedModules[moduleId] = { id: moduleId, loaded: false, exports: {} };
        webpackModules[moduleId](module, module.exports, __webpack_require__);
        module.loaded = true;
        return module.exports;
    }

    __webpack_require__.e = function(chunkId) {
        if (installedChunks[chunkId] === 0) return Promise.resolve();
        return Promise.reject(new Error('Chunk ' + chunkId + ' not preloaded'));
    };

    var jsonp = self.webpackChunkapp = self.webpackChunkapp || [];
    // Process any chunks that were pushed before runtime loaded
    var existing = jsonp.slice();
    for (var i = 0; i < existing.length; i++) {
        var data = existing[i];
        var chunkIds = data[0], moreModules = data[1];
        for (var id in moreModules) webpackModules[id] = moreModules[id];
        for (var j = 0; j < chunkIds.length; j++) installedChunks[chunkIds[j]] = 0;
    }
    // Install custom push for future chunks
    jsonp.push = function(data) {
        var chunkIds = data[0], moreModules = data[1];
        for (var id in moreModules) webpackModules[id] = moreModules[id];
        for (var i = 0; i < chunkIds.length; i++) installedChunks[chunkIds[i]] = 0;
    };

    self.__webpack_require__ = __webpack_require__;
})();
"#;

    let entry = r#"
Promise.all([
    __webpack_require__.e('react'),
    __webpack_require__.e('app')
]).then(function() {
    var React = __webpack_require__('react');
    var App = __webpack_require__('app');
    App.render(React);
});
"#;

    let mut engine = Engine::new();

    let mut scripts = HashMap::new();
    scripts.insert("/chunk.react.js".to_string(), chunk_react.to_string());
    scripts.insert("/chunk.app.js".to_string(), chunk_app.to_string());
    scripts.insert("/runtime.js".to_string(), runtime.to_string());
    scripts.insert("/entry.js".to_string(), entry.to_string());

    let descriptors = engine.parse_and_collect_scripts(html);
    assert_eq!(descriptors.len(), 4, "should find 4 script descriptors");

    let errors = engine.execute_scripts_lossy(
        &descriptors,
        &FetchedResources::scripts_only(scripts),
    );
    assert!(errors.is_empty(), "should have no JS errors: {errors:?}");

    // Settle to let promises resolve
    engine.settle();

    let snapshot = engine.snapshot(braille_wire::SnapMode::Compact);
    eprintln!("snapshot: {snapshot}");

    // The loader should be gone
    assert!(
        !snapshot.contains("Loading"),
        "loader should be replaced, got: {snapshot}"
    );
    // The app should have rendered
    assert!(
        snapshot.contains("Sign In"),
        "should show Sign In heading, got: {snapshot}"
    );
    assert!(
        snapshot.contains("Log in"),
        "should show Log in button, got: {snapshot}"
    );
}

/// Pattern 2: Runtime creates <script> tags dynamically (webpack chunk loading)
/// This tests that dynamically inserted <script> elements fire onload after fetch+eval.
#[test]
fn dynamic_script_tag_insertion_fires_onload() {
    let html = r#"<!DOCTYPE html>
<html><head><title>Test</title></head>
<body>
<div id="app">Before</div>
<script src="/boot.js"></script>
</body></html>"#;

    let boot = r#"
// Dynamically load a script by creating a <script> tag
var loaded = false;
var script = document.createElement('script');
script.src = '/dynamic.js';
script.onload = function() {
    loaded = true;
    // After dynamic script loads, use its global
    if (typeof DYNAMIC_VALUE !== 'undefined') {
        document.getElementById('app').textContent = 'Loaded: ' + DYNAMIC_VALUE;
    } else {
        document.getElementById('app').textContent = 'onload fired but DYNAMIC_VALUE missing';
    }
};
script.onerror = function() {
    document.getElementById('app').textContent = 'Script error';
};
document.head.appendChild(script);
"#;

    // This script won't be pre-fetched since it's created dynamically
    // It will be fetched via __braille_maybe_load_script
    let dynamic = r#"
self.DYNAMIC_VALUE = 'hello from dynamic script';
"#;

    let mut engine = Engine::new();

    let mut scripts = HashMap::new();
    scripts.insert("/boot.js".to_string(), boot.to_string());
    // dynamic.js is NOT pre-fetched — it needs to be fetched via the fetch mechanism

    let descriptors = engine.parse_and_collect_scripts(html);
    assert_eq!(descriptors.len(), 1, "should find 1 script descriptor (boot.js)");

    let errors = engine.execute_scripts_lossy(
        &descriptors,
        &FetchedResources::scripts_only(scripts),
    );
    assert!(errors.is_empty(), "should have no JS errors: {errors:?}");

    // boot.js created a <script src="/dynamic.js"> — this should be a pending fetch
    assert!(
        engine.has_pending_fetches(),
        "should have pending fetch for dynamic.js"
    );

    let pending = engine.pending_fetches();
    assert_eq!(pending.len(), 1, "should have exactly 1 pending fetch");
    assert!(
        pending[0].url.contains("dynamic.js"),
        "pending fetch should be for dynamic.js, got: {}",
        pending[0].url
    );

    // Simulate the host fetching dynamic.js
    let response = braille_wire::FetchResponseData {
        url: pending[0].url.clone(),
        status: 200,
        status_text: "OK".to_string(),
        headers: vec![("content-type".to_string(), "application/javascript".to_string())],
        body: dynamic.to_string(),
        redirect_chain: vec![],
    };
    engine.resolve_fetch(pending[0].id, &response);

    // Settle to fire the onload callback
    engine.settle();

    let snapshot = engine.snapshot(braille_wire::SnapMode::Compact);
    eprintln!("snapshot: {snapshot}");

    // The dynamic script should have executed and onload should have fired
    assert!(
        snapshot.contains("Loaded: hello from dynamic script"),
        "should show dynamic script value via onload callback, got: {snapshot}"
    );
}

/// Pattern 3: Webpack chunk loading via <script> tags with promise resolution
/// Entry script calls __webpack_require__.e() which creates script tags,
/// chunks load and register, promises resolve, then app renders.
#[test]
fn webpack_chunk_loading_via_script_tags() {
    let html = r#"<!DOCTYPE html>
<html><head><title>Test</title></head>
<body>
<div class="root">Loading...</div>
<script src="/runtime.js"></script>
<script src="/entry.js"></script>
</body></html>"#;

    let runtime = r#"
(function() {
    var installedModules = {};
    var installedChunks = {};
    var webpackModules = {};

    function require(moduleId) {
        if (installedModules[moduleId]) return installedModules[moduleId].exports;
        var module = installedModules[moduleId] = { exports: {} };
        webpackModules[moduleId](module, module.exports, require);
        return module.exports;
    }

    require.e = function(chunkId) {
        if (installedChunks[chunkId] === 0) return Promise.resolve();
        if (installedChunks[chunkId]) return installedChunks[chunkId][2];

        var promise = new Promise(function(resolve, reject) {
            installedChunks[chunkId] = [resolve, reject];
        });
        installedChunks[chunkId][2] = promise;

        var script = document.createElement('script');
        script.src = '/chunks/' + chunkId + '.js';
        document.head.appendChild(script);
        return promise;
    };

    var jsonp = self.webpackChunk = self.webpackChunk || [];
    var origPush = jsonp.push.bind(jsonp);
    jsonp.push = function(data) {
        var chunkIds = data[0], moreModules = data[1];
        for (var id in moreModules) webpackModules[id] = moreModules[id];
        for (var i = 0; i < chunkIds.length; i++) {
            if (installedChunks[chunkIds[i]]) installedChunks[chunkIds[i]][0]();
            installedChunks[chunkIds[i]] = 0;
        }
        origPush(data);
    };
    for (var i = 0; i < jsonp.length; i++) jsonp.push(jsonp[i]);

    self.__r = require;
})();
"#;

    let entry = r#"
__r.e('ui').then(function() {
    var UI = __r('ui-mod');
    var root = document.querySelector('.root');
    root.textContent = '';
    root.appendChild(UI.createLoginForm());
});
"#;

    let chunk_ui = r#"
(self.webpackChunk = self.webpackChunk || []).push([['ui'], {
    'ui-mod': function(module) {
        module.exports = {
            createLoginForm: function() {
                var div = document.createElement('div');
                div.setAttribute('class', 'login');
                var h = document.createElement('h1');
                h.textContent = 'Welcome Back';
                div.appendChild(h);
                var inp = document.createElement('input');
                inp.setAttribute('id', 'user');
                inp.setAttribute('placeholder', 'Username');
                div.appendChild(inp);
                var btn = document.createElement('button');
                btn.textContent = 'Continue';
                div.appendChild(btn);
                return div;
            }
        };
    }
}]);
"#;

    let mut engine = Engine::new();

    let mut scripts = HashMap::new();
    scripts.insert("/runtime.js".to_string(), runtime.to_string());
    scripts.insert("/entry.js".to_string(), entry.to_string());

    let descriptors = engine.parse_and_collect_scripts(html);
    let errors = engine.execute_scripts_lossy(
        &descriptors,
        &FetchedResources::scripts_only(scripts),
    );
    assert!(errors.is_empty(), "no JS errors during initial load: {errors:?}");

    // Entry created a script tag for /chunks/ui.js — should be pending
    assert!(engine.has_pending_fetches(), "should have pending fetch for chunk");
    let pending = engine.pending_fetches();
    assert!(
        pending[0].url.contains("/chunks/ui.js"),
        "should be fetching ui chunk, got: {}",
        pending[0].url
    );

    // Resolve the chunk fetch
    engine.resolve_fetch(
        pending[0].id,
        &braille_wire::FetchResponseData {
            url: pending[0].url.clone(),
            status: 200,
            status_text: "OK".to_string(),
            headers: vec![],
            body: chunk_ui.to_string(),
            redirect_chain: vec![],
        },
    );

    // Settle — this should:
    // 1. eval the chunk code (webpackChunk.push registers modules)
    // 2. The push resolves the installedChunks promise
    // 3. The .then() callback runs (creates login form)
    engine.settle();

    let snapshot = engine.snapshot(braille_wire::SnapMode::Compact);
    eprintln!("snapshot: {snapshot}");

    assert!(
        !snapshot.contains("Loading"),
        "loader should be gone, got: {snapshot}"
    );
    assert!(
        snapshot.contains("Welcome Back"),
        "should show Welcome Back heading, got: {snapshot}"
    );
    assert!(
        snapshot.contains("Continue"),
        "should show Continue button, got: {snapshot}"
    );
}
