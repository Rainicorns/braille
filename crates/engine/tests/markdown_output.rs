//! Tests for the Markdown snapshot mode.
//!
//! Verifies that SnapMode::Markdown converts HTML pages into well-formed
//! markdown suitable for LLM content ingestion (e.g., TF-Mem pipelines).

use braille_engine::Engine;
use braille_wire::SnapMode;

fn engine_with_html(html: &str) -> Engine {
    let mut engine = Engine::new();
    engine.load_html(html);
    engine
}

// ---------------------------------------------------------------------------
// Headings: h1-h6 map to # through ######
// ---------------------------------------------------------------------------

#[test]
fn headings_render_as_markdown_hashes() {
    let html = r#"<html><body>
        <h1>Title</h1>
        <h2>Section</h2>
        <h3>Subsection</h3>
        <h4>Level 4</h4>
        <h5>Level 5</h5>
        <h6>Level 6</h6>
    </body></html>"#;

    let mut engine = engine_with_html(html);
    let snap = engine.snapshot(SnapMode::Markdown);

    assert!(snap.contains("# Title"), "h1 should render as '# Title': {}", snap);
    assert!(snap.contains("## Section"), "h2 should render as '## Section': {}", snap);
    assert!(snap.contains("### Subsection"), "h3 should render as '### Subsection': {}", snap);
    assert!(snap.contains("#### Level 4"), "h4 should render as '#### Level 4': {}", snap);
    assert!(snap.contains("##### Level 5"), "h5 should render as '##### Level 5': {}", snap);
    assert!(snap.contains("###### Level 6"), "h6 should render as '###### Level 6': {}", snap);
}

// ---------------------------------------------------------------------------
// Links: <a href="url">text</a> -> [text](url)
// ---------------------------------------------------------------------------

#[test]
fn links_render_as_markdown_links() {
    let html = r#"<html><body>
        <p>Visit <a href="https://example.com">Example Site</a> for more info.</p>
    </body></html>"#;

    let mut engine = engine_with_html(html);
    let snap = engine.snapshot(SnapMode::Markdown);

    assert!(
        snap.contains("[Example Site](https://example.com)"),
        "links should render as [text](url): {}",
        snap
    );
}

#[test]
fn link_without_href_renders_as_plain_text() {
    let html = r#"<html><body>
        <p><a>No link</a></p>
    </body></html>"#;

    let mut engine = engine_with_html(html);
    let snap = engine.snapshot(SnapMode::Markdown);

    assert!(snap.contains("No link"), "anchor without href should render text: {}", snap);
    assert!(!snap.contains("[No link]"), "anchor without href should not be a link: {}", snap);
}

// ---------------------------------------------------------------------------
// Images: <img src="url" alt="text"> -> ![text](url)
// ---------------------------------------------------------------------------

#[test]
fn images_render_as_markdown_images() {
    let html = r#"<html><body>
        <img src="logo.png" alt="Company Logo">
    </body></html>"#;

    let mut engine = engine_with_html(html);
    let snap = engine.snapshot(SnapMode::Markdown);

    assert!(
        snap.contains("![Company Logo](logo.png)"),
        "images should render as ![alt](src): {}",
        snap
    );
}

#[test]
fn image_without_alt_uses_empty_alt() {
    let html = r#"<html><body>
        <img src="photo.jpg">
    </body></html>"#;

    let mut engine = engine_with_html(html);
    let snap = engine.snapshot(SnapMode::Markdown);

    assert!(
        snap.contains("![](photo.jpg)"),
        "image without alt should use empty alt text: {}",
        snap
    );
}

// ---------------------------------------------------------------------------
// Unordered lists: <ul><li> -> - item
// ---------------------------------------------------------------------------

#[test]
fn unordered_list_renders_with_dashes() {
    let html = r#"<html><body>
        <ul>
            <li>First</li>
            <li>Second</li>
            <li>Third</li>
        </ul>
    </body></html>"#;

    let mut engine = engine_with_html(html);
    let snap = engine.snapshot(SnapMode::Markdown);

    assert!(snap.contains("- First"), "ul items should have dash prefix: {}", snap);
    assert!(snap.contains("- Second"), "ul items should have dash prefix: {}", snap);
    assert!(snap.contains("- Third"), "ul items should have dash prefix: {}", snap);
}

