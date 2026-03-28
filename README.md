# Braille

A browser for those who read, not see.

Braille is a lightweight browser engine that maintains a virtual DOM with full JavaScript execution but skips graphical rendering entirely. It outputs structured text representations of web pages for LLM agents to read and interact with.

## The Problem

Most of what people do on computers happens in a browser. Booking flights, filing forms, checking inventory, managing accounts. AI agents today can write code and call APIs — but they can't use the web the way a person does.

That's not because the task is hard for AI. It's because the plumbing doesn't exist. There's no fast, lightweight way for an agent to load a page, run its JavaScript, read what's on screen, click a button, and continue — across multiple steps, with state preserved.

Braille is that plumbing.

## Why This Exists

AI agents need to use the web. The current options are bad:

- **Fetch the HTML** — fails on any JavaScript-rendered page (most modern sites)
- **Headless Chrome** — slow (seconds per page), resource-hungry, expensive to scale
- **Screenshots + vision** — even slower, burns vision tokens, pixel-based clicking is fragile

Braille is the missing middle: a real browser engine that runs JavaScript, handles events, manages state — but outputs text instead of pixels. **3.5ms per page.** No Chromium. No screenshots.

### How It Compares

| | Raw fetch | Headless Chrome | Screenshots + Vision | **Braille** |
|---|---|---|---|---|
| JavaScript execution | No | Yes | Yes | **Yes** |
| Speed per page | ~50ms | ~2-5s | ~5-10s | **~3.5ms** |
| Token cost per page | ~50K (raw HTML) | ~50K (raw HTML) | ~1K (vision) | **~2K (text)** |
| Multi-step state | No | Yes (process) | Yes (process) | **Yes (checkpoint)** |
| Infrastructure | None | Chromium fleet | Chromium + vision | **~15MB binary** |
| Can interact (click/type) | No | Yes | Yes (pixel coords) | **Yes (refs/selectors)** |
| Works on SPAs | No | Yes | Yes | **Yes** |
| Deterministic | Yes | No | No | **Yes** |

## Real-World Comparison

Here's what actually happens when you try to browse Wikipedia with each approach:

**Raw fetch (what most AI tools use):**
```
HTTP 403 Forbidden
```
Many real websites reject bot-like fetch requests outright. No content, no interaction, nothing.

**Headless Chrome (Playwright/Puppeteer):**
```
[launches 200MB Chromium process]
[waits 2-5 seconds for page load + JS execution]
[returns 500KB of raw HTML or a screenshot]
```
Works, but heavy. The raw HTML is full of `<div class="sc-bwzfXH iJvPOa">` noise. A screenshot costs vision tokens and the agent can't programmatically identify what's clickable.

**Braille:**
```
[3.17 seconds including network fetch]

# Welcome to Wikipedia
the free encyclopedia that anyone can edit.
283,248 active editors | 7,159,332 articles in English

[@e19 input[type=search]]
[@e21 button "Search"]

## From today's featured article
The Boat Races 2016 took place on 27 March 2016...
```
Clean text. Every interactive element labeled. An agent can immediately type into `@e19` and click `@e21` to search. Total output: ~2,000 tokens instead of 50,000.

## How It Works

```
HTML + JS → Parse → DOM Tree → JavaScript Execution → Accessibility Tree → Text Output
```

1. **html5ever** parses HTML into an arena-based DOM (100% html5lib compliance)
2. **QuickJS** executes JavaScript with full Web API bindings
3. **CSS cascade** computes styles for visibility and layout decisions
4. **Accessibility serializer** converts the live DOM into token-efficient text
5. An LLM agent reads the output, decides what to do, and sends commands back

## What It Looks Like

Given a page with navigation, a search form, a product list, and JavaScript-rendered content, here's what an agent sees:

**Raw HTML** (~50,000 tokens — what fetch gives you):
```html
<!DOCTYPE html><html lang="en"><head><meta charset="utf-8"><meta name="viewport"
content="width=device-width,initial-scale=1"><title>Store</title><link rel="stylesheet"
href="/static/css/main.a3b2c1.css"><script defer src="/static/js/bundle.7f8e9d.js">
</script></head><body><div id="root"><div class="sc-bwzfXH iJvPOa"><div class="sc-htpNat
kEbgjr"><nav class="sc-bxivhb cNVMSP" role="navigation"><div class="sc-ifAKCX ...
<!-- 2000 more lines of div soup, CSS class hashes, and bundled JS -->
```

