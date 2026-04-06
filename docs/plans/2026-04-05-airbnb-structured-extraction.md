# Airbnb Structured Data Extraction — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Extract structured listing data from Airbnb search results via Braille, with deep links for booking — enabling agent-driven accommodation decisioning.

**Architecture:** Airbnb embeds 400KB+ of structured JSON in `<script id="data-deferred-state-0">` on every search page. We extract directly from SSR data — no React rendering needed. Phase 1 ships a reusable JS extraction function + Braille integration test. Phase 2 adds a generalizable `SnapMode::Data` that auto-detects embedded JSON on any SPA. Phase 3 wires into TF-OS agents via Braille MCP.

**Tech Stack:** Rust (Braille engine), JavaScript (extraction), Braille MCP (agent integration)

---

## Proven Data Schema

From live probing, Airbnb's `data-deferred-state-0` contains:

```
data.niobeClientData[0][1].data.presentation.staysSearch.results.searchResults[]
```

Each result has:
| Field | Path | Example |
|-------|------|---------|
| Title | `.title` | "Hotel Zeppelin San Francisco" |
| Type | `.subtitle` | "Hotel in San Francisco" |
| Rating | `.avgRatingLocalized` | "4.64 (861)" |
| Rating detail | `.avgRatingA11yLabel` | "4.64 out of 5 average rating, 861 reviews" |
| Price | `.structuredDisplayPrice.primaryLine.accessibilityLabel` | "$826 for 5 nights" |
| Original price | `.structuredDisplayPrice.secondaryLine.accessibilityLabel` | (when discounted) |
| Bedrooms/beds | `.structuredContent.primaryLine[].body` | "1 bedroom · 1 queen bed" |
| Dates | `.structuredContent.secondaryLine[].body` | "May 24 – 29" |
| Photos | `.contextualPictures[].picture` | muscache.com URL |
| Listing ID | `.demandStayListing.id` | Base64: `RGVtYW5kU3RheUxpc3Rpbmc6MjM4MjkzNzI=` → `23829372` |
| Check-in | `.listingParamOverrides.checkin` | "2026-05-24" |
| Check-out | `.listingParamOverrides.checkout` | "2026-05-29" |
| Adults | `.listingParamOverrides.adults` | 1 |
| Badges | `.badges[].text` | "Guest favorite", "Superhost" |

Deep link formula: `https://www.airbnb.com/rooms/{decoded_id}?checkin={checkin}&checkout={checkout}&adults={adults}`

---

## Phase 1: Extraction Test + JS Utility (Braille PR)

### Task 1: Write the extraction JS function

**Files:**
- Create: `crates/engine/tests/fixtures/airbnb_extract.js`

This is a standalone JS function that extracts structured data from any Airbnb search page. It runs via `eval_js` after page load.

**Step 1: Write the extraction function**

```javascript
// airbnb_extract.js — Reusable extraction for Airbnb search results
// Returns JSON string with all listings + deep links
(function() {
    var el = document.getElementById('data-deferred-state-0');
    if (!el) return JSON.stringify({error: 'no deferred state found', listings: []});

    var raw;
    try { raw = JSON.parse(el.textContent); }
    catch(e) { return JSON.stringify({error: 'parse failed: ' + e.message, listings: []}); }

    var niobe = raw.niobeClientData;
    if (!niobe || !niobe[0] || !niobe[0][1]) return JSON.stringify({error: 'no niobe data', listings: []});

    var search = niobe[0][1].data;
    if (!search || !search.presentation || !search.presentation.staysSearch)
        return JSON.stringify({error: 'no staysSearch', listings: []});

    var results = search.presentation.staysSearch.results.searchResults || [];

    function decodeListingId(b64) {
        if (!b64) return null;
        try {
            var decoded = atob(b64);
            var parts = decoded.split(':');
            return parts.length > 1 ? parts[1] : decoded;
        } catch(e) { return null; }
    }

    var listings = results.map(function(r) {
        var id = null;
        if (r.propertyId) {
            id = r.propertyId;
        } else if (r.demandStayListing && r.demandStayListing.id) {
            id = decodeListingId(r.demandStayListing.id);
        }

        var checkin = '', checkout = '', adults = 1;
        if (r.listingParamOverrides) {
            checkin = r.listingParamOverrides.checkin || '';
            checkout = r.listingParamOverrides.checkout || '';
            adults = r.listingParamOverrides.adults || 1;
        }

        var deepLink = id
            ? 'https://www.airbnb.com/rooms/' + id +
              '?checkin=' + checkin + '&checkout=' + checkout + '&adults=' + adults
            : null;

        var price = '', originalPrice = '';
        if (r.structuredDisplayPrice) {
            var p = r.structuredDisplayPrice;
            if (p.primaryLine) price = p.primaryLine.accessibilityLabel || '';
            if (p.secondaryLine) originalPrice = p.secondaryLine.accessibilityLabel || '';
        }

        var details = '', dates = '';
        if (r.structuredContent) {
            if (r.structuredContent.primaryLine)
                details = r.structuredContent.primaryLine.map(function(x){return x.body||''}).join(' · ');
            if (r.structuredContent.secondaryLine)
                dates = r.structuredContent.secondaryLine.map(function(x){return x.body||''}).join(' · ');
        }

        var photoUrl = '';
        if (r.contextualPictures && r.contextualPictures[0])
            photoUrl = r.contextualPictures[0].picture || '';

        var badges = (r.badges || []).map(function(b){return b.text || b}).filter(Boolean);

        var lat = null, lng = null;
        if (r.demandStayListing && r.demandStayListing.location &&
            r.demandStayListing.location.coordinate) {
            lat = r.demandStayListing.location.coordinate.latitude;
            lng = r.demandStayListing.location.coordinate.longitude;
        }

        return {
            title: r.title || '',
            type: r.subtitle || '',
            rating: r.avgRatingLocalized || '',
            ratingDetail: r.avgRatingA11yLabel || '',
            price: price,
            originalPrice: originalPrice,
            details: details,
            dates: dates,
            badges: badges,
            photo: photoUrl,
            listingId: id,
            deepLink: deepLink,
            lat: lat,
            lng: lng,
            checkin: checkin,
            checkout: checkout,
            adults: adults
        };
    });

    // Pagination info
    var pagination = search.presentation.staysSearch.results.paginationInfo || {};

    return JSON.stringify({
        query: (niobe[0][0] || '').substring(0, 200),
        totalListings: results.length,
        listings: listings,
        hasNextPage: pagination.hasNextPage || false,
        nextPageCursor: pagination.nextPageCursor || null
    }, null, 2);
})()
```

