use braille_engine::Engine;
use braille_wire::SnapMode;

/// Helper: load HTML, settle, then eval JS to get a numeric layout value.
fn layout_val(html: &str, js: &str) -> f64 {
    let mut engine = Engine::new();
    engine.load_html(html);
    // snapshot forces settle (style computation + layout)
    let _ = engine.snapshot(SnapMode::Accessibility);
    let result = engine.eval_js(js).unwrap();
    result.trim().parse::<f64>().unwrap_or_else(|_| panic!("Expected number, got: {}", result))
}

// ---------------------------------------------------------------------------
// 1. Flex container children have non-zero dimensions
// ---------------------------------------------------------------------------

#[test]
fn flex_container_children_have_nonzero_dimensions() {
    let html = r#"<html><body>
        <div id="container" style="display:flex; width:800px; height:100px;">
            <div id="a" style="flex-grow:1;">A</div>
            <div id="b" style="flex-grow:1;">B</div>
        </div>
    </body></html>"#;

    let w = layout_val(html, "document.getElementById('a').getBoundingClientRect().width");
    assert!(w > 100.0, "flex child A should have width > 100, got {}", w);
    assert!((w - 400.0).abs() < 50.0, "flex child A should be ~400px (half of 800), got {}", w);
}

// ---------------------------------------------------------------------------
// 2. flex-direction: column stacks vertically
// ---------------------------------------------------------------------------

#[test]
fn flex_direction_column_stacks_vertically() {
    let html = r#"<html><body>
        <div style="display:flex; flex-direction:column; width:200px; height:400px;">
            <div id="a" style="flex-grow:1;">A</div>
            <div id="b" style="flex-grow:1;">B</div>
        </div>
    </body></html>"#;

    let h = layout_val(html, "document.getElementById('a').getBoundingClientRect().height");
    assert!(h > 50.0, "column flex child should have non-trivial height, got {}", h);

    let h_b = layout_val(html, "document.getElementById('b').getBoundingClientRect().height");
    assert!(h_b > 50.0, "second column flex child should have non-trivial height, got {}", h_b);
}

// ---------------------------------------------------------------------------
// 3. flex-grow distributes space proportionally
// ---------------------------------------------------------------------------

#[test]
fn flex_grow_distributes_space() {
    let html = r#"<html><body>
        <div style="display:flex; width:800px;">
            <div id="big" style="flex-grow:3;">Big</div>
            <div id="small" style="flex-grow:1;">Small</div>
        </div>
    </body></html>"#;

    let big = layout_val(html, "document.getElementById('big').getBoundingClientRect().width");
    let small = layout_val(html, "document.getElementById('small').getBoundingClientRect().width");

    assert!(big > small, "flex-grow:3 child ({}) should be wider than flex-grow:1 child ({})", big, small);
    // 3:1 ratio means big should be ~3x small. Allow some tolerance for text content.
    let ratio = big / small;
    assert!(ratio > 2.0, "ratio should be > 2.0 (expecting ~3.0), got {}", ratio);
}

// ---------------------------------------------------------------------------
// 4. min-width constrains layout
// ---------------------------------------------------------------------------

#[test]
fn min_width_constrains_layout() {
    let html = r#"<html><body>
        <div style="display:flex; width:300px;">
            <div id="constrained" style="min-width:200px;">Constrained</div>
            <div id="other" style="flex-grow:1;">Other</div>
        </div>
    </body></html>"#;

    let w = layout_val(html, "document.getElementById('constrained').getBoundingClientRect().width");
    assert!(w >= 200.0, "min-width:200px child should be at least 200px wide, got {}", w);
}

// ---------------------------------------------------------------------------
// 5. max-width constrains layout
// ---------------------------------------------------------------------------

#[test]
fn max_width_constrains_layout() {
    let html = r#"<html><body>
        <div style="display:flex; width:800px;">
            <div id="capped" style="flex-grow:1; max-width:100px;">Capped</div>
            <div id="free" style="flex-grow:1;">Free</div>
        </div>
    </body></html>"#;

    let w = layout_val(html, "document.getElementById('capped').getBoundingClientRect().width");
    assert!(w <= 100.0, "max-width:100px child should be at most 100px, got {}", w);
}

// ---------------------------------------------------------------------------
// 6. overflow:hidden constrains scroll container
// ---------------------------------------------------------------------------

#[test]
fn overflow_hidden_constrains_scroll_container() {
    let html = r#"<html><body>
        <div id="parent" style="overflow:hidden; height:200px; width:300px;">
            <div id="child" style="height:500px;">Tall child</div>
        </div>
    </body></html>"#;

    let parent_h = layout_val(html, "document.getElementById('parent').getBoundingClientRect().height");
    assert!((parent_h - 200.0).abs() < 5.0, "parent with overflow:hidden should be 200px tall, got {}", parent_h);
}
