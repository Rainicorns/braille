# Braille

A lightweight browser that maintains a virtual DOM with full JavaScript execution but skips graphical rendering entirely. Outputs structured text representations of pages for LLM agents to read and interact with.

A browser for those who read, not see.

**Spike:** [SPIKE.md](./SPIKE.md) — COMPLETE (46 tests, core loop proven)
**API Reference:** [REFERENCE.md](./REFERENCE.md) — Boa and html5ever API details

## Status

All 6 phases complete (770 tests). The engine has a full DOM API surface (~70 methods), CSS cascade with selector matching wired into the load pipeline, full event system (addEventListener/dispatchEvent with capture/bubble/at-target), getComputedStyle, HTMLElement-specific properties (input.value/checked/type/disabled, select.value/selectedIndex/options, option.value/selected/text, a.href, form.action/method/elements, element.dataset/hidden/tabIndex/title/lang/dir, focus/blur/click stubs, getBoundingClientRect stub), and JS bindings for querySelector, innerHTML, classList, element.style, node mutation, window/console, and more. CLI has all commands routed through session manager, network client with cookie jar, navigation history, and external script loading. Full integration smoke tests (20) and CSS edge case tests (32) verify end-to-end behavior.

### What exists (770 tests)

| Component | Status | What works |
|-----------|--------|------------|
| DOM tree | Arena-based, full ops | createElement, appendChild, removeChild, insertBefore, replaceChild, cloneNode, getElementById, getElementsByTagName, querySelector/All, textContent, innerHTML, attribute CRUD, class list, node traversal |
| HTML parser | html5ever TreeSink | Full spec-compliant HTML parsing into DomTree, fragment parsing for innerHTML setter |
| JS engine | Boa bindings (~70 methods) | document: createElement, getElementById, querySelector/All, getElementsByClassName/TagName, createTextNode, body, head, title. element: appendChild, textContent, classList, getAttribute/setAttribute/removeAttribute, parentNode, children, firstChild, lastChild, siblings, nodeType/nodeName/tagName, innerHTML/outerHTML, insertAdjacentHTML, insertBefore, replaceChild, cloneNode, element.style, querySelector/All, getElementsByClassName/TagName. input: value, checked, type, disabled, name, placeholder. select: value, selectedIndex, options. option: value, selected, text. anchor: href. form: action, method, elements. element: hidden, dataset, tabIndex, title, lang, dir, getBoundingClientRect (stub), focus/blur (stubs), click (dispatches event) |
| CSS cascade | Parsing + matching + cascade + computed + wired + JS | cssparser stylesheet/inline parsing, selectors Element trait impl, selector matching (tag, class, id, attribute, pseudo-classes), cascade algorithm (origin, importance, specificity, source order), computed style resolution (inherit/initial/unset, em→px, color names), style tree DFS walk, compute_all_styles called in load_html/execute_scripts, getComputedStyle(el) JS binding with camelCase property accessors |
| Event system | Full W3C dispatch | Event/CustomEvent constructors, addEventListener/removeEventListener (capture, once options), dispatchEvent with capture/bubble/at-target phases, stopPropagation, stopImmediatePropagation, preventDefault |
| A11y serializer | Roles + values + CSS | headings, paragraphs, links, buttons, inputs (with value display), selects (with selected option), lists, images, nav, main, form; interactive refs (@e1); display:none skips element+descendants, visibility:hidden suppresses text but keeps structure |
| Wire protocol | serde types | Command/Response/SnapMode/Select/Focus/NavigateRequest/EngineAction enums |
| CLI | Fully wired | `new`, `goto` (live fetch + render), `click`/`type`/`select`/`focus`/`snap`/`back`/`forward`/`close` all routed through session manager, network client with cookie jar + URL resolution, navigation history, clear error messages |
| Engine | Integration + scripts + styles | `load_html` (parse + execute scripts + compute styles), `snapshot` (a11y mode), `parse_and_collect_scripts`/`execute_scripts` for external script loading, window/console globals, 32 end-to-end integration tests |

### What doesn't exist yet

| Component | Gap |
|-----------|-----|
| Layout | Not started. Taffy integration, real getBoundingClientRect, offsetWidth/Height |
| WASM sandbox | Not started — engine runs in-process |

## Implementation Plan

