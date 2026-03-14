# WPT Test Failure Fixes

21 WPT tests were misclassified as PASS. This document categorizes each failure and tracks fixes.

## Wave 1: Trivial Fixes + Skip Additions (3 parallel agents)

### Agent 1: isEqualNode + insertAdjacentElement

**Node-isEqualNode.html (8/9 → 9/9)**
- Bug: Attribute equality compares prefix, but spec says only namespace + localName + value matter
- Fix: Remove `&& a.prefix == attr.prefix` from:
  - `dom/tree.rs:908` (is_equal_node)
  - `js/bindings/element.rs:40` (cross_tree_is_equal_node)

**insert-adjacent.html (13/14 → 14/14)**
- Bug: insertAdjacentElement accepts DocumentType nodes, should throw TypeError
- Fix: Add nodeType==1 check in `js/bindings/mutation.rs` after line 1303

### Agent 2: DOMParser on window + Skip List

**Node-cloneNode-document-with-doctype.html (2/3 → 3/3)**
- Bug: `window.DOMParser` is undefined — DOMParser registered on global but not copied to window object
- Fix: Copy DOMParser onto window in `js/bindings/window.rs` (~line 591-607)

**Skip list additions** (in `crates/engine/tests/wpt_dom.rs`):
- `("NodeList-static-length-getter-tampered", "performance test, too slow for interpreter")` — 4 tests, ~280s each
- `("createDocument-with-null-browsing-context", "requires iframes")`
- `("createHTMLDocument-with-null-browsing-context", "requires iframes")`
- `("createHTMLDocument-with-saved-implementation", "requires iframes")`

### Agent 3: Event Constructor new-check

**Event-constructors.any.js (12/14 → 14/14)**
- Bug: `Event("test")` without `new` doesn't throw TypeError
- Fix: Check `new_target` is undefined in `js/bindings/event.rs` JsEvent::data_constructor, JsCustomEvent::data_constructor, and ui_event_subclass macro

## Wave 2: Medium Fixes (2 parallel agents)

### Agent 4: XML Name Validation

**Document-createProcessingInstruction.html**
- Bug: No validation of target name — should throw INVALID_CHARACTER_ERR for invalid XML Names
- Fix: Create `validate_xml_name()` helper. Add validation in `js/bindings/document.rs:336-368`. Also check data doesn't contain `"?>"`.

**DOMImplementation-createDocument.html (0/2)**
- Bug: No namespace/qualifiedName validation, no argument count check
- Fix: Add validation to `domimpl_create_document` in `document.rs:1503-1594`. Reuse `validate_xml_name()`.

### Agent 5: PLAN.md Update

Update test manifest with corrected statuses for all 21 tests.

## Accepted Partial Passes (no fix needed)

These tests have some subtests that pass and some that require missing infrastructure:

| Test | Pass/Total | Blocked by |
|------|-----------|------------|
| Node-appendChild.html | ~pass/fail | frames (iframes) |
| Node-removeChild.html | 19/28 | frames (iframes) |
| Node-isConnected.html | 1/2 | iframes |
| ParentNode-replaceChildren.html | 25/29 | MutationObserver |
| EventListener-handleEvent.html | 3/6 | promise_test |
| rootNode.html | 4/5 | Shadow DOM (attachShadow) |
| Node-normalize.html | 3/4 | CDATASection distinct node type |
