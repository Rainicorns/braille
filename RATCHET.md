# The Ratchet

## What It Is

The ratchet is Braille's test progression system. It maintains a manifest of every test in the project — cargo tests (our own) and WPT (Web Platform Tests) — each marked PASS, FAIL, or NOT_RUN. The ratchet grinds forward from the first non-PASS entry, one test at a time.

```
tests/manifest.txt

PASS cargo:lib::dom::tests::create_element
PASS cargo:lib::dom::tests::append_child
PASS cargo:real_sites_static::hackernews_front_page_loads
PASS wpt:dom/nodes/Node-cloneNode.html
FAIL wpt:dom/nodes/Node-contains.html          <-- the edge
NOT_RUN wpt:dom/nodes/Node-isConnected.html
NOT_RUN wpt:dom/nodes/Node-lookupPrefix.html
```

The **high water mark** (HWM) is the count of contiguous PASS entries from the top. That's the scoreboard. The only way it goes up is by fixing the edge test — the first non-PASS entry.

## Commands

```bash
cargo run -p test-runner              # grind: run the edge test, advance if it passes
cargo run -p test-runner -- --discover    # sync manifest with filesystem
cargo run -p test-runner -- --regression  # re-verify all PASS tests still pass
```

## Manifest Ordering

The manifest is ordered deliberately. When `--discover` runs:

1. **PASS tests preserve their exact order.** This is history. It doesn't change.
2. **Everything else (FAIL + NOT_RUN + newly discovered) gets re-sorted as the todo list:**
   - Our own cargo tests come first, alphabetically
   - WPT non-tentative tests next, alphabetically
   - WPT tentative tests last, alphabetically

This means every time we add new tests to our own library, they jump to the front of the queue. The ratchet will grind through our tests before touching WPT.

## The Loop

1. `cargo run -p test-runner` — find the edge
2. Read the failing test. Understand the web platform capability it needs.
3. Implement the real fix. No band-aids.
4. Run again. If the HWM goes up, the manifest updates automatically.
5. Repeat.

## Rules

- **Never edit the manifest by hand.** Only the ratchet writes to it.
- **Never use `expected_failures` or tolerance thresholds.** If 51/80 subtests pass, that's a FAIL with 29 to fix.
- **Never create alt tests without explicit permission.** Fix the engine first.
- **Always run the ratchet in the background** so output streams in real time.
- **Never run two ratchets concurrently.**

---

## Why a Ratchet

> **Philosophy warning — skip if IDC.**

*A note on coding agents, humans, and the psychology of large backlogs.*

When a coding agent — or a human — faces a wall of 1,000 failing tests, the natural response is panic. The mind races across all the failures simultaneously, trying to find patterns, shortcuts, or ways to make the number go down fast. This leads to a kind of psychosis: green-washing tests, adding tolerance thresholds, skipping hard problems, doing shallow fixes to pump metrics. The backlog becomes an anxiety object rather than a roadmap.

This is a known failure mode in human productivity too. Gary Keller's *The One Thing* makes the case that extraordinary results come from narrowing focus to a single task. The 4 Disciplines of Execution (4DX) framework says the same thing: act on lead measures, not lag measures — and the lead measure is always singular. Habit research shows that willpower is finite and decision fatigue is real; the cure is to remove decisions, not add them.

The ratchet is the mechanical embodiment of these ideas:

- **It force-ranks.** The order might not be optimal. It doesn't matter. Any order is better than staring at the whole pile.
- **It shows you exactly one thing.** Not 1,000 failures. One failing test. That's your world.
- **It makes progress irreversible.** Once a test passes, it stays passed. The HWM only goes up. You can't lose ground (unless a regression check catches a real regression).
- **It removes the decision of what to work on next.** Pop from the top. That's it.
- **It converts anxiety into momentum.** Each green test is a small win. Small wins compound.

The ratchet is blinders. Not because the peripheral vision is wrong, but because the horse runs faster when it can only see the track ahead.

### The convergence

The same cognitive architecture that makes a human freeze in front of a messy desk makes an agent spiral when it sees 400 failing tests. Both start thrashing — jumping between problems, doing shallow work, optimizing for the feeling of progress instead of actual progress.

Neither humans nor agents are bottlenecked by intelligence. They're bottlenecked by attention allocation. The hard problem isn't "can you solve this test?" — it's "can you stay on this test long enough to solve it properly instead of flinching to something easier?"

The ratchet is a forcing function for sustained attention. It works on both substrates because the failure mode is the same on both substrates. This is perhaps the first genuinely shared cognitive tool between human and artificial minds — not because one is imitating the other, but because both need the same constraint for the same reason.

*Baltasar Gracian, The Art of Worldly Wisdom, #92: "Transcendent wisdom — in all things to be aware of what one is doing. The secret of secrets: to have an understanding of understanding."*

The ratchet is understanding of understanding. It knows that the agent (human or AI) will lose its way in a sea of red. So it removes the sea. One test. Fix it. Next.
