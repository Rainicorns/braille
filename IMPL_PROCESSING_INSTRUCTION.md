# Feature: ProcessingInstruction Node Type

## Fixes: Node-textContent 80→81/81, Node-cloneNode 132→133/135

## Problem

ProcessingInstruction (PI) nodes don't exist. `document.createProcessingInstruction()` currently creates a Comment node as a stub, discarding the `target` parameter. Multiple tests are skipped because of this.

## What ProcessingInstruction Is

- nodeType = 7 (PROCESSING_INSTRUCTION_NODE)
- Has `target` (read-only string) and `data` (read-write string, via CharacterData)
- nodeName = target, nodeValue = data
- textContent: PI.textContent returns PI.data; Element.textContent ignores PI children
- Created with `document.createProcessingInstruction(target, data)`
- Implements CharacterData interface (like Text/Comment)

## Implementation

### File 1: `crates/engine/src/dom/node.rs`

Add variant to NodeData enum:
```
ProcessingInstruction {
    target: String,
    data: String,
},
```

### File 2: `crates/engine/src/dom/tree.rs`

**2a. Add `create_processing_instruction(&mut self, target: &str, data: &str) -> NodeId`**

Same pattern as `create_comment()`.

**2b. Update ALL CharacterData methods to handle PI:**
- `character_data_get()` — return PI.data
- `character_data_set()` — set PI.data
- `character_data_length()` — PI.data UTF-16 length
- `character_data_append()` — append to PI.data
- `character_data_delete()` — delete from PI.data
- `character_data_insert()` — insert into PI.data
- `character_data_replace()` — replace in PI.data
- `character_data_substring()` — substring of PI.data

In each, add PI to the existing Text/Comment match arm (they all do the same thing with `data`/`content`).

**2c. Update `node_type()`** — return 7 for PI

**2d. Update `is_equal_node()`** — compare target and data for PI nodes

**2e. Verify `get_text_content()` / `collect_descendant_text()`** — should already skip PI (only collects Text nodes). Confirm this.

### File 3: `crates/engine/src/js/bindings/element.rs`

**3a. Add `ProcessingInstruction { target: String }` to `NodeKind` enum**

**3b. Add PI match in `get_or_create_js_element()`:**
- Match `NodeData::ProcessingInstruction { target, .. }` → `NodeKind::ProcessingInstruction { target }`
- Set prototype to `pi_proto` from DomPrototypes
- Set `target` as own read-only property on the JS object

**3c. Add `pi_proto` field to `DomPrototypes` struct**

Initialize ProcessingInstruction.prototype in the prototypes setup. It needs CharacterData methods (data getter/setter, appendData, etc.) — check if these are already on Comment.prototype and reuse.

**3d. Update `cross_tree_is_equal_node()`** — add PI comparison (target + data)

### File 4: `crates/engine/src/js/bindings/node_info.rs`

- `get_node_type()` — return 7 for PI
- `get_node_name()` — return target for PI
- `get_node_value()` — return data for PI
- `set_node_value()` — set data for PI (add PI to the matches clause)

### File 5: `crates/engine/src/js/bindings/element.rs` (textContent)

- `get_text_content()` — for PI nodes, return PI.data directly (like Text/Comment)
- `set_text_content()` — for PI nodes, set PI.data (add PI to the matches clause)

### File 6: `crates/engine/src/js/bindings/document.rs`

Rewrite `document_create_processing_instruction()` (currently creates Comment stub):
- Extract target and data from args
- Call `tree.create_processing_instruction(&target, &data)` instead of `create_comment()`
- Return via `get_or_create_js_element()`

### File 7: `crates/engine/src/js/bindings/mutation.rs`

- `adopt_node()` match — add PI arm (same as Comment: just copy data)
- Pre-insertion validation — PI should be allowed as child of Element/DocumentFragment (same rules as Comment)
- `clone_node()` — handle PI (clone target + data)

### File 8: `crates/engine/src/js/runtime.rs`

Register `ProcessingInstruction` as a global constructor (for `instanceof` checks).

### File 9: `crates/engine/tests/wpt_dom.rs`

Remove skip pattern: `("ProcessingInstruction", "requires ProcessingInstruction")`

### All other files with NodeData matches

Grep for `NodeData::` match expressions across all files. Add `NodeData::ProcessingInstruction { .. } => ...` arms. The compiler will catch any missing ones.

Expected files: css_support.rs, query.rs, traversal.rs, any serialization code.

## Verification

Run these tests:
```bash
cargo test -p braille-engine --test wpt_dom -- "Node-textContent.html"
cargo test -p braille-engine --test wpt_dom -- "Node-cloneNode.html"
```

Expected: Node-textContent 81/81, Node-cloneNode 133/135 (remaining 2 need Attr nodes).

## Scope

- Add ProcessingInstruction variant to NodeData
- Update all match arms across the codebase (compiler-assisted)
- Rewrite createProcessingInstruction to create real PI nodes
- Register ProcessingInstruction global constructor
- Enable skipped tests
- Do NOT implement Attr nodes (separate task)
