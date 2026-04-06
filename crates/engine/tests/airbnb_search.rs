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
// Structured data extraction from SSR-embedded JSON
// ---------------------------------------------------------------------------

/// End-to-end extraction: load Airbnb-like HTML with embedded deferred data,
/// run the extraction JS, verify structured output with deep links.
#[test]
fn extract_structured_listings_from_deferred_data() {
    let extract_js = include_str!("fixtures/airbnb_extract.js");

    // Synthetic Airbnb page with realistic deferred state structure
    let deferred_json = serde_json::json!({
        "niobeClientData": [[
            "StaysSearch:{}", {
                "data": {
                    "presentation": {
                        "staysSearch": {
                            "results": {
                                "searchResults": [
                                    {
                                        "title": "Cozy Loft in SOMA",
                                        "subtitle": "Apartment in San Francisco",
                                        "avgRatingLocalized": "4.92 (128)",
                                        "avgRatingA11yLabel": "4.92 out of 5 average rating, 128 reviews",
                                        "propertyId": null,
                                        "structuredDisplayPrice": {
                                            "primaryLine": {"accessibilityLabel": "$750 for 3 nights"},
                                            "secondaryLine": null
                                        },
                                        "structuredContent": {
                                            "primaryLine": [{"body": "1 bedroom"}, {"body": "1 queen bed"}],
                                            "secondaryLine": [{"body": "Apr 10 – 13"}]
                                        },
                                        "contextualPictures": [{"picture": "https://example.com/photo1.jpg"}],
                                        "badges": [{"text": "Guest favorite"}],
                                        "demandStayListing": {
                                            "id": "RGVtYW5kU3RheUxpc3Rpbmc6MTIzNDU2Nzg=",
                                            "location": {
                                                "coordinate": {"latitude": 37.7749, "longitude": -122.4194}
                                            }
                                        },
                                        "listingParamOverrides": {
                                            "checkin": "2026-04-10",
                                            "checkout": "2026-04-13",
                                            "adults": 2
                                        }
                                    },
                                    {
                                        "title": "Victorian Gem in the Mission",
                                        "subtitle": "Home in San Francisco",
                                        "avgRatingLocalized": "4.88 (256)",
                                        "avgRatingA11yLabel": "4.88 out of 5 average rating, 256 reviews",
                                        "propertyId": "9876543210",
                                        "structuredDisplayPrice": {
                                            "primaryLine": {"accessibilityLabel": "$1,200 for 3 nights"},
                                            "secondaryLine": {"accessibilityLabel": "Originally $1,500"}
                                        },
                                        "structuredContent": {
                                            "primaryLine": [{"body": "2 bedrooms"}, {"body": "2 beds"}],
                                            "secondaryLine": [{"body": "Apr 10 – 13"}]
                                        },
                                        "contextualPictures": [{"picture": "https://example.com/photo2.jpg"}],
                                        "badges": [],
                                        "demandStayListing": {
                                            "id": "RGVtYW5kU3RheUxpc3Rpbmc6OTg3NjU0MzIxMA==",
                                            "location": {
                                                "coordinate": {"latitude": 37.7599, "longitude": -122.4148}
                                            }
                                        },
                                        "listingParamOverrides": {
                                            "checkin": "2026-04-10",
                                            "checkout": "2026-04-13",
                                            "adults": 2
                                        }
                                    }
                                ],
                                "paginationInfo": {"hasNextPage": true, "nextPageCursor": "abc123"}
                            }
                        }
                    }
                }
            }
        ]]
    });

    let html = format!(
        r#"<html><head>
            <script id="data-deferred-state-0" type="application/json">{}</script>
        </head><body>
            <h1>Search results</h1>
        </body></html>"#,
        deferred_json.to_string().replace('<', "\\u003c")
    );

    let mut engine = Engine::new();
    engine.load_html(&html);

    let result = engine.eval_js(extract_js).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result)
        .unwrap_or_else(|e| panic!("extraction should return valid JSON: {}\nraw: {}", e, result));

    // Verify structure
    assert_eq!(parsed["totalListings"], 2);
    assert_eq!(parsed["hasNextPage"], true);

    let listings = parsed["listings"].as_array().unwrap();

    // First listing — ID from base64 demandStayListing.id
    let l0 = &listings[0];
    assert_eq!(l0["title"], "Cozy Loft in SOMA");
    assert_eq!(l0["type"], "Apartment in San Francisco");
    assert_eq!(l0["rating"], "4.92 (128)");
    assert_eq!(l0["price"], "$750 for 3 nights");
    assert!(l0["details"].as_str().unwrap().contains("1 bedroom"));
    assert_eq!(l0["badges"][0], "Guest favorite");
    // Base64 "RGVtYW5kU3RheUxpc3Rpbmc6MTIzNDU2Nzg=" decodes to "DemandStayListing:12345678"
    assert_eq!(l0["listingId"], "12345678");
    assert!(l0["deepLink"].as_str().unwrap().contains("/rooms/12345678"));
    assert!(l0["deepLink"].as_str().unwrap().contains("checkin=2026-04-10"));
    assert!(l0["deepLink"].as_str().unwrap().contains("adults=2"));
    assert!((l0["lat"].as_f64().unwrap() - 37.7749).abs() < 0.001);

    // Second listing — propertyId takes precedence over decoded base64
    let l1 = &listings[1];
    assert_eq!(l1["title"], "Victorian Gem in the Mission");
    assert_eq!(l1["listingId"], "9876543210");
    assert!(l1["deepLink"].as_str().unwrap().contains("/rooms/9876543210"));
    assert_eq!(l1["originalPrice"], "Originally $1,500");
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
    fn airbnb_live_extraction_returns_listings_with_deep_links() {
        let extract_js = include_str!("fixtures/airbnb_extract.js");

        let mut fetcher = ReplayFetcher::load("tests/fixtures/airbnb_search.json").unwrap();
        let mut engine = Engine::new();
        let _snap = engine
            .navigate("https://www.airbnb.com/s/San-Francisco/homes", &mut fetcher, SnapMode::Text)
            .expect("navigation should succeed");

        let result = engine.eval_js(extract_js).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result)
            .expect("extraction should return valid JSON");

        let listings = parsed["listings"].as_array().expect("should have listings array");
        assert!(listings.len() >= 10, "should have at least 10 listings, got {}", listings.len());

        // Every listing should have critical decision fields
        for (i, listing) in listings.iter().enumerate() {
            assert!(!listing["title"].as_str().unwrap_or("").is_empty(),
                "listing {} should have title", i);
            assert!(!listing["price"].as_str().unwrap_or("").is_empty(),
                "listing {} should have price", i);
            assert!(listing["deepLink"].as_str().unwrap_or("").contains("/rooms/"),
                "listing {} should have deep link, got: {}", i, listing["deepLink"]);
        }
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
