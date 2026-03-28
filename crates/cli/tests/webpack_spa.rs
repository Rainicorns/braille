//! Webpack-style SPA test — reproduces the ProtonMail loading architecture.
//!
//! Pattern: HTML has a loader div + entry scripts. Entry scripts set up a
//! webpack-like runtime, dynamically load chunk scripts via <script> tag
//! insertion, chunks register modules, entry resolves when chunks load,
//! then renders the app and makes API fetches.
//!
//! This catches bugs in:
//! - Dynamic script tag insertion + onload firing
//! - Webpack chunk callback resolution
//! - React-style async rendering after chunk loading
//! - API fetches triggered after app bootstrap

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use braille_wire::{DaemonCommand, DaemonRequest, DaemonResponse, SnapMode};

// ---------------------------------------------------------------------------
// Test HTTP server
// ---------------------------------------------------------------------------

struct TestServer {
    port: u16,
    shutdown_tx: mpsc::Sender<()>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl TestServer {
    fn start() -> Self {
        let server = tiny_http::Server::http("127.0.0.1:0").expect("failed to bind test server");
        let port = server.server_addr().to_ip().unwrap().port();
        let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>();

        let handle = std::thread::spawn(move || loop {
            if shutdown_rx.try_recv().is_ok() {
                break;
            }
            match server.recv_timeout(Duration::from_millis(50)) {
                Ok(Some(req)) => {
                    let url = req.url().to_string();
                    let (status, content_type, body) = handle_request(&url);
                    let response = tiny_http::Response::from_string(body)
                        .with_status_code(status)
                        .with_header(
                            tiny_http::Header::from_bytes(
                                b"Content-Type" as &[u8],
                                content_type.as_bytes(),
                            )
                            .unwrap(),
                        );
                    let _ = req.respond(response);
                }
                Ok(None) => {}
                Err(_) => break,
            }
        });

        TestServer {
            port,
            shutdown_tx,
            handle: Some(handle),
        }
    }

    fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        let _ = self.shutdown_tx.send(());
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

fn handle_request(url: &str) -> (u16, &'static str, String) {
    match url {
        "/" => (
            200,
            "text/html",
            include_str!("../../../tests/fixtures/webpack_spa.html").to_string(),
        ),

        // Preloaded variant: chunks in HTML body before runtime
        "/preloaded" => (
            200,
            "text/html",
            r#"<!DOCTYPE html>
<html><head><meta charset="utf-8"><title>Loading App</title></head>
<body>
<div class="app-root"><div class="loader">Loading App</div></div>
<script src="/assets/chunk.react.js"></script>
<script src="/assets/chunk.app.js"></script>
<script src="/assets/runtime-preload.js"></script>
<script src="/assets/app-entry-preload.js"></script>
</body></html>"#
                .to_string(),
        ),

        // Runtime for preloaded pattern — processes existing chunks
        "/assets/runtime-preload.js" => (
            200,
            "application/javascript",
            r#"
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
    var existing = jsonp.slice();
    for (var i = 0; i < existing.length; i++) {
        var data = existing[i];
        for (var id in data[1]) webpackModules[id] = data[1][id];
        for (var j = 0; j < data[0].length; j++) installedChunks[data[0][j]] = 0;
    }
    jsonp.push = function(data) {
        for (var id in data[1]) webpackModules[id] = data[1][id];
        for (var i = 0; i < data[0].length; i++) installedChunks[data[0][i]] = 0;
    };

    self.__webpack_require__ = __webpack_require__;
})();
"#
            .to_string(),
        ),

        // Entry for preloaded pattern
        "/assets/app-entry-preload.js" => (
            200,
            "application/javascript",
            r#"
Promise.all([
    __webpack_require__.e('react'),
    __webpack_require__.e('app')
]).then(function() {
    var React = __webpack_require__('react');
    var App = __webpack_require__('app');
    App.render(React);
}).catch(function(err) {
    console.error('Entry failed:', err);
});
"#
            .to_string(),
        ),

        // Webpack runtime — sets up chunk loading infrastructure
        "/assets/runtime.js" => (
            200,
            "application/javascript",
            r#"
// Simplified webpack 5 runtime
(function() {
    var installedModules = {};
    var installedChunks = {};
    var deferredModules = [];
    var webpackModules = {};

    // __webpack_require__ — the module loader
    function __webpack_require__(moduleId) {
        if (installedModules[moduleId]) {
            return installedModules[moduleId].exports;
        }
        var module = installedModules[moduleId] = { id: moduleId, loaded: false, exports: {} };
        if (!webpackModules[moduleId]) {
            throw new Error('Module not found: ' + moduleId);
        }
        webpackModules[moduleId](module, module.exports, __webpack_require__);
        module.loaded = true;
        return module.exports;
    }

    // Chunk loading via script tag insertion (like real webpack)
    __webpack_require__.e = function(chunkId) {
        if (installedChunks[chunkId] === 0) {
            return Promise.resolve();
        }
        if (installedChunks[chunkId]) {
            return installedChunks[chunkId][2];
        }
        var promise = new Promise(function(resolve, reject) {
            installedChunks[chunkId] = [resolve, reject];
        });
        installedChunks[chunkId][2] = promise;

        // Create a script tag — this is what webpack does in real browsers
        var script = document.createElement('script');
        script.src = '/assets/chunk.' + chunkId + '.js';
        script.onload = function() {};
        script.onerror = function() {
            var chunk = installedChunks[chunkId];
            if (chunk) {
                chunk[1](new Error('Loading chunk ' + chunkId + ' failed'));
                installedChunks[chunkId] = undefined;
            }
        };
        document.head.appendChild(script);
        return promise;
    };

    // Public path
    __webpack_require__.p = '';

    // webpackJsonpCallback — called by chunk scripts to register their modules
    var webpackJsonp = self.webpackChunkapp = self.webpackChunkapp || [];
    var oldPush = webpackJsonp.push.bind(webpackJsonp);
    webpackJsonp.push = function(data) {
        var chunkIds = data[0];
        var moreModules = data[1];
        var executeModules = data[2];

        // Register modules from this chunk
        for (var moduleId in moreModules) {
            webpackModules[moduleId] = moreModules[moduleId];
        }

        // Mark chunks as installed and resolve their promises
        for (var i = 0; i < chunkIds.length; i++) {
            var chunkId = chunkIds[i];
            if (installedChunks[chunkId]) {
                installedChunks[chunkId][0](); // resolve the promise
            }
            installedChunks[chunkId] = 0;
        }

        // Push to original array too
        oldPush(data);

        // Execute deferred modules if all chunks are ready
        if (executeModules) {
            for (var j = 0; j < executeModules.length; j++) {
                deferredModules.push(executeModules[j]);
            }
        }
    };

    // Process any chunks that were pushed before the runtime loaded
    for (var i = 0; i < webpackJsonp.length; i++) {
        webpackJsonp.push(webpackJsonp[i]);
    }

    // Expose __webpack_require__ for the entry script
    self.__webpack_require__ = __webpack_require__;
})();
"#
            .to_string(),
        ),

        // Entry script — loads chunks and renders
        "/assets/app-entry.js" => (
            200,
            "application/javascript",
            r#"
// Entry script: load chunks, then render
Promise.all([
    __webpack_require__.e('react'),
    __webpack_require__.e('app')
]).then(function() {
    // All chunks loaded — require the app module
    var React = __webpack_require__('react');
    var App = __webpack_require__('app');
    App.render(React);
}).catch(function(err) {
    console.error('Chunk loading failed:', err);
});
"#
            .to_string(),
        ),

        // React chunk — provides a minimal React-like API
        "/assets/chunk.react.js" => (
            200,
            "application/javascript",
            r#"
(self.webpackChunkapp = self.webpackChunkapp || []).push([['react'], {
    'react': function(module, exports, __webpack_require__) {
        function createElement(tag, props) {
            var el = document.createElement(tag);
            if (props) {
                for (var k in props) {
                    if (k === 'className') el.setAttribute('class', props[k]);
                    else if (k === 'id') el.id = props[k];
                    else if (k === 'textContent') el.textContent = props[k];
                    else if (k.indexOf('on') === 0) el.addEventListener(k.substring(2).toLowerCase(), props[k]);
                    else el.setAttribute(k, props[k]);
                }
            }
            for (var i = 2; i < arguments.length; i++) {
                var child = arguments[i];
                if (child == null) continue;
                if (typeof child === 'string') child = document.createTextNode(child);
                el.appendChild(child);
            }
            return el;
        }
        function render(element, container) {
            // Clear container and append
            while (container.firstChild) container.removeChild(container.firstChild);
            container.appendChild(element);
        }
        module.exports = { createElement: createElement, render: render };
    }
}]);
"#
            .to_string(),
        ),

        // App chunk — the actual application code
        "/assets/chunk.app.js" => (
            200,
            "application/javascript",
            r#"
(self.webpackChunkapp = self.webpackChunkapp || []).push([['app'], {
    'app': function(module, exports, __webpack_require__) {
        module.exports = {
            render: function(React) {
                var root = document.querySelector('.app-root');
                if (!root) { console.error('No .app-root found'); return; }

                // First render: loading state while we fetch config
                React.render(
                    React.createElement('div', {className: 'app-loading'}, 'Fetching config...'),
                    root
                );

                // Fetch app config (like ProtonMail fetches /api/core/v4/...)
                fetch('/api/config').then(function(resp) {
                    return resp.json();
                }).then(function(config) {
                    // Render login form with config data
                    var form = React.createElement('div', {className: 'login-page'},
                        React.createElement('h1', {}, config.appName),
                        React.createElement('form', {id: 'login-form'},
                            React.createElement('label', {}, 'Email'),
                            React.createElement('input', {type: 'email', id: 'email', placeholder: 'user@example.com'}),
                            React.createElement('label', {}, 'Password'),
                            React.createElement('input', {type: 'password', id: 'password'}),
                            React.createElement('button', {type: 'submit', id: 'signin-btn', textContent: 'Sign in'})
                        ),
                        React.createElement('a', {href: '/signup', textContent: 'Create account'})
                    );
                    React.render(form, root);
                }).catch(function(err) {
                    React.render(
                        React.createElement('div', {className: 'error'}, 'Failed to load: ' + err),
                        root
                    );
                });
            }
        };
    }
}]);
"#
            .to_string(),
        ),

        // API: config endpoint
        "/api/config" => (
            200,
            "application/json",
            r#"{"appName":"Test App Login","version":"1.0.0","features":["auth","crypto"]}"#
                .to_string(),
        ),

        _ => (404, "text/plain", "Not found".to_string()),
    }
}

