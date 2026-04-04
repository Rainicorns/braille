# Test Coverage Analysis — Braille Browser Engine

Date: 2026-04-04
Branch: feat/ventilla/test-coverage-expansion

## Current State

| Suite | Count | Status |
|-------|-------|--------|
| Engine unit tests | ~482 | Inline in lib_tests.rs, dom/tree/tests.rs, dom_bridge/tests.rs |
| html5lib tree construction | 1778/1778 | 100% pass |
| html5lib serializer | 204/204 | 100% pass |
| Integration tests | 39 files, ~16K LOC | Real web patterns, adversarial, framework tests |
| CLI tests | 8 files, ~3.6K LOC | Session, fetch, SPA |
| WPT DOM | 1186/2323 | Tracked via ratchet |
| Wire protocol | ~30 roundtrip tests | Serde only |

**Test-to-source ratio:** ~53% (16K test LOC / 31K source LOC)

## Gap Inventory (prioritized by risk)

### P0 — Security-Critical, Minimal Coverage

| Module | LOC | Current Tests | Gap |
|--------|-----|---------------|-----|
| **cookies.rs** | 207 | cookie_sync.rs (7 tests) | Domain matching edge cases, Secure flag over HTTP, SameSite (unimplemented), cookie injection via malformed headers, path scoping, expiry bypass |
| **JS sandbox** | ~15K (js/) | Scattered across 10+ files | No focused tests for prototype pollution, DOM clobbering, globalThis enumeration, eval/Function restrictions |

### P1 — High Complexity, No Dedicated Tests

| Module | LOC | Current Tests | Gap |
|--------|-----|---------------|-----|
| **navigation.rs** | 377 | Tested indirectly via smoke_integration | No focused tests for redirect depth (>5), fetch failures, cookie attachment across redirects, settle-with-fetches loop |
| **CSS cascade** | 5,295 (10 files) | css_edge_cases.rs only | Missing: specificity tie-breaking, inheritance chains, !important, cascade layers, computed style idempotency |
| **settle loop** | ~70 LOC in lib.rs | No focused tests | Termination guarantee, idempotency (settle(settle(x)) == settle(x)), timer advancement behavior |
| **Event dispatch** | Via JS runtime | Scattered | No focused capture/target/bubble ordering tests, stopPropagation correctness, event.target vs event.currentTarget |

### P2 — Important, Under-covered

| Module | LOC | Current Tests | Gap |
|--------|-----|---------------|-----|
| **MCP crate** | ~600 (3 files) | Zero tests | Tool dispatch, parameter parsing, snap mode routing, error handling |
| **Layout** | ~500 (3 files) | No dedicated tests | Depends on snapshot working; needs computed layout verification |
| **DOM tree invariants** | 2,527 (9 files) | Inline tree tests | No integration-level invariant tests after complex mutation sequences |
| **meta_refresh.rs** | 119 | meta_refresh_tests.rs + refresh_header.rs | Adequate for now |

### P3 — Moderate, Existing Coverage Sufficient

| Module | LOC | Coverage |
|--------|-----|---------|
| **HTML parser** | html5ever integration | 1778 tree construction + 204 serializer — excellent |
| **Wire protocol** | ~570 | 30+ roundtrip tests — good for serde, missing edge cases |
| **Commands** | ~400 (8 files) | Covered by adversarial.rs, dom_bridge_forms.rs, integration.rs |
| **Snapshot/a11y** | ~2K (7 files) | snapshot_views.rs covers 11 modes |

## Testing Strategy

### Principles (per Deming)

1. **Integration tests using real web patterns** — this is a browser engine, not a library. Tests should mimic what real sites do.
2. **Red tests are roadmap items** — write tests that expose real engine gaps. A failing test is more valuable than a passing test that avoids the gap.
3. **Security boundaries first** — cookie isolation, JS sandbox, DOM clobbering resistance. These affect trust in the engine.
4. **Structural invariants** — DOM tree consistency, CSS cascade determinism, settle loop termination. These prevent classes of bugs, not just specific bugs.

### Priority for New Tests (this PR)

1. **cookie_security.rs** — Domain matching, HttpOnly enforcement, Secure flag, injection attacks, path scoping
2. **navigation_error_paths.rs** — Redirect depth, fetch failures, cookie persistence across navigations
3. **dom_tree_invariants.rs** — Parent/child consistency, sibling chains, mutation sequence correctness via JS
4. **settle_loop_properties.rs** — Termination, idempotency, timer behavior
5. **js_sandbox_boundaries.rs** — Prototype pollution, DOM clobbering, internal binding exposure
6. **css_cascade_determinism.rs** — Specificity ordering, inheritance, computed style stability
7. **event_dispatch_propagation.rs** — Capture/target/bubble, stopPropagation, event.target

## Results — This PR

| Test File | Tests | Pass | Fail | Notes |
|-----------|-------|------|------|-------|
| **cookie_security.rs** | 21 | 14 | 7 | Domain matching, Secure flag, path scoping, expiry gaps |
| **navigation_error_paths.rs** | 9 | 9 | 0 | Full pass — redirect depth, fetch failures, cookie attachment |
| **dom_tree_invariants.rs** | 10 | 7 | 3 | replaceChild, sibling chain after remove, complex mutations |
| **settle_loop_properties.rs** | 13 | 13 | 0 | Full pass — termination, idempotency, timer behavior |
| **js_sandbox_boundaries.rs** | 13 | 13 | 0 | Full pass — prototype pollution, DOM clobbering, error containment |
| **css_cascade_determinism.rs** | 13 | 12 | 1 | Color inheritance not propagating |
| **event_dispatch_propagation.rs** | 10 | 10 | 0 | Full pass — capture/bubble, stopPropagation, delegation |
| **TOTAL** | **89** | **78** | **11** | 87.6% pass rate |

### Red Tests = Roadmap Items

The 11 failing tests expose real engine gaps:

**Cookie Security (7 failures):**
- `cookie_domain_no_superdomain_match` — no public suffix protection (Domain=com accepted)
- `cookie_domain_different_domain_no_match` — cross-domain cookie leakage
- `cookie_domain_prefix_attack` — notexample.com matches example.com cookies
- `cookie_path_scoping` — Path=/app cookies sent to /other
- `secure_cookie_not_sent_over_http` — Secure flag not enforced
- `expired_cookie_not_sent` — Expires in past not checked
- `negative_max_age_treated_as_expired` — negative Max-Age not handled

**DOM Tree (3 failures):**
- `replace_child_maintains_tree_integrity` — replaceChild not implemented in JS bridge
- `sibling_chain_after_remove_middle` — nextSibling/previousSibling not updated after remove
- `rapid_mutation_sequence_maintains_consistency` — complex mutation sequences break tree

**CSS Cascade (1 failure):**
- `color_inherits_from_parent` — CSS color inheritance returns black instead of parent color

### Known Untestable Gap

- **JS stack overflow** — infinite recursion in JS overflows the Rust thread stack (SIGABRT) before QuickJS can catch it. QuickJS needs stack depth checking configured. Cannot be tested safely — kills the test process.

### Future Work (not in this PR)

- MCP crate tests (requires daemon infrastructure or trait extraction)
- Layout integration tests (requires Taffy output verification)
- Wire protocol fuzzing (malformed JSON, truncated messages)
- WPT DOM ratchet expansion (via grind workflow)
- Performance regression tests (Criterion benchmarks for settle loop, CSS computation)
- QuickJS stack depth configuration (prevent SIGABRT on deep recursion)
