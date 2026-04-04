//! Real-site static/SSR extraction tests.
//!
//! These tests validate Braille against recorded HTTP transcripts from real
//! websites that TF-AIs agents actually need. Each fixture was recorded once
//! via a Python recording script and is replayed deterministically.
//!
//! Sites in this file are all static/SSR — content is in the initial HTML,
//! with minimal JavaScript required for core content visibility.
//!
//! Recording date: 2026-04-04
//! Recording script: scripts/record-fixture.py
//!
//! Tests are expected to be HONEST: if a test fails, the failure represents
//! a real gap in Braille's ability to extract content from that site.

use braille_engine::navigation::FetchProvider;
use braille_engine::transcript::Transcript;
use braille_engine::Engine;
use braille_wire::{FetchOutcome, FetchRequest, FetchResult, SnapMode};

/// A replay fetcher that serves recorded exchanges for the page and scripts,
/// then returns network errors for any additional fetch_batch calls during
/// the JS settlement phase. This mirrors real-world behavior where dynamic
/// fetches to unrecorded URLs would fail.
struct StaticSiteReplayer {
    exchanges: Vec<braille_engine::transcript::Exchange>,
    cursor: usize,
}

impl StaticSiteReplayer {
    fn load(path: &str) -> Self {
        let data = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("failed to read transcript {path}: {e}"));
        let transcript: Transcript = serde_json::from_str(&data)
            .unwrap_or_else(|e| panic!("failed to parse transcript {path}: {e}"));
        Self {
            exchanges: transcript.exchanges,
            cursor: 0,
        }
    }
}

impl FetchProvider for StaticSiteReplayer {
    fn fetch_batch(&mut self, requests: Vec<FetchRequest>) -> Vec<FetchResult> {
        if self.cursor >= self.exchanges.len() {
            // Settlement phase: JS triggered fetches we didn't record.
            // Return network errors — same as if those endpoints were unreachable.
            return requests
                .into_iter()
                .map(|r| FetchResult {
                    id: r.id,
                    outcome: FetchOutcome::Err(format!("not recorded: {}", r.url)),
                })
                .collect();
        }
        let exchange = &self.exchanges[self.cursor];
        self.cursor += 1;
        requests
            .iter()
            .zip(exchange.results.iter())
            .map(|(req, recorded)| FetchResult {
                id: req.id,
                outcome: recorded.outcome.clone(),
            })
            .collect()
    }
}

// ===========================================================================
// Hacker News — news.ycombinator.com
// Agent: Doraemon (tech discussion, practitioner reports)
// Exercises: Static HTML with table-based layout, minimal JS
// ===========================================================================

#[test]
fn hackernews_front_page_loads() {
    let mut fetcher = StaticSiteReplayer::load("tests/fixtures/hackernews-front.json");
    let mut engine = Engine::new();
    let snapshot = engine
        .navigate(
            "https://news.ycombinator.com/",
            &mut fetcher,
            SnapMode::Compact,
        )
        .unwrap();

    assert!(
        snapshot.contains("Hacker News"),
        "page title missing from snapshot:\n{snapshot}"
    );
}

#[test]
fn hackernews_has_story_links() {
    let mut fetcher = StaticSiteReplayer::load("tests/fixtures/hackernews-front.json");
    let mut engine = Engine::new();
    let snapshot = engine
        .navigate(
            "https://news.ycombinator.com/",
            &mut fetcher,
            SnapMode::Accessibility,
        )
        .unwrap();

    // HN front page should have multiple story links visible
    // The a11y snapshot should contain link elements with story titles
    let link_count = snapshot.matches("link ").count();
    assert!(
        link_count >= 5,
        "expected at least 5 links on HN front page, found {link_count}:\n{snapshot}"
    );
}

#[test]
fn hackernews_has_score_indicators() {
    let mut fetcher = StaticSiteReplayer::load("tests/fixtures/hackernews-front.json");
    let mut engine = Engine::new();
    let snapshot = engine
        .navigate(
            "https://news.ycombinator.com/",
            &mut fetcher,
            SnapMode::Compact,
        )
        .unwrap();

    // Stories on HN have point counts like "123 points"
    assert!(
        snapshot.contains("point"),
        "no point/score indicators found on HN front page:\n{snapshot}"
    );
}

// ===========================================================================
// old.reddit.com — r/BuyItForLife
// Agent: Q (real-world product longevity reports)
// Exercises: Server-rendered HTML, comment structure, vote counts
// ===========================================================================

#[test]
fn reddit_buyitforlife_loads() {
    let mut fetcher =
        StaticSiteReplayer::load("tests/fixtures/reddit-buyitforlife.json");
    let mut engine = Engine::new();
    let snapshot = engine
        .navigate(
            "https://old.reddit.com/r/BuyItForLife/top/?t=month",
            &mut fetcher,
            SnapMode::Compact,
        )
        .unwrap();

    assert!(
        snapshot.contains("BuyItForLife"),
        "subreddit name missing from snapshot:\n{snapshot}"
    );
}

#[test]
fn reddit_has_post_titles() {
    let mut fetcher =
        StaticSiteReplayer::load("tests/fixtures/reddit-buyitforlife.json");
    let mut engine = Engine::new();
    let snapshot = engine
        .navigate(
            "https://old.reddit.com/r/BuyItForLife/top/?t=month",
            &mut fetcher,
            SnapMode::Accessibility,
        )
        .unwrap();

    // Reddit listings should have multiple post links
    let link_count = snapshot.matches("link ").count();
    assert!(
        link_count >= 10,
        "expected at least 10 links on BIFL top page, found {link_count}:\n{snapshot}"
    );
}

// ===========================================================================
// Serious Eats — Chicken Paprikash recipe
// Agent: Gral (recipe extraction, technique rationale)
// Exercises: SSR with recipe schema markup, structured content
// ===========================================================================

