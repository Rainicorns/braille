# Spike Implementation Plan — COMPLETE

**Status:** All 4 waves done. 46 tests pass. Core loop proven.

**API Reference:** [REFERENCE.md](./REFERENCE.md) — Boa and html5ever API details for implementing agents

## Goal

Prove the core loop: **HTML in → parse → DOM → execute JS → DOM mutates → text snapshot out**

Test case: parse a simple HTML page with an inline `<script>` that modifies the DOM, then serialize the resulting DOM to an accessibility tree.

```html
<html>
<body>
  <h1>Hello</h1>
  <div id="app"></div>
  <script>
    let el = document.createElement("p");
    el.textContent = "Created by JavaScript";
    document.getElementById("app").appendChild(el);
  </script>
</body>
</html>
```

Expected output (accessibility tree):
```
heading "Hello"
paragraph "Created by JavaScript"
```

## Wave 0: Scaffold Workspace

**1 agent, must complete before all other waves.**

Create the Cargo workspace with 3 crates. Minimal Cargo.toml files with dependencies. No implementation code — just enough that `cargo check` passes.

### Files to create:

**`/Cargo.toml`** (workspace root)
```toml
[workspace]
members = ["crates/engine", "crates/wire", "crates/cli"]
resolver = "2"
```

**`/crates/wire/Cargo.toml`**
```toml
[package]
name = "braille-wire"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { version = "1", features = ["derive"] }
```

**`/crates/wire/src/lib.rs`**
```rust
// placeholder
```

**`/crates/engine/Cargo.toml`**
```toml
[package]
name = "braille-engine"
version = "0.1.0"
edition = "2021"

[dependencies]
braille-wire = { path = "../wire" }
boa_engine = "0.21"
boa_gc = "0.21"
html5ever = "0.38"
markup5ever = "0.38"
tendril = "0.5"

[dev-dependencies]
```

**`/crates/engine/src/lib.rs`**
```rust
pub mod dom;
pub mod html;
pub mod js;
pub mod a11y;
```

**`/crates/engine/src/dom/mod.rs`**, **`html/mod.rs`**, **`js/mod.rs`**, **`a11y/mod.rs`** — empty module files

**`/crates/cli/Cargo.toml`**
```toml
[package]
name = "braille-cli"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "braille"
path = "src/main.rs"

[dependencies]
braille-wire = { path = "../wire" }
braille-engine = { path = "../engine" }
clap = { version = "4", features = ["derive"] }
reqwest = { version = "0.12", features = ["blocking"] }
```

**`/crates/cli/src/main.rs`**
```rust
fn main() {
    println!("braille");
}
```

### Verification
```
cargo check
```

---

## Wave 1: Foundation (3 agents in parallel)

All 3 can run simultaneously. No dependencies between them.

### Agent 1A: DOM Types (`crates/engine/src/dom/`)

Implement the core DOM tree data structures. These are plain Rust types — no Boa, no html5ever yet.

**Files:**
- `crates/engine/src/dom/mod.rs` — re-exports
- `crates/engine/src/dom/node.rs` — NodeId, NodeType enum, Node struct
- `crates/engine/src/dom/tree.rs` — DomTree (arena-based node storage, parent/child operations)
- `crates/engine/src/dom/element.rs` — Element-specific data (tag name, attributes)
- `crates/engine/src/dom/document.rs` — Document node, getElementById, createElement

**Types to implement:**

```rust
// node.rs
pub type NodeId = usize;

pub enum NodeData {
    Document,
    Element {
        tag_name: String,
        attributes: Vec<(String, String)>,
    },
    Text {
        content: String,
    },
    Comment {
        content: String,
    },
}

pub struct Node {
    pub id: NodeId,
    pub data: NodeData,
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
}
```

```rust
// tree.rs
pub struct DomTree {
    nodes: Vec<Node>,
}

impl DomTree {
    pub fn new() -> Self;
    pub fn create_document(&mut self) -> NodeId;
    pub fn create_element(&mut self, tag_name: &str) -> NodeId;
    pub fn create_text(&mut self, content: &str) -> NodeId;
    pub fn append_child(&mut self, parent: NodeId, child: NodeId);
    pub fn remove_child(&mut self, parent: NodeId, child: NodeId);
    pub fn get_node(&self, id: NodeId) -> &Node;
    pub fn get_node_mut(&mut self, id: NodeId) -> &mut Node;
    pub fn get_element_by_id(&self, id: &str) -> Option<NodeId>;
    pub fn get_elements_by_tag_name(&self, tag: &str) -> Vec<NodeId>;
    pub fn get_text_content(&self, node: NodeId) -> String;
    pub fn set_text_content(&mut self, node: NodeId, text: &str);
    pub fn document(&self) -> NodeId;
    // Find <body> element
    pub fn body(&self) -> Option<NodeId>;
    // Find <head> element
    pub fn head(&self) -> Option<NodeId>;
}
```

