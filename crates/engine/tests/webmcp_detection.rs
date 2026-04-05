//! Tests for WebMCP tool detection in snapshot output.
//!
//! The W3C WebMCP standard (Chrome 146) lets websites declare structured tool
//! interfaces for AI agents via `<meta name="webmcp:tool" ...>` tags.
//! Braille should detect these and surface them in snapshot output.

use braille_engine::Engine;
use braille_wire::SnapMode;

fn snap(html: &str, mode: SnapMode) -> String {
    let mut engine = Engine::new();
    engine.load_html(html);
    engine.snapshot(mode)
}

// ---------------------------------------------------------------------------
// Basic detection
// ---------------------------------------------------------------------------

#[test]
fn webmcp_tools_appear_in_compact_snapshot() {
    let html = r#"<html>
    <head>
        <meta name="webmcp:tool" content='{"name":"search","description":"Search products","parameters":{"query":"string"}}'>
        <meta name="webmcp:tool" content='{"name":"checkout","description":"Checkout cart"}'>
    </head>
    <body>
        <h1>Shop</h1>
        <p>Welcome to the store</p>
    </body>
    </html>"#;

    let result = snap(html, SnapMode::Compact);

    assert!(
        result.contains("[WebMCP Tools]"),
        "compact snapshot should contain WebMCP Tools section: {}",
        result
    );
    assert!(
        result.contains("- search: Search products (params: query)"),
        "should list search tool with params: {}",
        result
    );
    assert!(
        result.contains("- checkout: Checkout cart"),
        "should list checkout tool without params: {}",
        result
    );
}

#[test]
fn webmcp_tools_appear_in_accessibility_snapshot() {
    let html = r#"<html>
    <head>
        <meta name="webmcp:tool" content='{"name":"search","description":"Search products","parameters":{"query":"string"}}'>
    </head>
    <body>
        <p>Content</p>
    </body>
    </html>"#;

    let result = snap(html, SnapMode::Accessibility);

    assert!(
        result.contains("[WebMCP Tools]"),
        "accessibility snapshot should contain WebMCP Tools section: {}",
        result
    );
    assert!(
        result.contains("- search: Search products (params: query)"),
        "should list search tool: {}",
        result
    );
}

// ---------------------------------------------------------------------------
// No WebMCP tags = no section
// ---------------------------------------------------------------------------

#[test]
fn no_webmcp_section_when_no_meta_tags() {
    let html = r#"<html>
    <head><title>Normal Page</title></head>
    <body><p>Hello world</p></body>
    </html>"#;

    let result = snap(html, SnapMode::Compact);

    assert!(
        !result.contains("[WebMCP Tools]"),
        "should not contain WebMCP section when no meta tags: {}",
        result
    );
}

#[test]
fn no_webmcp_section_when_unrelated_meta_tags() {
    let html = r#"<html>
    <head>
        <meta name="viewport" content="width=device-width">
        <meta name="description" content="A test page">
    </head>
    <body><p>Hello</p></body>
    </html>"#;

    let result = snap(html, SnapMode::Compact);

    assert!(
        !result.contains("[WebMCP Tools]"),
        "should not contain WebMCP section for non-webmcp meta tags: {}",
        result
    );
}

// ---------------------------------------------------------------------------
// Multiple parameters
// ---------------------------------------------------------------------------

#[test]
fn webmcp_tool_with_multiple_parameters() {
    let html = r#"<html>
    <head>
        <meta name="webmcp:tool" content='{"name":"book_flight","description":"Book a flight","parameters":{"origin":"string","destination":"string","date":"string"}}'>
    </head>
    <body><p>Travel</p></body>
    </html>"#;

    let result = snap(html, SnapMode::Compact);

    assert!(
        result.contains("[WebMCP Tools]"),
        "should have WebMCP section: {}",
        result
    );
    assert!(
        result.contains("- book_flight: Book a flight (params: "),
        "should list tool with params: {}",
        result
    );
    // All three params should appear (order may vary since JSON object keys aren't ordered)
    assert!(
        result.contains("origin") && result.contains("destination") && result.contains("date"),
        "should list all parameters: {}",
        result
    );
}

// ---------------------------------------------------------------------------
// Malformed content is skipped gracefully
// ---------------------------------------------------------------------------

#[test]
fn malformed_webmcp_content_is_skipped() {
    let html = r#"<html>
    <head>
        <meta name="webmcp:tool" content="not valid json">
        <meta name="webmcp:tool" content='{"name":"valid_tool","description":"Works fine"}'>
    </head>
    <body><p>Content</p></body>
    </html>"#;

    let result = snap(html, SnapMode::Compact);

    assert!(
        result.contains("[WebMCP Tools]"),
        "should still have WebMCP section for the valid tool: {}",
        result
    );
    assert!(
        result.contains("- valid_tool: Works fine"),
        "should show the valid tool: {}",
        result
    );
    assert!(
        !result.contains("not valid json"),
        "should not show malformed content: {}",
        result
    );
}

// ---------------------------------------------------------------------------
// WebMCP section appears after page content
// ---------------------------------------------------------------------------

#[test]
fn webmcp_section_appears_after_page_content() {
    let html = r#"<html>
    <head>
        <meta name="webmcp:tool" content='{"name":"search","description":"Search"}'>
    </head>
    <body><h1>Page Title</h1></body>
    </html>"#;

    let result = snap(html, SnapMode::Compact);

    let content_pos = result.find("Page Title").expect("should have page content");
    let webmcp_pos = result.find("[WebMCP Tools]").expect("should have WebMCP section");

    assert!(
        webmcp_pos > content_pos,
        "WebMCP section should appear after page content: {}",
        result
    );
}