#[test]
fn seriouseats_recipe_loads() {
    let mut fetcher =
        StaticSiteReplayer::load("tests/fixtures/seriouseats-paprikash.json");
    let mut engine = Engine::new();
    let snapshot = engine
        .navigate(
            "https://www.seriouseats.com/chicken-paprikash",
            &mut fetcher,
            SnapMode::Compact,
        )
        .unwrap();

    assert!(
        snapshot.to_lowercase().contains("paprika"),
        "recipe content (paprika) missing from snapshot:\n{snapshot}"
    );
}

#[test]
fn seriouseats_has_ingredients() {
    let mut fetcher =
        StaticSiteReplayer::load("tests/fixtures/seriouseats-paprikash.json");
    let mut engine = Engine::new();
    let snapshot = engine
        .navigate(
            "https://www.seriouseats.com/chicken-paprikash",
            &mut fetcher,
            SnapMode::Compact,
        )
        .unwrap();

    // A paprikash recipe should mention key ingredients
    assert!(
        snapshot.to_lowercase().contains("chicken"),
        "ingredient (chicken) missing from recipe:\n{snapshot}"
    );
    assert!(
        snapshot.to_lowercase().contains("sour cream")
            || snapshot.to_lowercase().contains("paprika"),
        "key ingredients missing from recipe:\n{snapshot}"
    );
}

// ===========================================================================
// Just One Cookbook — Sushi Rice
// Agent: Gral (Japanese cooking technique, ingredient ratios)
// Exercises: WordPress site, recipe plugin markup, heavy JS
// ===========================================================================

#[test]
fn justonecookbook_sushi_rice_loads() {
    let mut fetcher =
        StaticSiteReplayer::load("tests/fixtures/justonecookbook-sushi-rice.json");
    let mut engine = Engine::new();
    let snapshot = engine
        .navigate(
            "https://www.justonecookbook.com/how-to-make-sushi-rice/",
            &mut fetcher,
            SnapMode::Compact,
        )
        .unwrap();

    assert!(
        snapshot.to_lowercase().contains("sushi"),
        "sushi rice content missing from snapshot:\n{snapshot}"
    );
}

#[test]
fn justonecookbook_has_recipe_content() {
    let mut fetcher =
        StaticSiteReplayer::load("tests/fixtures/justonecookbook-sushi-rice.json");
    let mut engine = Engine::new();
    let snapshot = engine
        .navigate(
            "https://www.justonecookbook.com/how-to-make-sushi-rice/",
            &mut fetcher,
            SnapMode::Compact,
        )
        .unwrap();

    // Sushi rice recipe needs rice, vinegar, and sugar
    assert!(
        snapshot.to_lowercase().contains("rice"),
        "ingredient (rice) missing:\n{snapshot}"
    );
    assert!(
        snapshot.to_lowercase().contains("vinegar"),
        "ingredient (vinegar) missing:\n{snapshot}"
    );
}

// ===========================================================================
// iFixit — Teardown index
// Agent: Q (repairability assessment, product longevity)
// Exercises: Webpack-chunked SPA with SSR initial content
// ===========================================================================

#[test]
fn ifixit_teardown_index_loads() {
    let mut fetcher =
        StaticSiteReplayer::load("tests/fixtures/ifixit-teardown.json");
    let mut engine = Engine::new();
    let snapshot = engine
        .navigate(
            "https://www.ifixit.com/Teardown",
            &mut fetcher,
            SnapMode::Compact,
        )
        .unwrap();

    assert!(
        snapshot.to_lowercase().contains("teardown"),
        "teardown content missing from snapshot:\n{snapshot}"
    );
}

#[test]
fn ifixit_has_device_listings() {
    let mut fetcher =
        StaticSiteReplayer::load("tests/fixtures/ifixit-teardown.json");
    let mut engine = Engine::new();
    let snapshot = engine
        .navigate(
            "https://www.ifixit.com/Teardown",
            &mut fetcher,
            SnapMode::Accessibility,
        )
        .unwrap();

    // Teardown index should list multiple devices as links
    let link_count = snapshot.matches("link ").count();
    assert!(
        link_count >= 5,
        "expected at least 5 device links on teardown index, found {link_count}:\n{snapshot}"
    );
}

// ===========================================================================
// Python docs — sqlite3 module
// Agent: Doraemon (API documentation lookup)
// Exercises: Sphinx-generated static docs, code blocks, function signatures
// ===========================================================================

#[test]
fn python_docs_sqlite3_loads() {
    let mut fetcher =
        StaticSiteReplayer::load("tests/fixtures/python-docs-sqlite3.json");
    let mut engine = Engine::new();
    let snapshot = engine
        .navigate(
            "https://docs.python.org/3/library/sqlite3.html",
            &mut fetcher,
            SnapMode::Compact,
        )
        .unwrap();

    assert!(
        snapshot.contains("sqlite3"),
        "sqlite3 module name missing from snapshot:\n{snapshot}"
    );
}

#[test]
fn python_docs_has_api_content() {
    let mut fetcher =
        StaticSiteReplayer::load("tests/fixtures/python-docs-sqlite3.json");
    let mut engine = Engine::new();
    let snapshot = engine
        .navigate(
            "https://docs.python.org/3/library/sqlite3.html",
            &mut fetcher,
            SnapMode::Compact,
        )
        .unwrap();

    // sqlite3 docs should contain the connect() function
    assert!(
        snapshot.contains("connect"),
        "connect() function missing from docs:\n{snapshot}"
    );
    // Should mention database concepts
    assert!(
        snapshot.to_lowercase().contains("database"),
        "database references missing from docs:\n{snapshot}"
    );
}