**Tests:** Unit tests for tree operations — create nodes, append, remove, get_element_by_id, text_content.

### Agent 1B: Wire Types (`crates/wire/src/`)

Define the command/response protocol. Keep it simple for the spike.

**File: `crates/wire/src/lib.rs`**

```rust
pub enum Command {
    Goto { url: String },
    Click { selector: String },
    Type { selector: String, text: String },
    Snap { mode: SnapMode },
    Back,
    Forward,
    Close,
}

pub enum SnapMode {
    Accessibility,
    Dom,
    Markdown,
}

pub enum Response {
    SessionCreated { session_id: String },
    Snapshot { content: String, url: String },
    Error { message: String },
}
```

**Tests:** Serde roundtrip tests.

### Agent 1C: CLI Skeleton (`crates/cli/src/`)

Set up clap arg parsing for the session-based interface. No actual implementation — just parse args and print what would happen.

**File: `crates/cli/src/main.rs`**

Parse these commands:
```
braille new
braille <sid> goto <url>
braille <sid> click <selector>
braille <sid> type <selector> <text>
braille <sid> snap [--mode=accessibility|dom|markdown]
braille <sid> back
braille <sid> forward
braille <sid> close
```

Use clap subcommands. For the spike, `new` and `goto` are the only ones that need to do anything real. The rest can print "not yet implemented".

**Tests:** None needed for spike — just verify arg parsing works.

---

## Wave 2: Parsing + Bindings + Serialization (3 agents in parallel)

All depend on Wave 1A (DOM types). Can run simultaneously with each other.

### Agent 2A: HTML Parser (`crates/engine/src/html/`)

Implement a `TreeSink` for html5ever that builds our `DomTree`.

**Files:**
- `crates/engine/src/html/mod.rs` — re-exports
- `crates/engine/src/html/parser.rs` — TreeSink implementation + `parse_html(html: &str) -> DomTree` function