// ---------------------------------------------------------------------------
// Ordered lists: <ol><li> -> 1. item
// ---------------------------------------------------------------------------

#[test]
fn ordered_list_renders_with_numbers() {
    let html = r#"<html><body>
        <ol>
            <li>Alpha</li>
            <li>Beta</li>
            <li>Gamma</li>
        </ol>
    </body></html>"#;

    let mut engine = engine_with_html(html);
    let snap = engine.snapshot(SnapMode::Markdown);

    assert!(snap.contains("1. Alpha"), "ol items should be numbered: {}", snap);
    assert!(snap.contains("2. Beta"), "ol items should be numbered: {}", snap);
    assert!(snap.contains("3. Gamma"), "ol items should be numbered: {}", snap);
}

// ---------------------------------------------------------------------------
// Tables: HTML tables render as markdown tables
// ---------------------------------------------------------------------------

#[test]
fn table_renders_as_markdown_table() {
    let html = r#"<html><body>
        <table>
            <thead><tr><th>Name</th><th>Age</th></tr></thead>
            <tbody>
                <tr><td>Alice</td><td>30</td></tr>
                <tr><td>Bob</td><td>25</td></tr>
            </tbody>
        </table>
    </body></html>"#;

    let mut engine = engine_with_html(html);
    let snap = engine.snapshot(SnapMode::Markdown);

    assert!(snap.contains("| Name | Age |"), "table header should render as pipe-delimited row: {}", snap);
    assert!(snap.contains("| --- | --- |") || snap.contains("|---|---|"), "table should have separator row: {}", snap);
    assert!(snap.contains("| Alice | 30 |"), "table body rows should render: {}", snap);
    assert!(snap.contains("| Bob | 25 |"), "table body rows should render: {}", snap);
}

#[test]
fn table_without_thead_uses_first_row_as_header() {
    let html = r#"<html><body>
        <table>
            <tr><td>X</td><td>Y</td></tr>
            <tr><td>1</td><td>2</td></tr>
        </table>
    </body></html>"#;

    let mut engine = engine_with_html(html);
    let snap = engine.snapshot(SnapMode::Markdown);

    assert!(snap.contains("| X | Y |"), "first row should become header: {}", snap);
    assert!(snap.contains("| 1 | 2 |"), "subsequent rows should render: {}", snap);
}

// ---------------------------------------------------------------------------
// Code: <pre>/<code> become fenced code blocks
// ---------------------------------------------------------------------------

#[test]
fn pre_code_renders_as_fenced_code_block() {
    let html = r#"<html><body>
        <pre><code>fn main() {
    println!("hello");
}</code></pre>
    </body></html>"#;

    let mut engine = engine_with_html(html);
    let snap = engine.snapshot(SnapMode::Markdown);

    assert!(snap.contains("```"), "pre/code should produce fenced code block: {}", snap);
    assert!(snap.contains("fn main()"), "code content should be preserved: {}", snap);
    assert!(snap.contains("println!"), "code content should be preserved: {}", snap);
}

#[test]
fn inline_code_renders_with_backticks() {
    let html = r#"<html><body>
        <p>Use the <code>println!</code> macro.</p>
    </body></html>"#;

    let mut engine = engine_with_html(html);
    let snap = engine.snapshot(SnapMode::Markdown);

    assert!(
        snap.contains("`println!`"),
        "inline code should use single backticks: {}",
        snap
    );
}

// ---------------------------------------------------------------------------
// Blockquotes
// ---------------------------------------------------------------------------

#[test]
fn blockquote_renders_with_angle_bracket() {
    let html = r#"<html><body>
        <blockquote>This is a quote.</blockquote>
    </body></html>"#;

    let mut engine = engine_with_html(html);
    let snap = engine.snapshot(SnapMode::Markdown);

    assert!(
        snap.contains("> This is a quote."),
        "blockquote should render with > prefix: {}",
        snap
    );
}

// ---------------------------------------------------------------------------
// Bold and italic
// ---------------------------------------------------------------------------

