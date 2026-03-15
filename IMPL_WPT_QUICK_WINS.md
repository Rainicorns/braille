# WPT Quick Wins: 7 Fixes (4 test-passing + 3 partial features)

## Context
Unblock WPT tests that require small, targeted fixes. 4 tests can pass, 3 tests are blocked by larger features (iframes, onclick handlers) but we add the underlying features anyway for spec coverage.

## Execution Order
Sequential — implement each fix, run its test, then move to the next.

---

## Fix 1: Event-dispatch-handlers-changed — DONE

Scoped `downcast_ref` in `dispatch_event` to drop the `Ref<JsElement>` borrow before callbacks execute. Committed as `5326afe`.

---

## Fix 2: name-validation.html — DONE

5/5 subtests pass. Added `is_valid_element_name`, `is_valid_attribute_name`, `is_valid_doctype_name` to tree.rs. Added `toggleAttribute`. Added name validation to createElement, setAttribute, createAttribute, createDocumentType, createElementNS (via `validate_and_extract`), setAttributeNS, createAttributeNS. Committed as `d12e761`.

---

## Fix 3: CharacterData-remove — DONE

Just unskipped — test already passes. Script loader resolves ChildNode-remove.js helper, and remove() is registered on all node types via JsElement class.

---

## Fix 4: svg-template-querySelector (LIKELY JUST UNSKIP)

Tests querySelector on `template.content` (DocumentFragment) with HTML and SVG elements. Our querySelector works on DocumentFragment (query.rs:84-117 handles any JsElement). Template content is set up correctly (element.rs:542-572). SVG elements are created by the parser with SVG namespace. querySelector matching uses tag names — `svg` as a tag name should match.

**Files:**
- `crates/engine/tests/wpt_dom.rs` — remove skip for `svg-template-querySelector`

**Verification:** `cargo test --package braille-engine --test wpt_dom -- "svg-template-querySelector"`

---

## Fix 5 (partial): webkitMatchesSelector alias

Add `webkitMatchesSelector` as an alias for `matches()`. Won't make the WPT test pass (test requires iframe src loading) but adds spec coverage.

**File:** `crates/engine/src/js/bindings/query.rs` — in `register_query`, add:
```
class.method(js_string!("webkitMatchesSelector"), 1, NativeFunction::from_fn_ptr(element_matches));
```

Update skip reason in wpt_dom.rs to reflect true blocker (iframe loading, not missing alias).

**Verification:** `cargo build --package braille-engine`

---

## Fix 6 (partial): Symbol.unscopables on Element.prototype

Add `Symbol.unscopables` property to Element.prototype listing: `before`, `after`, `replaceWith`, `remove`, `prepend`, `append`. Won't make the WPT test pass (test uses onclick attribute handlers we don't support) but adds spec coverage.

**File:** `crates/engine/src/js/bindings/element.rs` — in `JsElement::init()`, after all method registrations, define `Symbol.unscopables` on the class prototype. Use `JsSymbol::unscopables()` and `ObjectInitializer` to build the object with each key set to `true`.

Update skip reason in wpt_dom.rs.

**Verification:** `cargo build --package braille-engine`

---

## Fix 7 (partial): Document.URL already exists

Document.URL and documentURI are already defined (document.rs:2729-2755) as "about:blank". The test requires iframe redirect tracking. No code change needed — just update skip reason in wpt_dom.rs to "requires iframe src loading with redirect".

**Verification:** N/A — skip reason update only.