**Compact view** (~2,000 tokens — what Braille gives you):
```
Store - Online Shopping

[nav] Home | Products | About | Cart (3)

Search: [input @e1 ""] [button @e2 "Search"]

# Products

Product A - $29.99
  [button @e3 "Add to Cart"]
Product B - $49.99
  [button @e4 "Add to Cart"]
Product C - $19.99
  [button @e5 "Add to Cart"]

[Showing 1-3 of 47] [button @e6 "Next Page"]
```

**Interactive view** (~200 tokens — just what you can act on):
```
@e1 input[text] ""
@e2 button "Search"
@e3 button "Add to Cart"
@e4 button "Add to Cart"
@e5 button "Add to Cart"
@e6 button "Next Page"
```

The agent reads the Compact view to understand the page, switches to Interactive to pick an action, then runs `braille click @e4` to add Product B to cart.

## Smart Snapshot Views

Not every task needs the full page. Braille offers specialized views that show only what's relevant — dramatically reducing token usage:

| View | What it shows | Use case |
|------|--------------|----------|
| **Compact** | Text + interactive elements (default) | General browsing |
| **Interactive** | Only clickable/typeable elements with `@eN` refs | "What can I click?" |
| **Links** | Only `<a>` elements with URLs | Link discovery, navigation |
| **Forms** | Form structure with fields and current values | Form filling |
| **Headings** | h1-h6 outline with hierarchy | Page structure, navigation |
| **Text** | Readable content only, no structure | Reading articles |
| **Selector** | Elements matching a CSS selector | Targeted extraction |
| **Region** | Subtree under a specific element | Focused interaction |
| **Accessibility** | Full a11y tree with roles and hierarchy | Detailed inspection |
| **Dom** | Raw DOM structure | Debugging |
| **Markdown** | Page content as markdown | Content extraction |

### Stable References

Every interactive element gets a stable `@eN` reference (e.g., `@e1`, `@e2`) that persists across all views. An agent can discover a button in the Interactive view, switch to Text view to read surrounding context, then click `@e3` — the reference always points to the same element.

### Token Efficiency

A typical page might produce:
- **Raw HTML**: ~50,000 tokens
- **Compact view**: ~2,000 tokens
- **Interactive view**: ~200 tokens

That's a 25-250x reduction, which means lower cost, more pages in context, and better comprehension.

### Example: Agent Filling Out a Form

```bash
# Agent navigates to a form page
$ braille goto https://example.com/apply --mode forms

form action="/api/apply" method="POST"
  @e1 input[text] name="full_name" value=""
  @e2 input[email] name="email" value=""
  @e3 select name="country" value="US"
    option "US" (selected)
    option "UK"
    option "CA"
  @e4 textarea name="cover_letter" value=""
  @e5 button[submit] "Submit Application"

# Agent fills in the fields
$ braille type "@e1" "Jane Smith"
$ braille type "@e2" "jane@example.com"
$ braille select "@e3" "CA"
$ braille type "@e4" "I am writing to express my interest..."

# Agent checks its work before submitting
$ braille snap --mode forms

form action="/api/apply" method="POST"
  @e1 input[text] name="full_name" value="Jane Smith"
  @e2 input[email] name="email" value="jane@example.com"
  @e3 select name="country" value="CA"
    option "US"
    option "CA" (selected)
    option "UK"
  @e4 textarea name="cover_letter" value="I am writing to express my interest..."
  @e5 button[submit] "Submit Application"

# Looks good — submit
$ braille click "@e5"
```

The agent used **Forms view** throughout — it never saw navigation, headers, footers, or any content irrelevant to its task.

## Agent Commands

```bash
# Start a session and navigate
braille new
braille goto https://example.com

# Read the page in different views
braille snap --mode compact
braille snap --mode interactive
braille snap --mode forms
braille snap --mode headings
braille snap --mode selector --query "nav a"
braille snap --mode region --target "main"

# Interact
braille click "@e3"              # Click element by ref
braille click "#submit-btn"      # Click by CSS selector
braille type "#search" "query"   # Type into an input
braille select "#country" "US"   # Select a dropdown option

# Navigate
braille back
braille forward

# Debug
braille console                  # View JS console output

# Record & replay
braille $SID goto --record URL   # Record network transcript
braille $SID transcript          # View the transcript
```