Three directions, all running. Shared dependencies noted. **51 agent tasks total across all three directions.**

Cross-direction dependencies:
- **C → B:** Direction C's selector matching (Agent C-1B) produces `query_selector`/`query_selector_all` that Direction B needs for `querySelector` JS binding (Agent B-2A). Run C Wave 0-1 before or alongside B Wave 2.
- **A → B:** Direction A's external script loading (Agent A-2C) benefits from Direction A's network client (Agent A-2A).
- **B → A:** Direction A's click/type/select commands need Direction B's attribute accessors. Direction A Wave 0B covers this but Direction B Agent 1B duplicates — merge or share.

### Detailed Implementation Blueprints

The planning agents produced detailed blueprints with API signatures, design rationale, test expectations, and merge strategies:

- **[IMPL_SESSIONS.md](./IMPL_SESSIONS.md)** — Direction A: daemon architecture, click/type/select/focus semantics, form data collection, navigation history, cookie jar, external script two-phase loading
- **[IMPL_DOM_API.md](./IMPL_DOM_API.md)** — Direction B: `register_on_class` pattern, classList/style/dataset object designs, event propagation algorithm, input.value vs attribute distinction, merge conflict mitigation
- **[IMPL_CSS_CASCADE.md](./IMPL_CSS_CASCADE.md)** — Direction C: `DomElement<'a>` wrapper, SelectorImpl trait types, cascade ordering (origin+importance+specificity), ComputedStyle struct, UA stylesheet, unit resolution, restyle strategy

These contain critical details not in the tables below.

### Direction A: Sessions + Interaction (18 agents, 4 waves, max 6 concurrent)

Make `braille` a working tool: persistent sessions, click/type/select/focus, external scripts, cookies, navigation history.

**Wave A-0: Foundations (5 agents, all parallel)**

| Agent | What | Files | Tests |
|-------|------|-------|-------|
| A-0A | Ref map — a11y serializer returns `HashMap<String, NodeId>`, Engine stores it, `resolve_ref("@e1")` | `a11y/serialize.rs`, `lib.rs` | Resolve @e1→NodeId, @e99→None |
| A-0B | Attribute accessors — `get_attribute`, `set_attribute`, `remove_attribute`, `has_attribute` on DomTree | `dom/tree.rs` | CRUD on attributes |
| A-0C | Wire protocol expansion — `Select`, `Focus` commands, `NavigateRequest`, `EngineAction` enum | `wire/src/lib.rs` | Serde roundtrips |
| A-0D | Element finder — resolve `@eN` refs, `#id` shorthand, tag name fallback to NodeId | `dom/find.rs` | Each resolution strategy |
| A-0E | Traversal helpers — `find_ancestor(tag)`, `find_descendants_by_tag`, `get_parent` | `dom/tree.rs` | Ancestor search, descendant collection |

**Wave A-1: Core Commands (6 agents, all parallel)**

| Agent | What | Depends |
|-------|------|---------|
| A-1A | Session manager — daemon on Unix socket, `HashMap<SessionId, Session>`, auto-start | A-0C |
| A-1B | Click: links — read `href`, return `EngineAction::Navigate` | A-0A, A-0B, A-0D |
| A-1C | Click: forms — `find_ancestor("form")`, collect inputs, build `NavigateRequest` (GET/POST) | A-0B, A-0E |
| A-1D | Type command — set `value` attr on `<input>`, text content on `<textarea>` | A-0B, A-0D |
| A-1E | Select command — find matching `<option>`, set `selected` attr | A-0B, A-0E |
| A-1F | Focus command — `focused_element: Option<NodeId>` on Engine, `[focused]` in a11y output | A-0A, A-0D |

**Wave A-2: Network + Scripts (4 agents, 3 parallel + 1 sequential)**

| Agent | What | Depends |
|-------|------|---------|
| A-2A | Network client — per-session cookie jar (reqwest cookies feature), redirect following, URL resolution | A-1A |
| A-2B | Navigation history — `Vec<String>` + index, back/forward re-fetch + load | A-1A |
| A-2C | External `<script src>` loading — two-phase: `parse_and_find_scripts` → CLI fetches → `execute_with_scripts` | A-2A |
| A-2D | A11y value display — show `value="..."` for inputs, selected option for selects | A-0B, A-1D |