**Key implementation details:**
- `TreeSink::Handle` = `NodeId`
- `create_element` → `tree.create_element(name)`
- `append` → `tree.append_child(parent, child)` (merge adjacent text nodes)
- `parse_html()` is the public API — takes an HTML string, returns a populated `DomTree`
- `elem_name` lifetime issue: use an approach that works with arena/index-based handles. Can store QualName directly on Node to avoid the borrow issue.
- Store the original `QualName` from html5ever on element nodes (not just a string tag name — may need to adjust Agent 1A's types to store `QualName` or store both)

**Tests:**
- Parse `<html><body><p>Hello</p></body></html>`, verify tree structure
- Parse `<div id="app"><span class="x">text</span></div>`, verify attributes
- Parse a page with a `<script>` tag, verify script content is a text child of the script element

### Agent 2B: Boa DOM Bindings (`crates/engine/src/js/`)

Wire our DomTree to Boa so JavaScript can manipulate it. For the spike, implement only what the test case needs.

**Files:**
- `crates/engine/src/js/mod.rs` — re-exports
- `crates/engine/src/js/runtime.rs` — JsRuntime struct (owns Boa Context + reference to DomTree)
- `crates/engine/src/js/bindings.rs` — Register `document` and `Element` with Boa

**Minimum DOM API surface for the spike:**
- `document.createElement(tagName)` → creates an element in DomTree, returns a JS Element object
- `document.getElementById(id)` → finds element in DomTree, returns a JS Element object or null
- `element.textContent` (getter/setter) → reads/writes text content on the DomTree node
- `element.appendChild(child)` → appends child node in DomTree

**Implementation approach:**
- DomTree is wrapped in `Rc<RefCell<DomTree>>` so both the parser and Boa bindings can access it
- Each JS Element object holds a `NodeId` pointing into the shared DomTree
- Use Boa's `Class` trait or `ObjectInitializer::with_native_data` for `document` and elements
- Register `document` as a global on the Boa Context

**Key Boa patterns to use:**
- `ObjectInitializer::with_native_data` for the `document` singleton
- Boa's `Class` trait for `Element` (so we can create instances from `createElement`)
- `NativeFunction::from_fn_ptr` for methods
- `ClassBuilder::accessor` for `textContent` getter/setter
- Native data structs must derive `Trace`, `Finalize`, `JsData`

**Tests:**
- Create a DomTree with a document + body, wire to Boa, eval `document.createElement("p")` — verify a new node exists in DomTree
- Eval `document.getElementById("app")` — verify it returns the right element
- Eval `el.textContent = "hello"` — verify DomTree node text content changed

### Agent 2C: Accessibility Tree Serializer (`crates/engine/src/a11y/`)

Walk a DomTree and produce a compact text representation.

**Files:**
- `crates/engine/src/a11y/mod.rs` — re-exports
- `crates/engine/src/a11y/serialize.rs` — `serialize_a11y(tree: &DomTree) -> String`

**Rules for the spike (simplified):**
- Skip `<head>`, `<script>`, `<style>`, `<meta>`, `<link>` elements
- Skip elements with no visible content
- Map elements to roles:
  - `<h1>`-`<h6>` → `heading` (include level)
  - `<p>` → `paragraph`
  - `<a>` → `link`
  - `<button>` → `button`
  - `<input>` → `input` (include type attribute)
  - `<img>` → `image` (include alt text)
  - `<ul>`, `<ol>` → `list`
  - `<li>` → `listitem`
  - `<div>`, `<span>`, `<section>`, `<main>`, `<article>` → container (omit from output if no semantic meaning, just recurse into children)
- Include text content in quotes after the role
- Assign stable element references: `@e1`, `@e2`, etc. for interactive elements
- Indent nested content

**Example output:**
```
heading[1] "Hello"
paragraph "Created by JavaScript"
```

**Tests:**
- Serialize a simple tree with h1, p, a, button — verify output format
- Verify script/style/head elements are excluded
- Verify interactive elements get `@e` references

---

## Wave 3: Integration (1 agent)

Depends on all of Wave 2. Wires everything together.

### Agent 3A: Script Execution + End-to-End Pipeline

**Files:**
- `crates/engine/src/lib.rs` — public `Engine` struct that orchestrates parse → bind → execute → serialize
- `crates/engine/src/js/executor.rs` — extract `<script>` tags from parsed DOM, execute their content via Boa in order

**Engine API:**

```rust
pub struct Engine {
    tree: Rc<RefCell<DomTree>>,
    runtime: JsRuntime,
}

impl Engine {
    pub fn new() -> Self;
    pub fn load_html(&mut self, html: &str);  // parse + execute scripts
    pub fn snapshot(&self, mode: SnapMode) -> String;  // serialize current DOM
}
```

**Script execution flow:**
1. `load_html` parses HTML via html5ever → DomTree
2. Walk the DomTree to find all `<script>` elements (in document order)
3. For each script, extract text content
4. Execute each script's content via Boa (which has access to the shared DomTree)
5. After all scripts run, DOM is in its final state

**Wire up CLI:**
- `braille new` → create an Engine instance, generate session ID, print it
- `braille <sid> goto <url>` → fetch URL with reqwest, pass HTML to `engine.load_html()`, print `engine.snapshot(Accessibility)`

**Integration test:**
```rust
#[test]
fn test_end_to_end() {
    let html = r#"
    <html><body>
      <h1>Hello</h1>
      <div id="app"></div>
      <script>
        let el = document.createElement("p");
        el.textContent = "Created by JavaScript";
        document.getElementById("app").appendChild(el);
      </script>
    </body></html>"#;

    let mut engine = Engine::new();
    engine.load_html(html);
    let snapshot = engine.snapshot(SnapMode::Accessibility);

    assert!(snapshot.contains("heading"));
    assert!(snapshot.contains("Hello"));
    assert!(snapshot.contains("paragraph"));
    assert!(snapshot.contains("Created by JavaScript"));
}
```

---

## Wave Summary

| Wave | Agents | What | Depends On |
|------|--------|------|------------|
| 0 | 1 | Scaffold workspace (`cargo check` passes) | nothing |
| 1 | 3 parallel | DOM types, Wire types, CLI skeleton | Wave 0 |
| 2 | 3 parallel | HTML parser, Boa bindings, A11y serializer | Wave 1A (DOM types) |
| 3 | 1 | Script execution + integration + end-to-end test | Wave 2 (all) |

**Total: 4 waves, 8 agent tasks, max 3 concurrent.**
