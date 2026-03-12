# Braille

A lightweight browser that maintains a virtual DOM with full JavaScript execution but skips graphical rendering entirely. Outputs structured text representations of pages for LLM agents to read and interact with.

A browser for those who read, not see.

**Spike:** [SPIKE.md](./SPIKE.md) — COMPLETE (46 tests, core loop proven)
**API Reference:** [REFERENCE.md](./REFERENCE.md) — Boa and html5ever API details

## Status

All 6 phases complete (770 tests). html5lib-tests tree-construction suite: **1778 passed, 0 failed, 0 ignored** out of 1778 test cases (**100% pass rate**). html5lib-tests serializer suite: **204 passed, 0 failed, 26 ignored** (core + optionaltags fully passing; options/injectmeta/whitespace skipped as non-default serializer config). Fixed foster parenting text merge (8 tests), template contents with DocumentFragment (112 tests), test harness trailing newline (1 test), annotation-xml integration point polyfill (4 tests), and selectedcontent cloning polyfill (4 tests). Two polyfills in parser.rs are marked `POLYFILL` for removal when html5ever handles them internally: `is_mathml_annotation_xml_integration_point` flag storage and `polyfill_selectedcontent` post-processing (workaround for html5ever issue #712). The engine has a full DOM API surface (~70 methods), CSS cascade with selector matching wired into the load pipeline, full event system (addEventListener/dispatchEvent with capture/bubble/at-target), getComputedStyle, HTMLElement-specific properties (input.value/checked/type/disabled, select.value/selectedIndex/options, option.value/selected/text, a.href, form.action/method/elements, element.dataset/hidden/tabIndex/title/lang/dir, focus/blur/click stubs, getBoundingClientRect stub), and JS bindings for querySelector, innerHTML, classList, element.style, node mutation, window/console, and more. CLI has all commands routed through session manager, network client with cookie jar, navigation history, and external script loading. Full integration smoke tests (20) and CSS edge case tests (32) verify end-to-end behavior.

**WPT Phase 1 — Steps 1-3 DONE, Step 4 IN PROGRESS.** **31 WPT tests passing** out of 263 files (228 ignored, 4 failing). NodeId→JsObject cache added for DOM object identity (`el.parentNode === el.parentNode` now returns `true`), which un-blocked Event-dispatch-propagation-stopped. Previous fixes: JsCustomEvent dual-type dispatch support (element + document), AT_TARGET capture-first ordering, cancelBubble pre-check + reset after dispatch, dispatching flag blocks initEvent, TypeError for missing Event constructor/initEvent args, timeStamp returns 1 (>0), initCustomEvent/initEvent on CustomEvent, propagation flag reset after dispatch, srcElement own property. 4 remaining failures: Event-isTrusted (needs isTrusted as own accessor property), Element-closest (CSS selector gaps), Element-classlist (classList edge cases), Node-isConnected (iframe only). All prior tests still pass.

### What exists (770 unit/integration + 1778 tree-construction + 204 serializer = 2752 tests, all passing)

| Component | Status | What works |
|-----------|--------|------------|
| DOM tree | Arena-based, full ops | createElement, appendChild, removeChild, insertBefore, replaceChild, cloneNode, getElementById, getElementsByTagName, querySelector/All, textContent, innerHTML, attribute CRUD, class list, node traversal. Nodes carry namespace (svg/math/"") and Doctype variant. |
| HTML parser | html5ever TreeSink, 100% html5lib-tests (1778/1778 tree-construction, 204/204 serializer) | Full spec-compliant HTML parsing into DomTree, fragment parsing for innerHTML setter and html5lib fragment tests. Stores element namespace (SVG/MathML/HTML), doctype nodes (name/public_id/system_id), namespaced attribute prefixes (xlink/xml/xmlns). Supports scripting on/off flag. Template elements have proper content DocumentFragment. Foster parenting text merge in `append_before_sibling`. Two polyfills (grep `POLYFILL`): annotation-xml integration point flag storage, selectedcontent post-parse cloning (html5ever #712). Token-stream serializer test harness validates attribute quoting, text escaping, void elements, DOCTYPE serialization, and all HTML optional tag omission rules. |
| JS engine | Boa bindings (~70 methods), NodeId→JsObject cache | document: createElement, getElementById, querySelector/All, getElementsByClassName/TagName, createTextNode, body, head, title. element: appendChild, textContent, classList, getAttribute/setAttribute/removeAttribute, parentNode, children, firstChild, lastChild, siblings, nodeType/nodeName/tagName, innerHTML/outerHTML, insertAdjacentHTML, insertBefore, replaceChild, cloneNode, element.style, querySelector/All, getElementsByClassName/TagName. input: value, checked, type, disabled, name, placeholder. select: value, selectedIndex, options. option: value, selected, text. anchor: href. form: action, method, elements. element: hidden, dataset, tabIndex, title, lang, dir, getBoundingClientRect (stub), focus/blur (stubs), click (dispatches event). **Object identity**: thread-local `NODE_CACHE` ensures `el.parentNode === el.parentNode` (same JsObject for same NodeId). |
| CSS cascade | Parsing + matching + cascade + computed + wired + JS | cssparser stylesheet/inline parsing, selectors Element trait impl, selector matching (tag, class, id, attribute, pseudo-classes), cascade algorithm (origin, importance, specificity, source order), computed style resolution (inherit/initial/unset, em→px, color names), style tree DFS walk, compute_all_styles called in load_html/execute_scripts, getComputedStyle(el) JS binding with camelCase property accessors |
| Event system | Full W3C dispatch | Event/CustomEvent constructors, addEventListener/removeEventListener (capture, once options), dispatchEvent with capture/bubble/at-target phases, stopPropagation, stopImmediatePropagation, preventDefault |
| A11y serializer | Roles + values + CSS | headings, paragraphs, links, buttons, inputs (with value display), selects (with selected option), lists, images, nav, main, form; interactive refs (@e1); display:none skips element+descendants, visibility:hidden suppresses text but keeps structure |
| Wire protocol | serde types | Command/Response/SnapMode/Select/Focus/NavigateRequest/EngineAction enums |
| CLI | Fully wired | `new`, `goto` (live fetch + render), `click`/`type`/`select`/`focus`/`snap`/`back`/`forward`/`close` all routed through session manager, network client with cookie jar + URL resolution, navigation history, clear error messages |
| Engine | Integration + scripts + styles | `load_html` (parse + execute scripts + compute styles), `snapshot` (a11y mode), `parse_and_collect_scripts`/`execute_scripts` for external script loading, window/console globals, 32 end-to-end integration tests |

### What doesn't exist yet

| Component | Gap |
|-----------|-----|
| WPT harness | Phase 1 Step 4 done (31/263 passing, 4 failing, 228 ignored). **Phase 2 planned** — 29 agents across 6 waves to implement missing DOM APIs and un-ignore ~135 tests |
| Layout | Not started. Taffy integration, real getBoundingClientRect, offsetWidth/Height |
| WASM sandbox | Not started — engine runs in-process |

## Implementation Plan

Three directions, all running. Shared dependencies noted. **51 agent tasks total across all three directions.**

Cross-direction dependencies:
- **C → B:** Direction C's selector matching (Agent C-1B) produces `query_selector`/`query_selector_all` that Direction B needs for `querySelector` JS binding (Agent B-2A). Run C Wave 0-1 before or alongside B Wave 2.
- **A → B:** Direction A's external script loading (Agent A-2C) benefits from Direction A's network client (Agent A-2A).
- **B → A:** Direction A's click/type/select commands need Direction B's attribute accessors. Direction A Wave 0B covers this but Direction B Agent 1B duplicates — merge or share.

### Detailed Implementation Blueprints

The planning agents produced detailed blueprints with API signatures, design rationale, test expectations, and merge strategies:

- **[IMPL_SESSIONS.md](./IMPL_SESSIONS.md)** — Direction A: daemon architecture, click/type/select/focus semantics, form data collection, navigation history, cookie jar, external script two-phase loading
- **[IMPL_DOM_API.md](./IMPL_DOM_API.md)** — Direction B: `register_on_class` pattern, classList/style/dataset object designs, event propagation algorithm, input.value vs attribute distinction, merge conflict mitigation
- **[IMPL_CSS_CASCADE.md](./IMPL_CSS_CASCADE.md)** — Direction C: `DomElement<'a>` wrapper, SelectorImpl trait types, cascade ordering (origin+importance+specificity), ComputedStyle struct, UA stylesheet, unit resolution, restyle strategy

These contain critical details not in the tables below.

### Direction A: Sessions + Interaction (18 agents, 4 waves, max 6 concurrent)

Make `braille` a working tool: persistent sessions, click/type/select/focus, external scripts, cookies, navigation history.

**Wave A-0: Foundations (5 agents, all parallel)**

| Agent | What | Files | Tests |
|-------|------|-------|-------|
| A-0A | Ref map — a11y serializer returns `HashMap<String, NodeId>`, Engine stores it, `resolve_ref("@e1")` | `a11y/serialize.rs`, `lib.rs` | Resolve @e1→NodeId, @e99→None |
| A-0B | Attribute accessors — `get_attribute`, `set_attribute`, `remove_attribute`, `has_attribute` on DomTree | `dom/tree.rs` | CRUD on attributes |
| A-0C | Wire protocol expansion — `Select`, `Focus` commands, `NavigateRequest`, `EngineAction` enum | `wire/src/lib.rs` | Serde roundtrips |
| A-0D | Element finder — resolve `@eN` refs, `#id` shorthand, tag name fallback to NodeId | `dom/find.rs` | Each resolution strategy |
| A-0E | Traversal helpers — `find_ancestor(tag)`, `find_descendants_by_tag`, `get_parent` | `dom/tree.rs` | Ancestor search, descendant collection |

**Wave A-1: Core Commands (6 agents, all parallel)**

| Agent | What | Depends |
|-------|------|---------|
| A-1A | Session manager — daemon on Unix socket, `HashMap<SessionId, Session>`, auto-start | A-0C |
| A-1B | Click: links — read `href`, return `EngineAction::Navigate` | A-0A, A-0B, A-0D |
| A-1C | Click: forms — `find_ancestor("form")`, collect inputs, build `NavigateRequest` (GET/POST) | A-0B, A-0E |
| A-1D | Type command — set `value` attr on `<input>`, text content on `<textarea>` | A-0B, A-0D |
| A-1E | Select command — find matching `<option>`, set `selected` attr | A-0B, A-0E |
| A-1F | Focus command — `focused_element: Option<NodeId>` on Engine, `[focused]` in a11y output | A-0A, A-0D |

**Wave A-2: Network + Scripts (4 agents, 3 parallel + 1 sequential)**

| Agent | What | Depends |
|-------|------|---------|
| A-2A | Network client — per-session cookie jar (reqwest cookies feature), redirect following, URL resolution | A-1A |
| A-2B | Navigation history — `Vec<String>` + index, back/forward re-fetch + load | A-1A |
| A-2C | External `<script src>` loading — two-phase: `parse_and_find_scripts` → CLI fetches → `execute_with_scripts` | A-2A |
| A-2D | A11y value display — show `value="..."` for inputs, selected option for selects | A-0B, A-1D |

**Wave A-3: Integration (3 agents, all parallel)**

| Agent | What |
|-------|------|
| A-3A | CLI wiring — all commands through daemon/session, goto/click/type/select/focus/snap/back/forward/close |
| A-3B | End-to-end integration tests — link click flow, form submission, select+submit, script execution+click |
| A-3C | Error hardening — clear panic messages for bad refs, non-input type targets, invalid sessions, network failures |

---

### Direction B: DOM API Surface (19 agents, 6 waves, max 8 concurrent)

Expand from 4 JS-accessible DOM methods to ~50+. Each agent implements both the Rust DomTree method and the Boa JS binding.

**Wave B-0: Structural Prep (1 agent)**

| Agent | What |
|-------|------|
| B-0A | Restructure `bindings.rs` into `bindings/` module directory. Each cluster gets its own file. `register_on_class(class)` pattern so `JsElement::init` only adds one line per cluster. |

**Wave B-1: Core Infrastructure (5 agents, all parallel)**

| Agent | What | Key APIs |
|-------|------|----------|
| B-1A | Node traversal | `parentNode`, `parentElement`, `children`, `childNodes`, `firstChild`, `lastChild`, `nextSibling`, `previousSibling`, `nextElementSibling`, `previousElementSibling`, `hasChildNodes`, `contains` |
| B-1B | Attributes | `getAttribute`, `setAttribute`, `removeAttribute`, `hasAttribute`, `element.id`, `element.className` |
| B-1C | Node info | `nodeType`, `nodeName`, `tagName`, `nodeValue`, `innerText` |
| B-1D | classList | `add`, `remove`, `toggle`, `contains`, `item`, `length` — JsClassList class backed by class attribute |
| B-1E | Document methods | `document.body`, `document.head`, `document.title`, `document.createTextNode`, `document.createDocumentFragment` (new `NodeData::DocumentFragment` variant) |

**Wave B-2: Querying + Mutation (5 agents, all parallel)**

| Agent | What | Key APIs |
|-------|------|----------|
| B-2A | querySelector | `querySelector`, `querySelectorAll`, `getElementsByClassName`, `getElementsByTagName` on Element — uses `selectors` crate, implements `selectors::Element` trait (**shared with Direction C**) |
| B-2B | Node mutation | `insertBefore`, `replaceChild`, `removeChild` (JS binding), `cloneNode(deep)` |
| B-2C | innerHTML | `innerHTML` get/set, `outerHTML` get, `insertAdjacentHTML`, `insertAdjacentElement` — HTML serializer + `parse_fragment` for setter |
| B-2D | element.style | `style.color = "red"`, `style.setProperty`, `style.removeProperty`, `style.cssText`, `style.getPropertyValue` — JsStyleDeclaration class |
| B-2E | Window + console | `window` global (self-referential), `window.document`, `window.location` (href/pathname/etc), `setTimeout`/`clearTimeout`/`setInterval`/`clearInterval` (stub — store callbacks), `console.log`/`warn`/`error` (buffer, no stdout) |

**Wave B-3: Events (3 agents, 2 parallel then 1)**

| Agent | What | Depends |
|-------|------|---------|
| B-3A | Event constructors | `new Event(type, options)`, `new CustomEvent(type, {detail})`, properties: `type`, `bubbles`, `cancelable`, `defaultPrevented`, `target`, `currentTarget`, `preventDefault()`, `stopPropagation()` | B-0 |
| B-3B | Listener registration | `addEventListener(type, fn, options)`, `removeEventListener` — listeners stored on JsRuntime (`HashMap<NodeId, Vec<ListenerEntry>>`) | B-3A |
| B-3C | Event dispatch | `dispatchEvent(event)` — capture phase (top-down), at-target, bubble phase (bottom-up), `stopPropagation`, `stopImmediatePropagation`, `once` removal | B-3A, B-3B |

**Wave B-4: HTMLElement Specifics (4 agents, all parallel)**

| Agent | What | Key APIs |
|-------|------|----------|
| B-4A | Input properties | `input.value` (live vs attribute), `input.checked`, `input.type`, `input.disabled`, `input.name` — separate property storage from attributes |
| B-4B | Select/Option | `select.value`, `select.selectedIndex`, `select.options`, `option.value`, `option.selected`, `option.text` |
| B-4C | Anchor/Form/Dataset | `a.href`, `form.submit()`, `form.action`, `form.method`, `element.hidden`, `element.dataset` (Proxy over data-* attributes) |
| B-4D | Common HTMLElement | `tabIndex`, `title`, `lang`, `dir`, `getBoundingClientRect()` (stub: returns zeros), `focus()`, `blur()`, `click()` (stub) |

**Wave B-5: Integration (1 agent)**

| Agent | What |
|-------|------|
| B-5A | Integration + smoke tests — React-like reconciler simulation, Svelte-like direct DOM manipulation, event round-trips, full API surface exercise |

---

### Direction C: CSS Cascade (14 agents, 6 waves, max 3 concurrent)

Full CSS cascade — the jsdom killer feature. Built on Servo's `cssparser` + `selectors`.

**Wave C-0: Foundation Types (3 agents, all parallel)**

| Agent | What | Files |
|-------|------|-------|
| C-0A | Property system — `PropertyId` enum (~50 properties), `CssValue` types (Length, Color, Keyword, etc.), inheritance flags, initial values, shorthand expansion | `css/properties.rs`, `css/values.rs` |
| C-0B | SelectorImpl — `BrailleSelectorImpl` for the `selectors` crate, pseudo-class/pseudo-element enums, `BrailleSelectorParser` | `css/selector_impl.rs` |
| C-0C | DOM modifications — `computed_style` slot on Node, traversal helpers for selector matching: `parent_element`, `prev_sibling_element`, `next_sibling_element`, `is_root_element` | `dom/node.rs`, `dom/tree.rs` |

**Wave C-1: Parsing + Matching (3 agents, all parallel)**

| Agent | What | Depends |
|-------|------|---------|
| C-1A | CSS parser — `parse_stylesheet(css) → Stylesheet`, `parse_inline_style(attr) → Vec<Declaration>`, implements cssparser's `DeclarationParser` + `QualifiedRuleParser` | C-0A, C-0B |
| C-1B | Element trait — `selectors::Element` impl on `DomElement<'a>` wrapper, `query_selector`/`query_selector_all` functions (**shared with B-2A**) | C-0B, C-0C |
| C-1C | Stylesheet collection — walk DOM for `<style>` + inline `style=""`, hardcoded UA stylesheet (display:block for div/p/h1-h6, display:none for head/script/style, etc.) | C-0A, C-1A |

**Wave C-2: Cascade (2 agents, all parallel)**

| Agent | What | Depends |
|-------|------|---------|
| C-2A | Cascade algorithm — collect matching rules per element, sort by (origin, importance, specificity, source order), produce `CascadedValues` | C-1A, C-1B, C-1C |
| C-2B | Computed style resolution — `resolve_style(cascaded, parent_style) → ComputedStyle`, handle `inherit`/`initial`/`unset`, em→px, percentage resolution, color name lookup | C-0A, C-2A |

**Wave C-3: Integration (3 agents, all parallel)**

| Agent | What | Depends |
|-------|------|---------|
| C-3A | Style tree orchestration — DFS walk computing styles top-down (parent before child for inheritance), `compute_all_styles(tree, collection)` | C-2A, C-2B |
| C-3B | A11y integration — `display:none` → skip element+descendants, `visibility:hidden` → keep structure but hide text | C-3A |
| C-3C | Engine wiring — call `collect_styles` + `compute_all_styles` in `Engine::load_html` after script execution | C-3A, C-3B |

**Wave C-4: JS Bindings (2 agents, all parallel)**

| Agent | What |
|-------|------|
| C-4A | `getComputedStyle(el)` — returns read-only `JsCSSStyleDeclaration`, `getPropertyValue(name)`, camelCase property getters |
| C-4B | `element.style` — mutable `JsCSSStyleDeclaration`, `setProperty`/`removeProperty`, camelCase setters, triggers restyle |

**Wave C-5: Edge Cases (1 agent)**

| Agent | What |
|-------|------|
| C-5A | Integration tests — full cascade with UA+author+inline+!important, inheritance chains, display:none in a11y, getComputedStyle after JS mutation, specificity edge cases |

---

### Execution Strategy

Run all three directions concurrently where dependencies allow. Recommended interleaving:

| Phase | What runs | Agents | Status |
|-------|-----------|--------|--------|
| 1 | A Wave 0 + B Wave 0 + C Wave 0 | 9 parallel | DONE (319 tests) |
| 2 | A Wave 1 + B Wave 1 + C Wave 1 | 14 parallel (peak) | DONE (319 tests) |
| 3 | A Wave 2 + B Wave 2 + C Wave 2 | 11 parallel | DONE (539 tests) |
| 4 | A Wave 3 + B Wave 3 + C Wave 3 | 9 parallel | DONE (650 tests) |
| 5 | B Wave 4 + C Wave 4 | 5 agents (C-4B skipped — covered by B-2D) | DONE (718 tests) |
| 6 | B Wave 5 + C Wave 5 | 2 parallel | DONE (770 tests) |

**Total: 51 agent tasks. Peak concurrency: 14 agents. 6 phases. ALL PHASES COMPLETE. 770 unit/integration tests + 1778 tree-construction + 204 serializer = 2752 tests passing.**

---

### WPT Phase 1: DOM Conformance (4 steps)

Validate DOM implementation against Web Platform Tests (`dom/nodes/`, `dom/events/`).

| Step | What | Status |
|------|------|--------|
| 1 | WPT submodule setup — sparse checkout of `resources`, `dom/nodes`, `dom/events` | DONE |
| 2 | DOM API gaps — 6 batches of missing APIs needed by WPT tests | DONE |
| 3 | WPT test harness — `wpt_dom.rs` using libtest-mimic, minimal preamble shim, result extraction via `window.__wpt_results` | DONE |
| 4 | Run and triage — execute WPT tests, build skip list, fix failures | IN PROGRESS — 31 passing, 4 failing, 228 ignored |

**Step 2 details (all 6 batches DONE):**
- Batch 1: `document.documentElement`, `createComment`, `createDocumentFragment`, `createEvent`
- Batch 2: `node.ownerDocument`, `node.isConnected`, Node type constants (ELEMENT_NODE..DOCUMENT_FRAGMENT_NODE)
- Batch 3: `element.remove()`, `contains()`, `matches(selector)`, `closest(selector)`
- Batch 4: `firstElementChild`, `lastElementChild`, `nextElementSibling`, `previousElementSibling`, `childElementCount`
- Batch 5: Event properties — `isTrusted`, `timeStamp`, `composed`, `srcElement`, `cancelBubble`, `returnValue`, `initEvent()`
- Batch 6: `NodeData::DocumentFragment` variant + handling in appendChild, insertBefore, all match arms

**Step 3 design:**
- Create `crates/engine/tests/wpt_dom.rs` using libtest-mimic
- Add `[[test]] name = "wpt_dom" harness = false` to `crates/engine/Cargo.toml`
- For each `.html` test file: replace `<script src="/resources/testharness.js">` with inline testharness.js, replace testharnessreport.js with shim that writes results to `window.__wpt_results`
- Shim: `add_completion_callback(function(tests, status) { window.__wpt_results = tests.map(function(t) { return { name: t.name, status: t.status, message: t.message }; }); });`
- Status codes: 0=PASS, 1=FAIL, 2=TIMEOUT, 3=NOTRUN
- Skip tests needing iframes, workers, Range API, Shadow DOM, etc.

### WPT Phase 2: Expanding DOM Conformance (29 agents, 6 waves)

Implement missing DOM APIs to un-ignore ~135 of the 228 ignored WPT tests. Each wave is a group of independent agents that run in parallel. After each wave: remove corresponding skip patterns from `wpt_dom.rs`, run tests, triage new failures.

**Execution phases:**

| Phase | Waves | Agents | Why sequential |
|-------|-------|--------|----------------|
| A | Wave 1 | 8 parallel | Foundation — methods that later waves' tests use as setup |
| B | Wave 2 | 6 parallel | Full-spec compliance — builds on Wave 1 primitives |
| C | Waves 3+4+5+6 | 15 parallel | All independent — queries, namespaces, document APIs, events |

Total: 29 agents, 3 sequential phases, peak concurrency 15.

**Permanently deferred** (~50 tests, architectural limitations or disproportionate effort):
- Iframes / cross-document / browsing contexts
- Shadow DOM
- MutationObserver (12 tests — Phase 3 candidate)
- Range / Selection / TreeWalker / NodeIterator
- Workers
- Server-side substitution (`.sub.` tests)
- Activation behavior (checkbox/radio click, form submission, label activation)
- `window.event` / `window.onerror`
- FocusEvent / PointerEvent / AnimationEvent / TransitionEvent / KeyEvent
- Custom elements / `Symbol.unscopables`
- XML / XHTML / CDATASection / DOMParser

**Wave 1: Core DOM Data Methods (8 agents, all parallel)**

All independent, no cross-dependencies. Each agent adds Rust DomTree methods + JS bindings + removes skip patterns + runs affected tests.

| Agent | What | Target Tests | Key APIs |
|-------|------|-------------|----------|
| W1-A | CharacterData interface | CharacterData-{appendData,deleteData,insertData,replaceData,substringData,data,appendChild,remove,surrogates}.html (9) | `data` get/set, `length`, `appendData()`, `deleteData()`, `insertData()`, `replaceData()`, `substringData()` on Text+Comment. IndexSizeError for bad offsets. UTF-16 code unit semantics. |
| W1-B | ChildNode mixin | ChildNode-{after,before,replaceWith}.html (3) | `before(...nodes)`, `after(...nodes)`, `replaceWith(...nodes)` — variadic, accept Node and string (strings become Text nodes). Works on Element, Text, Comment. |
| W1-C | ParentNode mixin | ParentNode-{append,prepend,replaceChildren}.html, {append,prepend}-on-Document.html (5) | `append(...nodes)`, `prepend(...nodes)`, `replaceChildren(...nodes)` — variadic, accept Node and string. Works on Element, Document, DocumentFragment. |
| W1-D | Node comparison | Node-{isEqualNode,isSameNode,compareDocumentPosition}.html (3) | `isEqualNode(other)` — deep equality. `isSameNode(other)` — reference identity. `compareDocumentPosition(other)` — returns bitmask + 6 `DOCUMENT_POSITION_*` constants on Node. |
| W1-E | Text node methods | Text-{splitText,wholeText}.html (2) | `splitText(offset)` — split text node, insert new node as next sibling. `wholeText` getter — concatenated data of all logically adjacent Text nodes. IndexSizeError for bad offset. |
| W1-F | insertAdjacentElement/Text | Element-{insertAdjacentElement,insertAdjacentText}.html, insert-adjacent.html (3) | `insertAdjacentElement(pos, el)` — returns el. `insertAdjacentText(pos, text)` — creates Text node. Positions: beforebegin/afterbegin/beforeend/afterend. SyntaxError for invalid pos. |
| W1-G | Node.normalize + getRootNode | Node-normalize.html, rootNode.html (2) | `normalize()` — merge adjacent Text nodes, remove empty Text nodes. `getRootNode({composed})` — walk parent chain to root (composed crosses shadow boundaries, but Shadow DOM deferred). |
| W1-H | Node/Text/Comment constructors | Node-constants.html, Text-constructor.html, Comment-constructor.html, Document-{createComment,createTextNode}.html (5) | Node type constants on Node constructor (`Node.ELEMENT_NODE=1`, `TEXT_NODE=3`, etc.). `new Text(data)`, `new Comment(data)` as global constructors. Ensure createComment/createTextNode return proper typed objects. |

**Wave 1 targets: 32 test files. Expected: ~20-25 passing (focused tests likely to fully pass), remainder reveal sub-test gaps.**

**Wave 2: Node Full-Spec Compliance (6 agents, all parallel, after Wave 1)**

| Agent | What | Target Tests | Key APIs |
|-------|------|-------------|----------|
| W2-A | textContent + nodeName + nodeValue full spec | Node-{textContent,nodeName,nodeValue}.html (3) | Full spec for all node types: Document→null, Doctype→null, DocumentFragment→concat children, Comment→data, Text→data. nodeValue setter on Text/Comment. nodeName: Document→"#document", DocumentFragment→"#document-fragment", Comment→"#comment", Text→"#text". |
| W2-B | cloneNode full spec | Node-cloneNode*.html (2-7) | Deep clone all node types including attributes, namespace, doctype info. Clone template contents. Some sub-tests need DOMImplementation (partial pass expected). |
| W2-C | contains + parentNode + parentElement | Node-{contains,parentNode,parentElement}.html (3) | Full spec edge cases — Document as parent, doctype nodes, detached trees. `parentElement` returns null when parent is Document. `contains(null)` returns false. |
| W2-D | Node mutation full spec | Node-{appendChild,insertBefore,removeChild,replaceChild}.html (4) | Pre-insertion validation: HierarchyRequestError for invalid parent/child combos (e.g. Element under another Element when parent is Document). DocumentFragment children transfer. Doctype insertion constraints. |
| W2-E | Node-properties + Element.remove | Node-properties.html, Element-remove.html (2) | Comprehensive property tests across all node types (needs most Wave 1 APIs). `Element.prototype.remove()` — ChildNode mixin on Element. |
| W2-F | HTMLCollection + NodeList | Element-children.html, ParentNode-children.html, Node-childNodes.html, NodeList-*.html (6-8) | `HTMLCollection` — live, element-only, `length`+`item()`+bracket access. `NodeList` — live for `childNodes`, static for `querySelectorAll`. Iterable (`forEach`/`keys`/`values`/`entries`). |

**Wave 2 targets: 20-27 test files. Expected: ~10-15 passing (complex tests with cross-feature dependencies).**

**Waves 3-6 can all run in parallel (Phase C). No inter-wave dependencies.**

**Wave 3: Query & Selector APIs (4 agents, all parallel)**

| Agent | What | Target Tests | Key APIs |
|-------|------|-------------|----------|
| W3-A | querySelector spec fixes | ParentNode-querySelector-All.html, -scope.html, -exclusive.html, -removed-elements.html, -space-and-dash-attribute-value.html, DocumentFragment-querySelectorAll-after-modification.html, query-target-in-load-event.html (7) | Fix: exclude root element from results (root's descendants only). Add `:scope` pseudo-class (matches the context element). Verify static NodeList behavior. |
| W3-B | CSS selector edge cases | ParentNode-querySelector-{case-insensitive,escapes,namespaces}.html, querySelector-mixed-case.html (4) | Attribute selector case flags (`[attr=val i]` / `[attr=val s]`). CSS escape sequences in selectors. Namespace-aware attribute case sensitivity (HTML=insensitive, SVG=sensitive). |
| W3-C | getElementsByClassName | Document/Element-getElementsByClassName.html, getElementsByClassName-{32,empty-set,whitespace-class-names}.html (5) | Full spec: multiple class names (space-separated), whitespace handling, live HTMLCollection, case-sensitive matching. |
| W3-D | getElementsByTagName + matches/closest fixes | Document/Element-getElementsByTagName.html, Element-{matches,closest}.html, case.html (5) | Full spec: live HTMLCollection, wildcard `*`, HTML case-insensitive / non-HTML case-sensitive. Fix `closest()` edge cases. Fix `matches()` edge cases. **Fixes currently-failing Element-closest.html.** |

**Wave 3 targets: 21 test files + 1 currently-failing fix.**

**Wave 4: Namespace & Attribute APIs (3 agents, all parallel)**

| Agent | What | Target Tests | Key APIs |
|-------|------|-------------|----------|
| W4-A | Namespace attribute methods | Element-{hasAttribute,hasAttributes,setAttribute,removeAttribute,removeAttributeNS,firstElementChild-namespace,setAttribute-crbug}.html (7) | `setAttributeNS(ns, qname, val)`, `getAttributeNS(ns, localName)`, `hasAttributeNS(ns, localName)`, `removeAttributeNS(ns, localName)`. Namespace + prefix handling on Attribute. Also fix classList validation edge cases. **Fixes currently-failing Element-classlist.html.** |
| W4-B | Namespace element creation | Document-{createElementNS,createElement-namespace,createElement}.html, Element-tagName.html (3-4) | `document.createElementNS(ns, qualifiedName)`. Namespace validation (InvalidCharacterError, NamespaceError). Prefix handling. `tagName` returns qualified name with correct case per namespace. |
| W4-C | NamedNodeMap + Attr interface | attributes-namednodemap.html, attributes.html, Document-createAttribute.html (3) | `Element.attributes` returns NamedNodeMap with `getNamedItem(name)`, `setNamedItem(attr)`, `removeNamedItem(name)`, `item(index)`, `length`. Attr node: `name`, `value`, `namespaceURI`, `prefix`, `localName`, `ownerElement`. `document.createAttribute(name)`. |

**Wave 4 targets: 14 test files + 1 currently-failing fix.**

**Wave 5: Document APIs & DOMImplementation (4 agents, all parallel)**

| Agent | What | Target Tests | Key APIs |
|-------|------|-------------|----------|
| W5-A | DOMImplementation | DOMImplementation-{createDocument,createHTMLDocument,createDocumentType,hasFeature,*crash}.html, Document-implementation.html (8) | `document.implementation` object. `createHTMLDocument(title)` — returns new Document with doctype+html+head+title+body. `createDocument(ns, qname, doctype)` — returns XMLDocument. `createDocumentType(qname, publicId, systemId)`. `hasFeature()` — always returns true. |
| W5-B | Document metadata | Document-{URL,doctype,getElementById,characterSet-*}.html, Node-baseURI.html (5-6) | `Document.URL` (getter, default "about:blank"). `Document.doctype` (returns first Doctype child or null). `Document.characterSet` (returns "UTF-8"). `Node.baseURI` (returns document URL). Full `getElementById` spec (first in tree order, dynamic id changes). |
| W5-C | Document constructors + adoptNode | Document-constructor.html, DocumentFragment-{constructor,getElementById}.html, Document-{adoptNode,importNode}.html (5) | `new Document()` global constructor. `new DocumentFragment()` global constructor. `document.adoptNode(node)` — change ownerDocument, remove from old parent. `document.importNode(node, deep)` — clone into this document. |
| W5-D | DocumentType interface | DocumentType-{literal,remove}.html, DOMTokenList-coverage.html (2-3) | DocumentType: `name`, `publicId`, `systemId` as JS-visible properties. `DocumentType.remove()` (ChildNode mixin). DOMTokenList: `value` property, `replace()`, `supports()`, `toString()`, validation (SyntaxError for empty/whitespace tokens). |

**Wave 5 targets: 20-22 test files.**

**Wave 6: Event System Enhancements (4 agents, all parallel)**

| Agent | What | Target Tests | Key APIs |
|-------|------|-------------|----------|
| W6-A | Event dispatch edge cases | Event-dispatch-{target-moved,target-removed,handlers-changed,reenter,multiple-cancelBubble,multiple-stopPropagation}.html, Event-{propagation,stopPropagation-cancel-bubbling,stopImmediatePropagation}.html, remove-all-listeners.html (10) | Snapshot event propagation path at dispatch start (target moving/removal doesn't change path). Snapshot listener list per-node before invoking. Support re-entrant dispatch (dispatch inside listener). Fix propagation flag edge cases across multiple dispatches. |
| W6-B | EventTarget constructor + isTrusted | EventTarget-{constructible,addEventListener,add-remove-listener,removeEventListener,dispatchEvent,add-listener-platform-object}.{any.js,html}, AddEventListenerOptions-{once,passive}.any.js, EventListenerOptions-capture.html (9) | Standalone `new EventTarget()` constructor — no DOM node, just event support. Full `once`/`passive`/`capture` option handling. `isTrusted` as unforgeable own accessor property (not prototype property). **Fixes currently-failing Event-isTrusted.any.js.** |
| W6-C | Event subclasses + createEvent | Event-subclasses-constructors.html, Document-createEvent{,.https}.html, Event-dispatch-{bubbles-true,bubbles-false}.html (5) | UIEvent, MouseEvent, KeyboardEvent, FocusEvent, WheelEvent, CompositionEvent constructors with proper property defaults. `document.createEvent(interface)` — case-insensitive alias matching for all legacy event types. |
| W6-D | EventTarget this + timestamp | EventTarget-this-of-listener.html, Event-dispatch-omitted-capture.html, Event-timestamp-*.html (4-5) | Proper `this` binding in function listeners (bound to currentTarget). handleEvent protocol (listener object with `handleEvent()` method, re-looked-up each dispatch). `event.timeStamp` returns `DOMHighResTimeStamp` (monotonic ms, use `performance.now()` stub). |

**Wave 6 targets: 28-29 test files + 1 currently-failing fix.**

**WPT Phase 2 summary:**

| Metric | Count |
|--------|-------|
| Total agents | 29 |
| Total waves | 6 |
| Sequential phases | 3 (A→B→C) |
| Peak concurrency | 15 (Phase C) |
| Test files targeted | ~135 |
| Currently-failing tests fixed | 3 (Element-closest, Element-classlist, Event-isTrusted) |
| Expected new passing tests | 60-100 (many tests have sub-tests needing cross-wave features) |
| Estimated final passing | ~95-135 / 263 (36-51%) |
| Permanently deferred | ~50 tests |

## Core Thesis

LLMs don't need pixels — they need text. The DOM is already a text structure. The graphical rendering pipeline (layout, paint, compositing, GPU) is the expensive part of a browser, and agents don't need any of it. An LLM can look at raw DOM with inline styles and understand the visual intent: `display: none` means hidden, `position: fixed; top: 0` means sticky header.

## Requirements

- Full JavaScript execution against a live DOM
- Agents can navigate, click links, fill forms, follow redirects
- Handles modern SPA frameworks (React, SvelteKit at minimum)
- CSS changes from JS are reflected in the DOM text output
- Lightweight — the whole point is avoiding headless Chrome overhead
- Structured text output designed for LLM consumption
- WASM sandboxing — untrusted page JS executes inside a sandboxed module
- Distributed as a single CLI binary agents can `brew install`

## Stack

### Language: Rust
- Entire engine written in Rust — DOM, CSS cascade, layout, all in one codebase
- Compiles to native binary for CLI distribution
- The engine compiles to WASM for sandboxed execution of untrusted page JS
- No FFI boundaries within the engine — DOM and JS engine are both Rust, same memory model
- Single binary output, no runtime dependencies

### JS Engine: Boa
- Pure Rust JavaScript engine — no C dependencies, no language bridging
- 94.12% Test262 conformance (v0.21), actively improving
- Embeds directly into the Rust codebase — DOM calls from page JS are just Rust function calls, no FFI overhead
- Compiles to WASM natively alongside the DOM implementation
- Risk: younger than V8/JSC/QuickJS, may hit spec gaps on real-world sites. Mitigated by active development and our ability to contribute upstream.

### Servo Crates (dependencies)

| Crate | Purpose | License |
|-------|---------|---------|
| **html5ever** | HTML parsing, spec-compliant, passes html5lib-tests | MIT/Apache-2.0 |
| **cssparser** | CSS tokenization and parsing (CSS Syntax Level 3) | MPL-2.0 |
| **selectors** | CSS selector parsing and matching against DOM elements | MPL-2.0 |
| **taffy** | Layout computation — CSS Block, Flexbox, and Grid | MIT/Apache-2.0 |

### Custom Implementation (what we build)
- **DOM** — Rust implementation from scratch, spec-compliant, validated against WPT
- **CSS Cascade Engine** — specificity, cascade ordering, inheritance, computed values. Built on top of `cssparser` + `selectors`.
- **`getComputedStyle()`** — full spec compliance. React and other frameworks depend on this.
- **Accessibility tree generator** — transforms live DOM into compact text representation for agents
- **Session manager** — stateful sessions with cookies, history, navigation
- **CLI interface** — the `braille` command
- **Event loop** — frozen time model with pump-to-settled semantics

### Project Structure

Cargo workspace with 3 crates, separated by the WASM compilation boundary:

```
braille/
├── Cargo.toml                  # workspace root
├── PLAN.md
├── crates/
│   ├── engine/                 # compiles to WASM — the sandboxed browser
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── dom/            # Node, Element, Document, Event, etc.
│   │       ├── css/            # cascade, specificity, computed styles
│   │       ├── layout/         # Taffy integration
│   │       ├── html/           # html5ever integration
│   │       ├── js/             # Boa integration, Web API bindings
│   │       ├── a11y/           # accessibility tree generation
│   │       └── event_loop/     # frozen time pump-to-settled loop
│   │
│   ├── wire/                   # shared types — the WASM boundary contract
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── lib.rs          # commands, responses, snapshot formats, element refs
│   │
│   └── cli/                    # native binary — the host
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs
│           ├── session.rs      # session lifecycle, ID management
│           └── network.rs      # reqwest, cookie jar, fetch proxying
│
└── tests/
    ├── html5lib-tests/         # git submodule — html5lib/html5lib-tests (tree-construction .dat files)
    ├── wpt/                    # WPT test harness (future)
    └── fixtures/               # reference HTML pages for integration tests
```

**`engine`** — Everything that runs inside the WASM sandbox. Zero I/O, zero networking, pure computation. Takes parsed HTML + fetched resources as input, produces snapshots as output. Hard rule: if it touches the outside world, it doesn't go here.

**`wire`** — The protocol between engine and CLI. Command types (`Goto`, `Click`, `Type`), response types (`Snapshot`, `Error`), element reference formats (`@button3`), snapshot mode enums. Both `engine` and `cli` depend on this. Forces the WASM boundary to be explicit and well-defined.

**`cli`** — The native host. Loads the engine, provides networking on behalf of the sandbox (engine requests a URL → CLI fetches it → passes the response back in), manages sessions, parses CLI args, formats output.

## Form Factor: CLI (`braille`)

Single native binary, distributed via `brew install`, `cargo install`, or direct download.
No SDK to import, no server to start, no runtime dependencies.
Cross-platform: macOS, Linux, Windows.

### Session-based interface
```
braille new                              # → returns session ID
braille <sid> goto <url>                 # navigate, returns snapshot
braille <sid> click <selector>           # click element, returns snapshot
braille <sid> type <selector> "text"     # fill input, returns snapshot
braille <sid> select <selector> "value"  # select dropdown, returns snapshot
braille <sid> focus <selector>           # focus element
braille <sid> snap                       # get current snapshot
braille <sid> snap --mode=markdown       # snapshot in specific mode
braille <sid> back                       # go back
braille <sid> forward                    # go forward
braille <sid> close                      # end session
```

Every mutation command returns a snapshot automatically.
Verbs mirror what a human does in a browser.

## Agent Interface

Agent chooses output mode based on use case:
- **Accessibility tree (default)** — compact, semantic, ~200-400 tokens. Interactive elements get stable references (e.g. `@button3`, `@input7`)
- **Simplified DOM** — strips noise (scripts, hidden elements, SVG internals, empty wrappers), keeps meaningful structure with interactive markers
- **Raw DOM** — full HTML serialization for debugging or when the agent needs everything
- **Markdown** — readable content extraction for articles, docs, etc.

Agent interacts via element references: `click @button3`, `type @input7 "search query"`, `select @dropdown2 "option1"`

Must support: clicking links/buttons, filling form inputs, selecting dropdowns, submitting forms, scrolling.

## CSS: Full Spec Compliance

- Full CSS cascade: parsing, selector matching, specificity, inheritance, computed values
- `getComputedStyle()` must work correctly
- Built on Servo's `cssparser` (parsing) and `selectors` (matching) crates
- Cascade algorithm, inheritance, initial values, shorthand expansion — custom implementation on top of those crates
- This is a significant differentiator — jsdom never solved this (open PR from 2019, never merged)
- WPT has CSS tests to validate against

## Layout: Taffy

- **Taffy** provides real CSS Block, Flexbox, and Grid layout computation
- Input: tree of nodes with CSS Style structs → Output: Layout structs with position (x, y) and size (width, height)
- This gives us correct `getBoundingClientRect()`, `offsetWidth`, `offsetHeight` values
- Viewport is configurable, defaults to 1280x720, agent can change it
- Text measurement approximated (char count × avg char width × font size) — no font renderer
- Skips: subpixel rendering, paint order, z-index stacking contexts

## Navigation: Stateful Sessions

- Browser interaction is through stateful sessions. Agent gets a session ID, issues commands against it.
- Session owns: DOM, cookies, history, navigation context
- Full page navigation: teardown current DOM, fetch new page, parse and rebuild
- SPA routing: `pushState`/`replaceState`/`popstate` handled naturally by executing the page's JS
- Back/forward history: supported
- Iframes: supported — each iframe gets its own document within the parent session

## Network

- **Rust HTTP client** (reqwest or similar) for HTTP requests
- **Cookie jar per session** — persists across navigations, attaches correct cookies to outgoing requests
- **CORS: skip by default** — no user to protect, agent wants cross-origin access. Optionally enforceable.
- **Service workers: supported** — needed for sites that rely on them for request interception/routing

## Security: WASM Sandboxing

- Page JS executes inside a WASM sandbox — even memory exploits can't escape
- The entire engine (Rust DOM + Boa JS) compiles to a WASM module
- The host (native CLI binary) only provides controlled capabilities: network access, session management
- No file system access, no process spawning from within the sandbox

## Execution Model: Frozen Time

- Time freezes between agent commands. No JS runs until the agent acts.
- On each command (goto, click, type, etc.):
  1. Execute the action
  2. Pump the event loop — process microtasks, fire ready timers, handle network callbacks
  3. Keep pumping until "settled" (no pending microtasks, no ready timers, network quiet)
  4. Freeze. Return snapshot.
- `setTimeout(fn, 5000)` doesn't fire on wall clock — it fires when the event loop is pumped past that point
- Every mutation command returns a snapshot automatically. `snap` is for looking without acting.
- More deterministic than a real browser — no race conditions, agent always sees consistent state

### Needs deeper design work:
- **Web Workers** — separate threads in real browsers. Same event loop? Separate WASM instances? Freeze between commands?
- **Async/await and Promises** — microtask queue should pump naturally during "settle", but edge cases need verification
- **Streams** — ReadableStream, WritableStream, fetch body streams. Behavior when time is frozen?
- **WebSockets** — persistent connections. Messages queue up and deliver on next pump?
- **requestAnimationFrame** — no screen to paint, but frameworks use it for scheduling. Treat as a timer?

## Compliance Testing

- **WPT (Web Platform Tests)** — 56,000+ test files, BSD licensed, the canonical browser conformance suite
  - Git submodule at `tests/wpt/` with sparse checkout: `resources`, `dom/nodes`, `dom/events`
  - 164 HTML test files in `dom/nodes/`, 78 in `dom/events/`
  - jsdom's `to-run.yaml` provides a curated roadmap of which tests are feasible for non-browser DOM implementations
  - **Phase 1 IN PROGRESS** — Steps 1-3 done, Step 4 (triage) in progress — 31/263 passing, 4 failing, 228 ignored
  - Future phases: `html/dom/`, `css/selectors/`
- **html5lib-tests** — integrated as git submodule at `tests/html5lib-tests/`
  - **Tree-construction:** 1778 test cases from 56 `.dat` files, run via `cargo test --test html5lib_tree_construction`
    - **1778 passed** (100%), **0 failed**, **0 ignored**
    - Two polyfills in `parser.rs` (grep `POLYFILL`): annotation-xml integration point flag, selectedcontent post-parse cloning
    - Uses `libtest-mimic` for custom test runner with `.dat` file parser and DOM-to-pipe-indented serializer
  - **Serializer:** 230 test cases from 5 `.test` JSON files, run via `cargo test --test html5lib_serializer`
    - **204 passed**, **0 failed**, **26 ignored** (options/injectmeta/whitespace skipped — non-default serializer config)
    - Token-stream serializer with attribute quoting rules, text escaping, DOCTYPE variants, and full HTML optional tag omission
    - Uses `libtest-mimic` + `serde_json` for JSON test file parsing
- **Test262** — Boa already runs this; monitor their conformance progress

## Licensing

- Braille: MIT or Apache-2.0
- Boa: MIT — compatible
- Servo crates: MPL-2.0 (cssparser, selectors) and MIT/Apache-2.0 (html5ever, taffy) — all compatible
- AGPL is a dealbreaker (ruled out Lightpanda)

## Landscape: What Exists and Why It's Not Enough

### Agent browser tools (full Chrome underneath)
- **agent-browser** (Vercel Labs) — great text output via accessibility trees, but Playwright/headless Chrome under the hood
- **browser-use** — LLM-driven browser automation, still Playwright
- **Stagehand** (Browserbase) — same, real Chromium
- **Playwright MCP** (Microsoft) — accessibility tree for LLMs, still a full browser

These solve the output format problem but not the rendering overhead problem.

### Virtual DOMs (no real JS execution)
- **jsdom** — most complete DOM, but weak in-page JS execution, slow
- **happy-dom** — faster, but had an RCE vulnerability (CVE-2025-61927), less spec-compliant
- **linkedom** — minimal, designed for SSR not browser emulation

These are DOM parsers, not browsers.

### Lightpanda
- Closest to our vision architecturally — Zig-based, V8, real DOM, no rendering
- But: AGPL licensed (dealbreaker), beta quality, small team, Zig pre-1.0
- Missing many Web APIs, SPAs don't work reliably

### Content extraction tools (Firecrawl, Jina Reader, Crawl4AI)
- One-shot extraction, not interactive browsers
- Still use full browsers internally

## Open Items

### Framework Acceptance Criteria
- React's reconciler touches a LOT of DOM APIs — need to enumerate which ones
- SvelteKit compiles to direct DOM manipulation — different API surface
- Need to identify the minimum DOM API surface that makes these frameworks functional
- **TODO (later):** Identify real-world reference sites/apps to use as test targets:
  - Official/canonical framework examples
  - Real production SPAs
  - Minimal reproduction apps for specific DOM API surfaces (forms, routing, dynamic content)