#[test]
fn bold_renders_with_double_asterisks() {
    let html = r#"<html><body>
        <p>This is <strong>important</strong> text.</p>
    </body></html>"#;

    let mut engine = engine_with_html(html);
    let snap = engine.snapshot(SnapMode::Markdown);

    assert!(
        snap.contains("**important**"),
        "strong/b should render as **text**: {}",
        snap
    );
}

#[test]
fn italic_renders_with_single_asterisks() {
    let html = r#"<html><body>
        <p>This is <em>emphasized</em> text.</p>
    </body></html>"#;

    let mut engine = engine_with_html(html);
    let snap = engine.snapshot(SnapMode::Markdown);

    assert!(
        snap.contains("*emphasized*"),
        "em/i should render as *text*: {}",
        snap
    );
}

// ---------------------------------------------------------------------------
// Horizontal rules
// ---------------------------------------------------------------------------

#[test]
fn hr_renders_as_markdown_rule() {
    let html = r#"<html><body>
        <p>Above</p>
        <hr>
        <p>Below</p>
    </body></html>"#;

    let mut engine = engine_with_html(html);
    let snap = engine.snapshot(SnapMode::Markdown);

    assert!(snap.contains("---"), "hr should render as ---: {}", snap);
    assert!(snap.contains("Above"), "content above hr should appear: {}", snap);
    assert!(snap.contains("Below"), "content below hr should appear: {}", snap);
}

// ---------------------------------------------------------------------------
// Script/style/head content should not appear
// ---------------------------------------------------------------------------

#[test]
fn skips_script_style_head() {
    let html = r#"<html>
        <head><title>Page</title><style>body { color: red; }</style></head>
        <body>
            <script>var x = 1;</script>
            <p>Visible text</p>
        </body>
    </html>"#;

    let mut engine = engine_with_html(html);
    let snap = engine.snapshot(SnapMode::Markdown);

    assert!(snap.contains("Visible text"), "visible content should appear: {}", snap);
    assert!(!snap.contains("var x"), "script content should be hidden: {}", snap);
    assert!(!snap.contains("color: red"), "style content should be hidden: {}", snap);
}

// ---------------------------------------------------------------------------
// Nested structures: list containing links
// ---------------------------------------------------------------------------

#[test]
fn nested_list_with_links() {
    let html = r#"<html><body>
        <ul>
            <li><a href="/home">Home</a></li>
            <li><a href="/about">About</a></li>
        </ul>
    </body></html>"#;

    let mut engine = engine_with_html(html);
    let snap = engine.snapshot(SnapMode::Markdown);

    assert!(snap.contains("[Home](/home)"), "links inside list items should render: {}", snap);
    assert!(snap.contains("[About](/about)"), "links inside list items should render: {}", snap);
    assert!(snap.contains("- "), "list markers should be present: {}", snap);
}

// ---------------------------------------------------------------------------
// Paragraphs should be separated by blank lines
// ---------------------------------------------------------------------------

#[test]
fn paragraphs_separated_by_blank_lines() {
    let html = r#"<html><body>
        <p>First paragraph.</p>
        <p>Second paragraph.</p>
    </body></html>"#;

    let mut engine = engine_with_html(html);
    let snap = engine.snapshot(SnapMode::Markdown);

    assert!(snap.contains("First paragraph."), "first p should appear: {}", snap);
    assert!(snap.contains("Second paragraph."), "second p should appear: {}", snap);
    // Paragraphs should be separated by at least one blank line
    assert!(
        snap.contains("First paragraph.\n\nSecond paragraph."),
        "paragraphs should be separated by blank line: {:?}",
        snap
    );
}

// ---------------------------------------------------------------------------
// Line break: <br> should produce a newline
// ---------------------------------------------------------------------------

#[test]
fn br_produces_line_break() {
    let html = r#"<html><body>
        <p>Line one<br>Line two</p>
    </body></html>"#;

    let mut engine = engine_with_html(html);
    let snap = engine.snapshot(SnapMode::Markdown);

    assert!(snap.contains("Line one"), "first line should appear: {}", snap);
    assert!(snap.contains("Line two"), "second line should appear: {}", snap);
}
