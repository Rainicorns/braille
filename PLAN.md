# Braille

A lightweight browser that maintains a virtual DOM with full JavaScript execution but skips graphical rendering entirely. Outputs structured text representations of pages for LLM agents to read and interact with.

A browser for those who read, not see.

**Spike:** [SPIKE.md](./SPIKE.md) — COMPLETE (46 tests, core loop proven)
**WPT Status:** [WPT_STATUS.md](./WPT_STATUS.md) — per-file test status, skip reasons

## Current Status (2026-03-27)

### What's Working
- **600+ tests passing** across lib, integration, WPT, framework suites
- **Wikipedia** loads and renders correctly (homepage, articles, search flow)
- **Codeberg/GitHub** loads fully (JS-heavy sites with Gitea/GitHub frontend)
- **ES modules** — custom `ModuleRegistry` with `BrailleResolver` + `BrailleLoader`. Import maps resolve bare specifiers. import/export, dynamic import(), top-level await, re-exports all working.
- **Form features complete** — input/change/invalid events, requestSubmit(), constraint validation (stepMismatch, badInput, all input types), label association, document.forms, live HTMLCollections, select.options/selectedOptions, textarea properties, form element properties (enctype, noValidate, target, etc.)
- **React integration fixed** — `__reactProps$` hack removed, proper capture-phase event delegation, property/attribute separation (spec-compliant)
- **Meta refresh** — engine detects `<meta http-equiv="refresh">`, CLI follows redirects automatically
- **Anubis challenge support** — metarefresh challenges work end-to-end. Preact challenges partially work (ES modules execute, SHA-256 works, URL.searchParams.set() fixed). PoW challenges blocked on Web Workers.
- **Container persistence foundation** — Dockerfile (musl static build), session storage module, Close command bug fixed

### TDD Backlog (red tests = real gaps)

From `crates/engine/tests/anubis_challenges.rs`:
1. **`preact_full_solver_flow`** — URL.searchParams.set() fixed, may need re-verify
2. **`pow_web_worker_basic_functionality`** — Worker constructor exists but doesn't execute code
3. **`anubis_dependency_secure_context`** — Fixed (window.isSecureContext = false)

### Architectural Decisions

#### Web Workers: Process-Based with Host Delegation (DECIDED)

Workers will be implemented as **separate OS processes**, following the same host-delegation pattern as fetch:

1. JS calls `new Worker(url)` → engine returns `NeedWorkers` message to CLI
2. CLI spawns lightweight QuickJS child processes
3. `worker.postMessage(data)` → CLI serializes and pipes to child stdin
4. Child computes, writes results to stdout
5. CLI pipes results back → engine fires `worker.onmessage` callback

**Why processes over threads:**
- CRIU-safe — each process checkpoints independently, no multi-thread complexity
- No Arc/Mutex refactoring — engine internals stay single-threaded
- Workers are isolated by OS — exactly what the Web Workers spec says
- Fits existing architecture (engine is already a child process of CLI)
- Workers don't access DOM (per spec), so no shared state needed

**settle() interaction:** Workers live **outside** the settle loop. settle() stays pure and deterministic (microtasks, observers, virtual timers). Workers are host-delegated — same as fetch. The engine never blocks on worker computation.

**Future optimization:** Native Rust crypto offload for known compute patterns (SHA-256 brute force) to solve PoW challenges at native speed instead of QuickJS speed.

#### document.cookie: Redo After Refactor (PENDING)

Cookie sync between JS and HTTP cookie jar was implemented but couldn't merge cleanly due to lib.rs size. Implementation spec (from completed agent work):
- `StoredCookie` struct in engine
- `engine.inject_response_cookies(url, headers)` — parses Set-Cookie, stores in Rust jar, syncs non-HttpOnly to JS
- `engine.get_cookies_for_url(url)` — returns all matching cookies for outgoing requests
- HttpOnly cookies hidden from JS but available for HTTP
- 7 TDD tests covering read/write, HTTP sync, HttpOnly, deletion
- Redo after lib.rs refactor lands (will go in its own `cookies.rs` module)

#### Session Persistence: Container Checkpoint/Restore (IN PROGRESS)

