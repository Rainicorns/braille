# Feature: Namespace-Aware Attribute Methods

## Target: 7 WPT test files

## Problem

4 namespace-aware attribute methods are missing: `setAttributeNS`, `getAttributeNS`, `hasAttributeNS`, `removeAttributeNS`. Also missing: `hasAttributes()` (no-arg, returns bool if any attributes exist).

Skipped tests:
- `Element-hasAttribute.html`
- `Element-hasAttributes.html`
- `Element-setAttribute.html` (namespace variant tests)
- `Element-removeAttribute.html` (namespace variant tests)
- `Element-removeAttributeNS.html`
- `Element-firstElementChild-namespace.html`
- `Element-setAttribute-crbug.html`

## Background

Attributes are stored as `Vec<(String, String)>` in `NodeData::Element`. The parser stores namespaced attributes as `"prefix localname"` (space-separated) via `format_attr_name()` in `parser.rs`. For example, `xlink:href` is stored as `"xlink href"`.

The NS-aware methods need a different lookup strategy — they match on `(namespace, localName)` rather than the formatted name string.

## Approach: Structured Attribute Storage

Change `attributes: Vec<(String, String)>` to `attributes: Vec<DomAttribute>` where DomAttribute has `local_name`, `prefix`, `namespace`, `value` fields. Non-NS methods (`getAttribute("foo")`) match on qualified name (`prefix:localname` or just `localname`).

---

## Complete Change Inventory

Every file and line that must change, verified by grep.

### 1. Type Definition — `crates/engine/src/dom/node.rs`

**Line 17**: Element variant field
- FROM: `attributes: Vec<(String, String)>,`
- TO: `attributes: Vec<DomAttribute>,`

Add the DomAttribute struct definition (above NodeData):
- `pub struct DomAttribute { pub local_name: String, pub prefix: String, pub namespace: String, pub value: String }`
- `impl DomAttribute { pub fn qualified_name(&self) -> String { ... } }`
- Add a convenience constructor: `DomAttribute::new(name: &str, value: &str) -> Self` (empty prefix/ns) for the many sites that create simple attrs

### 2. Core Attribute Methods — `crates/engine/src/dom/attributes.rs`

| Line | Current Code | Change |
|------|-------------|--------|
| 11 | `attributes.iter().find(\|(k, _)\| k == name)` | Match on `qualified_name()` or `local_name` |
| 12 | `.map(\|(_, v)\| v.clone())` | `.map(\|a\| a.value.clone())` |
| 24 | `attributes.iter_mut().find(\|(k, _)\| k == name)` | Match on `qualified_name()` or `local_name` |
| 25 | `existing.1 = value.to_string()` | `existing.value = value.to_string()` |
| 28 | `attributes.push((name.to_string(), value.to_string()))` | `attributes.push(DomAttribute::new(name, value))` |
| 41 | `attributes.retain(\|(k, _)\| k != name)` | `attributes.retain(\|a\| a.qualified_name() != name && a.local_name != name)` |
| 52 | `attributes.iter().any(\|(k, _)\| k == name)` | `attributes.iter().any(\|a\| a.qualified_name() == name \|\| a.local_name == name)` |

Add 5 new methods after existing ones:
- `get_attribute_ns(id, namespace, local_name) -> Option<String>`
- `set_attribute_ns(id, namespace, qualified_name, value)` — parse prefix from qname
- `remove_attribute_ns(id, namespace, local_name) -> bool`
- `has_attribute_ns(id, namespace, local_name) -> bool`
- `has_attributes(id) -> bool`

Tests (lines 59-265) — update all attribute tuple creation to `DomAttribute::new(...)`.

### 3. Tree Creation Methods — `crates/engine/src/dom/tree.rs`

| Line | Current Code | Change |
|------|-------------|--------|
| 105 | `attributes.iter().any(\|(k, v)\| k == "id" && v == id)` | `attributes.iter().any(\|a\| a.local_name == "id" && a.value == id)` |
| 266 | `fn create_element_with_attrs(&mut self, tag_name: &str, attributes: Vec<(String, String)>)` | Change param to `Vec<DomAttribute>` |
| 306 | `fn create_element_ns(&mut self, tag_name: &str, attributes: Vec<(String, String)>, namespace: &str)` | Change param to `Vec<DomAttribute>` |
| 521 | `for (k, v) in attributes { o.push_str(k); ... o.push_str(v); }` | `for a in attributes { o.push_str(&a.qualified_name()); ... o.push_str(&a.value); }` |
| 901 | `for (name, value) in a1 { a2.iter().any(\|(n, v)\| n == name && v == value) }` | Match on struct fields for `is_equal_node` |