## Architecture

```
crates/
├── engine/          # Core browser engine
│   ├── src/
│   │   ├── html/        # html5ever TreeSink (HTML parsing)
│   │   ├── dom/         # Arena-based DOM tree
│   │   ├── css/         # CSS cascade + selector matching
│   │   ├── js/          # QuickJS runtime + Web API bindings
│   │   ├── a11y/        # Accessibility tree serializer
│   │   └── commands/    # click, type, select, form submission
│   ├── tests/           # 2600+ tests
│   └── benches/         # Criterion benchmarks
├── cli/             # Command-line interface
├── wire/            # Session protocol (serde serialization)
└── spike-quickjs/   # QuickJS evaluation spike
```

### Key Design Decisions

**Arena-based DOM.** Nodes are `usize` indices into a `Vec`. No reference counting, no GC, deterministic iteration. Cheap to clone, easy to serialize.

**QuickJS over V8.** V8 is faster but impossible to checkpoint. QuickJS is embeddable, deterministic, and supports the full ES2020 spec. Good enough for DOM manipulation — we're not running compute-heavy JS.

**Accessibility tree output.** Instead of inventing a format, we reuse the mental model of screen readers. Roles, labels, states, hierarchy — a structure designed for non-visual consumption. Perfect for LLMs.

**Virtual clock.** Timers run on a virtual clock that advances during `settle()`. No real-time waiting. A page with `setTimeout(fn, 5000)` resolves instantly in virtual time. This makes agent interactions deterministic and fast.

**Settle loop.** After every interaction (click, type, navigate), the engine runs a settle loop: flush microtasks, fire MutationObservers, advance timers, repeat until quiescent. This ensures the page reaches a stable state before the agent sees it.

## Web Compatibility

| Test Suite | Result |
|---|---|
| html5lib tree construction | **1778/1778 (100%)** |
| html5lib serializer | **204/204** |
| Engine unit tests | **369 passed** |
| DOM bridge integration | **63 passed** |
| Framework tests (React/Vue/Svelte) | **31 passed** |
| React controlled inputs | **9 passed** |
| Smoke integration | **20 passed** |
| CSS adversarial | **32 passed** |
| Snapshot views | **16 passed** |
| WPT DOM | ~187/353 (spec compliance gaps, not regressions) |

**Zero clippy warnings.** Strict workspace-wide lints enabled.

## Session Persistence

This is the hard problem. An agent conversation might span minutes or hours, with many turns between web interactions. The browser state — DOM, JS heap, closures, event listeners, localStorage, everything — needs to survive between invocations. But there's no long-lived process. Agent conversations may run across different containers, different machines.

**The solution:** each session is a minimal container (~15-20MB static binary on a `scratch` image). Between CLI invocations, the container is checkpointed to disk using CRIU — the entire process memory is frozen, including the JS heap and all runtime state. Next invocation restores the container. The engine never knows it stopped.

The container has **zero network access**. All HTTP is handled by the CLI on the host side and piped in via stdin/stdout. The container *is* the sandbox — no need for WASM isolation or separate security layers.

No daemon. No sidecar. No serialization of JS state. Just freeze and restore.

## Performance

**3.5ms per page** — full pipeline from HTML parse through JavaScript execution to snapshot output. Benchmarked on a single-page app with navigation, 30 dynamically-generated product cards, and event handlers.

## What This Enables

When web interaction is fast and cheap enough, tasks that were theoretically possible but practically never happened start happening routinely:

- "Monitor 15 supplier websites for price changes every morning"
- "Log into my insurance portal, download last year's claims, put them in a spreadsheet"
- "Fill out this 10-page government form using info from my documents"
- "Check if my flight has a better seat available, and if so, switch to it"
- "Go through my inbox, find all receipts, and categorize them"

These aren't new capabilities — a person could always do them. But the friction was too high, so they didn't get done. Braille removes the friction for AI agents the same way Uber removed the friction for getting a ride: not by inventing transportation, but by making it effortless enough that behavior changes.

## Philosophy

Braille is built on a few beliefs:

1. **The web is where the work is.** Most human workflows happen in browsers. Agents that can't use the web are limited to APIs — and most of the world doesn't have an API.

2. **Text is the right interface for LLMs.** Screenshots are expensive and lossy. Raw HTML is noisy. An accessibility tree is the information an LLM actually needs, in the format it processes best.

