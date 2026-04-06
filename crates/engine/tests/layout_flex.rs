//! Tests for CSS flex layout support.
//!
//! Verifies that flex container/item properties are correctly parsed through
//! the CSS pipeline and applied to computed styles. Tests both style computation
//! and layout behavior via taffy integration.

use braille_engine::Engine;
use braille_wire::SnapMode;

fn snap(html: &str) -> String {
    let mut engine = Engine::new();
    engine.load_html(html);
    engine.snapshot(SnapMode::Accessibility)
}

fn eval(html: &str, js: &str) -> String {
    let mut engine = Engine::new();
    engine.load_html(html);
    engine.eval_js(js).unwrap_or_else(|e| format!("ERROR: {e}"))
}

// ---------------------------------------------------------------------------
// 1. display:flex is recognized
// ---------------------------------------------------------------------------

#[test]
fn display_flex_computed() {
    let result = eval(
        r#"<html><body><div id="c" style="display:flex"></div></body></html>"#,
        "getComputedStyle(document.getElementById('c')).display",
    );
    assert_eq!(result.trim_matches('"'), "flex");
}

// ---------------------------------------------------------------------------
// 2. flex-direction
// ---------------------------------------------------------------------------

#[test]
fn flex_direction_column() {
    let result = eval(
        r#"<html><body><div id="c" style="display:flex; flex-direction:column"></div></body></html>"#,
        "getComputedStyle(document.getElementById('c')).flexDirection",
    );
    assert_eq!(result.trim_matches('"'), "column");
}

#[test]
fn flex_direction_row_default() {
    let result = eval(
        r#"<html><body><div id="c" style="display:flex"></div></body></html>"#,
        "getComputedStyle(document.getElementById('c')).flexDirection",
    );
    assert_eq!(result.trim_matches('"'), "row");
}

// ---------------------------------------------------------------------------
// 3. flex-wrap
// ---------------------------------------------------------------------------

#[test]
fn flex_wrap_wrap() {
    let result = eval(
        r#"<html><body><div id="c" style="display:flex; flex-wrap:wrap"></div></body></html>"#,
        "getComputedStyle(document.getElementById('c')).flexWrap",
    );
    assert_eq!(result.trim_matches('"'), "wrap");
}

// ---------------------------------------------------------------------------
// 4. justify-content / align-items
// ---------------------------------------------------------------------------

#[test]
fn justify_content_center() {
    let result = eval(
        r#"<html><body><div id="c" style="display:flex; justify-content:center"></div></body></html>"#,
        "getComputedStyle(document.getElementById('c')).justifyContent",
    );
    assert_eq!(result.trim_matches('"'), "center");
}

#[test]
fn align_items_center() {
    let result = eval(
        r#"<html><body><div id="c" style="display:flex; align-items:center"></div></body></html>"#,
        "getComputedStyle(document.getElementById('c')).alignItems",
    );
    assert_eq!(result.trim_matches('"'), "center");
}

// ---------------------------------------------------------------------------
// 5. flex-grow / flex-shrink / flex-basis
// ---------------------------------------------------------------------------

#[test]
fn flex_grow_on_item() {
    let result = eval(
        r#"<html><body><div style="display:flex"><div id="item" style="flex-grow:2">A</div></div></body></html>"#,
        "getComputedStyle(document.getElementById('item')).flexGrow",
    );
    assert_eq!(result.trim_matches('"'), "2");
}

#[test]
fn flex_shrink_on_item() {
    let result = eval(
        r#"<html><body><div style="display:flex"><div id="item" style="flex-shrink:0">A</div></div></body></html>"#,
        "getComputedStyle(document.getElementById('item')).flexShrink",
    );
    assert_eq!(result.trim_matches('"'), "0");
}