**Step 2: Commit**
```bash
git add crates/engine/tests/fixtures/airbnb_extract.js
git commit -m "feat: add Airbnb structured data extraction JS utility"
```

### Task 2: Write integration test using the extraction

**Files:**
- Modify: `crates/engine/tests/airbnb_search.rs` (add extraction test)

**Step 1: Write the failing test**

Add to `airbnb_search.rs`:

```rust
/// End-to-end extraction test: load Airbnb-like HTML with embedded deferred data,
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
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();

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
    assert_eq!(l0["details"], "1 bedroom · 1 queen bed");
    assert_eq!(l0["badges"][0], "Guest favorite");
    // Base64 "RGVtYW5kU3RheUxpc3Rpbmc6MTIzNDU2Nzg=" decodes to "DemandStayListing:12345678"
    assert_eq!(l0["listingId"], "12345678");
    assert!(l0["deepLink"].as_str().unwrap().contains("/rooms/12345678"));
    assert!(l0["deepLink"].as_str().unwrap().contains("checkin=2026-04-10"));
    assert!(l0["deepLink"].as_str().unwrap().contains("adults=2"));
    assert!((l0["lat"].as_f64().unwrap() - 37.7749).abs() < 0.001);

    // Second listing — ID from propertyId (takes precedence)
    let l1 = &listings[1];
    assert_eq!(l1["title"], "Victorian Gem in the Mission");
    assert_eq!(l1["listingId"], "9876543210");
    assert!(l1["deepLink"].as_str().unwrap().contains("/rooms/9876543210"));
    assert_eq!(l1["originalPrice"], "Originally $1,500");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p braille-engine --test airbnb_search -- extract_structured`