// ---------------------------------------------------------------------------
// Daemon helpers (same as kitchen_sink)
// ---------------------------------------------------------------------------

fn send(socket: &std::path::Path, request: &DaemonRequest) -> DaemonResponse {
    let mut stream = UnixStream::connect(socket).expect("connect to daemon");
    stream.set_read_timeout(Some(Duration::from_secs(30))).ok();
    let json = serde_json::to_string(request).unwrap();
    stream.write_all(json.as_bytes()).unwrap();
    stream.write_all(b"\n").unwrap();
    stream.flush().unwrap();
    let mut reader = BufReader::new(&stream);
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    serde_json::from_str(&line).unwrap()
}

fn new_session(socket: &std::path::Path) -> String {
    let resp = send(
        socket,
        &DaemonRequest {
            session_id: None,
            command: DaemonCommand::NewSession,
        },
    );
    assert!(resp.success, "NewSession failed: {:?}", resp.error);
    resp.session_id.expect("session_id missing")
}

fn goto(socket: &std::path::Path, sid: &str, url: &str) -> String {
    let resp = send(
        socket,
        &DaemonRequest {
            session_id: Some(sid.to_string()),
            command: DaemonCommand::Goto {
                url: url.to_string(),
                mode: SnapMode::Compact,
                record_path: None,
            },
        },
    );
    assert!(resp.success, "Goto {url} failed: {:?}", resp.error);
    resp.content.unwrap_or_default()
}

