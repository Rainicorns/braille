//! Airbnb search page tests.
//!
//! Airbnb's search results page is a heavy SPA that relies on:
//! - PerformanceObserver for paint/LCP/layout-shift metrics
//! - Flex layout extensively (search cards, filters, nav)
//! - CSS custom properties, overflow handling, min/max widths
//!
//! These tests exercise the engine features needed to handle Airbnb-like
//! patterns without requiring a full site recording.

use braille_engine::Engine;
use braille_wire::SnapMode;

fn eval(html: &str, js: &str) -> String {
    let mut engine = Engine::new();
    engine.load_html(html);
    engine.eval_js(js).unwrap_or_else(|e| format!("ERROR: {e}"))
}

// ---------------------------------------------------------------------------
// 1. PerformanceObserver pattern used by Airbnb's metrics collection
// ---------------------------------------------------------------------------

#[test]
fn airbnb_perf_observer_pattern() {
    // Airbnb creates multiple PerformanceObserver instances for different
    // metric types. This must not crash.
    let result = eval(
        "<html><body></body></html>",
        r#"
        var results = [];

        // LCP observer
        var lcpObs = new PerformanceObserver(function(list) {
            results.push('lcp:' + list.getEntries().length);
        });
        lcpObs.observe({ type: 'largest-contentful-paint', buffered: true });

        // Layout shift observer
        var clsObs = new PerformanceObserver(function(list) {
            results.push('cls:' + list.getEntries().length);
        });
        clsObs.observe({ type: 'layout-shift', buffered: true });

        // FID observer
        var fidObs = new PerformanceObserver(function(list) {
            results.push('fid:' + list.getEntries().length);
        });
        fidObs.observe({ type: 'first-input', buffered: true });

        // Cleanup
        lcpObs.disconnect();
        clsObs.disconnect();
        fidObs.disconnect();

        'ok'
    "#,
    );
    assert_eq!(result.trim_matches('"'), "ok");
}

// ---------------------------------------------------------------------------
// 2. Flex layout pattern: Airbnb search card
// ---------------------------------------------------------------------------

#[test]
fn airbnb_search_card_flex_layout() {
    let html = r#"<html><head><style>
        .card { display: flex; flex-direction: column; overflow: hidden; min-width: 0; }
        .card-image { flex-shrink: 0; overflow: hidden; }
        .card-info { display: flex; flex-direction: column; flex-grow: 1; gap: 4px; }
        .card-row { display: flex; justify-content: space-between; align-items: center; }
    </style></head><body>
        <div class="card">
            <div class="card-image">image placeholder</div>
            <div class="card-info">
                <div class="card-row">
                    <span>Mountain View</span>
                    <span>4.92</span>
                </div>
                <div>$150 night</div>
            </div>
        </div>
    </body></html>"#;

    let mut engine = Engine::new();
    engine.load_html(html);
    let snap = engine.snapshot(SnapMode::Accessibility);

    assert!(snap.contains("Mountain View"), "Should extract location: {snap}");
    assert!(snap.contains("$150"), "Should extract price: {snap}");
    assert!(snap.contains("4.92"), "Should extract rating: {snap}");
}

// ---------------------------------------------------------------------------
// 3. Flex with overflow and min-width patterns
// ---------------------------------------------------------------------------

#[test]
fn flex_with_overflow_hidden_and_min_width() {
    let result = eval(
        r#"<html><body>
            <div id="c" style="display:flex; overflow:hidden; min-width:300px; max-width:1200px">
                <div id="item" style="flex: 1 1 0%; min-width:0; overflow:hidden">content</div>
            </div>
        </body></html>"#,
        r#"
        var c = getComputedStyle(document.getElementById('c'));
        var item = getComputedStyle(document.getElementById('item'));
        JSON.stringify({
            container_display: c.display,
            container_overflow: c.overflow,
            container_min_width: c.minWidth,
            container_max_width: c.maxWidth,
            item_flex_grow: item.flexGrow,
            item_min_width: item.minWidth,
            item_overflow: item.overflow
        })
    "#,
    );

    assert!(result.contains("\"container_display\":\"flex\""), "display should be flex: {result}");
    assert!(result.contains("\"container_overflow\":\"hidden\""), "overflow should be hidden: {result}");
    assert!(result.contains("\"container_min_width\":\"300px\""), "min-width should be 300px: {result}");
    assert!(
        result.contains("\"container_max_width\":\"1200px\""),
        "max-width should be 1200px: {result}"
    );
}

// ---------------------------------------------------------------------------
// 4. Airbnb-style extraction test using fixture JS
// ---------------------------------------------------------------------------

#[test]
fn airbnb_extraction_from_search_html() {
    let extract_js = std::fs::read_to_string(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/airbnb_extract.js"),
    )
    .expect("airbnb_extract.js fixture should exist");

    let html = r#"<html><head><style>
        .results { display: flex; flex-wrap: wrap; gap: 20px; }
        .listing { display: flex; flex-direction: column; min-width: 280px; max-width: 400px; flex: 1; }
        .listing-title { font-weight: bold; }
        .listing-price { color: #222; }
        .listing-rating { display: flex; align-items: center; gap: 4px; }
    </style></head><body>
        <div class="results">
            <div class="listing" data-testid="listing-card">
                <div class="listing-title">Cozy Cabin in the Woods</div>
                <div class="listing-price">$120 / night</div>
                <div class="listing-rating"><span>4.85</span><span>(42 reviews)</span></div>
            </div>
            <div class="listing" data-testid="listing-card">
                <div class="listing-title">Beachfront Studio</div>
                <div class="listing-price">$95 / night</div>
                <div class="listing-rating"><span>4.72</span><span>(118 reviews)</span></div>
            </div>
            <div class="listing" data-testid="listing-card">
                <div class="listing-title">Downtown Loft</div>
                <div class="listing-price">$200 / night</div>
                <div class="listing-rating"><span>4.91</span><span>(67 reviews)</span></div>
            </div>
        </div>
    </body></html>"#;

    let result = eval(html, &extract_js);

    // The extraction script should find listing data
    assert!(result.contains("Cozy Cabin"), "Should extract first listing: {result}");
    assert!(result.contains("Beachfront"), "Should extract second listing: {result}");
    assert!(result.contains("Downtown"), "Should extract third listing: {result}");
}

// ---------------------------------------------------------------------------
// 5. Tests gated on real-sites feature (for full recorded transcripts)
// ---------------------------------------------------------------------------

#[cfg(feature = "real-sites")]
mod real_sites {
    // Placeholder for future tests that require recorded Airbnb transcripts.
    // These would use the StaticSiteReplayer pattern from real_sites_static.rs
    // with an actual recorded airbnb fixture.
}
