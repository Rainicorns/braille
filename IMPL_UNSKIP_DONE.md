# Unskip Already-Implemented Features

## Target: ~8 test files, 0 new code (just remove skip patterns)

## Problem

Several skip patterns in `wpt_dom.rs` reference features that have since been implemented. These tests should be unskipped and run to see how many pass.

## Skip Patterns to Remove

### 1. ProcessingInstruction (implemented this session)
```
("ProcessingInstruction", "requires ProcessingInstruction"),
```
Unlocks: ProcessingInstruction-related tests (if any exist as standalone files).

### 2. Attr / createAttribute (implemented this session)
```
("Attr-", "requires Attr node interface"),
("Document-createAttribute", "requires Attr interface"),
```
Unlocks: `Document-createAttribute.html` and any `Attr-*.html` tests.

### 3. Document constructor + DocumentFragment constructor (already in runtime.rs)
```
("Document-constructor", "requires Document constructor"),
("DocumentFragment-constructor", "requires DocumentFragment constructor"),
("DocumentFragment-getElementById", "requires DocumentFragment constructor"),
```
Unlocks: 3 test files. Constructors exist in `runtime.rs` lines 844-973.

### 4. DocumentType interface (Doctype properties already exposed in element.rs)
```
("DocumentType-literal", "requires DocumentType interface"),
("DocumentType-remove", "requires DocumentType interface"),
```
Unlocks: 2 test files. Doctype nodes already have `name`, `publicId`, `systemId` as JS properties (element.rs line 436). ChildNode.remove() exists on all node types.

### 5. DOMTokenList-coverage (classList is 1420/1420)
```
("DOMTokenList-coverage", "requires full DOMTokenList"),
```
Unlocks: 1 test file. classList is fully implemented with replace(), value, toString, token validation.

### 6. Document metadata (already implemented as hardcoded values)
```
("Document-URL", "requires Document.URL"),
("Document-characterSet", "requires characterSet"),
("Document-doctype", "requires doctype node access"),
```
Unlocks: 3 test files. URL, characterSet, doctype getter all exist. Some sub-tests may fail (iframe-dependent) but the files should run.

### 7. adoptNode-related (adopt_node exists in mutation.rs as internal fn)
```
("Document-adoptNode", "requires adoptNode"),
```
NOTE: `adopt_node()` exists as an internal Rust helper in mutation.rs but is NOT exposed as a JS method. This needs the `document.adoptNode()` JS binding added (see IMPL_ADOPT_NODE.md). Only unskip after that's done.

### 8. XML-related (DOMParser implemented, createProcessingInstruction works)
```
("xml", "requires XML support"),
("XHTML", "requires XHTML"),
("xhtml", "requires XHTML"),
```
NOTE: These are broad patterns. Some XML tests may now pass (DOMParser handles XML), but many may need features we don't have. Unskip cautiously — maybe comment them out and see what fails.

## Implementation

### File: `crates/engine/tests/wpt_dom.rs`

Remove or comment out the skip patterns listed above (groups 1-6 are safe, group 7-8 need their features first).

### Verification

Run each unskipped test individually:
```bash
cargo test -p braille-engine --test wpt_dom -- "Document-createAttribute"
cargo test -p braille-engine --test wpt_dom -- "Document-constructor"
cargo test -p braille-engine --test wpt_dom -- "DocumentFragment-constructor"
cargo test -p braille-engine --test wpt_dom -- "DocumentType-literal"
cargo test -p braille-engine --test wpt_dom -- "DocumentType-remove"
cargo test -p braille-engine --test wpt_dom -- "DOMTokenList-coverage"
cargo test -p braille-engine --test wpt_dom -- "Document-doctype"
```

Some tests may reveal new sub-test failures that need small fixes. That's expected — triage and fix inline.