- Engine binary is a complete stdin/stdout REPL (done)
- CLI spawns engine processes, handles fetch delegation (done)
- Daemon manages sessions via Unix socket (done)
- Dockerfile for musl static build (done)
- Session storage module with metadata.json (done)
- **Not started:** CRIU checkpoint/restore integration, podman commands, cross-invocation persistence
- Path to ship: ~4-6 weeks (Phases C1-C3 below)

### Priority Roadmap

1. **lib.rs / dom_bridge.rs refactor** — in progress, unblocks everything else
2. **document.cookie redo** — redo in new cookies.rs after refactor
3. **Web Workers (process-based)** — new wire protocol messages, CLI worker management
4. **innerText vs textContent** — should exclude display:none content
5. **Real-site testing grind** — Amazon, Gmail, Salesforce, etc.
6. **Session persistence (CRIU)** — container checkpoint/restore for production

## Completed Work Summary

### Engine Core
- Arena-based DOM (NodeId = usize into Vec), parsed by html5ever (100% html5lib tree-construction)
- QuickJS JS runtime via rquickjs with custom ES module loader
- CSS cascade with selector matching via cssparser + selectors crate
- Full event system: capture/bubble/at-target, standalone EventTarget, W3C dispatch
- Accessibility tree serializer with 11 snapshot modes
- Virtual clock settle loop: microtasks → MutationObserver → timers → quiescent
- **3.5ms per page** (SPA with 30 product cards, criterion benchmark)

### Web APIs Implemented
- DOM: querySelector/All, getElementById, getElementsByTagName/ClassName (live), innerHTML, outerHTML, classList, dataset, element.style, cloneNode, importNode, adoptNode
- Events: addEventListener (once, passive, signal), dispatchEvent, Event/CustomEvent/MouseEvent/KeyboardEvent/FocusEvent/WheelEvent/CompositionEvent constructors, inline event handlers
- Forms: form.submit/reset/elements/requestSubmit, input.value/checked/type/disabled, select.value/selectedIndex/options/selectedOptions, textarea properties, constraint validation (all types), label association
- Navigation: History pushState/replaceState/popstate, back/forward
- Network: fetch with Promise, XMLHttpRequest stub, FormData
- Storage: localStorage/sessionStorage
- Timers: setTimeout/setInterval/clearTimeout/clearInterval, requestAnimationFrame
- URL: URL constructor with searchParams (get/set/has/delete/append), URLSearchParams
- Crypto: crypto.subtle.digest (SHA-256), crypto.getRandomValues
- Encoding: TextEncoder/TextDecoder (UTF-8)
- Observers: MutationObserver (full), IntersectionObserver/ResizeObserver (stubs)
- DOM Traversal: TreeWalker, NodeIterator, Range (21 methods), StaticRange
- Shadow DOM: attachShadow, shadowRoot, composed event dispatch
- Custom Elements: customElements.define/get/whenDefined, lifecycle callbacks
- Other: structuredClone, matchMedia, DOMParser, AbortController/AbortSignal, MessageChannel, document.currentScript, window.isSecureContext

### Test Results

| Test suite | Result |
|---|---|
| `cargo test -p braille-engine --lib` | **482 passed** |
| `--test html5lib_tree_construction` | **1778 passed (100%)** |
| `--test html5lib_serializer` | **204 passed** |
| `--test frameworks` | **31 passed** |
| `--test react_controlled_input` | **9 passed** |
| `--test smoke_integration` | **20+ passed** |
| `--test adversarial` | **32 passed** |
| `--test dom_bridge` | **100+ passed** |
| `--test snapshot_views` | **16 passed** |
| `--test es_modules` | **10 passed** |
| `--test anubis_challenges` | **13 passed, 3 failing (TDD backlog)** |
| `--test wpt_dom` | ~279/353 (74 skipped, see [WPT_STATUS.md](./WPT_STATUS.md)) |
| `./dev.sh check` (clippy) | **0 warnings** |

### Session Architecture (Complete)