Expected: FAIL (extraction JS hasn't been created yet — actually it has from Task 1, but the test HTML embedding might fail)

**Step 3: Fix any issues and verify it passes**

Run: `cargo test -p braille-engine --test airbnb_search -- extract_structured`
Expected: PASS

**Step 4: Commit**
```bash
git add crates/engine/tests/airbnb_search.rs
git commit -m "test: end-to-end Airbnb structured extraction with deep links"
```

### Task 3: Feature-gated live site extraction test

**Files:**
- Modify: `crates/engine/tests/airbnb_search.rs` (add to real_site module)

**Step 1: Add live extraction test to the real_site feature gate**

Add inside `#[cfg(feature = "real-sites")] mod real_site`:

```rust
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
```

**Step 2: Commit**
```bash
git add crates/engine/tests/airbnb_search.rs
git commit -m "test: feature-gated live Airbnb extraction with deep link verification"
```

---

## Phase 2: SnapMode::Data — Generalizable SPA Data Extraction

<!-- GATING: user-approval-required -->

> **Gating question 1:** Should this be a new `SnapMode::Data` variant in the Braille wire protocol (upstream PR to Rainicorns), or a standalone `browse_extract` MCP tool that runs the JS extraction without a new snapshot mode?
>
> - **Option A: SnapMode::Data** — cleaner architecture, auto-detects embedded JSON on any site. Adds a variant to the wire protocol + a serializer in the engine.
> - **Option B: browse_extract MCP tool** — ships faster, doesn't require upstream changes. Wraps `eval_js` with site-specific extraction scripts.
> - **Recommended: Option A** — it's the right abstraction for Braille as a browser engine. Every modern SPA embeds data this way (Next.js, Nuxt, Remix, Gatsby). A `Data` mode that extracts all `<script type="application/json">` blocks and `__NEXT_DATA__` would be universally useful, not Airbnb-specific.

### Task 4: Add SnapMode::Data variant (if Option A)

**Files:**
- Modify: `crates/wire/src/lib.rs` — add `Data` variant to SnapMode enum
- Modify: `crates/cli/src/main.rs` — add `data` to CLI mode parser
- Create: `crates/engine/src/a11y/serialize/data.rs` — serializer
- Modify: `crates/engine/src/a11y/serialize/mod.rs` — export
- Modify: `crates/engine/src/lib.rs` — add match arm in snapshot()
- Create: `crates/engine/tests/snapshot_data.rs` — tests

The serializer walks the DOM looking for:
1. `<script id="data-deferred-state-*" type="application/json">` (Airbnb pattern)
2. `<script id="__NEXT_DATA__" type="application/json">` (Next.js pattern)
3. Any `<script type="application/json">` with id containing "data"
4. `<script>window.__INITIAL_STATE__` or similar hydration globals

Returns JSON:
```json
{
  "embedded_data": [
    {"id": "data-deferred-state-0", "type": "application/json", "size": 402279, "data": {...}},
    {"id": "__NEXT_DATA__", "type": "application/json", "size": 8192, "data": {...}}
  ],
  "meta": {"url": "...", "title": "..."}
}
```

---

## Phase 3: Agent Integration via Braille MCP

<!-- GATING: user-approval-required -->

> **Gating question 2:** Where should accommodation search live in the agent ecosystem?
>
> - **Option A: New "Travel" agent** (L5 Advisor, `~/travel/`, reports to null)
> - **Option B: Extend Q** to cover services/accommodations (broadens Q's current "products only" scope)
> - **Option C: No dedicated agent** — the CTO or user invokes Braille MCP directly for ad-hoc searches
> - **Recommended: Option C for now, Option A later.** Accommodation search doesn't yet need a persistent agent personality. The immediate value is: user says "find me an Airbnb in SF this weekend" → CTO uses Braille MCP → runs extraction → presents decision matrix → provides deep links. When the pattern stabilizes, birth a Travel agent via `/birth`.

> **Gating question 3:** Should the extraction script live in the Braille MCP server (as a `browse_extract_airbnb` tool), or stay as a JS file that any agent can load and pass to `browse_eval`?
>
> - **Option A: Dedicated MCP tool** — `browse_extract_airbnb(session, url)` that handles goto + extract in one call
> - **Option B: JS file in tf-ais** — agents load from `~/tf-ais/scripts/extractors/airbnb.js` and pass to `browse_eval`
> - **Recommended: Option B** — site-specific extractors shouldn't be baked into the engine. They change when sites update. A `tf-ais/scripts/extractors/` directory with per-site JS files is more maintainable and doesn't require engine rebuilds.

### Task 5: Ship the extractor to tf-ais (if Option B)

**Files:**
- Create: `~/tf-ais/scripts/extractors/airbnb.js` — copy of the extraction function
- Create: `~/tf-ais/scripts/extractors/README.md` — usage docs

### Task 6: CTO prompt for accommodation search

Usage pattern (no dedicated agent needed):

```
User: find me an Airbnb in SF for next weekend, 2 adults
CTO: [uses Braille MCP]
  1. browse_new → session
  2. browse_goto → https://www.airbnb.com/s/San-Francisco/homes?checkin=2026-04-11&checkout=2026-04-13&adults=2
  3. browse_eval → contents of airbnb.js extractor
  4. Parse JSON → present decision matrix
  5. User picks → CTO opens deep link
```

---

## Verification

1. `cargo test -p braille-engine --test airbnb_search` — all extraction tests pass
2. `cargo clippy --workspace` — zero warnings
3. Live smoke: `braille $SID goto "https://www.airbnb.com/s/SF/homes" && braille $SID eval "$(cat airbnb_extract.js)"` — returns 18 listings with deep links
4. Deep link verification: open a returned deep link in browser, confirm it loads the correct listing

## Risks

- **Airbnb schema changes** — `data-deferred-state-0` is an internal implementation detail. If Airbnb changes the key names or data structure, the extractor breaks. Mitigation: the extractor returns `{error: "..."}` with diagnostic info when structure doesn't match, making it easy to update.
- **Anti-bot detection** — Airbnb may serve different HTML to headless browsers. Braille's lightweight HTTP client may evade detection better than Chromium-based tools, but needs monitoring.
- **Rate limiting** — repeated searches from same IP. Mitigation: use sparingly, cache results.
- **Base64 ID format** — Airbnb uses `DemandStayListing:{numeric_id}` format. If they change the encoding, deep links break. The `propertyId` field (when present) is more stable.