**Wave A-3: Integration (3 agents, all parallel)**

| Agent | What |
|-------|------|
| A-3A | CLI wiring — all commands through daemon/session, goto/click/type/select/focus/snap/back/forward/close |
| A-3B | End-to-end integration tests — link click flow, form submission, select+submit, script execution+click |
| A-3C | Error hardening — clear panic messages for bad refs, non-input type targets, invalid sessions, network failures |

---

### Direction B: DOM API Surface (19 agents, 6 waves, max 8 concurrent)

Expand from 4 JS-accessible DOM methods to ~50+. Each agent implements both the Rust DomTree method and the Boa JS binding.

**Wave B-0: Structural Prep (1 agent)**

| Agent | What |
|-------|------|
| B-0A | Restructure `bindings.rs` into `bindings/` module directory. Each cluster gets its own file. `register_on_class(class)` pattern so `JsElement::init` only adds one line per cluster. |

**Wave B-1: Core Infrastructure (5 agents, all parallel)**

| Agent | What | Key APIs |
|-------|------|----------|
| B-1A | Node traversal | `parentNode`, `parentElement`, `children`, `childNodes`, `firstChild`, `lastChild`, `nextSibling`, `previousSibling`, `nextElementSibling`, `previousElementSibling`, `hasChildNodes`, `contains` |
| B-1B | Attributes | `getAttribute`, `setAttribute`, `removeAttribute`, `hasAttribute`, `element.id`, `element.className` |
| B-1C | Node info | `nodeType`, `nodeName`, `tagName`, `nodeValue`, `innerText` |
| B-1D | classList | `add`, `remove`, `toggle`, `contains`, `item`, `length` — JsClassList class backed by class attribute |
| B-1E | Document methods | `document.body`, `document.head`, `document.title`, `document.createTextNode`, `document.createDocumentFragment` (new `NodeData::DocumentFragment` variant) |

**Wave B-2: Querying + Mutation (5 agents, all parallel)**

| Agent | What | Key APIs |
|-------|------|----------|
| B-2A | querySelector | `querySelector`, `querySelectorAll`, `getElementsByClassName`, `getElementsByTagName` on Element — uses `selectors` crate, implements `selectors::Element` trait (**shared with Direction C**) |
| B-2B | Node mutation | `insertBefore`, `replaceChild`, `removeChild` (JS binding), `cloneNode(deep)` |
| B-2C | innerHTML | `innerHTML` get/set, `outerHTML` get, `insertAdjacentHTML`, `insertAdjacentElement` — HTML serializer + `parse_fragment` for setter |
| B-2D | element.style | `style.color = "red"`, `style.setProperty`, `style.removeProperty`, `style.cssText`, `style.getPropertyValue` — JsStyleDeclaration class |
| B-2E | Window + console | `window` global (self-referential), `window.document`, `window.location` (href/pathname/etc), `setTimeout`/`clearTimeout`/`setInterval`/`clearInterval` (stub — store callbacks), `console.log`/`warn`/`error` (buffer, no stdout) |

**Wave B-3: Events (3 agents, 2 parallel then 1)**

| Agent | What | Depends |
|-------|------|---------|
| B-3A | Event constructors | `new Event(type, options)`, `new CustomEvent(type, {detail})`, properties: `type`, `bubbles`, `cancelable`, `defaultPrevented`, `target`, `currentTarget`, `preventDefault()`, `stopPropagation()` | B-0 |
| B-3B | Listener registration | `addEventListener(type, fn, options)`, `removeEventListener` — listeners stored on JsRuntime (`HashMap<NodeId, Vec<ListenerEntry>>`) | B-3A |
| B-3C | Event dispatch | `dispatchEvent(event)` — capture phase (top-down), at-target, bubble phase (bottom-up), `stopPropagation`, `stopImmediatePropagation`, `once` removal | B-3A, B-3B |

**Wave B-4: HTMLElement Specifics (4 agents, all parallel)**

