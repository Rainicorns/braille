//! End-to-end tests for the fetch API pipeline.
//!
//! Spins up a real HTTP server (tiny_http), loads HTML that calls `fetch()`,
//! and verifies the full round-trip:
//!   JS fetch() → engine PendingFetch → Rust reads it → real HTTP via reqwest
//!   → engine.resolve_fetch() → JS .then() callback fires → DOM updates
//!
//! This exercises the native fetch implementation (not a JS mock).

use std::sync::mpsc;
use std::thread;

use braille_engine::{Engine, FetchedResources};
use braille_wire::{FetchResponseData, SnapMode};

// ---------------------------------------------------------------------------
// Test HTTP server helpers
// ---------------------------------------------------------------------------

struct TestServer {
    port: u16,
    shutdown_tx: mpsc::Sender<()>,
    handle: Option<thread::JoinHandle<()>>,
}

impl TestServer {
    /// Start a tiny_http server on a random port. `handler` is called for each
    /// request and returns (status_code, content_type, body).
    fn start<F>(handler: F) -> Self
    where
        F: Fn(&str, &str) -> (u16, &'static str, String) + Send + Sync + 'static,
    {
        let server = tiny_http::Server::http("127.0.0.1:0").expect("failed to bind test server");
        let port = server.server_addr().to_ip().unwrap().port();
        let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>();

        let handle = thread::spawn(move || loop {
            if shutdown_rx.try_recv().is_ok() {
                break;
            }
            match server.recv_timeout(std::time::Duration::from_millis(50)) {
                Ok(Some(req)) => {
                    let method = req.method().to_string();
                    let url = req.url().to_string();
                    let (status, content_type, body) = handler(&url, &method);
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

    fn base_url(&self) -> String {
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

// ---------------------------------------------------------------------------
// Fetch servicing — uses reqwest directly (same as CLI's NetworkClient)
// ---------------------------------------------------------------------------

fn service_fetches(client: &reqwest::blocking::Client, engine: &mut Engine) {
    for _ in 0..20 {
        if !engine.has_pending_fetches() {
            break;
        }
        let pending = engine.pending_fetches();
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
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "",
    }
}

/// Load HTML into the engine and service any pending fetches.
fn load_and_service(engine: &mut Engine, client: &reqwest::blocking::Client, html: &str) {
    load_and_service_with_url(engine, client, html, "http://localhost/");
}

fn load_and_service_with_url(
    engine: &mut Engine,
    client: &reqwest::blocking::Client,
    html: &str,
    url: &str,
) {
    let descriptors = engine.parse_and_collect_scripts(html);
    engine.execute_scripts(&descriptors, &FetchedResources::default());
    engine.set_url(url);
    engine.settle();
    service_fetches(client, engine);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn fetch_get_json_resolves_and_updates_dom() {
    let server = TestServer::start(|url, _method| match url {
        "/api/data" => (
            200,
            "application/json",
            r#"{"name":"Alice","score":42}"#.to_string(),
        ),
        _ => (404, "text/plain", "not found".to_string()),
    });

    let html = format!(
        concat!(
            "<html><body>\n",
            "<p id=\"result\">pending</p>\n",
            "<script>\n",
            "fetch(\"{base}/api/data\")\n",
            "  .then(function(r) {{ return r.json(); }})\n",
            "  .then(function(data) {{\n",
            "    document.getElementById(\"result\").textContent = data.name + \":\" + data.score;\n",
            "  }});\n",
            "</script>\n",
            "</body></html>"
        ),
        base = server.base_url()
    );

    let mut engine = Engine::new();
    let client = reqwest::blocking::Client::new();
    load_and_service(&mut engine, &client, &html);

    let snap = engine.snapshot(SnapMode::Accessibility);
    assert!(
        snap.contains("Alice:42"),
        "fetch should resolve and update DOM: {}",
        snap
    );
}

#[test]
fn fetch_get_text_resolves() {
    let server = TestServer::start(|url, _method| match url {
        "/hello" => (200, "text/plain", "Hello from server!".to_string()),
        _ => (404, "text/plain", "not found".to_string()),
    });

    let html = format!(
        concat!(
            "<html><body>\n",
            "<p id=\"result\">pending</p>\n",
            "<script>\n",
            "fetch(\"{base}/hello\")\n",
            "  .then(function(r) {{ return r.text(); }})\n",
            "  .then(function(text) {{\n",
            "    document.getElementById(\"result\").textContent = text;\n",
            "  }});\n",
            "</script>\n",
            "</body></html>"
        ),
        base = server.base_url()
    );

    let mut engine = Engine::new();
    let client = reqwest::blocking::Client::new();
    load_and_service(&mut engine, &client, &html);

    let snap = engine.snapshot(SnapMode::Accessibility);
    assert!(
        snap.contains("Hello from server!"),
        "fetch text should resolve: {}",
        snap
    );
}

#[test]
fn fetch_post_sends_correctly() {
    let server = TestServer::start(|url, method| {
        if url == "/api/echo" {
            (
                200,
                "application/json",
                format!(r#"{{"method":"{}","ok":true}}"#, method),
            )
        } else {
            (404, "text/plain", "not found".to_string())
        }
    });

    let html = format!(
        concat!(
            "<html><body>\n",
            "<p id=\"result\">pending</p>\n",
            "<script>\n",
            "fetch(\"{base}/api/echo\", {{\n",
            "  method: \"POST\",\n",
            "  headers: {{ \"Content-Type\": \"application/json\" }},\n",
            "  body: JSON.stringify({{ key: \"value\" }})\n",
            "}})\n",
            "  .then(function(r) {{ return r.json(); }})\n",
            "  .then(function(data) {{\n",
            "    document.getElementById(\"result\").textContent = data.method + \":\" + data.ok;\n",
            "  }});\n",
            "</script>\n",
            "</body></html>"
        ),
        base = server.base_url()
    );

    let mut engine = Engine::new();
    let client = reqwest::blocking::Client::new();
    load_and_service(&mut engine, &client, &html);

    let snap = engine.snapshot(SnapMode::Accessibility);
    assert!(
        snap.contains("POST:true"),
        "POST fetch should resolve: {}",
        snap
    );
}

#[test]
fn fetch_checks_ok_and_status() {
    let server = TestServer::start(|url, _method| match url {
        "/ok" => (200, "text/plain", "fine".to_string()),
        "/not-found" => (404, "text/plain", "gone".to_string()),
        _ => (500, "text/plain", "error".to_string()),
    });

    let html = format!(
        concat!(
            "<html><body>\n",
            "<p id=\"r1\">pending</p>\n",
            "<p id=\"r2\">pending</p>\n",
            "<script>\n",
            "fetch(\"{base}/ok\").then(function(r) {{\n",
            "  document.getElementById(\"r1\").textContent = \"ok=\" + r.ok + \" status=\" + r.status;\n",
            "}});\n",
            "fetch(\"{base}/not-found\").then(function(r) {{\n",
            "  document.getElementById(\"r2\").textContent = \"ok=\" + r.ok + \" status=\" + r.status;\n",
            "}});\n",
            "</script>\n",
            "</body></html>"
        ),
        base = server.base_url()
    );

    let mut engine = Engine::new();
    let client = reqwest::blocking::Client::new();
    load_and_service(&mut engine, &client, &html);

    let snap = engine.snapshot(SnapMode::Accessibility);
    assert!(snap.contains("ok=true status=200"), "200 should be ok=true: {}", snap);
    assert!(
        snap.contains("ok=false status=404"),
        "404 should be ok=false: {}",
        snap
    );
}

#[test]
fn fetch_response_headers_get() {
    let server = TestServer::start(|url, _method| match url {
        "/with-headers" => (200, "application/json", r#"{"a":1}"#.to_string()),
        _ => (404, "text/plain", "nope".to_string()),
    });

    let html = format!(
        concat!(
            "<html><body>\n",
            "<p id=\"result\">pending</p>\n",
            "<script>\n",
            "fetch(\"{base}/with-headers\").then(function(r) {{\n",
            "  var ct = r.headers.get(\"content-type\");\n",
            "  document.getElementById(\"result\").textContent = \"ct=\" + ct;\n",
            "}});\n",
            "</script>\n",
            "</body></html>"
        ),
        base = server.base_url()
    );

    let mut engine = Engine::new();
    let client = reqwest::blocking::Client::new();
    load_and_service(&mut engine, &client, &html);

    let snap = engine.snapshot(SnapMode::Accessibility);
    assert!(
        snap.contains("ct=application/json"),
        "headers.get should work: {}",
        snap
    );
}

#[test]
fn fetch_network_error_rejects() {
    // Port 1 is almost certainly not listening
    let html = concat!(
        "<html><body>\n",
        "<p id=\"result\">pending</p>\n",
        "<script>\n",
        "fetch(\"http://127.0.0.1:1/will-fail\")\n",
        "  .then(function(r) {\n",
        "    document.getElementById(\"result\").textContent = \"unexpected-success\";\n",
        "  })\n",
        "  .catch(function(err) {\n",
        "    document.getElementById(\"result\").textContent = \"caught-error\";\n",
        "  });\n",
        "</script>\n",
        "</body></html>"
    );

    let mut engine = Engine::new();
    let client = reqwest::blocking::Client::new();
    load_and_service(&mut engine, &client, html);

    let snap = engine.snapshot(SnapMode::Accessibility);
    assert!(
        snap.contains("caught-error"),
        "network error should reject the promise: {}",
        snap
    );
}

#[test]
fn fetch_chained_sequential_requests() {
    let server = TestServer::start(|url, _method| match url {
        "/step1" => (200, "application/json", r#"{"next":"/step2"}"#.to_string()),
        "/step2" => (200, "application/json", r#"{"value":"chain-done"}"#.to_string()),
        _ => (404, "text/plain", "not found".to_string()),
    });

    let html = format!(
        concat!(
            "<html><body>\n",
            "<p id=\"result\">pending</p>\n",
            "<script>\n",
            "fetch(\"{base}/step1\")\n",
            "  .then(function(r) {{ return r.json(); }})\n",
            "  .then(function(data) {{ return fetch(\"{base}\" + data.next); }})\n",
            "  .then(function(r) {{ return r.json(); }})\n",
            "  .then(function(data) {{\n",
            "    document.getElementById(\"result\").textContent = data.value;\n",
            "  }});\n",
            "</script>\n",
            "</body></html>"
        ),
        base = server.base_url()
    );

    let mut engine = Engine::new();
    let client = reqwest::blocking::Client::new();
    load_and_service(&mut engine, &client, &html);

    let snap = engine.snapshot(SnapMode::Accessibility);
    assert!(
        snap.contains("chain-done"),
        "chained fetches should resolve: {}",
        snap
    );
}

#[test]
fn fetch_parallel_requests() {
    let server = TestServer::start(|url, _method| match url {
        "/a" => (200, "application/json", r#"{"v":"alpha"}"#.to_string()),
        "/b" => (200, "application/json", r#"{"v":"beta"}"#.to_string()),
        _ => (404, "text/plain", "not found".to_string()),
    });

    let html = format!(
        concat!(
            "<html><body>\n",
            "<p id=\"ra\">pending</p>\n",
            "<p id=\"rb\">pending</p>\n",
            "<script>\n",
            "fetch(\"{base}/a\")\n",
            "  .then(function(r) {{ return r.json(); }})\n",
            "  .then(function(d) {{ document.getElementById(\"ra\").textContent = d.v; }});\n",
            "fetch(\"{base}/b\")\n",
            "  .then(function(r) {{ return r.json(); }})\n",
            "  .then(function(d) {{ document.getElementById(\"rb\").textContent = d.v; }});\n",
            "</script>\n",
            "</body></html>"
        ),
        base = server.base_url()
    );

    let mut engine = Engine::new();
    let client = reqwest::blocking::Client::new();
    load_and_service(&mut engine, &client, &html);

    let snap = engine.snapshot(SnapMode::Accessibility);
    assert!(snap.contains("alpha"), "first fetch should resolve: {}", snap);
    assert!(snap.contains("beta"), "second fetch should resolve: {}", snap);
}

#[test]
fn fetch_triggered_by_click() {
    let server = TestServer::start(|url, _method| match url {
        "/api/click-data" => (200, "application/json", r#"{"message":"clicked!"}"#.to_string()),
        _ => (404, "text/plain", "not found".to_string()),
    });

    let html = format!(
        concat!(
            "<html><body>\n",
            "<p id=\"result\">waiting</p>\n",
            "<button id=\"btn\">Load</button>\n",
            "<script>\n",
            "document.getElementById(\"btn\").addEventListener(\"click\", function() {{\n",
            "  fetch(\"{base}/api/click-data\")\n",
            "    .then(function(r) {{ return r.json(); }})\n",
            "    .then(function(data) {{\n",
            "      document.getElementById(\"result\").textContent = data.message;\n",
            "    }});\n",
            "}});\n",
            "</script>\n",
            "</body></html>"
        ),
        base = server.base_url()
    );

    let mut engine = Engine::new();
    let client = reqwest::blocking::Client::new();
    load_and_service(&mut engine, &client, &html);

    // Initially "waiting" — no fetch yet
    let snap1 = engine.snapshot(SnapMode::Accessibility);
    assert!(snap1.contains("waiting"), "initial state: {}", snap1);

    // Click triggers fetch
    engine.handle_click("#btn");
    engine.settle();
    service_fetches(&client, &mut engine);

    let snap2 = engine.snapshot(SnapMode::Accessibility);
    assert!(
        snap2.contains("clicked!"),
        "fetch after click should resolve: {}",
        snap2
    );
}

#[test]
fn fetch_spa_list_detail_with_history() {
    let server = TestServer::start(|url, _method| match url {
        "/api/items" => (
            200,
            "application/json",
            r#"[{"id":1,"title":"First"},{"id":2,"title":"Second"}]"#.to_string(),
        ),
        "/api/items/1" => (
            200,
            "application/json",
            r#"{"id":1,"title":"First","body":"Details here"}"#.to_string(),
        ),
        _ => (404, "text/plain", "not found".to_string()),
    });

    let base = server.base_url();

    // Build HTML with JS that uses fetch + history
    // Using concat! to avoid raw string issues with format
    let html = [
        "<html><body>",
        "<p id=\"status\">idle</p>",
        "<button id=\"load\">Load</button>",
        "<div id=\"list\"></div>",
        "<div id=\"detail\"></div>",
        "<script>",
        &format!("var BASE = \"{}\";", base),
        "document.getElementById('load').addEventListener('click', function() {",
        "  document.getElementById('status').textContent = 'loading';",
        "  fetch(BASE + '/api/items')",
        "    .then(function(r) { return r.json(); })",
        "    .then(function(items) {",
        "      var ul = document.createElement('ul');",
        "      ul.id = 'item-list';",
        "      for (var i = 0; i < items.length; i++) {",
        "        var li = document.createElement('li');",
        "        var a = document.createElement('a');",
        "        a.textContent = items[i].title;",
        "        a.id = 'item-' + items[i].id;",
        "        a.setAttribute('data-id', items[i].id);",
        "        a.addEventListener('click', function(e) {",
        "          var id = this.getAttribute('data-id');",
        "          window.history.pushState({id: id}, '', '/items/' + id);",
        "          fetch(BASE + '/api/items/' + id)",
        "            .then(function(r) { return r.json(); })",
        "            .then(function(item) {",
        "              document.getElementById('detail').textContent = item.body;",
        "              document.getElementById('status').textContent = 'detail:' + item.title;",
        "            });",
        "        });",
        "        li.appendChild(a);",
        "        ul.appendChild(li);",
        "      }",
        "      document.getElementById('list').innerHTML = '';",
        "      document.getElementById('list').appendChild(ul);",
        "      document.getElementById('status').textContent = items.length + ' items loaded';",
        "    });",
        "});",
        "</script>",
        "</body></html>",
    ]
    .join("\n");

    let mut engine = Engine::new();
    let client = reqwest::blocking::Client::new();
    let url = format!("{}/", server.base_url());
    load_and_service_with_url(&mut engine, &client, &html, &url);

    // 1. Initial state
    let snap0 = engine.snapshot(SnapMode::Accessibility);
    assert!(snap0.contains("idle"), "initial: {}", snap0);

    // 2. Click Load → fetch items
    engine.handle_click("#load");
    engine.settle();
    service_fetches(&client, &mut engine);
    let snap1 = engine.snapshot(SnapMode::Accessibility);
    assert!(snap1.contains("items loaded"), "items should load: {}", snap1);
    assert!(snap1.contains("First"), "should show First: {}", snap1);
    assert!(snap1.contains("Second"), "should show Second: {}", snap1);

    // 3. Click first item → pushState + fetch detail
    engine.handle_click("#item-1");
    engine.settle();
    service_fetches(&client, &mut engine);
    let snap2 = engine.snapshot(SnapMode::Accessibility);
    assert!(snap2.contains("detail:First"), "should show detail status: {}", snap2);

    // 4. Verify history was updated
    let len = engine.eval_js("window.history.length").unwrap();
    assert_eq!(len, "2", "history should have 2 entries: {}", len);

    let path = engine.eval_js("window.location.pathname").unwrap();
    assert_eq!(path, "/items/1", "pathname: {}", path);
}