fn stop_daemon(socket: &std::path::Path) {
    let _ = send(
        socket,
        &DaemonRequest {
            session_id: None,
            command: DaemonCommand::DaemonStop,
        },
    );
}

fn start_daemon_in_thread(tmp: &std::path::Path) -> PathBuf {
    std::fs::create_dir_all(tmp).ok();
    let socket = tmp.join("daemon.sock");
    let pid = tmp.join("daemon.pid");
    let sock_clone = socket.clone();
    std::thread::spawn(move || {
        braille_cli::daemon::run_daemon(sock_clone, pid);
    });
    for _ in 0..50 {
        std::thread::sleep(Duration::from_millis(100));
        if socket.exists() && UnixStream::connect(&socket).is_ok() {
            return socket;
        }
    }
    panic!("daemon did not start");
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn webpack_dynamic_chunks_load_and_app_renders() {
    let server = TestServer::start();
    let tmp = std::env::temp_dir().join(format!("braille-webpack-spa-{}", std::process::id()));
    let socket = start_daemon_in_thread(&tmp);
    let sid = new_session(&socket);

    let content = goto(&socket, &sid, &server.url());

    // The app should have:
    // 1. Loaded runtime.js (webpack runtime)
    // 2. Loaded app-entry.js (triggers chunk loading)
    // 3. Dynamically loaded chunk.react.js and chunk.app.js via <script> tag insertion
    // 4. Chunks registered modules via webpackJsonpCallback
    // 5. Entry script resolved chunk promises and called App.render()
    // 6. App.render() fetched /api/config
    // 7. Config response triggered login form render

    // Must NOT still show the loading state
    assert!(
        !content.contains("Loading App"),
        "should not still show initial loader, got: {content}"
    );
    assert!(
        !content.contains("Fetching config"),
        "should not still show fetching state, got: {content}"
    );

    // Must show the rendered login form
    assert!(
        content.contains("Test App Login"),
        "should show app name from config API, got: {content}"
    );
    assert!(
        content.contains("Sign in"),
        "should show sign in button, got: {content}"
    );
    assert!(
        content.contains("Email"),
        "should show email label, got: {content}"
    );
    assert!(
        content.contains("Create account"),
        "should show create account link, got: {content}"
    );

    stop_daemon(&socket);
    std::fs::remove_dir_all(&tmp).ok();
}

/// ProtonMail pattern: chunks are preloaded in HTML body before runtime.
/// The chunks execute and push to a plain array. Runtime processes them.
/// Entry resolves chunk promises immediately and triggers API fetches.
#[test]
fn preloaded_chunks_with_api_fetch() {
    let server = TestServer::start();
    let tmp = std::env::temp_dir().join(format!("braille-webpack-preload-{}", std::process::id()));
    let socket = start_daemon_in_thread(&tmp);
    let sid = new_session(&socket);

    let content = goto(&socket, &sid, &format!("{}/preloaded", server.url()));

    // Must show the rendered login form from the API config
    assert!(
        !content.contains("Loading App"),
        "should not still show loader, got: {content}"
    );
    assert!(
        content.contains("Test App Login"),
        "should show app name from config API, got: {content}"
    );
    assert!(
        content.contains("Sign in"),
        "should show sign in button, got: {content}"
    );

    stop_daemon(&socket);
    std::fs::remove_dir_all(&tmp).ok();
}