| Agent | What | Key APIs |
|-------|------|----------|
| B-4A | Input properties | `input.value` (live vs attribute), `input.checked`, `input.type`, `input.disabled`, `input.name` — separate property storage from attributes |
| B-4B | Select/Option | `select.value`, `select.selectedIndex`, `select.options`, `option.value`, `option.selected`, `option.text` |
| B-4C | Anchor/Form/Dataset | `a.href`, `form.submit()`, `form.action`, `form.method`, `element.hidden`, `element.dataset` (Proxy over data-* attributes) |
| B-4D | Common HTMLElement | `tabIndex`, `title`, `lang`, `dir`, `getBoundingClientRect()` (stub: returns zeros), `focus()`, `blur()`, `click()` (stub) |

**Wave B-5: Integration (1 agent)**

| Agent | What |
|-------|------|
| B-5A | Integration + smoke tests — React-like reconciler simulation, Svelte-like direct DOM manipulation, event round-trips, full API surface exercise |

---

### Direction C: CSS Cascade (14 agents, 6 waves, max 3 concurrent)

Full CSS cascade — the jsdom killer feature. Built on Servo's `cssparser` + `selectors`.

**Wave C-0: Foundation Types (3 agents, all parallel)**

| Agent | What | Files |
|-------|------|-------|
| C-0A | Property system — `PropertyId` enum (~50 properties), `CssValue` types (Length, Color, Keyword, etc.), inheritance flags, initial values, shorthand expansion | `css/properties.rs`, `css/values.rs` |
| C-0B | SelectorImpl — `BrailleSelectorImpl` for the `selectors` crate, pseudo-class/pseudo-element enums, `BrailleSelectorParser` | `css/selector_impl.rs` |
| C-0C | DOM modifications — `computed_style` slot on Node, traversal helpers for selector matching: `parent_element`, `prev_sibling_element`, `next_sibling_element`, `is_root_element` | `dom/node.rs`, `dom/tree.rs` |

**Wave C-1: Parsing + Matching (3 agents, all parallel)**

| Agent | What | Depends |
|-------|------|---------|
| C-1A | CSS parser — `parse_stylesheet(css) → Stylesheet`, `parse_inline_style(attr) → Vec<Declaration>`, implements cssparser's `DeclarationParser` + `QualifiedRuleParser` | C-0A, C-0B |
| C-1B | Element trait — `selectors::Element` impl on `DomElement<'a>` wrapper, `query_selector`/`query_selector_all` functions (**shared with B-2A**) | C-0B, C-0C |
| C-1C | Stylesheet collection — walk DOM for `<style>` + inline `style=""`, hardcoded UA stylesheet (display:block for div/p/h1-h6, display:none for head/script/style, etc.) | C-0A, C-1A |

**Wave C-2: Cascade (2 agents, all parallel)**

| Agent | What | Depends |
|-------|------|---------|
| C-2A | Cascade algorithm — collect matching rules per element, sort by (origin, importance, specificity, source order), produce `CascadedValues` | C-1A, C-1B, C-1C |
| C-2B | Computed style resolution — `resolve_style(cascaded, parent_style) → ComputedStyle`, handle `inherit`/`initial`/`unset`, em→px, percentage resolution, color name lookup | C-0A, C-2A |

**Wave C-3: Integration (3 agents, all parallel)**

| Agent | What | Depends |
|-------|------|---------|
| C-3A | Style tree orchestration — DFS walk computing styles top-down (parent before child for inheritance), `compute_all_styles(tree, collection)` | C-2A, C-2B |
| C-3B | A11y integration — `display:none` → skip element+descendants, `visibility:hidden` → keep structure but hide text | C-3A |
| C-3C | Engine wiring — call `collect_styles` + `compute_all_styles` in `Engine::load_html` after script execution | C-3A, C-3B |

**Wave C-4: JS Bindings (2 agents, all parallel)**

| Agent | What |
|-------|------|
| C-4A | `getComputedStyle(el)` — returns read-only `JsCSSStyleDeclaration`, `getPropertyValue(name)`, camelCase property getters |
| C-4B | `element.style` — mutable `JsCSSStyleDeclaration`, `setProperty`/`removeProperty`, camelCase setters, triggers restyle |

**Wave C-5: Edge Cases (1 agent)**

| Agent | What |
|-------|------|
| C-5A | Integration tests — full cascade with UA+author+inline+!important, inheritance chains, display:none in a11y, getComputedStyle after JS mutation, specificity edge cases |

---

### Execution Strategy

Run all three directions concurrently where dependencies allow. Recommended interleaving:

