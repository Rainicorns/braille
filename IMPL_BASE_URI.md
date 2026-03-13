# Feature: Node.baseURI Property

## Target: 1-2 WPT test files

## Problem

`Node.baseURI` is not implemented. Per spec, it returns the document's URL for all nodes.

Skipped tests:
- `Node-baseURI.html`

## What baseURI Does (per spec)

For any node, `node.baseURI` returns:
1. The node's ownerDocument's URL (or the document itself if it IS a document)
2. For detached nodes: the associated document's URL
3. Default: `"about:blank"`

The full spec also considers `<base>` elements, but we can start with just returning document.URL.

## Implementation

### File 1: `crates/engine/src/js/bindings/node_info.rs`

Add a `get_base_uri()` getter function:
- Return `"about:blank"` (matches our Document.URL default)
- Could be smarter later (check for `<base>` element, use actual page URL)

Register as a property getter on all node types. Look at how `nodeType` getter is registered — follow the same pattern. The `baseURI` getter should be on the Node prototype level (so accessible from all node types).

### File 2: `crates/engine/tests/wpt_dom.rs`

Remove skip pattern:
```
("Node-baseURI", "requires baseURI"),
```

## Verification

```bash
cargo test -p braille-engine --test wpt_dom -- "Node-baseURI"
```

## Scope

- Add baseURI as getter on all nodes returning "about:blank"
- Do NOT implement `<base>` element resolution
- Do NOT implement per-document URL tracking (that's for when we have real navigation)