Engine binary is a stdin/stdout REPL communicating via JSON-line protocol. Wire protocol: `HostMessage` (Command | FetchResults) and `EngineMessage` (NeedFetch | CommandResult). CLI daemon spawns one engine process per session. Fetch delegation: engine has zero network access, CLI does all HTTP. Sessions persist across CLI invocations while daemon is running.

### Completed Phases
- Phase S1-S3: fetch/History/FormData, URL/localStorage, ES modules
- Phase S4: Real-site gap fixes (iterative tree walking, script type filtering, custom elements)
- Phase S5: Persistent sessions (daemon with Unix socket IPC)
- Phase S6: Container-based session architecture (engine binary, process-per-session)
- Phase R1-R2: DOM bridge completeness for React SPA rendering
- Phases 1-24b: WPT DOM conformance (see [WPT_STATUS.md](./WPT_STATUS.md))

## Container Persistence Roadmap

### Phase C1: Container Binary (1-2 weeks)
- [ ] Add `--target x86_64-unknown-linux-musl` to build config
- [ ] Produce static binary ~15-20MB
- [ ] Test: `podman build`, `podman run`, verify binary executes and reads stdin

### Phase C2: Container Lifecycle (2-3 weeks)
- [ ] `podman checkpoint <container-id>` on session pause
- [ ] `podman restore <checkpoint-path>` on session resume
- [ ] Session storage at `~/.braille/sessions/<session-id>/`
- [ ] CLI: restore-before-command, checkpoint-after-command

### Phase C3: Cross-Invocation Persistence (1 week)
- [ ] Remove daemon dependency (make optional with `--daemon` flag)
- [ ] Each invocation: detect checkpoint → restore → command → checkpoint
- [ ] Garbage collection: delete old checkpoints

## Core Thesis

LLMs don't need pixels — they need text. The DOM is already a text structure. The graphical rendering pipeline (layout, paint, compositing, GPU) is the expensive part of a browser, and agents don't need any of it.

## Requirements

- Full JavaScript execution against a live DOM
- Agents can navigate, click links, fill forms, follow redirects
- Handles modern SPA frameworks (React, Vue, Svelte, Preact)
- CSS changes from JS are reflected in the DOM text output
- Lightweight — avoiding headless Chrome overhead
- Structured text output designed for LLM consumption
- Container sandboxing — untrusted page JS executes inside a container with zero network access
- Distributed as a single CLI binary

## Stack

- **Language:** Rust (compiles to native binary, no runtime dependencies)
- **JS Engine:** QuickJS via rquickjs (was Boa, switched for QuickJS's better ES2020 compliance and checkpointability)
- **HTML Parser:** html5ever (Servo project, 100% spec compliant)
- **CSS:** cssparser + selectors (Servo project)
- **HTTP:** reqwest (CLI-side only, engine has zero network access)
- **Serialization:** serde + serde_json (wire protocol)

## Execution Model: Frozen Time

Time freezes between agent commands. No JS runs until the agent acts.

On each command (goto, click, type, etc.):
1. Execute the action
2. Pump the event loop — process microtasks, fire ready timers, handle callbacks
3. Keep pumping until "settled" (no pending microtasks, no ready timers)
4. Freeze. Return snapshot.

`setTimeout(fn, 5000)` doesn't wait 5 real seconds — it fires when the virtual clock advances past that point during settle. Deterministic, fast, no race conditions.

## Landscape: What Exists and Why It's Not Enough

### Agent browser tools (full Chrome underneath)
- **agent-browser** (Vercel Labs), **browser-use**, **Stagehand** (Browserbase), **Playwright MCP** (Microsoft)
- Great output format, but Playwright/headless Chrome under the hood. Solve the output problem, not the weight problem.

### Virtual DOMs (no real JS execution)
- **jsdom** — most complete DOM, weak JS execution, slow
- **happy-dom** — faster, less spec-compliant, had RCE vulnerability
- **linkedom** — minimal, designed for SSR not browsing

### Lightpanda
- Closest architecturally — Zig-based, V8, real DOM, no rendering
- AGPL licensed (dealbreaker), beta quality, Zig pre-1.0

### Content extraction tools (Firecrawl, Jina Reader, Crawl4AI)
- One-shot extraction, not interactive. Still use full browsers internally.