Tests (lines ~1217, ~1383-1442) — update attribute tuple creation.

### 4. HTML Parser (CRITICAL) — `crates/engine/src/html/parser.rs`

| Line | Current Code | Change |
|------|-------------|--------|
| 88-94 | `let attributes: Vec<(String, String)> = attrs.into_iter().map(\|a\| { let key = format_attr_name(&a.name); (key, a.value.to_string()) })` | Create `DomAttribute` with `local_name` from `a.name.local`, `prefix` from `a.name.prefix`, `namespace` from `a.name.ns`, `value` from `a.value` |
| 274 | `attributes.iter().map(\|(k, _)\| k.clone()).collect()` | `.map(\|a\| a.qualified_name()).collect()` |
| 276-278 | `let name = format_attr_name(&attr.name); attributes.push((name, attr.value.to_string()))` | Create structured `DomAttribute` from html5ever attr |
| 393, 401 | `attributes.iter().any(\|(k, _)\| k == "selected")` | `attributes.iter().any(\|a\| a.local_name == "selected")` |
| 439-445 | `fn format_attr_name(name: &QualName) -> String` | Keep for backward compat or replace; the function currently builds `"prefix localname"` strings |

**Note**: `format_attr_name()` can be replaced by the DomAttribute constructor that takes a `QualName`. This is the PRIMARY site where namespace info enters the system — html5ever provides `a.name.ns`, `a.name.prefix`, `a.name.local`.

### 5. DOMParser (MISSED in original blueprint) — `crates/engine/src/js/bindings/dom_parser.rs`

| Line | Current Code | Change |
|------|-------------|--------|
| 63-71 | `let attrs: Vec<(String, String)> = e.attributes().filter_map(\|a\| { Some((key, val)) }).collect()` | Create `DomAttribute` structs. quick-xml attrs have key (which may include prefix) and value. Need to parse prefix from key. |
| 107-115 | Same pattern for `Event::Empty` elements | Same change as above |

### 6. JS Bindings — Attributes — `crates/engine/src/js/bindings/attributes.rs`

| Line | Current Code | Change |
|------|-------------|--------|
| 181 | `attributes.iter().find(\|(n, _)\| n == &name).map(\|(n, v)\| (n.clone(), v.clone()))` | Match on struct fields |

Add JS bindings for new methods on Element class:
- `setAttributeNS(namespace, qualifiedName, value)`
- `getAttributeNS(namespace, localName)`
- `removeAttributeNS(namespace, localName)`
- `hasAttributeNS(namespace, localName)`
- `hasAttributes()` (no args)

Handle null namespace: `args[0]` could be `JsValue::null()` → treat as `""`.

Tests (lines ~291, ~1119, ~1147) — update attribute tuple creation.

### 7. JS Bindings — Element — `crates/engine/src/js/bindings/element.rs`

| Line | Current Code | Change |
|------|-------------|--------|
| 38-40 | `for (name, value) in a1 { if !a2.iter().any(\|(n, v)\| n == name && v == value)` | Match on struct fields in `cross_tree_is_equal_node` |

### 8. JS Bindings — Node Info — `crates/engine/src/js/bindings/node_info.rs`

| Line | Current Code | Change |
|------|-------------|--------|
| 359 | `for (i, (name, value)) in attributes.iter().enumerate()` | `for (i, attr) in attributes.iter().enumerate()` — use `attr.qualified_name()` and `attr.value`, also populate `attr.namespace` and `attr.prefix` on the Attr JS objects |

### 9. JS Bindings — Mutation — `crates/engine/src/js/bindings/mutation.rs`

| Line | Current Code | Change |
|------|-------------|--------|
| 106-117 | `let attrs = attributes.clone(); ... dst_tree.borrow_mut().create_element_ns(&tag, attrs, &ns)` | `Vec<DomAttribute>` is Clone-able, so `.clone()` still works. Ensure `create_element_ns` signature matches. |

### 10. JS Bindings — HTML Element — `crates/engine/src/js/bindings/html_element.rs`

| Line | Current Code | Change |
|------|-------------|--------|
| 41 | `attributes.iter().find(\|(k, _)\| k == "tabindex").map(\|(_, v)\| v.clone())` | `attributes.iter().find(\|a\| a.local_name == "tabindex").map(\|a\| a.value.clone())` |

### 11. A11y Serializer — `crates/engine/src/a11y/serialize.rs`

| Line | Current Code | Change |
|------|-------------|--------|
| 99-103 | `attributes.iter().find(\|(k, _)\| k == "alt").map(\|(_, v)\| v.clone())` | Match on `.local_name` and `.value` |
| 179 | `attributes.iter().find(\|(k, _)\| k == "type").map(\|(_, v)\| v)` | Same pattern |

