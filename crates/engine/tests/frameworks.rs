//! Framework compatibility tests.
//!
//! Loads small apps written with framework-like patterns (React/Preact/Vue-style
//! virtual DOM, reactive state, component composition) and verifies they render
//! correctly and respond to interactions via the Engine API.
//!
//! These tests validate that the DOM API surface is sufficient for real framework
//! patterns: createElement, appendChild, innerHTML, addEventListener, event
//! dispatch, Proxy, input.value, form submission, etc.

use braille_engine::Engine;
use braille_wire::{FetchResponseData, SnapMode};

/// Service all pending fetches on the engine with mock data, matching URLs to responses.
fn service_spa_fetches(engine: &mut Engine) {
    for _ in 0..20 {
        if !engine.has_pending_fetches() {
            break;
        }
        let pending = engine.pending_fetches();
        for req in pending {
            let (status, body) = match req.url.as_str() {
                url if url.ends_with("/api/users/1") => (200, r#"{"id":1,"name":"Alice","email":"alice@example.com","bio":"Software engineer"}"#.to_string()),
                url if url.ends_with("/api/users") => (200, r#"[{"id":1,"name":"Alice","email":"alice@example.com"},{"id":2,"name":"Bob","email":"bob@example.com"},{"id":3,"name":"Charlie","email":"charlie@test.com"}]"#.to_string()),
                url if url.contains("/api/search") => {
                    // Extract query parameter
                    let query = url.split("?q=").nth(1).unwrap_or("").to_string();
                    let query = urlencoding_decode(&query);
                    let all_users = vec![
                        ("Alice", "alice@example.com"),
                        ("Bob", "bob@example.com"),
                        ("Charlie", "charlie@test.com"),
                    ];
                    let results: Vec<String> = all_users.iter()
                        .filter(|(name, _)| name.to_lowercase().contains(&query.to_lowercase()))
                        .map(|(name, email)| format!(r#"{{"id":1,"name":"{}","email":"{}"}}"#, name, email))
                        .collect();
                    (200, format!("[{}]", results.join(",")))
                }
                url if url.ends_with("/api/contact") => (200, r#"{"success":true,"message":"Message sent!"}"#.to_string()),
                _ => (404, r#"{"error":"Not found"}"#.to_string()),
            };
            engine.resolve_fetch(req.id, &FetchResponseData {
                status,
                status_text: if status == 200 { "OK".to_string() } else { "Not Found".to_string() },
                headers: vec![("content-type".to_string(), "application/json".to_string())],
                body,
                url: req.url.clone(),
                redirect_chain: vec![],
            });
        }
        engine.settle();
    }
}

fn urlencoding_decode(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.bytes();
    while let Some(b) = chars.next() {
        if b == b'%' {
            let h = chars.next().unwrap_or(0);
            let l = chars.next().unwrap_or(0);
            let hex = String::from_utf8(vec![h, l]).unwrap_or_default();
            if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                result.push(byte as char);
            }
        } else if b == b'+' {
            result.push(' ');
        } else {
            result.push(b as char);
        }
    }
    result
}

fn load_framework_test(filename: &str) -> (Engine, String) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tests/frameworks")
        .join(filename);
    let html = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", path.display(), e));
    let mut engine = Engine::new();
    engine.load_html(&html);
    let snapshot = engine.snapshot(SnapMode::Accessibility);
    (engine, snapshot)
}

fn snap_framework(filename: &str) -> String {
    let (_engine, snapshot) = load_framework_test(filename);
    snapshot
}

// ---------------------------------------------------------------------------
// Preact-like counter
// ---------------------------------------------------------------------------

#[test]
fn preact_counter_initial_render() {
    let snapshot = snap_framework("preact_counter.html");

    assert!(snapshot.contains("Counter: 0"), "should show initial count");
    assert!(snapshot.contains("Current value: 0"), "should show current value");
}

#[test]
fn preact_counter_increment() {
    let (mut engine, _snapshot) = load_framework_test("preact_counter.html");

    // Click the increment button
    engine.handle_click("#inc");
    let snapshot = engine.snapshot(SnapMode::Accessibility);

    assert!(snapshot.contains("Counter: 1"), "count should be 1 after increment");
    assert!(snapshot.contains("Current value: 1"), "current value should update");
}

#[test]
fn preact_counter_multiple_clicks() {
    let (mut engine, _snapshot) = load_framework_test("preact_counter.html");

    engine.handle_click("#inc");
    engine.handle_click("#inc");
    engine.handle_click("#inc");
    engine.handle_click("#dec");

    let snapshot = engine.snapshot(SnapMode::Accessibility);

    assert!(snapshot.contains("Counter: 2"), "count should be 2");
}

// ---------------------------------------------------------------------------
// Preact-like todo
// ---------------------------------------------------------------------------

#[test]
fn preact_todo_initial_render() {
    let snapshot = snap_framework("preact_todo.html");

    assert!(snapshot.contains("Todo App"), "should show title");
    assert!(snapshot.contains("0 of 0 done"), "should show empty count");
}

#[test]
fn preact_todo_add_item() {
    let (mut engine, _snapshot) = load_framework_test("preact_todo.html");

    // Type into the input and submit
    engine.handle_type("#new-todo", "Buy groceries").unwrap();
    engine.handle_click("button[type=submit]");

    let snapshot = engine.snapshot(SnapMode::Accessibility);

    assert!(snapshot.contains("Buy groceries"), "should show added todo");
    assert!(snapshot.contains("0 of 1 done"), "should update count");
}

// ---------------------------------------------------------------------------
// React-like counter
// ---------------------------------------------------------------------------

#[test]
fn react_counter_initial_render() {
    let snapshot = snap_framework("react_counter.html");

    assert!(snapshot.contains("React Counter"), "should show title");
    assert!(snapshot.contains("Count: 0"), "should show initial count");
}

#[test]
fn react_counter_increment() {
    let (mut engine, _snapshot) = load_framework_test("react_counter.html");

    engine.handle_click("#inc");
    let snapshot = engine.snapshot(SnapMode::Accessibility);

    assert!(snapshot.contains("Count: 1"), "count should be 1");
}

#[test]
fn react_counter_decrement_and_reset() {
    let (mut engine, _snapshot) = load_framework_test("react_counter.html");

    engine.handle_click("#inc");
    engine.handle_click("#inc");
    engine.handle_click("#inc");
    engine.handle_click("#dec");

    let snapshot = engine.snapshot(SnapMode::Accessibility);
    assert!(snapshot.contains("Count: 2"), "count should be 2");

    engine.handle_click("#reset");
    let snapshot = engine.snapshot(SnapMode::Accessibility);
    assert!(snapshot.contains("Count: 0"), "count should reset to 0");
}

// ---------------------------------------------------------------------------
// React-like todo
// ---------------------------------------------------------------------------

#[test]
fn react_todo_initial_render() {
    let snapshot = snap_framework("react_todo.html");

    assert!(snapshot.contains("React Todo"), "should show title");
    assert!(snapshot.contains("0 of 0 completed"), "should show empty count");
}

#[test]
fn react_todo_add_item() {
    let (mut engine, _snapshot) = load_framework_test("react_todo.html");

    engine.handle_type("#new-todo", "Write tests").unwrap();
    engine.handle_click("button[type=submit]");

    let snapshot = engine.snapshot(SnapMode::Accessibility);

    assert!(snapshot.contains("Write tests"), "should show added todo");
    assert!(snapshot.contains("0 of 1 completed"), "should update count");
}

// ---------------------------------------------------------------------------
// Vue-like counter
// ---------------------------------------------------------------------------

#[test]
fn vue_counter_initial_render() {
    let snapshot = snap_framework("vue_counter.html");

    assert!(snapshot.contains("Vue Counter"), "should show title");
    assert!(snapshot.contains("Count: 0"), "should show initial count");
    assert!(snapshot.contains("Zero"), "should show conditional text");
}

#[test]
fn vue_counter_increment() {
    let (mut engine, _snapshot) = load_framework_test("vue_counter.html");

    engine.handle_click("#inc");
    let snapshot = engine.snapshot(SnapMode::Accessibility);

    assert!(snapshot.contains("Count: 1"), "count should be 1");
    assert!(snapshot.contains("Positive"), "should show Positive");
}

#[test]
fn vue_counter_negative() {
    let (mut engine, _snapshot) = load_framework_test("vue_counter.html");

    engine.handle_click("#dec");
    let snapshot = engine.snapshot(SnapMode::Accessibility);

    assert!(snapshot.contains("Count: -1"), "count should be -1");
    assert!(snapshot.contains("Negative"), "should show Negative");
}

// ---------------------------------------------------------------------------
// Static HTML form submit (verifies activation behavior through handle_click)
// ---------------------------------------------------------------------------

#[test]
fn static_form_submit_via_handle_click() {
    let html = r#"
    <html><body>
    <p id="result">none</p>
    <form id="myform">
      <input type="text" id="inp" value="hello">
      <button type="submit" id="btn">Go</button>
    </form>
    <script>
      document.getElementById("myform").addEventListener("submit", function(e) {
        e.preventDefault();
        document.getElementById("result").textContent = "submitted: " + document.getElementById("inp").value;
      });
    </script>
    </body></html>
    "#;
    let mut engine = Engine::new();
    engine.load_html(html);
    engine.snapshot(SnapMode::Accessibility);

    engine.handle_click("#btn");
    let snap = engine.snapshot(SnapMode::Accessibility);
    assert!(snap.contains("submitted: hello"), "form submit should have fired");
}

// ---------------------------------------------------------------------------
// React SPA — comprehensive test of History API, fetch, FormData, routing
// ---------------------------------------------------------------------------

fn load_spa() -> (Engine, String) {
    load_framework_test("react_spa.html")
}

#[test]
fn spa_initial_render_shows_home() {
    let (_engine, snapshot) = load_spa();
    assert!(snapshot.contains("React SPA"), "should show home page title: {}", snapshot);
    assert!(
        snapshot.contains("Welcome to the single-page application demo"),
        "should show welcome text: {}",
        snapshot
    );
}

#[test]
fn spa_navbar_present() {
    let (_engine, snapshot) = load_spa();
    assert!(snapshot.contains("Home"), "nav should show Home: {}", snapshot);
    assert!(snapshot.contains("Users"), "nav should show Users: {}", snapshot);
    assert!(snapshot.contains("Search"), "nav should show Search: {}", snapshot);
    assert!(snapshot.contains("Contact"), "nav should show Contact: {}", snapshot);
}

// -- History API / Client-side routing --

#[test]
fn spa_navigate_to_users_page() {
    let (mut engine, _snap) = load_spa();

    // Navigate via the nav link
    engine.handle_click("#nav-users");
    let snap = engine.snapshot(SnapMode::Accessibility);

    assert!(snap.contains("Users"), "should show Users page title: {}", snap);
    assert!(
        snap.contains("Load Users") || snap.contains("No users loaded"),
        "should show load button or empty state: {}",
        snap
    );
}

#[test]
fn spa_navigate_to_search_page() {
    let (mut engine, _snap) = load_spa();

    engine.handle_click("#nav-search");
    let snap = engine.snapshot(SnapMode::Accessibility);

    assert!(
        snap.contains("Search Users"),
        "should show Search page title: {}",
        snap
    );
}

#[test]
fn spa_navigate_to_contact_page() {
    let (mut engine, _snap) = load_spa();

    engine.handle_click("#nav-contact");
    let snap = engine.snapshot(SnapMode::Accessibility);

    assert!(
        snap.contains("Contact Us"),
        "should show Contact page title: {}",
        snap
    );
    assert!(
        snap.contains("Send Message"),
        "should show submit button: {}",
        snap
    );
}

#[test]
fn spa_history_push_state_updates_route() {
    let (mut engine, _snap) = load_spa();

    // Navigate to users, then to search
    engine.handle_click("#nav-users");
    let snap1 = engine.snapshot(SnapMode::Accessibility);
    assert!(snap1.contains("Users"), "should be on Users page");

    engine.handle_click("#nav-search");
    let snap2 = engine.snapshot(SnapMode::Accessibility);
    assert!(snap2.contains("Search Users"), "should be on Search page");

    // Verify history length via JS
    let len = engine.eval_js("window.history.length").unwrap();
    // Initial + navigate to /users + navigate to /search = 3
    assert_eq!(len, "3", "history should have 3 entries, got: {}", len);
}

#[test]
fn spa_history_back_returns_to_previous_page() {
    let (mut engine, _snap) = load_spa();

    // Navigate to users then search
    engine.handle_click("#nav-users");
    engine.handle_click("#nav-search");
    let snap_search = engine.snapshot(SnapMode::Accessibility);
    assert!(snap_search.contains("Search Users"), "should be on Search");

    // Go back using the in-app back button
    engine.handle_click("#nav-back");
    engine.settle();
    let snap_users = engine.snapshot(SnapMode::Accessibility);
    assert!(
        snap_users.contains("Users"),
        "should be back on Users page: {}",
        snap_users
    );
}

#[test]
fn spa_404_for_unknown_route() {
    let (mut engine, _snap) = load_spa();

    // Click the dead link in the nav bar to navigate to /nonexistent
    engine.handle_click("#nav-nonexistent");
    engine.settle();
    let snap = engine.snapshot(SnapMode::Accessibility);

    assert!(snap.contains("404"), "should show 404: {}", snap);
    assert!(
        snap.contains("Page not found"),
        "should show not found message: {}",
        snap
    );
}

// -- fetch API (via mock) --

#[test]
fn spa_fetch_users_populates_list() {
    let (mut engine, _snap) = load_spa();

    // Navigate to users page
    engine.handle_click("#nav-users");
    engine.settle();

    // Click "Load Users" button — this triggers a real fetch("/api/users")
    engine.handle_click("#load-users");
    engine.settle();
    service_spa_fetches(&mut engine);
    let snap = engine.snapshot(SnapMode::Accessibility);

    assert!(snap.contains("Alice"), "should show Alice: {}", snap);
    assert!(snap.contains("Bob"), "should show Bob: {}", snap);
    assert!(snap.contains("Charlie"), "should show Charlie: {}", snap);
}

#[test]
fn spa_click_user_shows_detail() {
    let (mut engine, _snap) = load_spa();

    // Navigate to users page and load
    engine.handle_click("#nav-users");
    engine.settle();
    engine.handle_click("#load-users");
    engine.settle();
    service_spa_fetches(&mut engine);

    // Click on Alice — triggers fetch("/api/users/1")
    engine.handle_click("#user-1");
    engine.settle();
    service_spa_fetches(&mut engine);
    let snap = engine.snapshot(SnapMode::Accessibility);

    assert!(snap.contains("Alice"), "should show user name: {}", snap);
    assert!(
        snap.contains("alice@example.com"),
        "should show user email: {}",
        snap
    );
    assert!(
        snap.contains("Software engineer"),
        "should show user bio: {}",
        snap
    );
}

#[test]
fn spa_user_detail_back_to_list() {
    let (mut engine, _snap) = load_spa();

    engine.handle_click("#nav-users");
    engine.settle();
    engine.handle_click("#load-users");
    engine.settle();
    service_spa_fetches(&mut engine);
    engine.handle_click("#user-1");
    engine.settle();
    service_spa_fetches(&mut engine);

    // Click back button
    engine.handle_click("#back-to-users");
    engine.settle();
    let snap = engine.snapshot(SnapMode::Accessibility);

    assert!(
        snap.contains("Users"),
        "should be back on users list page: {}",
        snap
    );
}

// -- Search with fetch --

#[test]
fn spa_search_finds_user() {
    let (mut engine, _snap) = load_spa();

    engine.handle_click("#nav-search");
    engine.settle();

    // Type search query and submit — triggers fetch("/api/search?q=ali")
    engine.handle_type("#search-input", "ali").unwrap();
    engine.handle_click("#search-btn");
    engine.settle();
    service_spa_fetches(&mut engine);
    let snap = engine.snapshot(SnapMode::Accessibility);

    assert!(
        snap.contains("Alice"),
        "search should find Alice: {}",
        snap
    );
}

#[test]
fn spa_search_no_results() {
    let (mut engine, _snap) = load_spa();

    engine.handle_click("#nav-search");
    engine.settle();

    engine.handle_type("#search-input", "zzzzz").unwrap();
    engine.handle_click("#search-btn");
    engine.settle();
    service_spa_fetches(&mut engine);
    let snap = engine.snapshot(SnapMode::Accessibility);

    assert!(
        snap.contains("No results"),
        "should show no results message: {}",
        snap
    );
}

// -- FormData via contact form --

#[test]
fn spa_contact_form_renders() {
    let (mut engine, _snap) = load_spa();

    engine.handle_click("#nav-contact");
    engine.settle();
    let snap = engine.snapshot(SnapMode::Accessibility);

    assert!(snap.contains("Contact Us"), "should show title: {}", snap);
    assert!(snap.contains("Name"), "should show name label: {}", snap);
    assert!(snap.contains("Email"), "should show email label: {}", snap);
    assert!(snap.contains("Message"), "should show message label: {}", snap);
    assert!(
        snap.contains("Send Message"),
        "should show submit button: {}",
        snap
    );
}

#[test]
fn spa_contact_form_submit_uses_formdata() {
    let (mut engine, _snap) = load_spa();

    engine.handle_click("#nav-contact");
    engine.settle();

    // Fill in the form
    engine.handle_type("#contact-name", "Test User").unwrap();
    engine.handle_type("#contact-email", "test@example.com").unwrap();

    // Submit the form — triggers fetch POST to /api/contact
    engine.handle_click("#contact-submit");
    engine.settle();
    service_spa_fetches(&mut engine);
    let snap = engine.snapshot(SnapMode::Accessibility);

    // Check the success message (from resolved fetch response)
    assert!(
        snap.contains("Message sent!"),
        "should show success message: {}",
        snap
    );

    // Check that FormData methods worked correctly
    assert!(
        snap.contains("FormData.has(name): true"),
        "FormData.has should work: {}",
        snap
    );
    assert!(
        snap.contains("FormData.get(name): Test User"),
        "FormData.get should return name value: {}",
        snap
    );
    assert!(
        snap.contains("FormData.getAll(name).length: 1"),
        "FormData.getAll should return array: {}",
        snap
    );
}

#[test]
fn spa_contact_form_reset_and_resubmit() {
    let (mut engine, _snap) = load_spa();

    engine.handle_click("#nav-contact");
    engine.settle();
    engine.handle_type("#contact-name", "First").unwrap();
    engine.handle_click("#contact-submit");
    engine.settle();
    service_spa_fetches(&mut engine);

    let snap1 = engine.snapshot(SnapMode::Accessibility);
    assert!(snap1.contains("Message sent!"), "first submit should succeed");

    // Click "Send Another" to reset
    engine.handle_click("#reset-contact");
    engine.settle();
    let snap2 = engine.snapshot(SnapMode::Accessibility);
    assert!(
        snap2.contains("Contact Us"),
        "should show form again after reset: {}",
        snap2
    );
}

// -- Combined: navigate, fetch, FormData, back --

#[test]
fn spa_full_user_journey() {
    let (mut engine, _snap) = load_spa();

    // 1. Start at home
    let home = engine.snapshot(SnapMode::Accessibility);
    assert!(home.contains("React SPA"), "start at home");

    // 2. Navigate to users, load list
    engine.handle_click("#nav-users");
    engine.settle();
    engine.handle_click("#load-users");
    engine.settle();
    service_spa_fetches(&mut engine);
    let users = engine.snapshot(SnapMode::Accessibility);
    assert!(users.contains("Alice"), "users loaded");

    // 3. View user detail
    engine.handle_click("#user-1");
    engine.settle();
    service_spa_fetches(&mut engine);
    let detail = engine.snapshot(SnapMode::Accessibility);
    assert!(detail.contains("Software engineer"), "user detail shown");

    // 4. Navigate to search
    engine.handle_click("#nav-search");
    engine.settle();
    engine.handle_type("#search-input", "bob").unwrap();
    engine.handle_click("#search-btn");
    engine.settle();
    service_spa_fetches(&mut engine);
    let search = engine.snapshot(SnapMode::Accessibility);
    assert!(search.contains("Bob"), "search found Bob");

    // 5. Navigate to contact and submit
    engine.handle_click("#nav-contact");
    engine.settle();
    engine.handle_type("#contact-name", "Journey User").unwrap();
    engine.handle_click("#contact-submit");
    engine.settle();
    service_spa_fetches(&mut engine);
    let contact = engine.snapshot(SnapMode::Accessibility);
    assert!(contact.contains("Message sent!"), "contact form submitted");

    // 6. Verify history has accumulated
    let len = engine.eval_js("window.history.length").unwrap();
    let len_num: usize = len.parse().unwrap_or(0);
    assert!(
        len_num >= 4,
        "history should have at least 4 entries from navigation, got: {}",
        len
    );
}