#[test]
fn flex_basis_on_item() {
    let result = eval(
        r#"<html><body><div style="display:flex"><div id="item" style="flex-basis:200px">A</div></div></body></html>"#,
        "getComputedStyle(document.getElementById('item')).flexBasis",
    );
    assert_eq!(result.trim_matches('"'), "200px");
}

// ---------------------------------------------------------------------------
// 6. flex shorthand
// ---------------------------------------------------------------------------

#[test]
fn flex_shorthand_three_values() {
    let result = eval(
        r#"<html><body><div style="display:flex"><div id="item" style="flex: 1 0 auto">A</div></div></body></html>"#,
        r#"
        var s = getComputedStyle(document.getElementById('item'));
        s.flexGrow + '/' + s.flexShrink + '/' + s.flexBasis
    "#,
    );
    assert_eq!(result.trim_matches('"'), "1/0/auto");
}

#[test]
fn flex_shorthand_none() {
    let result = eval(
        r#"<html><body><div style="display:flex"><div id="item" style="flex: none">A</div></div></body></html>"#,
        r#"
        var s = getComputedStyle(document.getElementById('item'));
        s.flexGrow + '/' + s.flexShrink + '/' + s.flexBasis
    "#,
    );
    assert_eq!(result.trim_matches('"'), "0/0/auto");
}

// ---------------------------------------------------------------------------
// 7. gap property
// ---------------------------------------------------------------------------

#[test]
fn gap_on_flex_container() {
    let result = eval(
        r#"<html><body><div id="c" style="display:flex; gap:10px"></div></body></html>"#,
        r#"
        var s = getComputedStyle(document.getElementById('c'));
        s.rowGap + '/' + s.columnGap
    "#,
    );
    assert_eq!(result.trim_matches('"'), "10px/10px");
}

// ---------------------------------------------------------------------------
// 8. min/max width/height
// ---------------------------------------------------------------------------

#[test]
fn min_width_on_element() {
    let result = eval(
        r#"<html><body><div id="d" style="min-width:100px"></div></body></html>"#,
        "getComputedStyle(document.getElementById('d')).minWidth",
    );
    assert_eq!(result.trim_matches('"'), "100px");
}

#[test]
fn max_width_on_element() {
    let result = eval(
        r#"<html><body><div id="d" style="max-width:500px"></div></body></html>"#,
        "getComputedStyle(document.getElementById('d')).maxWidth",
    );
    assert_eq!(result.trim_matches('"'), "500px");
}

// ---------------------------------------------------------------------------
// 9. overflow
// ---------------------------------------------------------------------------

#[test]
fn overflow_hidden() {
    let result = eval(
        r#"<html><body><div id="d" style="overflow:hidden"></div></body></html>"#,
        "getComputedStyle(document.getElementById('d')).overflow",
    );
    assert_eq!(result.trim_matches('"'), "hidden");
}

// ---------------------------------------------------------------------------
// 10. Flex layout actually produces different ordering in snapshot
// ---------------------------------------------------------------------------

#[test]
fn flex_container_renders_children() {
    let snapshot = snap(
        r#"<html><body>
        <div style="display:flex; gap:10px">
            <div>Item 1</div>
            <div>Item 2</div>
            <div>Item 3</div>
        </div>
    </body></html>"#,
    );
    assert!(snapshot.contains("Item 1"), "flex children should appear: {snapshot}");
    assert!(snapshot.contains("Item 2"), "flex children should appear: {snapshot}");
    assert!(snapshot.contains("Item 3"), "flex children should appear: {snapshot}");
}

// ---------------------------------------------------------------------------
// 11. flex-flow shorthand
// ---------------------------------------------------------------------------

#[test]
fn flex_flow_shorthand() {
    let result = eval(
        r#"<html><body><div id="c" style="display:flex; flex-flow: column wrap"></div></body></html>"#,
        r#"
        var s = getComputedStyle(document.getElementById('c'));
        s.flexDirection + '/' + s.flexWrap
    "#,
    );
    assert_eq!(result.trim_matches('"'), "column/wrap");
}