3. **Speed changes behavior.** At seconds-per-page, web browsing is a bottleneck agents route around. At milliseconds-per-page, it becomes something agents do casually and repeatedly — checking, verifying, exploring.

4. **Less is more.** Don't render what you don't need. Don't send tokens the model will ignore. Don't spin up Chrome when QuickJS will do. Every architectural choice optimizes for the minimum viable browser that gets the job done.

## Agent Integration

The section below is designed to be copied into a tool description, MCP server config, or system prompt. It gives an AI agent everything it needs to use Braille without prior knowledge.

<details>
<summary><strong>Copy-pasteable tool description for agents</strong></summary>

### Braille — Text Browser Tool

You have access to a text browser called `braille` that lets you browse the web, interact with pages, and read their content. It runs JavaScript, handles SPAs, and maintains state across commands.

#### Session Lifecycle

```
braille new                    → returns a SESSION_ID
braille SESSION_ID goto URL    → navigates and returns page snapshot
braille SESSION_ID snap        → returns current page snapshot
braille SESSION_ID close       → ends the session
```

Always start with `braille new` to get a session ID. Use that ID for all subsequent commands.

#### Commands

| Command | Syntax | What it does |
|---------|--------|-------------|
| **goto** | `SESSION_ID goto URL [--mode MODE] [--record]` | Navigate to a URL. Returns a page snapshot. `--record` saves the network transcript. |
| **click** | `SESSION_ID click SELECTOR` | Click an element. Triggers JS event handlers, may navigate. |
| **type** | `SESSION_ID type SELECTOR TEXT` | Type text into an input or textarea. Replaces current value. |
| **select** | `SESSION_ID select SELECTOR VALUE` | Choose an option in a `<select>` dropdown by value. |
| **snap** | `SESSION_ID snap [--mode MODE]` | Take a snapshot of the current page in the given view mode. |
| **back** | `SESSION_ID back` | Go back in navigation history. |
| **forward** | `SESSION_ID forward` | Go forward in navigation history. |
| **console** | `SESSION_ID console` | Show JavaScript console output (log/warn/error). |
| **transcript** | `SESSION_ID transcript` | Show the last recorded network transcript (requires `--record` on goto). |

#### Selectors

Commands that take a SELECTOR accept:
- **Element refs**: `@e1`, `@e2`, etc. — stable references shown in snapshot output (preferred)
- **CSS selectors**: `#my-id`, `.my-class`, `button[type="submit"]`, `nav a:first-child`
- **Tag names**: `button`, `input`, `form`

Element refs (`@eN`) are the most reliable — they're assigned in document order, stable across view modes, and unambiguous.

#### View Modes

Use `--mode` with `goto` or `snap` to control what you see:

| Mode | Use when you want to... | Token cost |
|------|------------------------|------------|
| `compact` (default) | Read the page and see what you can interact with | Low |
| `interactive` | See only clickable/typeable elements | Very low |
| `links` | Find links and their URLs | Very low |
| `forms` | See form structure, field names, current values | Low |
| `headings` | Understand page structure via h1-h6 outline | Very low |
| `text` | Read article/content text without any structure | Low |
| `selector --query "CSS"` | See only elements matching a CSS selector | Very low |
| `region --target "CSS"` | See a subtree (e.g., `--target main` for main content only) | Low |
| `accessibility` | Full accessibility tree with roles and hierarchy | Medium |
| `markdown` | Page content as markdown | Medium |
| `dom` | Raw DOM structure (debugging only) | High |

**Strategy**: Start with `compact` to understand the page. Switch to `interactive` or `forms` when you need to act. Use `text` to read content. Use `region` to focus on a specific part of the page.

#### Output Format

**Success**: Returns the page snapshot as plain text. Interactive elements are labeled with `@eN` refs:
```
Search: [input @e1 ""] [button @e2 "Search"]
Product A - $29.99 [button @e3 "Add to Cart"]
```

**Error**: Returns a line starting with `error:`:
```
error: no element matches selector '#nonexistent'
```

**Console output**: If JavaScript produced console output, it appears at the end after `[console]`:
```
Page content here...
[console]
[log] App initialized
[warn] Deprecated API usage
```

#### Tips

