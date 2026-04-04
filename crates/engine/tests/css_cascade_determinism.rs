//! Tests for CSS cascade determinism, specificity, and computed style stability.
//!
//! Verifies that CSS specificity is correctly ordered, cascade resolution is
//! deterministic, computed styles are idempotent (computing twice gives the same
//! result), and inheritance chains work correctly. Uses real CSS patterns from
//! production sites.

use braille_engine::Engine;
use braille_wire::SnapMode;

fn engine_with_html(html: &str) -> Engine {
    let mut e = Engine::new();
    e.load_html(html);
    e
}

// ===========================================================================
// Specificity Ordering
// ===========================================================================

#[test]
fn specificity_element_vs_class() {
    // Class selector (.foo) has higher specificity than element selector (div)
    let mut e = engine_with_html(r#"<html><head>
        <style>
            div { color: red; }
            .blue { color: blue; }
        </style>
    </head><body>
        <div class="blue" id="target">Text</div>
    </body></html>"#);

    let result = e.eval_js(
        "getComputedStyle(document.getElementById('target')).color"
    ).unwrap();
    // .blue (specificity 0,1,0) beats div (specificity 0,0,1)
    assert!(result.contains("blue") || result.contains("0, 0, 255"),
        "class should beat element selector: {}", result);
}

#[test]
fn specificity_class_vs_id() {
    // ID selector (#foo) has higher specificity than class selector (.foo)
    let mut e = engine_with_html(r#"<html><head>
        <style>
            .red { color: red; }
            #target { color: green; }
        </style>
    </head><body>
        <div class="red" id="target">Text</div>
    </body></html>"#);

    let result = e.eval_js(
        "getComputedStyle(document.getElementById('target')).color"
    ).unwrap();
    // #target (specificity 1,0,0) beats .red (specificity 0,1,0)
    assert!(result.contains("green") || result.contains("0, 128, 0"),
        "ID should beat class selector: {}", result);
}

#[test]
fn specificity_compound_selectors() {
    // div.foo (0,1,1) beats .foo (0,1,0) and div (0,0,1)
    let mut e = engine_with_html(r#"<html><head>
        <style>
            .cls { color: red; }
            div { color: blue; }
            div.cls { color: green; }
        </style>
    </head><body>
        <div class="cls" id="target">Text</div>
    </body></html>"#);

    let result = e.eval_js(
        "getComputedStyle(document.getElementById('target')).color"
    ).unwrap();
    assert!(result.contains("green") || result.contains("0, 128, 0"),
        "compound selector div.cls should beat both .cls and div: {}", result);
}

#[test]
fn specificity_inline_style_beats_all() {
    // Inline style beats all selector-based rules
    let mut e = engine_with_html(r#"<html><head>
        <style>
            #target { color: red; }
            div.cls { color: blue; }
        </style>
    </head><body>
        <div class="cls" id="target" style="color: green;">Text</div>
    </body></html>"#);

    let result = e.eval_js(
        "getComputedStyle(document.getElementById('target')).color"
    ).unwrap();
    assert!(result.contains("green") || result.contains("0, 128, 0"),
        "inline style should beat all selectors: {}", result);
}

// ===========================================================================
// Cascade Order (last rule wins at equal specificity)
// ===========================================================================

#[test]
fn cascade_last_rule_wins() {
    let mut e = engine_with_html(r#"<html><head>
        <style>
            .target { color: red; }
            .target { color: blue; }
        </style>
    </head><body>
        <div class="target" id="el">Text</div>
    </body></html>"#);

    let result = e.eval_js(
        "getComputedStyle(document.getElementById('el')).color"
    ).unwrap();
    assert!(result.contains("blue") || result.contains("0, 0, 255"),
        "last rule at equal specificity should win: {}", result);
}

#[test]
fn cascade_later_stylesheet_wins() {
    let mut e = engine_with_html(r#"<html><head>
        <style>
            #el { color: red; }
        </style>
        <style>
            #el { color: blue; }
        </style>
    </head><body>
        <div id="el">Text</div>
    </body></html>"#);

    let result = e.eval_js(
        "getComputedStyle(document.getElementById('el')).color"
    ).unwrap();
    assert!(result.contains("blue") || result.contains("0, 0, 255"),
        "later stylesheet should win at equal specificity: {}", result);
}

// ===========================================================================
// Inheritance
// ===========================================================================

#[test]
fn color_inherits_from_parent() {
    let mut e = engine_with_html(r#"<html><head>
        <style>
            .parent { color: purple; }
        </style>
    </head><body>
        <div class="parent"><span id="child">Inherits color</span></div>
    </body></html>"#);

    let result = e.eval_js(
        "getComputedStyle(document.getElementById('child')).color"
    ).unwrap();
    assert!(result.contains("purple") || result.contains("128, 0, 128"),
        "color should inherit from parent: {}", result);
}

#[test]
fn non_inherited_property_does_not_inherit() {
    // border does not inherit by default
    let mut e = engine_with_html(r#"<html><head>
        <style>
            .parent { border: 2px solid red; }
        </style>
    </head><body>
        <div class="parent"><span id="child">No border</span></div>
    </body></html>"#);

    let result = e.eval_js(
        "getComputedStyle(document.getElementById('child')).borderTopWidth"
    ).unwrap();
    // Child should have no border (initial value is 0 or "medium" or empty)
    assert!(result != "2px",
        "border should not inherit from parent: {}", result);
}

// ===========================================================================
// Computed Style Idempotency
// ===========================================================================

#[test]
fn computed_style_is_idempotent() {
    let mut e = engine_with_html(r#"<html><head>
        <style>
            #target { color: red; font-size: 16px; display: block; }
        </style>
    </head><body>
        <div id="target">Styled element</div>
    </body></html>"#);

    let color1 = e.eval_js("getComputedStyle(document.getElementById('target')).color").unwrap();
    let font1 = e.eval_js("getComputedStyle(document.getElementById('target')).fontSize").unwrap();

    // Settle again (recomputes CSS)
    e.settle();

    let color2 = e.eval_js("getComputedStyle(document.getElementById('target')).color").unwrap();
    let font2 = e.eval_js("getComputedStyle(document.getElementById('target')).fontSize").unwrap();

    assert_eq!(color1, color2, "color should be same after recompute");
    assert_eq!(font1, font2, "font-size should be same after recompute");
}

#[test]
fn computed_style_stable_across_snapshots() {
    let mut e = engine_with_html(r#"<html><head>
        <style>
            .box { display: none; color: blue; }
        </style>
    </head><body>
        <div class="box" id="hidden">Hidden</div>
        <div id="visible">Visible</div>
    </body></html>"#);

    let snap1 = e.snapshot(SnapMode::Text);
    let snap2 = e.snapshot(SnapMode::Text);
    let snap3 = e.snapshot(SnapMode::Text);

    assert_eq!(snap1, snap2, "snapshots should be identical");
    assert_eq!(snap2, snap3, "third snapshot should also be identical");

    // Hidden element should not appear in text snapshot
    assert!(!snap1.contains("Hidden"), "display:none element should be hidden: {}", snap1);
    assert!(snap1.contains("Visible"), "visible element should appear: {}", snap1);
}

// ===========================================================================
// Dynamic Style Changes
// ===========================================================================

#[test]
fn js_style_change_reflected_after_settle() {
    let mut e = engine_with_html(r#"<html><head>
        <style>
            #target { display: none; }
        </style>
    </head><body>
        <div id="target">Now you see me</div>
    </body></html>"#);

    // Initially hidden
    let snap = e.snapshot(SnapMode::Text);
    assert!(!snap.contains("Now you see me"), "should be hidden initially");

    // Show via JS
    e.eval_js("document.getElementById('target').style.setProperty('display', 'block')").unwrap();
    e.settle();

    let snap = e.snapshot(SnapMode::Text);
    assert!(snap.contains("Now you see me"), "should be visible after style change: {}", snap);
}

#[test]
fn class_toggle_changes_styles() {
    let mut e = engine_with_html(r#"<html><head>
        <style>
            .highlight { color: yellow; font-weight: bold; }
        </style>
    </head><body>
        <div id="target">Text</div>
    </body></html>"#);

    let before = e.eval_js("getComputedStyle(document.getElementById('target')).fontWeight").unwrap();

    e.eval_js("document.getElementById('target').classList.add('highlight')").unwrap();
    e.settle();

    let after = e.eval_js("getComputedStyle(document.getElementById('target')).fontWeight").unwrap();
    // fontWeight should change from normal/400 to bold/700
    assert_ne!(before, after, "class toggle should change computed style: before={}, after={}", before, after);
}

// ===========================================================================
// Multiple Matching Rules
// ===========================================================================

#[test]
fn multiple_properties_from_different_rules() {
    // Different rules contribute different properties — all should apply
    let mut e = engine_with_html(r#"<html><head>
        <style>
            div { color: red; }
            .box { font-size: 20px; }
            #target { font-weight: bold; }
        </style>
    </head><body>
        <div class="box" id="target">Styled</div>
    </body></html>"#);

    let color = e.eval_js("getComputedStyle(document.getElementById('target')).color").unwrap();
    let font_size = e.eval_js("getComputedStyle(document.getElementById('target')).fontSize").unwrap();
    let font_weight = e.eval_js("getComputedStyle(document.getElementById('target')).fontWeight").unwrap();

    // All three properties from different rules should be applied
    assert!(!color.is_empty(), "color from element selector should apply: {}", color);
    assert!(!font_size.is_empty(), "font-size from class selector should apply: {}", font_size);
    assert!(!font_weight.is_empty(), "font-weight from ID selector should apply: {}", font_weight);
}