| Phase | What runs | Agents | Status |
|-------|-----------|--------|--------|
| 1 | A Wave 0 + B Wave 0 + C Wave 0 | 9 parallel | DONE (319 tests) |
| 2 | A Wave 1 + B Wave 1 + C Wave 1 | 14 parallel (peak) | DONE (319 tests) |
| 3 | A Wave 2 + B Wave 2 + C Wave 2 | 11 parallel | DONE (539 tests) |
| 4 | A Wave 3 + B Wave 3 + C Wave 3 | 9 parallel | DONE (650 tests) |
| 5 | B Wave 4 + C Wave 4 | 5 agents (C-4B skipped — covered by B-2D) | DONE (718 tests) |
| 6 | B Wave 5 + C Wave 5 | 2 parallel | DONE (770 tests) |

**Total: 51 agent tasks. Peak concurrency: 14 agents. 6 phases. ALL PHASES COMPLETE. 770 tests passing.**

## Core Thesis

LLMs don't need pixels — they need text. The DOM is already a text structure. The graphical rendering pipeline (layout, paint, compositing, GPU) is the expensive part of a browser, and agents don't need any of it. An LLM can look at raw DOM with inline styles and understand the visual intent: `display: none` means hidden, `position: fixed; top: 0` means sticky header.

## Requirements

- Full JavaScript execution against a live DOM
- Agents can navigate, click links, fill forms, follow redirects
- Handles modern SPA frameworks (React, SvelteKit at minimum)
- CSS changes from JS are reflected in the DOM text output
- Lightweight — the whole point is avoiding headless Chrome overhead
- Structured text output designed for LLM consumption
- WASM sandboxing — untrusted page JS executes inside a sandboxed module
- Distributed as a single CLI binary agents can `brew install`

## Stack

### Language: Rust
- Entire engine written in Rust — DOM, CSS cascade, layout, all in one codebase
- Compiles to native binary for CLI distribution
- The engine compiles to WASM for sandboxed execution of untrusted page JS
- No FFI boundaries within the engine — DOM and JS engine are both Rust, same memory model
- Single binary output, no runtime dependencies

### JS Engine: Boa
- Pure Rust JavaScript engine — no C dependencies, no language bridging
- 94.12% Test262 conformance (v0.21), actively improving
- Embeds directly into the Rust codebase — DOM calls from page JS are just Rust function calls, no FFI overhead
- Compiles to WASM natively alongside the DOM implementation
- Risk: younger than V8/JSC/QuickJS, may hit spec gaps on real-world sites. Mitigated by active development and our ability to contribute upstream.

### Servo Crates (dependencies)

| Crate | Purpose | License |
|-------|---------|---------|
| **html5ever** | HTML parsing, spec-compliant, passes html5lib-tests | MIT/Apache-2.0 |
| **cssparser** | CSS tokenization and parsing (CSS Syntax Level 3) | MPL-2.0 |
| **selectors** | CSS selector parsing and matching against DOM elements | MPL-2.0 |
| **taffy** | Layout computation — CSS Block, Flexbox, and Grid | MIT/Apache-2.0 |

### Custom Implementation (what we build)
- **DOM** — Rust implementation from scratch, spec-compliant, validated against WPT
- **CSS Cascade Engine** — specificity, cascade ordering, inheritance, computed values. Built on top of `cssparser` + `selectors`.
- **`getComputedStyle()`** — full spec compliance. React and other frameworks depend on this.
- **Accessibility tree generator** — transforms live DOM into compact text representation for agents
- **Session manager** — stateful sessions with cookies, history, navigation
- **CLI interface** — the `braille` command
- **Event loop** — frozen time model with pump-to-settled semantics

### Project Structure

Cargo workspace with 3 crates, separated by the WASM compilation boundary:

```
braille/
├── Cargo.toml                  # workspace root
├── PLAN.md
├── crates/
│   ├── engine/                 # compiles to WASM — the sandboxed browser
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── dom/            # Node, Element, Document, Event, etc.
│   │       ├── css/            # cascade, specificity, computed styles
│   │       ├── layout/         # Taffy integration
│   │       ├── html/           # html5ever integration
│   │       ├── js/             # Boa integration, Web API bindings
│   │       ├── a11y/           # accessibility tree generation
│   │       └── event_loop/     # frozen time pump-to-settled loop
│   │
│   ├── wire/                   # shared types — the WASM boundary contract
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── lib.rs          # commands, responses, snapshot formats, element refs
│   │
│   └── cli/                    # native binary — the host
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs
│           ├── session.rs      # session lifecycle, ID management
│           └── network.rs      # reqwest, cookie jar, fetch proxying
│
└── tests/
    ├── wpt/                    # WPT test harness
    ├── html5lib/               # html5lib-tests harness
    └── fixtures/               # reference HTML pages for integration tests
```