- After `click`, the page may have changed. Use `snap` to see the new state.
- `goto` automatically returns a snapshot — you don't need a separate `snap` after navigating.
- If a page is JavaScript-heavy, the engine automatically waits for it to settle (microtasks, timers, re-renders) before returning the snapshot.
- For multi-step form filling: `type` each field, then `click` the submit button.
- If you need to verify form state before submitting, use `snap --mode forms`.
- Use `console` to debug when JavaScript errors might be causing issues.

</details>

## Session Recording & Replay

Braille can record every HTTP exchange during a browsing session and replay it deterministically in tests. This lets you capture exactly what a live server returned — headers, cookies, HTML, scripts, API responses — and replay that session offline to reproduce bugs.

### Recording a session

Pass `--record` to `goto`. The transcript is saved to the session directory and can be retrieved by session ID:

```bash
braille new                                           # → sess_abc12345
braille sess_abc12345 goto --record "https://example.com"
braille sess_abc12345 transcript                      # → prints the JSON transcript
```

After the `goto` completes, the session directory contains a transcript of every `fetch_batch` exchange the engine made during navigation: the initial page fetch, script fetches, dynamic fetch() calls, and any meta-refresh or location.href redirect fetches.

When `--record` is not passed, there is zero overhead — no transcript is serialized.

### Transcript format

```json
{
  "url": "https://anubis.techaro.lol",
  "exchanges": [
    {
      "requests": [
        {"id": 0, "url": "https://anubis.techaro.lol", "method": "GET", "headers": [...], "body": null}
      ],
      "results": [
        {"id": 0, "outcome": {"Ok": {"status": 200, "status_text": "OK", "headers": [...], "body": "<!doctype html>..."}}}
      ]
    }
  ]
}
```

Each exchange is one `fetch_batch` call — a batch of requests sent together and their results. Exchanges are ordered chronologically: page fetch first, then script fetches, then dynamic fetches during settle.

### Replaying in tests

```rust
use braille_engine::transcript::ReplayFetcher;
use braille_engine::Engine;
use braille_wire::SnapMode;

#[test]
fn replay_anubis_session() {
    let mut fetcher = ReplayFetcher::load("tests/fixtures/anubis.json").unwrap();
    let mut engine = Engine::new();
    let snapshot = engine
        .navigate("https://anubis.techaro.lol", &mut fetcher, SnapMode::Text)
        .unwrap();
    assert!(snapshot.contains("Anubis"));
}
```

`ReplayFetcher` serves recorded responses sequentially, remapping request IDs by position (the engine assigns fresh IDs each run, but the order is deterministic).

### Anubis: a real-world proof point

[Anubis](https://github.com/TecharoHQ/anubis) is a proof-of-work bot challenge that protects websites. It serves a challenge page with an inline `<script type="module">` that bundles Preact and SHA-256, computes a hash, then redirects via `location.href` to a pass-challenge endpoint. The server responds with a 302 that sets a JWT auth cookie and redirects to the real site. Every subsequent request — page, scripts, assets — must include that cookie or get challenged again.

Braille now gets through Anubis end-to-end: solve the challenge, follow the redirects, capture the auth cookie, load the real Docusaurus site with all JavaScript hydrated. Getting here required fixing three distinct bugs, each found via session recording and isolated with a failing test before the fix:

1. **Cookie loss across HTTP redirects.** Reqwest's automatic redirect follower consumed Set-Cookie headers from 302 responses into its internal cookie jar — our engine's cookie jar never saw them. Fix: disabled reqwest's redirect following and cookie jar, implemented manual redirect loop in `do_fetch` that accumulates Set-Cookie headers from every hop and forwards cookies on subsequent redirect hops.

2. **Script fetches missing cookies.** `fetch_scripts()` sent requests with empty headers. The page fetch attached cookies, `resolve_pending_fetches_via()` attached cookies, but script fetches didn't. Fix: call `get_cookies_for_url()` for each script request, same as the other fetch paths.

3. **Relative URLs vs cookie lookup.** Script `src` attributes like `/assets/js/main.js` are relative. `get_cookies_for_url("/assets/js/main.js")` fails because `url::Url::parse` can't extract a domain from a relative URL. Fix: resolve relative script URLs against the page URL before looking up cookies.

Session recording was essential for diagnosing these — the transcript showed exactly which requests had cookies, which didn't, and what the server returned for each.

## Building

```bash
# Build everything
cargo build --workspace

# Run all tests
cargo test --workspace

# Run benchmarks
cargo bench -p braille-engine

# Lint
cargo clippy --workspace
```

## License

[TODO]
