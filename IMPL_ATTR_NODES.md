# Feature: Attr Node Type + createAttribute/createAttributeNS

## Fixes: Node-cloneNode 133→135/135 (depends on ProcessingInstruction being done first)

## Problem

2 test cases in Node-cloneNode.html fail because Attr nodes don't exist:
1. `createAttribute` test (lines 224-245) — `document.createAttribute("class")` doesn't exist
2. `createAttributeNS` test (lines 247-268) — `document.createAttributeNS(ns, "foo:class")` doesn't exist

Both tests create an Attr, clone it, and verify the clone has identical properties but is independent.

## What Attr Is

- nodeType = 2 (ATTRIBUTE_NODE)
- Properties: `name` (qualified name), `value` (read-write), `namespaceURI`, `prefix`, `localName`, `ownerElement` (null for detached)
- nodeName = name (qualified name)
- nodeValue = value
- Created with `document.createAttribute(localName)` or `document.createAttributeNS(namespace, qualifiedName)`
- Standalone node objects — NOT the same as element attribute storage (which is `Vec<(String, String)>`)
- For this task, Attr nodes only need to be standalone (detached). No need to wire to Element.attributes or setAttributeNode().

## Implementation

### File 1: `crates/engine/src/dom/node.rs`

Add variant to NodeData enum:
```
Attr {
    local_name: String,
    namespace: String,   // "" = null
    prefix: String,      // "" = null
    value: String,
},
```

### File 2: `crates/engine/src/dom/tree.rs`

**2a. Add `create_attr(&mut self, local_name: &str, namespace: &str, prefix: &str, value: &str) -> NodeId`**

Same pattern as other create methods.

**2b. Update `clone_node()`** — clone all 4 fields for Attr nodes

**2c. Update `is_equal_node()`** — compare all 4 fields

**2d. Update `node_type()`** — return 2 for Attr

### File 3: New file `crates/engine/src/js/bindings/attr.rs`

Create JsAttr class or use JsElement with Attr-specific handling. Properties needed:

| Property | Type | Behavior |
|----------|------|----------|
| `name` | readonly | Returns qualified name (prefix:localName or just localName) |
| `value` | read-write | Get/set the attr value |
| `namespaceURI` | readonly | Returns namespace or null |
| `prefix` | readonly | Returns prefix or null |
| `localName` | readonly | Returns local name |
| `ownerElement` | readonly | Always null for detached attrs |
| `specified` | readonly | Always true (legacy) |

**Note on null vs empty string:** JS should see `null` for empty namespace/prefix, not empty string. When namespace is `""` in Rust, return `JsValue::null()` in JS.

### File 4: `crates/engine/src/js/bindings/element.rs`

**4a. Add `Attr { local_name, namespace, prefix }` to `NodeKind` enum**

**4b. Add Attr handling in `get_or_create_js_element()`:**
- Match `NodeData::Attr { .. }`
- Set prototype to `attr_proto`
- Set own properties: name, value, namespaceURI, prefix, localName, ownerElement

**4c. Add `attr_proto` field to `DomPrototypes` struct**

**4d. Update `cross_tree_is_equal_node()`** — compare Attr fields

### File 5: `crates/engine/src/js/bindings/document.rs`

Add two functions:

**`document_create_attribute(localName)`:**
- Lowercase localName if HTML document (per spec)
- Call `tree.create_attr(localName, "", "", "")`
- Return via get_or_create_js_element

**`document_create_attribute_ns(namespace, qualifiedName)`:**
- Parse qualifiedName to extract prefix and localName (split on `:`)
- Call `tree.create_attr(localName, namespace, prefix, "")`
- Return via get_or_create_js_element

Register both on document objects (global and created docs via `add_document_properties_to_element`).

### File 6: `crates/engine/src/js/bindings/node_info.rs`

- `get_node_type()` — return 2 for Attr
- `get_node_name()` — return qualified name (prefix:localName or just localName)
- `get_node_value()` — return value
- `set_node_value()` — set value

### File 7: `crates/engine/src/js/bindings/mutation.rs`

- `adopt_node()` — add Attr arm
- Pre-insertion validation — Attr nodes should NOT be insertable as children (HierarchyRequestError)
- `clone_node()` — handle Attr (clone all 4 fields)

### File 8: `crates/engine/src/js/bindings/mod.rs`

Add `pub(crate) mod attr;`

### File 9: `crates/engine/src/js/runtime.rs`

Register `Attr` as a global constructor (for `instanceof` checks).

### All other files with NodeData matches

Add `NodeData::Attr { .. } => ...` arms. Compiler will catch missing ones.

## What the Tests Check

**createAttribute test:**
```javascript
var attr = document.createAttribute("class");
var copy = attr.cloneNode();
// Checks: instanceof Attr, same namespaceURI/prefix/localName/value
// Then: attr.value = "abc"; verify copy.value unchanged (independent)
```

**createAttributeNS test:**
```javascript
var attr = document.createAttributeNS("http://www.w3.org/1999/xhtml", "foo:class");
var copy = attr.cloneNode();
// Checks: instanceof Attr, namespaceURI = XHTML, prefix = "foo", localName = "class"
// Then: modify attr.value, verify copy unchanged
```

## Verification

Run only this test:
```bash
cargo test -p braille-engine --test wpt_dom -- "Node-cloneNode.html"
```

Expected: 135/135 (up from 133/135, assuming PI is already implemented).

## Scope

- Add Attr variant to NodeData
- Implement createAttribute and createAttributeNS on document
- Create JsAttr with required properties
- Handle Attr in cloneNode, nodeType, nodeName, nodeValue
- Do NOT implement Element.attributes NamedNodeMap
- Do NOT implement setAttributeNode/getAttributeNode
- Do NOT wire Attr nodes to element attribute storage
