# Braille — Development Guidelines

## Testing Philosophy — READ THIS

**We practice TDD. Red tests are good. Green-washing is the enemy.**

1. **Write tests that use REAL code patterns from the target site.** If a site uses `<script type="module">`, the test must use `<script type="module">`. If it uses Web Workers, the test must use Web Workers. Never rewrite site code to dodge missing engine features.

2. **Expect tests to FAIL.** A failing test is a roadmap item. A test that passes because you rewrote the code to avoid the gap tells us nothing.

3. **Never put workarounds in the test harness.** Test Braille the way a real user would use it.

4. **Each red test = one concrete gap to fix.** When you fix the engine and the test goes green, that's real progress. When you fudge the test, that's technical debt disguised as progress.

5. **Truth is always better than green.** If 13/16 tests pass, say so. Those 3 failures ARE the backlog.

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

### No tolerance / expected_failures

Never use `expected_failures` counts, tolerance thresholds, or any mechanism that lets a partially-failing test report as green. If 51/80 subtests pass, that's a FAIL with 29 subtests to fix. The moment you hide failures behind a count, you lose track of what's actually broken.

### Prefer strong foundations over green tests

More regressions reporting the true state of things on a correct foundation is BETTER than passing tests on a shaky one. If a proper refactor causes 50 tests to go red because the old tests relied on wrong behavior, that's progress — those reds are now the honest backlog. Never optimize for short-term green. Always optimize for correctness and architectural strength. A strong foundation lets you build upward indefinitely. A weak one collapses under its own weight.

### No shortcuts in architecture

When you encounter a structural problem (e.g., "all node types share one prototype but only elements should have `tagName`"), do the proper refactor. Don't:
- Add `if (this.__nid === undefined) return` guards as a permanent fix — that's a band-aid
- Create standalone objects with `Object.defineProperty` overrides to shadow broken inherited getters — that's another band-aid on top
- Say "this is a bigger effort, let me skip it" — you'll just hit the same wall in every subsequent test

Every shortcut creates more work later. The only question is how many crappy implementations you go through before converging at the correct one. Do the correct thing the first time.

### White-box test development

When testing against an external system (e.g., Anubis), read their source code and build test cases from the actual code paths. Don't hit the live site — it's designed to be random to prevent grinding. Develop deterministic tests from their source.

## Code Structure

- **New tests go in `crates/engine/tests/`**, not inline in source files and never in `/tmp`. Use the public API (`eval_js`, `handle_click`, `handle_type`, `snapshot`, etc.) from external test files.
- **Don't grow big files.** `lib.rs` and `dom_bridge.rs` are already too large. New Engine functionality goes in its own module. New JS bindings go in `js/bindings/` (one file per API surface).

## Code Style

- Zero clippy warnings. Workspace lints enforce `warnings = "deny"`.
- No try/catch in JS bindings — let errors explode with full stacktrace.
- No swallowing errors in Rust — fail fast, propagate up.
- Follow existing patterns: look at how similar features are implemented before adding new ones.
- Don't over-engineer. Minimum viable fix, move on.
- Don't add comments/docstrings to code you didn't change.

## Build & Test

```bash
cargo build --workspace
cargo test --workspace              # all tests
cargo test -p braille-engine --lib  # engine unit tests only
cargo clippy --workspace            # zero warnings required
```

## The Ratchet — High Water Mark Workflow

All tests (920 cargo + 1403 WPT = 2323 total) are tracked in `tests/manifest.txt` with status PASS, FAIL, or NOT_RUN. The test runner grinds through them in order, stopping at the first failure.

```bash
cargo run -p test-runner              # run from high water mark until failure
cargo run -p test-runner -- --regression  # re-check all PASS tests
cargo run -p test-runner -- --discover    # sync manifest with filesystem
```

**"Grind the high water mark"** means: run the runner, read the failing test, analyze what real web platform capability is missing, implement the proper fix (not a point hack), run again, watch the high water mark go up. Repeat.

The loop:
1. `cargo run -p test-runner` — find the edge (first failing test)
2. Read the test source. Understand what it's actually testing — not just the error, but the web platform pattern.
3. Think deeply about what's missing. What capability does the engine lack? What other tests and real sites would benefit from adding it properly?
4. Implement the real fix. No band-aids.
5. Run the runner again. If the high water mark went up, commit the updated manifest.

**Always use `cargo run -p test-runner` (the ratchet) to verify changes. Always run it in the background (`run_in_background: true`) so the user can see output streaming in real time.** Do NOT use `cargo test --workspace` as verification — the ratchet is the real scoreboard. Do NOT run `--regression` unless explicitly asked.

**Never run more than one ratchet at a time.** Before starting a ratchet, always check that no other ratchet is currently running. Wait for completion before starting a new one.

**Never create alt tests without explicit user permission.** When a test fails, the first instinct must be to polyfill or implement the missing feature — not to reach for an alt test. Only the user decides when a test should be alted. Ask and get explicit approval before creating any alt test.

The manifest is committed to the repo. The high water mark is the permanent scoreboard.

**Never read, edit, or peek at the manifest directly.** The manifest is the ratchet's data — only `cargo run -p test-runner` interacts with it. Don't `cat`, `grep`, `read`, or `sed` the manifest file. Let the ratchet tell you what failed.

## Background Agents & Worktrees

1. **Always instruct agents to commit their changes.** Worktrees are temporary — uncommitted work is lost on cleanup. Every agent prompt must include "commit your changes."

2. **Verify commits exist before cleaning up worktrees.** Run `git log worktree-agent-XXXX -1` to confirm commits exist.

3. **Merge promptly.** Don't let worktrees accumulate. More unmerged branches = more conflict risk.

4. **Check agent results skeptically.** "All tests pass" doesn't mean work was committed.

## Workflow

- Don't commit unless explicitly asked.
- Don't run DB migrations — ask the user.
- After finishing a task, wait for verification before moving on.
- Use `tsx` not `ts-node` for TypeScript execution.
- No background colors without explicit instructions.
- No code in PLAN documents.
- **NEVER revert code without explicit user confirmation.** 99% of the time revert is the wrong call. Fix forward. If you think a revert is needed, ask first — double and triple check.
