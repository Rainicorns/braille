# Feature: document.adoptNode() JS Binding

## Target: ~3 WPT test files

## Problem

`adopt_node()` exists as an internal Rust helper in `mutation.rs` (line 87) — it moves a node from one tree to another. But there is no `document.adoptNode(node)` JS binding exposed to scripts.

Skipped tests:
- `Document-adoptNode.html`
- `Node-mutation-adoptNode.html`
- `remove-and-adopt.html`
- `node-appendchild-crash.html` (also blocked by adoptNode)

## What adoptNode Does (per spec)

1. If node is a Document, throw `NotSupportedError`
2. Remove node from its parent (if any)
3. Change ownerDocument of node and all descendants to this document
4. Return node

Key difference from `importNode`: adoptNode **moves** the original node, importNode **clones** it.

## Implementation

### File 1: `crates/engine/src/js/bindings/document.rs`

Add `document_adopt_node()` function:
- Extract the node argument, get its node_id and source tree
- If source node is a Document (nodeType 9), throw NotSupportedError DOMException
- If same tree: just remove from parent, return same object
- If different tree: use existing `adopt_node()` from mutation.rs to move to this document's tree
- Remove from old parent first
- Return the JS object for the adopted node (via get_or_create_js_element)

Register on both global document (in `register_document`) and created documents (in `add_document_properties_to_element`).

### File 2: `crates/engine/src/js/bindings/mutation.rs`

The existing `adopt_node()` fn (line 87) already handles the cross-tree copy. Verify it:
- Copies the node and its descendants to the destination tree
- Returns the new NodeId in the destination tree

May need to also handle: invalidating the old JS object cache entry so the node_id in the source tree is no longer accessible.

### File 3: `crates/engine/tests/wpt_dom.rs`

Remove skip patterns:
```
("Document-adoptNode", "requires adoptNode"),
("Node-mutation-adoptNode", "requires adoptNode"),
("remove-and-adopt", "requires adoptNode"),
("node-appendchild-crash", "requires adoptNode"),
```

## Verification

```bash
cargo test -p braille-engine --test wpt_dom -- "Document-adoptNode"
cargo test -p braille-engine --test wpt_dom -- "Node-mutation-adoptNode"
```

## Scope

- Add document.adoptNode() JS binding
- Reuse existing adopt_node() internal helper
- Handle Document node error case
- Do NOT implement adoption agency algorithm (that's HTML parser related)
