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

## Code Structure

- **New tests go in `crates/engine/tests/`**, not inline in source files. Use the public API (`eval_js`, `handle_click`, `handle_type`, `snapshot`, etc.) from external test files.
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
cargo test -p braille-engine --test anubis_challenges  # Anubis TDD suite
./dev.sh check                      # clippy, zero warnings required
./dev.sh test                       # full suite
```

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
