# Fix: Refine querySelector Skip Pattern + getElementsBy Live Collections

## Target: ~9 WPT test files

## Problem 1: Overly broad "query" skip pattern

The skip pattern `("query", "requires full querySelector")` blanket-skips ALL files with "query" in the name. querySelector/querySelectorAll are implemented and working. Many of these tests likely pass already.

Skipped tests (by the "query" pattern):
- `ParentNode-querySelector-All.html`
- `ParentNode-querySelector-All-content.html` (if exists)
- Other query-related tests

These should be unskipped. Tests that need advanced features (`:scope` already works, most selectors work) will likely pass. Any individual failures can be triaged.

## Problem 2: getElementsByClassName/TagName return static arrays

Both `getElementsByClassName` and `getElementsByTagName` return `JsArray` (static snapshots) instead of live `HTMLCollection`. The WPT tests check:
- `result instanceof HTMLCollection`
- Collection updates when DOM changes (live behavior)
- `item()` and indexed access

Skipped tests:
- `Document-getElementsByClassName.html`
- `Element-getElementsByClassName.html`
- `getElementsByClassName-32.html`
- `Element-getElementsByTagName.html`
- `Document-getElementsByTagName.html`

## Implementation

### Part 1: Unskip querySelector tests

**File: `crates/engine/tests/wpt_dom.rs`**

Remove or refine the broad pattern:
```
("query", "requires full querySelector"),
```

Replace with specific skips for tests that genuinely need missing features (if any).

### Part 2: Live HTMLCollection for getElementsBy*

The infrastructure already exists in `collections.rs` — `create_live_htmlcollection()` creates a Proxy-based live collection. It's currently used for `.children`. Extend it for getElementsByClassName/TagName.

**File: `crates/engine/src/js/bindings/collections.rs`**

Add factory functions:
- `create_live_htmlcollection_by_class(node_id, tree, class_names, context)` — live collection filtered by class
- `create_live_htmlcollection_by_tag(node_id, tree, tag_name, context)` — live collection filtered by tag

These should use the same Proxy trap pattern as `create_live_htmlcollection()` but with different filtering logic on the get trap. Each access re-walks the tree (that's what "live" means).

**File: `crates/engine/src/js/bindings/query.rs`**

Update `element_get_elements_by_class_name()` and `element_get_elements_by_tag_name()` (and their document variants) to return live HTMLCollection instead of JsArray:
- Call the new collection factory instead of building a JsArray
- Store the filter criteria (class names or tag name) in the collection

**File: `crates/engine/tests/wpt_dom.rs`**

Remove skip patterns:
```
("getElementsByClassName", "requires full getElementsByClassName"),
("Element-getElementsByTagName", "requires full getElementsByTagName"),
("Document-getElementsByTagName", "requires full getElementsByTagName"),
```

### Verification

```bash
cargo test -p braille-engine --test wpt_dom -- "querySelector"
cargo test -p braille-engine --test wpt_dom -- "getElementsByClassName"
cargo test -p braille-engine --test wpt_dom -- "getElementsByTagName"
```

## Scope

- Unskip querySelector tests (likely already pass)
- Convert getElementsByClassName/TagName to return live HTMLCollection
- Reuse existing Proxy-based live collection infrastructure from collections.rs
- Do NOT implement getElementsByTagNameNS (separate, needs namespace support)
- Do NOT implement named property access on HTMLCollection (collection["id"] lookup)