**`engine`** — Everything that runs inside the WASM sandbox. Zero I/O, zero networking, pure computation. Takes parsed HTML + fetched resources as input, produces snapshots as output. Hard rule: if it touches the outside world, it doesn't go here.

**`wire`** — The protocol between engine and CLI. Command types (`Goto`, `Click`, `Type`), response types (`Snapshot`, `Error`), element reference formats (`@button3`), snapshot mode enums. Both `engine` and `cli` depend on this. Forces the WASM boundary to be explicit and well-defined.

**`cli`** — The native host. Loads the engine, provides networking on behalf of the sandbox (engine requests a URL → CLI fetches it → passes the response back in), manages sessions, parses CLI args, formats output.

## Form Factor: CLI (`braille`)

Single native binary, distributed via `brew install`, `cargo install`, or direct download.
No SDK to import, no server to start, no runtime dependencies.
Cross-platform: macOS, Linux, Windows.

### Session-based interface
```
braille new                              # → returns session ID
braille <sid> goto <url>                 # navigate, returns snapshot
braille <sid> click <selector>           # click element, returns snapshot
braille <sid> type <selector> "text"     # fill input, returns snapshot
braille <sid> select <selector> "value"  # select dropdown, returns snapshot
braille <sid> focus <selector>           # focus element
braille <sid> snap                       # get current snapshot
braille <sid> snap --mode=markdown       # snapshot in specific mode
braille <sid> back                       # go back
braille <sid> forward                    # go forward
braille <sid> close                      # end session
```

Every mutation command returns a snapshot automatically.
Verbs mirror what a human does in a browser.

## Agent Interface

Agent chooses output mode based on use case:
- **Accessibility tree (default)** — compact, semantic, ~200-400 tokens. Interactive elements get stable references (e.g. `@button3`, `@input7`)
- **Simplified DOM** — strips noise (scripts, hidden elements, SVG internals, empty wrappers), keeps meaningful structure with interactive markers
- **Raw DOM** — full HTML serialization for debugging or when the agent needs everything
- **Markdown** — readable content extraction for articles, docs, etc.

Agent interacts via element references: `click @button3`, `type @input7 "search query"`, `select @dropdown2 "option1"`

Must support: clicking links/buttons, filling form inputs, selecting dropdowns, submitting forms, scrolling.

## CSS: Full Spec Compliance

- Full CSS cascade: parsing, selector matching, specificity, inheritance, computed values
- `getComputedStyle()` must work correctly
- Built on Servo's `cssparser` (parsing) and `selectors` (matching) crates
- Cascade algorithm, inheritance, initial values, shorthand expansion — custom implementation on top of those crates
- This is a significant differentiator — jsdom never solved this (open PR from 2019, never merged)
- WPT has CSS tests to validate against

## Layout: Taffy

- **Taffy** provides real CSS Block, Flexbox, and Grid layout computation
- Input: tree of nodes with CSS Style structs → Output: Layout structs with position (x, y) and size (width, height)
- This gives us correct `getBoundingClientRect()`, `offsetWidth`, `offsetHeight` values
- Viewport is configurable, defaults to 1280x720, agent can change it
- Text measurement approximated (char count × avg char width × font size) — no font renderer
- Skips: subpixel rendering, paint order, z-index stacking contexts

## Navigation: Stateful Sessions

- Browser interaction is through stateful sessions. Agent gets a session ID, issues commands against it.
- Session owns: DOM, cookies, history, navigation context
- Full page navigation: teardown current DOM, fetch new page, parse and rebuild
- SPA routing: `pushState`/`replaceState`/`popstate` handled naturally by executing the page's JS
- Back/forward history: supported
- Iframes: supported — each iframe gets its own document within the parent session

## Network

