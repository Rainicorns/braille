# Braille — Text Browser for LLM Agents

## What This Is

A lightweight browser engine in Rust that outputs text instead of pixels. Lets AI agents browse the web — run JavaScript, click buttons, fill forms, read content — at millisecond speed with no Chromium dependency.

## Architecture

- **Workspace:** 4 crates — `engine` (core), `cli` (command line), `wire` (session protocol), `spike-quickjs` (experimental)
- **DOM:** Arena-based (`NodeId` = `usize` into `Vec`), parsed by html5ever
- **JS:** QuickJS via rquickjs. Bindings in `js/dom_bridge.rs` (main) and `js/bindings/` (per-API)
- **CSS:** cssparser + selectors crate
- **Output:** Accessibility tree serializer in `a11y/serialize.rs`
- **Tests:** 600+ tests across lib, integration, WPT, framework suites

## Testing Philosophy — READ THIS

**We practice TDD. Red tests are good. Green-washing is the enemy.**

When writing tests for missing features or real-world site compatibility:

1. **Write tests that use the REAL code patterns from the target site.** Do not rewrite the site's JS as inline `<script>` to dodge missing engine features. If a site uses `<script type="module">`, the test must use `<script type="module">`. If it uses Web Workers, the test must use Web Workers.

2. **Expect tests to FAIL.** A failing test is a roadmap item. It tells us exactly what Braille can't do yet. A test that passes because you rewrote the code to avoid the gap tells us nothing.

3. **Never put workarounds in the test harness.** The test should exercise Braille the way a real user would use it. If the CLI is supposed to handle meta refresh automatically, test via the CLI path — don't manually parse HTML and construct redirect URLs in the test.

4. **Each red test = one concrete gap to fix.** When you fix the engine and the test goes green, that's real progress. When you fudge the test to make it green, that's technical debt disguised as progress.

5. **Truth is always better than green.** If 13/16 tests pass, say so. Don't hide the 3 failures. Those failures ARE the backlog.

### Example of what NOT to do

```rust
// BAD: Rewrites Anubis's module script as inline to dodge ES module gaps
let html = r#"<script>
    // "simulates" what the module does...
    var info = JSON.parse(document.getElementById('preact_info').textContent);
    crypto.subtle.digest('SHA-256', ...).then(...)
</script>"#;
```

```rust
// GOOD: Uses the actual pattern — fails honestly if modules don't work
let html = r#"<script type="module">
    import { Sha256 } from './sha256.js';
    // ... actual code pattern
</script>"#;
```

## Build & Test

```bash
cargo build --workspace
cargo test --workspace              # all tests
cargo test -p braille-engine --lib  # engine unit tests only
cargo test -p braille-engine --test anubis_challenges  # Anubis TDD suite
./dev.sh check                      # clippy, zero warnings required
./dev.sh test                       # full suite
```

## Code Style

- Zero clippy warnings. Workspace lints enforce `warnings = "deny"`.
- No try/catch in JS bindings — let errors explode with full stacktrace.
- No swallowing errors in Rust — fail fast, propagate up.
- Follow existing patterns: look at how similar features are implemented before adding new ones.
- Don't over-engineer. Minimum viable fix, move on.
- Don't add comments/docstrings to code you didn't change.

## Key Files

| What | Where |
|------|-------|
| Engine entry, public API | `crates/engine/src/lib.rs` |
| HTML parser (TreeSink) | `crates/engine/src/html/parser.rs` |
| DOM tree | `crates/engine/src/dom/tree.rs` |
| JS runtime wrapper | `crates/engine/src/js/runtime.rs` |
| Main JS DOM bridge | `crates/engine/src/js/dom_bridge.rs` |
| Per-API bindings | `crates/engine/src/js/bindings/*.rs` |
| Accessibility serializer | `crates/engine/src/a11y/serialize.rs` |
| Commands (click/type/select) | `crates/engine/src/commands/*.rs` |
| CLI entry | `crates/cli/src/main.rs` |
| Wire protocol | `crates/wire/src/lib.rs` |
| Architecture + status | `PLAN.md` |
| Anubis challenge tests (TDD) | `crates/engine/tests/anubis_challenges.rs` |

## Current TDD Backlog (from anubis_challenges.rs)

These tests are RED — each one is a real gap:

1. **`preact_full_solver_flow`** — ~~URL.searchParams.set()~~ Fixed. May still fail for other reasons — verify.
2. **`pow_web_worker_basic_functionality`** — Web Worker constructor exists but doesn't execute code
3. **`anubis_dependency_secure_context`** — ~~window.isSecureContext undefined~~ Fixed (set to false).

## Background Agents & Worktrees

When launching background agents that work in git worktrees:

1. **Always instruct agents to commit their changes.** Worktrees are temporary — uncommitted work is lost when the worktree is cleaned up. Every agent prompt must include "commit your changes with a descriptive message."

2. **Verify commits exist before cleaning up worktrees.** Run `git log worktree-agent-XXXX -1` to confirm the branch has commits ahead of main. If the branch is at the same commit as main, the agent's work was lost.

3. **Merge promptly.** Don't let worktrees accumulate — merge and clean up as soon as agents finish. More unmerged branches = more conflict risk.

4. **Check agent results skeptically.** An agent reporting "all tests pass" doesn't mean the work was committed. Verify by checking the branch's commit history.

This was learned the hard way: two agents reported success (ES modules, cheating test cleanup) but their worktree branches had no commits. All work was lost on cleanup.

## Workflow

- Don't commit unless explicitly asked.
- Don't run DB migrations — ask the user.
- After finishing a task, wait for verification before moving on.
- Use `tsx` not `ts-node` for TypeScript execution.
- No background colors without explicit instructions.
- No code in PLAN documents.
