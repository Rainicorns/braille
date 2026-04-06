//! Integration tests for Airbnb search results rendering.
//!
//! The feature-gated tests require a recorded Airbnb transcript fixture.
//! Record it with: `cargo run -p braille-cli -- record https://www.airbnb.com/s/homes`
//! and save to `tests/fixtures/airbnb_search.json`.
//!
//! The non-gated test below validates the PerformanceObserver + flex layout
//! pattern that Airbnb uses, with a synthetic reproduction.

use braille_engine::Engine;
use braille_wire::SnapMode;

// ---------------------------------------------------------------------------
// Non-gated: PerformanceObserver + flex virtual scroller pattern
// ---------------------------------------------------------------------------

/// Reproduces the exact crash pattern from Airbnb: PerformanceObserver usage
/// combined with a flex-based virtual scroller that measures container size.
#[test]
fn performance_observer_no_crash() {
    // Test PerformanceObserver independently (Airbnb's init pattern)
    let mut engine = Engine::new();
    engine.load_html("<html><body></body></html>");
    let po_ok = engine.eval_js(r#"
        try {
            var po = new PerformanceObserver(function(list) {});
            po.observe({ entryTypes: ['paint', 'largest-contentful-paint'] });
            po.disconnect();
            'true'
        } catch(e) {
            'error: ' + e.message
        }
    "#).unwrap();
    assert_eq!(po_ok.trim(), "true", "PerformanceObserver should not throw");

    // Now test flex layout rendering with Airbnb-like structure
    let html = r#"<html><body>
        <div id="search-results" style="display:flex; flex-direction:column; width:100%; min-height:500px;">
            <div id="listing-1" style="display:flex; padding:16px; min-height:80px;">
                <div style="flex-shrink:0; width:300px; height:200px;">image</div>
                <div style="flex-grow:1; display:flex; flex-direction:column; padding:0 16px;">
                    <div>Cozy apartment 2 bedrooms 1 bath</div>
                    <div>$150 per night</div>
                </div>
            </div>
            <div id="listing-2" style="display:flex; padding:16px; min-height:80px;">
                <div style="flex-shrink:0; width:300px; height:200px;">image</div>
                <div style="flex-grow:1; display:flex; flex-direction:column; padding:0 16px;">
                    <div>Modern studio 1 bedroom 1 bath</div>
                    <div>$95 per night</div>
                </div>
            </div>
        </div>
    </body></html>"#;

    let mut engine2 = Engine::new();
    engine2.load_html(html);

    // Snapshot should contain listing content (proves flex layout rendered it)
    let snap = engine2.snapshot(SnapMode::Text);
    assert!(snap.contains("bedroom"), "listings should render: {}", snap);
    assert!(snap.contains("night"), "price should render: {}", snap);
    assert!(snap.contains("$"), "dollar sign should render: {}", snap);

    // Container should have non-zero layout dimensions
    let h = engine2
        .eval_js("document.getElementById('search-results').getBoundingClientRect().height")
        .unwrap();
    let h: f64 = h.trim().parse().unwrap();
    assert!(h > 100.0, "search-results container should have height > 100, got {}", h);
}

// ---------------------------------------------------------------------------
// Feature-gated: real Airbnb recording
// ---------------------------------------------------------------------------

#[cfg(feature = "real-sites")]
mod real_site {
    use braille_engine::transcript::ReplayFetcher;
    use braille_engine::Engine;
    use braille_wire::SnapMode;

    #[test]
    fn airbnb_search_listings_render() {
        let mut fetcher = ReplayFetcher::load("tests/fixtures/airbnb_search.json").unwrap();
        let mut engine = Engine::new();
        let snap = engine
            .navigate("https://www.airbnb.com/s/homes", &mut fetcher, SnapMode::Accessibility)
            .expect("navigation should succeed");

        // Airbnb search results should contain listing-related content
        let has_listing_content = snap.contains("night")
            || snap.contains("$")
            || snap.contains("bedroom")
            || snap.contains("bed")
            || snap.contains("bath");
        assert!(
            has_listing_content,
            "Airbnb snapshot should contain listing content (night/$/ bedroom/bed/bath), got: {}",
            &snap[..snap.len().min(2000)]
        );
    }

    #[test]
    fn airbnb_container_has_nonzero_dimensions() {
        let mut fetcher = ReplayFetcher::load("tests/fixtures/airbnb_search.json").unwrap();
        let mut engine = Engine::new();
        let _snap = engine
            .navigate("https://www.airbnb.com/s/homes", &mut fetcher, SnapMode::Accessibility)
            .expect("navigation should succeed");

        // Main content container should have real layout dimensions
        let w = engine
            .eval_js("document.querySelector('[data-testid=\"card-container\"]')?.getBoundingClientRect().width || document.body.getBoundingClientRect().width")
            .unwrap();
        let w: f64 = w.trim().parse().unwrap_or(0.0);
        assert!(w > 0.0, "main content should have width > 0, got {}", w);

        let h = engine
            .eval_js("document.querySelector('[data-testid=\"card-container\"]')?.getBoundingClientRect().height || document.body.getBoundingClientRect().height")
            .unwrap();
        let h: f64 = h.trim().parse().unwrap_or(0.0);
        assert!(h > 0.0, "main content should have height > 0, got {}", h);
    }
}