Tests (~306, ~1120, ~1148) — update attribute tuple creation.

### 12. Focus Command — `crates/engine/src/commands/focus.rs`

| Line | Current Code | Change |
|------|-------------|--------|
| 24 | `attributes.iter().any(\|(k, _)\| k == "tabindex")` | `attributes.iter().any(\|a\| a.local_name == "tabindex")` |

### 13. CSS Matching — `crates/engine/src/css/matching.rs`

| Line | Current Code | Change |
|------|-------------|--------|
| 199-222 | `attr_matches()` uses `tree.get_attribute()` abstraction | **No direct changes** — uses get_attribute() which handles the new storage. BUT the namespace constraint matching (line 207-214) can now actually look up by namespace instead of rejecting all non-empty namespaces. |

### 14. CSS Cascade Tests — `crates/engine/src/css/cascade.rs`

| Line | Current Code | Change |
|------|-------------|--------|
| 242 | `fn build_tree_with_element(tag: &str, attrs: Vec<(String, String)>)` | Change param type or convert inside |

### 15. Dom Find Tests — `crates/engine/src/dom/find.rs`

Lines ~85, ~98, ~111, ~175, ~212, ~232 — test code creates attribute tuples like `("id".to_string(), "myid".to_string())`. Change to `DomAttribute::new("id", "myid")`.

### 16. CSS Collection / Style — NO CHANGES NEEDED

`css/collection.rs` and `js/bindings/style.rs` use `Vec<(String, String)>` for **CSS properties** (not DOM attributes). These are separate types and don't need changing.

### 17. WPT Test Skip Patterns — `crates/engine/tests/wpt_dom.rs`

Remove these 7 skip patterns:
- Line 421: `("Element-firstElementChild-namespace", "requires setAttributeNS")`
- Line 422: `("Element-removeAttributeNS", "requires setAttributeNS")`
- Line 423: `("Element-setAttribute-crbug", "requires setAttributeNS")`
- Line 549: `("Element-hasAttribute", "requires setAttributeNS")`
- Line 550: `("Element-hasAttributes", "requires setAttributeNS")`
- Line 552: `("Element-setAttribute", "requires setAttributeNS")`
- Line 554: `("Element-removeAttribute", "requires setAttributeNS")`

---

## Implementation Order

1. Define `DomAttribute` struct in `node.rs` with `new()` convenience constructor
2. Update `attributes.rs` core methods to work with new struct
3. Update `tree.rs` signatures (`create_element_with_attrs`, `create_element_ns`, `serialize`, `is_equal_node`, `get_element_by_id`)
4. Update `parser.rs` — replace `format_attr_name()` tuple creation with structured DomAttribute from html5ever QualName
5. Update `dom_parser.rs` — quick-xml attribute creation
6. Update all JS binding files (element.rs, node_info.rs, mutation.rs, html_element.rs, attributes.rs)
7. Update `a11y/serialize.rs` and `commands/focus.rs`
8. Add new NS-aware methods to `attributes.rs`
9. Add JS bindings for `setAttributeNS`/`getAttributeNS`/`hasAttributeNS`/`removeAttributeNS`/`hasAttributes`
10. Update test files (find.rs, cascade.rs, a11y tests, attributes tests)
11. Remove WPT skip patterns and run tests

## Verification

Run each unskipped test individually:
- `cargo test -p braille-engine --test wpt_dom -- "Element-hasAttribute"`
- `cargo test -p braille-engine --test wpt_dom -- "Element-hasAttributes"`
- `cargo test -p braille-engine --test wpt_dom -- "Element-removeAttributeNS"`
- `cargo test -p braille-engine --test wpt_dom -- "Element-setAttribute-crbug"`
- `cargo test -p braille-engine --test wpt_dom -- "Element-setAttribute"`
- `cargo test -p braille-engine --test wpt_dom -- "Element-removeAttribute"`
- `cargo test -p braille-engine --test wpt_dom -- "Element-firstElementChild-namespace"`

Also run existing passing tests to verify no regressions:
- `cargo test -p braille-engine --test wpt_dom -- "Element-classlist"`
- `cargo test -p braille-engine --test wpt_dom -- "Element-closest"`
- `cargo test -p braille-engine --test wpt_dom -- "Element-tagName"`

## Scope

- Restructure attribute storage to DomAttribute struct
- Add 4 NS-aware methods + hasAttributes()
- Update all 16 files with attribute access sites
- Do NOT implement NamedNodeMap (element.attributes collection) — separate task
- Do NOT implement setAttributeNode/getAttributeNode
