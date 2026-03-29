//! Tests for console capture, eval error routing, and transcript recording.
//!
//! Part 1c of the observability plan: guards the console + transcript pipeline.

use braille_engine::Engine;

fn engine_with_html(html: &str) -> Engine {
    let mut e = Engine::new();
    e.load_html(html);
    e
}

// =========================================================================
// Console capture basics
// =========================================================================

#[test]
fn console_log_captured() {
    let mut e = engine_with_html("<html><body></body></html>");
    e.eval_js("console.log('hello')").unwrap();
    let console = e.drain_console();
    assert!(
        console.iter().any(|line| line.contains("hello")),
        "console.log output should be captured: {:?}",
        console
    );
}

#[test]
fn console_error_captured() {
    let mut e = engine_with_html("<html><body></body></html>");
    e.eval_js("console.error('boom')").unwrap();
    let console = e.drain_console();
    assert!(
        console.iter().any(|line| line.contains("boom")),
        "console.error output should be captured: {:?}",
        console
    );
}

#[test]
fn console_drain_clears_buffer() {
    let mut e = engine_with_html("<html><body></body></html>");
    e.eval_js("console.log('first')").unwrap();
    let first = e.drain_console();
    assert!(!first.is_empty(), "first drain should have content");
    let second = e.drain_console();
    assert!(second.is_empty(), "second drain should be empty: {:?}", second);
}

// =========================================================================
// Eval error routing (Part 1)
// =========================================================================

#[test]
fn eval_error_routes_to_console() {
    // When site JS throws during eval, the error should appear in the console buffer.
    // This validates the eval_or_log mechanism added in Part 1.
    let mut e = engine_with_html(r#"<html><body>
        <script>
            // Set up a timer that will throw when fired
            setTimeout(function() { throw new Error('timer boom'); }, 0);
        </script>
    </body></html>"#);

    // Drain any startup console output
    e.drain_console();

    // Settle to fire the 0ms timer — the throw should route to console, not vanish
    e.settle();

    let console = e.drain_console();
    assert!(
        console.iter().any(|line| line.contains("timer boom")),
        "timer error should appear in console via eval_or_log: {:?}",
        console
    );
}

#[test]
fn settle_timer_error_routes_to_console() {
    let mut e = engine_with_html("<html><body></body></html>");
    e.eval_js("setTimeout(function(){ throw new Error('deferred boom'); }, 0)").unwrap();
    e.drain_console(); // clear

    e.settle();

    let console = e.drain_console();
    assert!(
        console.iter().any(|line| line.contains("deferred boom")),
        "setTimeout error should route to console: {:?}",
        console
    );
}

#[test]
fn dispatch_error_routes_to_console() {
    // safeDispatch in fire_input_events should route errors to console.error,
    // not to a silent global __braille_dispatch_errors.
    let mut e = engine_with_html(r#"<html><body>
        <input id="i" type="text">
        <script>
            document.getElementById('i').addEventListener('focusin', function() {
                throw new Error('handler exploded');
            });
        </script>
    </body></html>"#);

    e.drain_console(); // clear startup output

    // Type into input — fires focusin (which throws), then input, change, etc.
    e.handle_type("#i", "test").unwrap();

    let console = e.drain_console();
    assert!(
        console.iter().any(|line| line.contains("handler exploded")),
        "dispatch errors should appear in console: {:?}",
        console
    );
}

// =========================================================================
// Console survives runtime rebind (fast mode)
// =========================================================================

#[test]
fn console_survives_runtime_rebind() {
    // In Fast mode, the runtime is reused across page loads via rebind_for_new_page.
    // Console output from the second page must work correctly (no stale state refs).
    let mut e = Engine::new();
    e.runtime_mode = braille_engine::RuntimeMode::Fast;

    // First page
    e.load_html("<html><body><script>console.log('page1')</script></body></html>");
    let page1_console = e.drain_console();
    assert!(
        page1_console.iter().any(|line| line.contains("page1")),
        "page1 console should work: {:?}",
        page1_console
    );

    // Second page — runtime is rebound, not recreated
    e.load_html("<html><body><script>console.log('page2')</script></body></html>");
    let page2_console = e.drain_console();
    assert!(
        page2_console.iter().any(|line| line.contains("page2")),
        "page2 console should work after rebind: {:?}",
        page2_console
    );
}

// =========================================================================
// Transcript records console per exchange (Part 1b)
// =========================================================================

#[test]
fn transcript_exchange_has_console_field() {
    // Verify the Exchange struct serializes/deserializes with the console field.
    use braille_engine::transcript::Exchange;
    let exchange = Exchange {
        label: Some("test".to_string()),
        requests: vec![],
        results: vec![],
        console: vec!["[error] something broke".to_string()],
    };

    let json = serde_json::to_string(&exchange).unwrap();
    assert!(json.contains("console"), "Exchange JSON should include console field: {json}");

    let parsed: Exchange = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.console.len(), 1);
    assert!(parsed.console[0].contains("something broke"));
}

#[test]
fn transcript_replay_ignores_console_field() {
    // ReplayFetcher must work with transcripts that have no console field
    // (backwards compat via serde(default)).
    use braille_engine::transcript::Transcript;

    let json = r#"{
        "url": "https://example.com",
        "exchanges": [
            {
                "requests": [{"id": 1, "url": "https://example.com", "method": "GET", "headers": [], "body": null}],
                "results": [{"id": 1, "outcome": {"Ok": {"status": 200, "status_text": "OK", "headers": [], "body": "<html></html>", "url": "https://example.com"}}}]
            }
        ]
    }"#;

    // Should parse without error even though console field is missing
    let transcript: Transcript = serde_json::from_str(json).unwrap();
    assert_eq!(transcript.exchanges[0].console.len(), 0);
}

#[test]
fn transcript_empty_console_not_serialized() {
    // When console is empty, it should be omitted from JSON (skip_serializing_if).
    use braille_engine::transcript::Exchange;

    let exchange = Exchange {
        label: None,
        requests: vec![],
        results: vec![],
        console: vec![],
    };

    let json = serde_json::to_string(&exchange).unwrap();
    assert!(
        !json.contains("console"),
        "Empty console should be omitted from JSON: {json}"
    );
}