- **Rust HTTP client** (reqwest or similar) for HTTP requests
- **Cookie jar per session** — persists across navigations, attaches correct cookies to outgoing requests
- **CORS: skip by default** — no user to protect, agent wants cross-origin access. Optionally enforceable.
- **Service workers: supported** — needed for sites that rely on them for request interception/routing

## Security: WASM Sandboxing

- Page JS executes inside a WASM sandbox — even memory exploits can't escape
- The entire engine (Rust DOM + Boa JS) compiles to a WASM module
- The host (native CLI binary) only provides controlled capabilities: network access, session management
- No file system access, no process spawning from within the sandbox

## Execution Model: Frozen Time

- Time freezes between agent commands. No JS runs until the agent acts.
- On each command (goto, click, type, etc.):
  1. Execute the action
  2. Pump the event loop — process microtasks, fire ready timers, handle network callbacks
  3. Keep pumping until "settled" (no pending microtasks, no ready timers, network quiet)
  4. Freeze. Return snapshot.
- `setTimeout(fn, 5000)` doesn't fire on wall clock — it fires when the event loop is pumped past that point
- Every mutation command returns a snapshot automatically. `snap` is for looking without acting.
- More deterministic than a real browser — no race conditions, agent always sees consistent state

### Needs deeper design work:
- **Web Workers** — separate threads in real browsers. Same event loop? Separate WASM instances? Freeze between commands?
- **Async/await and Promises** — microtask queue should pump naturally during "settle", but edge cases need verification
- **Streams** — ReadableStream, WritableStream, fetch body streams. Behavior when time is frozen?
- **WebSockets** — persistent connections. Messages queue up and deliver on next pump?
- **requestAnimationFrame** — no screen to paint, but frameworks use it for scheduling. Treat as a timer?

## Compliance Testing

- **WPT (Web Platform Tests)** — 56,000+ test files, BSD licensed, the canonical browser conformance suite
  - jsdom's `to-run.yaml` provides a curated roadmap of which tests are feasible for non-browser DOM implementations
  - Start with `dom/nodes/`, `dom/events/`, `html/dom/`, `css/selectors/`
- **html5lib-tests** — 9,200+ HTML parser test cases, MIT licensed
  - Validates that HTML parsing produces correct DOM trees
- **Test262** — Boa already runs this; monitor their conformance progress

## Licensing

- Braille: MIT or Apache-2.0
- Boa: MIT — compatible
- Servo crates: MPL-2.0 (cssparser, selectors) and MIT/Apache-2.0 (html5ever, taffy) — all compatible
- AGPL is a dealbreaker (ruled out Lightpanda)

## Landscape: What Exists and Why It's Not Enough

### Agent browser tools (full Chrome underneath)
- **agent-browser** (Vercel Labs) — great text output via accessibility trees, but Playwright/headless Chrome under the hood
- **browser-use** — LLM-driven browser automation, still Playwright
- **Stagehand** (Browserbase) — same, real Chromium
- **Playwright MCP** (Microsoft) — accessibility tree for LLMs, still a full browser

These solve the output format problem but not the rendering overhead problem.

### Virtual DOMs (no real JS execution)
- **jsdom** — most complete DOM, but weak in-page JS execution, slow
- **happy-dom** — faster, but had an RCE vulnerability (CVE-2025-61927), less spec-compliant
- **linkedom** — minimal, designed for SSR not browser emulation

These are DOM parsers, not browsers.

### Lightpanda
- Closest to our vision architecturally — Zig-based, V8, real DOM, no rendering
- But: AGPL licensed (dealbreaker), beta quality, small team, Zig pre-1.0
- Missing many Web APIs, SPAs don't work reliably

### Content extraction tools (Firecrawl, Jina Reader, Crawl4AI)
- One-shot extraction, not interactive browsers
- Still use full browsers internally

## Open Items

### Framework Acceptance Criteria
- React's reconciler touches a LOT of DOM APIs — need to enumerate which ones
- SvelteKit compiles to direct DOM manipulation — different API surface
- Need to identify the minimum DOM API surface that makes these frameworks functional
- **TODO (later):** Identify real-world reference sites/apps to use as test targets:
  - Official/canonical framework examples
  - Real production SPAs
  - Minimal reproduction apps for specific DOM API surfaces (forms, routing, dynamic content)
