# Braille DOM v2 — Specification

## Motivation

The current DOM is a single arena of nodes with a single JS context. This works for simple pages but fails structurally when the spec requires:

- Documents that know their URL, content type, character set, and compat mode
- Nodes that belong to a specific Document (and can be adopted between them)
- Events that track dispatch state, realm of origin, and trusted flag
- Iframes, `createHTMLDocument()`, and `DOMParser` producing isolated Documents
- Cross-realm property access using the correct global's constructors

These aren't features to bolt on. They're the foundation everything else sits on. This spec describes a ground-up replacement for the DOM layer.

---

## 1. Core Concepts

### Realm

A **Realm** is the unit of JavaScript execution context. Every Document lives in exactly one Realm. A Realm owns:

- A **global object** (the `window` or `WorkerGlobalScope`)
- A **prototype chain** (its own `Node.prototype`, `Element.prototype`, `Event.prototype`, etc.)
- A **constructor set** (`DOMException`, `TypeError`, `Event`, etc.)
- A **document** (the Realm's associated Document)
- A **settings object** (origin, base URL, referrer policy)

Two Documents in different Realms have *different* `Event` constructors. `instanceof` checks only match within the same Realm. This is how browsers work and is the source of most cross-realm test failures.

The top-level page has a Realm. Each `<iframe>` has a Realm. Each `createHTMLDocument()` call produces a Document that shares its creator's Realm (but is a distinct Document). `DOMParser.parseFromString()` also produces a Document in the caller's Realm.

### Document

A **Document** is a first-class object with identity and metadata:

- **URL** — the actual loaded URL (after redirects), or `"about:blank"`
- **content type** — `"text/html"`, `"application/xhtml+xml"`, `"text/xml"`, etc.
- **character set** — `"UTF-8"` (with aliases `charset`, `inputEncoding`)
- **compat mode** — `"CSS1Compat"` (standards) or `"BackCompat"` (quirks)
- **origin** — the security origin (scheme + host + port)
- **realm** — back-reference to the owning Realm
- **type** — `"html"` or `"xml"` (affects parsing, case sensitivity, namespace defaults)

A Document is the root owner of all its nodes. Every node has exactly one `ownerDocument`. Moving a node between Documents requires an **adopt** operation that updates ownership recursively.

Documents are created via:

| Creator | URL | Content Type | Realm |
|---------|-----|-------------|-------|
| Navigation (top-level load) | Response URL | Response MIME | New Realm |
| `<iframe src="...">` | Response URL | Response MIME | New Realm |
| `<iframe srcdoc="...">` | Parent's URL | `text/html` | New Realm |
| `createHTMLDocument(title)` | `about:blank` | `text/html` | Creator's Realm |
| `DOMParser.parseFromString(s, type)` | `about:blank` | `type` param | Creator's Realm |
| `document.implementation.createDocument(ns, qn, dt)` | `about:blank` | `application/xml` | Creator's Realm |
| `XMLHttpRequest responseType="document"` | Request URL | Response MIME | Creator's Realm |

### Node

A **Node** belongs to exactly one Document. The `ownerDocument` is set at creation time and updated on adoption.

Node identity is per-Document. Two Documents can each have a node with the same local structure, but they are distinct objects with distinct wrappers.

The key properties every node carries:

- **node type** — element, text, comment, doctype, document, document fragment, shadow root
- **owner document** — the Document this node belongs to (never null for non-Document nodes)
- **parent / children / siblings** — tree position (within a single Document's tree)
- **connected** — whether the node is in the Document's tree (reachable from document root)

### Event

An **Event** is a first-class object with dispatch lifecycle:

- **type** — string name (`"click"`, `"abort"`, etc.)
- **target** — the node the event is dispatched on (set during dispatch)
- **currentTarget** — the node whose listener is currently executing
- **eventPhase** — `NONE(0)`, `CAPTURING(1)`, `AT_TARGET(2)`, `BUBBLING(3)`
- **bubbles** — whether the event bubbles up the tree
- **cancelable** — whether `preventDefault()` has effect
- **defaultPrevented** — whether `preventDefault()` was called
- **composed** — whether the event crosses shadow DOM boundaries
- **isTrusted** — whether the event was created by the user agent (immutable after creation)
- **timeStamp** — `DOMHighResTimeStamp` from the Realm's `Performance.now()`
- **dispatch flag** — internal: true while the event is being dispatched
- **stop propagation flag** — internal: `stopPropagation()` was called
- **stop immediate propagation flag** — internal: `stopImmediatePropagation()` was called
- **initialized flag** — internal: whether the event has been initialized
- **realm** — the Realm in which the event was constructed

Events are **frozen during dispatch**: calling `initEvent()` on a dispatching event is a no-op (per spec). Re-dispatching a currently-dispatching event throws `InvalidStateError`. After dispatch completes, `eventPhase` resets to `NONE`, `currentTarget` becomes `null`, and `composedPath()` returns `[]`.

---

## 2. Realm Lifecycle

### Creation

A Realm is created when:

1. The engine loads a new top-level page → top Realm
2. An `<iframe>` loads content → child Realm
3. `window.open()` (if ever supported) → new Realm

A Realm is NOT created for `createHTMLDocument()`, `DOMParser`, or `XMLHttpRequest` — those produce Documents within an existing Realm.

### Realm Contents

Each Realm contains:

- **Global object** with standard properties (`window`, `self`, `document`, `location`, `navigator`, etc.)
- **Intrinsic constructors** — `Event`, `CustomEvent`, `MouseEvent`, `DOMException`, `TypeError`, `Node`, `Element`, `HTMLDivElement`, etc.
- **Prototype chains** — `Node.prototype`, `Element.prototype`, etc. These are per-Realm objects.

When code in Realm A does `new Event("click")`, it uses Realm A's `Event` constructor. The resulting event's `constructor` property points to Realm A's `Event`. If this event is dispatched into Realm B, `event instanceof Event` returns `false` in Realm B (because Realm B has its own `Event`).

### Realm Relationship to Engine

Each Realm maps to one JS context (QuickJS `Context`). QuickJS supports multiple contexts within a single runtime, sharing the same heap. This is the mechanism for cross-realm object passing — objects from context A are visible in context B as "foreign" objects.

The engine maintains a **Realm registry**: a map from Realm ID to Realm metadata (document, global, settings). The top-level Realm is always Realm 0.

### Realm Teardown

When an iframe is removed from the DOM or navigated, its Realm is destroyed. All references from other Realms to objects in the destroyed Realm become "dead" (accessing them throws or returns undefined per spec).

---

## 3. The Document Model

### Document Identity

Every Document has a unique ID (u64, monotonically increasing). This ID is used internally to track ownership. The Document object exposed to JS is a proper object with getters:

| Property | Type | Settable | Notes |
|----------|------|----------|-------|
| `URL` | string | no | Actual loaded URL after redirects |
| `documentURI` | string | no | Alias for URL |
| `contentType` | string | no | MIME type |
| `characterSet` | string | no | Always "UTF-8" |
| `charset` | string | no | Alias for characterSet |
| `inputEncoding` | string | no | Alias for characterSet |
| `compatMode` | string | no | "CSS1Compat" or "BackCompat" |
| `doctype` | Doctype? | no | The DOCTYPE node |
| `documentElement` | Element? | no | The `<html>` element |
| `head` | Element? | no | The `<head>` element |
| `body` | Element? | no | The `<body>` element |
| `location` | Location? | varies | null for non-navigable documents |
| `domain` | string | yes | Origin domain (legacy) |
| `referrer` | string | no | Referrer URL |
| `cookie` | string | yes | Document cookies |
| `title` | string | yes | Document title |
| `readyState` | string | no | "loading" / "interactive" / "complete" |

### Document Node Tree

A Document owns a single node tree (arena). The root of the tree is the Document node itself (node type 9). The tree structure:

```
Document (nodeType 9)
  ├── Doctype (nodeType 10)  [optional]
  └── Element <html> (nodeType 1)
        ├── Element <head>
        │     └── ...
        └── Element <body>
              └── ...
```

### Document Creation Methods

**`document.implementation.createHTMLDocument(title)`**

Creates a new Document in the same Realm with:
- Type: `"html"`
- URL: `"about:blank"`
- Content type: `"text/html"`
- Compat mode: `"CSS1Compat"`
- Structure: doctype + html + head + title + body
- Location: `null` (not navigable)

**`new DOMParser().parseFromString(markup, type)`**

Creates a new Document in the caller's Realm with:
- Type: inferred from `type` parameter
- URL: `"about:blank"`
- Content type: `type` parameter
- Tree: parsed from `markup`

**`document.createElementNS(namespaceURI, qualifiedName)`**

Creates an element in this Document with the given namespace. Validates:
- qualifiedName matches `Name` production
- If qualifiedName has prefix, namespaceURI must not be null
- `xml` prefix only with `http://www.w3.org/XML/1998/namespace`
- `xmlns` prefix only with `http://www.w3.org/2000/xmlns/`
- If namespaceURI is `http://www.w3.org/2000/xmlns/`, prefix must be `xmlns` or qualifiedName must be `xmlns`

### Node Adoption

When a node is inserted into a Document different from its current `ownerDocument`:

1. Remove from old parent (if any)
2. **Adopt**: recursively set `ownerDocument` to the new Document for the node and all descendants
3. Insert into new parent
4. Run adoption steps (custom element callbacks, etc.)

This is the mechanism for `document.adoptNode(node)` and for moving nodes between Documents implicitly (e.g., `appendChild` across documents).

---

## 4. The Event System

### Event Construction

Events are constructed within a Realm. The constructor determines:
- Which Realm's prototype chain the event inherits from
- Which Realm's `DOMException` is used for abort reasons
- The `timeStamp` (from the Realm's clock)

```
event = new Event("click", { bubbles: true })
                   ↓
           Uses current Realm's Event constructor
                   ↓
           event.__proto__ === currentRealm.Event.prototype
           event.timeStamp = currentRealm.performance.now()
```

### Event Dispatch Algorithm

The dispatch algorithm is a **state machine** with these phases:

```
[IDLE] ──dispatch()──> [CAPTURING] ──> [AT_TARGET] ──> [BUBBLING] ──> [IDLE]
                            │               │              │
                            └───────────────┴──────────────┘
                                   stopPropagation()
                                        or
                                   tree exhausted
```

**Full algorithm:**

1. **Pre-dispatch**
   - If event's dispatch flag is set → throw `InvalidStateError`
   - Set dispatch flag
   - Set target
   - Compute propagation path (ancestors from target to root)
   - If `composed`, include shadow-crossing ancestors

2. **Capture phase** (eventPhase = CAPTURING_PHASE = 1)
   - Walk propagation path from root toward target (exclusive)
   - At each node: set currentTarget, invoke listeners registered with `capture: true`
   - If stop immediate propagation flag → skip remaining listeners on this node
   - If stop propagation flag → skip remaining nodes (but finish current node's listeners)

3. **Target phase** (eventPhase = AT_TARGET = 2)
   - Set currentTarget = target
   - Invoke ALL listeners (capture and bubble) in registration order
   - Same stop propagation rules apply

4. **Bubble phase** (eventPhase = BUBBLING_PHASE = 3) — only if `bubbles` is true
   - Walk propagation path from target's parent toward root
   - At each node: set currentTarget, invoke listeners registered with `capture: false`
   - Same stop propagation rules

5. **Post-dispatch**
   - Clear dispatch flag
   - Set currentTarget = null
   - Set eventPhase = NONE (0)
   - Run **activation behavior** if applicable (see below)
   - Return `!defaultPrevented`

### Listener Invocation

When invoking a listener:

1. Set `window.event` = event (in the listener's Realm)
2. Call the listener's callback with `this` = currentTarget
3. If listener was registered with `once: true` → remove it
4. If callback throws:
   - Report the error to the listener's Realm (not the dispatcher's)
   - Continue with next listener
5. Clear `window.event` (in the listener's Realm)

### Listener Deduplication

`addEventListener(type, callback, options)` must deduplicate:

- Two listeners are "the same" if they have the same `type`, `callback` reference, and `capture` flag
- Adding a duplicate listener is silently ignored (not an error)
- The `once`, `passive`, and `signal` options of the existing listener are NOT updated

### Activation Behavior

Certain elements have **activation behavior** triggered by click events. The rules for WHEN activation fires are subtle and frequently tested:

#### Event class guard

Activation behavior **only** fires for `MouseEvent` or `PointerEvent` instances (or subclasses). A plain `new Event("click")` dispatched on a checkbox does NOT toggle it, even if `bubbles: true`. This is because activation is keyed on the event being a "click event" per spec, which requires MouseEvent lineage.

#### Trusted vs untrusted

| Source | Activates? | Notes |
|--------|-----------|-------|
| `el.click()` method | Yes | Produces trusted MouseEvent internally |
| `el.dispatchEvent(new MouseEvent("click"))` | Yes (with exceptions) | Untrusted but still MouseEvent — activates checkboxes/radios |
| `el.dispatchEvent(new Event("click"))` | No | Wrong class |
| Braille `handle_click()` | Yes | Trusted MouseEvent |

#### Disabled elements

- `el.click()` on a disabled form control is a **no-op** — no event dispatched, no activation
- `el.dispatchEvent(new MouseEvent("click"))` on a disabled checkbox **still activates** (toggles checked) — dispatch is not blocked by disabled state
- `el.dispatchEvent(new MouseEvent("click"))` on a disabled `<button type="submit">` does NOT activate (submit is blocked for disabled buttons)

The distinction: checkboxes/radios have activation behavior that fires regardless of disabled state when triggered by `dispatchEvent`, but submit/reset buttons check disabled state as part of their activation.

#### Activation table

| Element | Behavior | Pre-activation step |
|---------|----------|-------------------|
| `<input type="checkbox">` | Toggle `checked`, fire `input` + `change` | Toggle checked BEFORE dispatch |
| `<input type="radio">` | Set `checked`, uncheck others in group | Toggle checked BEFORE dispatch |
| `<input type="submit">` | Submit the form | None |
| `<input type="reset">` | Reset the form | None |
| `<button type="submit">` | Submit the form | None |
| `<button type="reset">` | Reset the form | None |
| `<a href="...">` | Navigate | None |
| `<label>` | Forward click to labeled control | None |

#### Activation timing

1. **Pre-activation step** (checkbox/radio only): Toggle checked state BEFORE dispatch begins, so listeners see the new state during the event
2. **Dispatch** runs normally (capture → target → bubble)
3. **Post-activation**: If `defaultPrevented` is false → run the activation behavior. If `defaultPrevented` is true → **rollback** the pre-activation step (un-toggle checkbox/radio)

#### Only the first activatable element fires

When a click bubbles through multiple elements with activation behavior (e.g., a checkbox inside a label inside a form with a submit button), only the **target element's** activation behavior fires. Ancestors with activation behavior do NOT fire during bubble.

#### Post-dispatch event state

After dispatch completes (before activation runs), the event's state is:
- `eventPhase` = 0 (NONE)
- `currentTarget` = null
- `composedPath()` = [] (empty array)

Listeners observing the event after dispatch (e.g., via a stored reference) see this reset state.

### Trusted Events

Events created by the user agent (click from user action, load, etc.) have `isTrusted: true`. This property is:

- Set at construction time in Rust (not in JS)
- Read-only (unforgeable getter defined on the instance, not the prototype)
- Cannot be overridden by Object.defineProperty, prototype mutation, or Proxy

`isTrusted` affects:
- Activation behavior (only trusted clicks trigger it — see below)
- `event.isTrusted` getter returns the internal flag

**Redispatch behavior:** When an event is dispatched a second time (on the same or different target), its `isTrusted` is set to `false` on the *same object* — no copy is made. The event object is mutated. This means a listener that captures a reference to a trusted event and later redispatches it will observe `isTrusted === false` after the second dispatch begins.

**Braille-specific:** Since Braille has no real user input device, `handle_click()` and `handle_type()` produce events with `isTrusted: true` because they represent the LLM agent's intentional actions (equivalent to user actions). Events created by `new Event()` or `document.createEvent()` in page JS are always untrusted.

### Event Handler Attributes

`onclick`, `onload`, etc. are **event handler IDL attributes**. They differ from `addEventListener`:

- Setting `el.onclick = fn` replaces any previous `onclick` handler (but not `addEventListener` listeners)
- `onclick` handler is invoked during the bubble phase (or at-target)
- `onclick` and `addEventListener("click", fn)` are separate — both fire
- Body/frameset element handlers for `onload`, `onerror`, `onblur`, `onfocus`, `onresize`, `onscroll` forward to the Window

### window.event

The `window.event` property:
- Returns the event currently being dispatched in this window's context
- Returns `undefined` when no event is being dispatched
- Is per-Realm (iframe's `window.event` is independent of parent's)
- Is set before each listener invocation and cleared after
- Nested dispatches (listener dispatches a new event) push/pop correctly

**Shadow DOM interaction:** For events dispatched inside a shadow tree, `window.event` is `undefined` for listeners in the shadow host's light DOM. This is because the event is retargeted — the "current event" in the outer Realm is the retargeted event, not the original. If no retargeted event exists for that Realm, `window.event` is `undefined`.

**Error handler interaction:** When a listener throws and `window.onerror` is called to report the exception, `window.event` is still set to the dispatching event in the error handler's Realm. The error reporting itself does not clear `window.event` — it remains set until the listener invocation frame unwinds.

**beforeunload coercion:** When a `beforeunload` handler returns a value and the browser coerces it to a string (via `toString()`), `window.event` is still set during that coercion. This means a `toString()` method on the return value can access `window.event`.

### EventTarget as standalone constructor

`EventTarget` is constructable without any DOM node:

```js
const target = new EventTarget();
target.addEventListener("foo", handler);
target.dispatchEvent(new Event("foo"));
```

Standalone EventTargets:
- Have no parent (no propagation path — dispatch is always AT_TARGET only)
- Have no ownerDocument
- Belong to the Realm in which they were constructed
- Support addEventListener, removeEventListener, dispatchEvent
- Are the base class for Window, Node, AbortSignal, etc.

---

## 5. Cross-Realm Interactions

### The Problem

When Realm A dispatches an event on a node that has listeners from Realm B:

1. The listener callback is a function from Realm B
2. `this` inside the callback is a node wrapper from Realm A
3. Errors thrown by the callback must be reported in Realm B
4. `window.event` must be set in Realm B (not Realm A)
5. `event instanceof Event` in Realm B is `false` (different `Event` constructor)

### The Solution: Listener Realm Tracking

Every registered event listener carries a **Realm ID** — the Realm in which `addEventListener` was called. During dispatch:

```
for each listener on currentTarget:
    activateRealm(listener.realmId)        // push Realm context
    set listener.realm.window.event = event
    call listener.callback(event)           // callback runs in its own Realm
    clear listener.realm.window.event
    deactivateRealm()                       // pop Realm context
```

### Cross-Realm Node Access

When code in Realm A accesses a node from Realm B's document:

- The node wrapper is created in Realm A (using Realm A's prototypes)
- But the underlying node data is from Realm B's Document
- Property access (`.textContent`, `.innerHTML`) reads from the node's Document tree
- Method calls (`.appendChild()`) operate on the node's Document tree
- Constructor identity follows the wrapper's Realm:
  `node instanceof Element` → true in Realm A (uses A's `Element`)

### Cross-Realm DOMException

When AbortSignal aborts, the `reason` is a `DOMException` constructed in the Signal's Realm. Code in other Realms accessing `signal.reason` gets the same DOMException object, but `reason instanceof DOMException` returns `false` in their Realm.

The spec requires the same *instance* to be returned every time (identity, not equality).

### postMessage

`postMessage` is the canonical cross-realm communication mechanism:

1. Sender calls `targetWindow.postMessage(data, origin)`
2. `data` is **structured cloned** (deep copy, no shared references)
3. A `MessageEvent` is constructed in the **target's Realm** with the cloned data
4. The event is dispatched on the target window asynchronously (queued as a task)

This means `event.data` is always a native object in the receiver's Realm.

---

## 6. Networking Contract — How Documents Get Their Metadata

The DOM spec describes what metadata Documents carry. This section specifies how that metadata arrives from the networking layer.

### Document metadata from HTTP responses

When the engine loads a resource (top-level navigation or iframe), the networking layer provides:

| Field | Source | Fallback |
|-------|--------|----------|
| URL | Final URL after all redirects (not the initial request URL) | `"about:blank"` |
| Content Type | `Content-Type` response header, parsed to MIME type only (strip parameters) | `"text/html"` |
| Character Set | `charset` parameter from Content-Type header | `"UTF-8"` |

The engine must track the **final URL** through redirect chains. If a request to `/redirect` returns `302 Location: /final.html`, the Document's URL is the absolute URL of `/final.html`, not `/redirect`.

### Content-Type determination for special URIs

| URI scheme | Content Type | URL |
|------------|-------------|-----|
| `data:text/html,...` | From data URI MIME type | The full data: URI |
| `javascript:...` | `"text/html"` | Inherits parent document's URL |
| `about:blank` | `"text/html"` | `"about:blank"` |
| `blob:...` | From Blob's type | The blob: URL |

### XHR responseType="document"

When `XMLHttpRequest` has `responseType = "document"`, the response must be parsed into a Document:
- URL: the request URL (after redirects)
- Content Type: from the response Content-Type header
- The Document is created in the XHR's Realm (same Realm as the calling code)
- `xhr.responseXML` returns this Document

### iframe Content-Type tracking

When an iframe loads, the engine must:
1. Record the response Content-Type header
2. Record the final URL (after redirects)
3. Create a new Realm
4. Create a DocumentData with the recorded metadata
5. Parse the response body according to the Content-Type
6. Make `iframe.contentDocument.contentType` return the recorded value

This requires the `FetchProvider` to return Content-Type and final URL alongside the response body. The current `FetchProvider` trait needs extending:

```
trait FetchProvider {
    fn fetch(&self, url: &str) -> FetchResponse;
}

struct FetchResponse {
    body: String,
    final_url: String,           // after redirects
    content_type: String,        // from Content-Type header
    headers: Vec<(String, String)>,
    status: u16,
}
```

---

## 6.5. CSS Animation & Transition Events

### Scope

CSS animation/transition events are partially in scope for the DOM v2 spec because they interact with the event dispatch system (prefixed event deduplication). The animation engine itself (computing keyframes, interpolation, timing) is out of scope.

### webkit-prefixed event deduplication

The DOM spec (step 8 of the "invoke" algorithm) requires deduplication between prefixed and unprefixed animation/transition event handlers:

| Unprefixed | Prefixed |
|-----------|----------|
| `animationstart` | `webkitAnimationStart` |
| `animationend` | `webkitAnimationEnd` |
| `animationiteration` | `webkitAnimationIteration` |
| `transitionend` | `webkitTransitionEnd` |
| `transitionrun` | `webkitTransitionRun` |
| `transitionstart` | `webkitTransitionStart` |
| `transitioncancel` | `webkitTransitionCancel` |

**Rules:**
1. If an element has BOTH `onanimationend` (unprefixed handler) AND `onwebkitAnimationEnd` (prefixed handler), only the UNPREFIXED handler fires
2. If an element has ONLY `onwebkitAnimationEnd`, it fires with event type `"webkitAnimationEnd"`
3. `addEventListener("animationend", fn)` and `addEventListener("webkitAnimationEnd", fn)` are SEPARATE listeners — both fire (dedup only applies to handler attributes, not addEventListener)
4. The event type string changes to match: prefixed handler receives prefixed event type

This deduplication logic must be implemented in the event dispatch engine (Phase 2), even if the CSS animation engine isn't complete. Tests that fire synthetic animation events must still obey the dedup rules.

---

## 7. Internal Architecture

### Data Layer (Rust)

The backing store remains an arena-based tree, but scoped per Document:

**DocumentStore** — owns all Documents and their trees

- `documents: HashMap<DocumentId, DocumentData>`
- Each `DocumentData` contains:
  - Metadata (URL, content type, compat mode, etc.)
  - Node arena (the tree)
  - Realm ID (which Realm this Document belongs to)
  - Style sheets (collected from `<style>`, `<link>`, inline styles)

**RealmStore** — owns all Realms

- `realms: HashMap<RealmId, RealmData>`
- Each `RealmData` contains:
  - JS context reference (QuickJS Context)
  - Document ID (the Realm's associated Document)
  - Parent Realm ID (for iframes; None for top-level)
  - Settings (origin, base URL)
  - `window.event` stack (for nested dispatch)

### JS Layer

Each Realm gets its own JS Context within the shared QuickJS Runtime. Contexts share the same heap, enabling cross-context object references.

**Template Realm:** On engine creation, a template Context is initialized with all prototypes (Node, Element, Event, HTMLDivElement, etc.) — this is the ~8.5ms cost paid once. The template is never used for execution, only as a source for cloning.

**Realm creation (from template):**

1. Create a new QuickJS Context
2. Copy prototype references from the template Context (shallow — prototypes are shared objects on the same heap)
3. Register per-Realm native functions (only those that reference Realm-specific state)
4. Create fresh per-Realm objects: `window`, `document`, `location`, `navigator`
5. The `document` global points to this Realm's Document

Cost: <1ms per new Realm (vs 8.5ms for full registration from scratch).

**Prototype isolation vs sharing:** Shared prototypes (same JS object across Contexts) are fast but LEAK: if page JS polyfills `Element.prototype.myMethod` in one Realm, all Realms see it. For Braille this is acceptable — pages don't intentionally pollute prototypes to attack other Realms (there's no real user). If spec-correct isolation is later needed, prototypes must be cloned (deep copy of each prototype object per Realm), which brings creation cost to ~2-3ms per Realm. Start with sharing; add isolation only if WPT tests require it.

**Lazy Realm creation:** Realms for iframes are not created at parse time. The engine records that an iframe exists but defers Context creation until triggered (script execution, cross-frame JS access, or cross-frame event). An iframe in "deferred" state has only its DocumentData (metadata + tree) — no JS Context, no prototypes, no wrapper cache.

**Node wrappers** are per-Realm. The same underlying node (DocumentId + NodeId) can have wrappers in multiple Realms. Each wrapper uses its Realm's prototype chain. The wrapper cache is per-Realm: `HashMap<(DocumentId, NodeId), JsObject>`.

**Runtime reuse:** On top-level navigation, the top-level Realm is rebound (new Document, cleared wrapper cache, cleared state) without re-creating the Context or re-registering prototypes. Child Realms are destroyed. This preserves the current 0.88ms rebind performance.

### Event Dispatch Engine

Event dispatch uses a **two-tier architecture** for performance:

#### Tier 1: Same-Realm Fast Path (JS-driven)

When all listeners on the propagation path are in the same Realm (the 99% case):

1. Rust receives dispatch request, builds propagation path, checks dispatch flag
2. Rust pushes event onto the Realm's `window.event` stack
3. Rust calls a single JS dispatch function: `__braille_dispatch_same_realm(eventData, path, listeners)`
4. JS iterates listeners directly (zero FFI per listener), calling each as a normal function
5. JS returns dispatch result (defaultPrevented, propagation stopped)
6. Rust pops `window.event` stack, runs activation behavior if applicable

**Total FFI crossings:** 2 (Rust→JS to start, JS→Rust to finish) regardless of listener count.

#### Tier 2: Cross-Realm Slow Path (Rust-driven)

When any node in the propagation path has `needs_cross_realm_dispatch = true`:

1. Rust receives dispatch request, builds propagation path
2. For each node in the path:
   - Look up registered listeners (stored in Rust, keyed by (DocumentId, NodeId, event_type))
   - For each listener:
     - Push event onto listener's Realm's `window.event` stack
     - Switch to that Realm's JS Context
     - Call the listener callback (by Vec index into callback registry)
     - Pop `window.event` stack
     - Handle errors (report in listener's Realm)
     - Check propagation flags
3. Run activation behavior if applicable
4. Return dispatch result

**Total FFI crossings:** N (one per listener). Acceptable because cross-Realm dispatch is rare.

#### Dispatch flag and state management

Regardless of tier, Rust owns the event state machine:
- Dispatch flag (prevents re-dispatch)
- Stop propagation / stop immediate flags
- eventPhase transitions
- Target / currentTarget (exposed via native getters)
- isTrusted mutation on redispatch

JS reads these via native getters (no cached copies that can go stale).

### Listener Storage

Listeners are stored in Rust, not in JS closure maps:

```
struct ListenerEntry {
    event_type: String,
    callback_id: u32,          // index into the Realm's callback Vec
    realm_id: RealmId,
    capture: bool,
    once: bool,
    passive: bool,
    signal: Option<AbortSignalId>,
    removed: bool,             // marked for removal during iteration
}
```

**Callback registry** per Realm is a `Vec<Option<JsFunction>>`. callback_id = index into this Vec. O(1) lookup, no hashing.

**Deduplication** uses a `HashSet<(String, u32, bool)>` keyed by (event_type, callback_id, capture). Checked on addEventListener, ignored on removeEventListener.

**Same-Realm optimization:** For the fast-path dispatch (Tier 1), Rust serializes the listener list into a compact format that JS can iterate directly — an array of `[callback_id, capture, once, passive]` tuples. JS looks up the callback by index into a JS-side mirror array. This avoids per-listener FFI during same-Realm dispatch.

**addEventListener registration cost:** Moving listener storage to Rust means every addEventListener/removeEventListener call crosses the JS→Rust boundary (~0.001ms each). A typical page registers 100-500 listeners during load = 0.1-0.5ms total overhead. This is acceptable. React-style event delegation (one listener on root) makes this even cheaper. If this ever becomes a bottleneck, listeners can be batched (register N listeners in one FFI call) — but premature optimization for now.

**Cross-Realm flag:** Each EventTarget tracks whether any foreign-Realm listener has been registered. This flag is set on addEventListener when `listener.realm_id !== target.realm_id`. Once set, it is never cleared (conservative — avoids expensive re-checking on removeEventListener). This flag determines whether dispatch uses Tier 1 or Tier 2.

### AbortSignal / AbortController

Implemented in Rust with JS wrappers:

```
struct AbortSignalData {
    id: AbortSignalId,
    aborted: bool,
    reason: Option<JsValueRef>,   // reference to JS value in the Signal's Realm
    realm_id: RealmId,
    dependent_signals: Vec<AbortSignalId>,
    listeners: Vec<ListenerEntry>,
}
```

When `abort(reason)` is called:
1. If already aborted → return
2. Set `aborted = true`
3. Set `reason` (default: create `DOMException("AbortError")` in Signal's Realm)
4. Fire `abort` event synchronously (trusted)
5. Propagate to dependent signals

`AbortSignal.timeout(ms)`:
1. Create signal in current Realm
2. Register a timer
3. When timer fires: abort with `DOMException("TimeoutError")`

`AbortSignal.any(signals)`:
1. Create dependent signal
2. If any source is already aborted → abort immediately with *that source's reason* (same instance)
3. Otherwise, listen for abort on all sources

---

## 8. Migration Strategy

This is a ground-up rewrite of the DOM layer, but it doesn't have to be a big-bang replacement. The migration path:

### Phase 1: DocumentData + FetchProvider extension

- Add `DocumentData` struct (URL, content_type, character_set, compat_mode, realm_id)
- Extend `FetchProvider` to return `FetchResponse` with final_url + content_type + headers
- Store DocumentData on `DomTree` (or alongside it)
- Wire Document JS getters to read from DocumentData (URL, contentType, characterSet, compatMode)
- Track final URL through redirects in the networking layer
- All existing code continues working — single Realm, single Document
- **Unlocks (8 tests)**: Document-URL, all 6 contenttype_* tests, Comment-constructor (basic, not cross-frame)
- **Partially unlocks**: xhr_responseType_document (needs XHR document mode too)

### Phase 2: Event dispatch state machine in Rust

- Move event dispatch from JS (`__dispatch`) to Rust
- Implement as Rust state machine: dispatch flag, propagation flags, eventPhase, currentTarget, target
- Listener storage in Rust (ListenerEntry struct with deduplication)
- JS listeners register via `__n_addEventListener`, Rust calls them back via callback_id
- Implement `window.event` tracking (single Realm first — just a push/pop stack)
- Implement activation behavior with:
  - MouseEvent class guard (only MouseEvent/PointerEvent activates)
  - Pre-activation step (checkbox/radio toggle before dispatch)
  - Rollback on preventDefault
  - Disabled element rules (click() no-op vs dispatchEvent still activates checkbox)
  - Post-dispatch state reset (eventPhase=0, currentTarget=null, composedPath=[])
- Implement initEvent no-op during dispatch (dispatch flag check)
- Implement isTrusted mutation on redispatch (same object, flipped to false)
- Implement EventTarget as standalone constructor
- Implement webkit-prefixed event handler dedup logic
- **Unlocks (9 tests)**: Event-dispatch-click, Event-dispatch-redispatch, Event-init-while-dispatching, EventListener-invoke-legacy, event-global-extra (single-realm parts), capture_phase_listener_fires_on_root, handler-count (logic, not testdriver), webkit-animation-end-event, webkit-animation-start-event
- **Prerequisite for Phase 3**

### Phase 3: Multi-Realm support

- Use QuickJS multi-Context (one Context per Realm, shared Runtime/heap)
- Prototype registration runs per Realm (GlobalsRegistry already supports this)
- Node wrapper cache becomes per-Realm (keyed by RealmId + NodeId)
- Listener entries carry Realm ID (set at addEventListener time)
- During dispatch: switch to listener's Realm for each callback invocation
- `window.event` tracking per Realm (each Realm has its own push/pop stack)
- Error reporting goes to listener's Realm (not dispatcher's)
- Per-Realm constructors: `DOMException`, `Event`, `TypeError` etc.
- iframe contentWindow/contentDocument return cross-Realm proxies
- **Unlocks (7 tests)**: Event-timestamp-cross-realm-getter, EventListener-handleEvent-cross-realm, EventListener-incumbent-global-1, EventListener-incumbent-global-2, event-global-extra (cross-realm parts), event-global-is-still-set-when-reporting-exception-onerror, abort/reason-constructor

### Phase 4: createHTMLDocument / DOMParser / createElementNS

- `createHTMLDocument(title)` creates new DocumentData + tree within same Realm
  - Structure: doctype + html + head + (title if provided) + body
  - Metadata: URL="about:blank", contentType="text/html", compatMode="CSS1Compat"
  - `doc.location === null` (not navigable)
- `DOMParser.parseFromString(markup, type)` creates new DocumentData + tree
  - Content type from `type` parameter (determines XML vs HTML parsing)
  - createElement namespace rules depend on contentType
- `createElementNS()` with full namespace validation (NAMESPACE_ERR, INVALID_CHARACTER_ERR)
- Node adoption: `adoptNode()` + implicit adoption on cross-document appendChild
- `createElement()` in HTML documents produces elements in XHTML namespace
- `createElement()` in XML documents produces elements in null namespace
- Proper HTML subclass selection (HTMLDivElement, HTMLSpanElement, HTMLUnknownElement) based on tag + namespace
- Comment/Text constructors with correct ownerDocument per-Realm
- **Unlocks (5 tests)**: DOMImplementation-createHTMLDocument, Document-createElement-namespace, Document-createElementNS, Comment-constructor (cross-frame), insertion-removing-steps (partial)

### Phase 5: AbortSignal completion

- Move AbortController/AbortSignal to Rust with JS wrappers
- Reason stored as persistent JS value reference (identity preserved)
- `signal.reason === signal.reason` always true (same instance returned)
- `abort()` creates DOMException in Signal's Realm (per-Realm constructor from Phase 3)
- `AbortSignal.any(signals)`:
  - Dependent signals marked aborted BEFORE events fire (synchronous propagation)
  - Reason is the SAME DOMException instance from the source signal
  - Abort events fire in source signal registration order
  - Handles reentrant aborts correctly
- `AbortSignal.timeout(ms)` with timer integration (DOMException("TimeoutError"))
- `throwIfAborted()` throws the stored reason
- Abort event is always trusted (isTrusted: true, unforgeable)
- **Unlocks (4 tests)**: abort-signal-any, event.any, reason-constructor, timeout.any

### Phase summary

| Phase | Tests unlocked | Cumulative | Dependency |
|-------|---------------|-----------|------------|
| 1 | 8 | 8/30 | None |
| 2 | 9 | 17/30 | None (parallel with 1) |
| 3 | 7 | 24/30 | Phase 2 |
| 4 | 5 | 29/30 | Phase 1, Phase 3 (for cross-frame) |
| 5 | 4 | 30/30* | Phase 3 (for per-Realm DOMException) |

*Note: 3 tests (abort) can partially pass without Phase 3 — only reason-constructor requires cross-Realm. The other 3 abort tests may pass with a Phase 5 implementation that doesn't need multi-Realm.

Phases 1 and 2 are independent and can be developed in parallel. Phase 3 depends on Phase 2 (event dispatch must be in Rust before adding per-Realm dispatch). Phase 4 depends on Phase 1 (DocumentData) and ideally Phase 3 (for cross-frame constructors). Phase 5 depends on Phase 3 (for per-Realm DOMException construction).

---

## 9. Performance Constraints

These are hard requirements. Implementations that violate them will regress page load and interaction latency.

### 9.1 Same-Realm Fast Path for Event Dispatch

The 99% case: all listeners on a propagation path belong to the same Realm. When this is true, dispatch MUST NOT pay cross-Realm overhead.

**Implementation:** Maintain a boolean `needs_cross_realm_dispatch` per EventTarget (set to `true` when a listener from a foreign Realm is registered). The dispatch engine checks this flag before choosing the dispatch path:

- **Fast path (same-Realm):** A JS-side dispatch function iterates listeners directly, calls them as normal JS function calls with zero FFI boundary crossings between listeners. Rust is only involved for state checks (dispatch flag, propagation flags) at the boundary — called once before dispatch begins and once after it ends.
- **Slow path (cross-Realm):** Rust drives the iteration, switching Realm context per listener. Only used when at least one node in the propagation path has `needs_cross_realm_dispatch = true`.

**Budget:** Same-Realm dispatch of a click event through 10 ancestor nodes with 3 total listeners must complete in <0.05ms (current: ~0.01ms in pure JS).

### 9.2 Lazy Realm Creation

Realms for iframes MUST NOT be created until one of:
1. The iframe's content contains a `<script>` tag (discovered during parse)
2. Parent JS accesses `iframe.contentWindow` or `iframe.contentDocument`
3. The iframe fires a cross-frame event (postMessage, load with listener)

Iframes that are decorative, tracking pixels, or never accessed by JS pay **zero initialization cost** — no QuickJS Context created, no prototypes registered.

**Budget:** A page with 10 non-scripted iframes must load with the same performance as a page with 0 iframes (±1ms).

### 9.3 Realm Template Cloning

Full prototype registration takes ~8.5ms. Creating multiple Realms must not multiply this cost linearly.

**Implementation:** Register all prototypes into a "template" Realm once. New Realms are created by:
1. Creating a bare QuickJS Context
2. Copying prototype references from the template (shallow copy of the global object's prototype properties)
3. Creating only the per-Realm state (new `document`, `window`, `location` objects)

**Budget:** Creating a new Realm after the first must cost <1ms (not 8.5ms).

### 9.4 Listener Callback Storage: O(1) Lookup

The callback registry MUST provide O(1) lookup by callback_id. Use a `Vec<Option<JsFunction>>` indexed by ID, not a HashMap.

Deduplication check (does this exact listener already exist?) uses a separate HashSet keyed by `(event_type, callback_id, capture)`.

### 9.5 window.event as Native Getter

`window.event` MUST be implemented as a Rust-backed getter on the global object, not a JS property written and cleared on every listener invocation.

**Implementation:** Rust maintains a per-Realm event stack (`Vec<EventId>`). The `window.event` getter calls into Rust to read the top of the stack. The setter is a no-op (or throws in strict mode).

**Cost:** Zero per-listener overhead during dispatch. Cost is paid only when JS code actually reads `window.event` (one FFI call at read time). Since `window.event` is a legacy API used by <5% of sites, this means zero overhead for the common case.

### 9.6 Runtime Reuse Across Navigations

The top-level Realm MUST support rebinding to a new Document without re-registering prototypes. This is the existing "Fast mode" (`rebind_for_new_page`).

**Contract:**
- First page load: ~8.5ms (full registration)
- Subsequent page loads (same tab): <1ms (rebind only)
- Rebind replaces: Document, DomTree, EngineState, wrapper cache, module registry
- Rebind preserves: all prototypes, all native functions, QuickJS Runtime/Context

Multi-Realm does not break this. The top-level Realm is always reusable. Child Realms (iframes) are destroyed on navigation and lazily recreated.

### 9.7 Separate Arenas: No Hot-Path Penalty

Each Document has its own node arena. This MUST NOT add overhead to the hot paths:

- **Node access by ID:** O(1) — arena index lookup within the Document's Vec
- **Tree walking (parent/child/sibling):** O(1) per step — same as current single-arena
- **Snapshot serialization:** Iterates each active Document's arena sequentially (better cache locality than jumping around one giant arena)
- **Cross-document adoptNode:** O(N) copy is acceptable — adoptNode is rare and always involves a subtree that must be physically moved

The engine maintains a flat `Vec<DocumentId>` of active Documents for iteration.

### 9.8 Event Dispatch Budget

Total time from `handle_click()` entry to return (including dispatch + settle):

| Scenario | Budget |
|----------|--------|
| Simple click (no listeners) | <0.1ms |
| Click with 5 listeners, same Realm | <0.2ms |
| Click with 5 listeners, 2 Realms | <0.5ms |
| Click triggering React state update + re-render | <5ms |
| Full settle after click (timers, observers, styles) | <50ms |

These budgets are per-interaction. The LLM agent waits for the full settle before reading the snapshot, so settle latency directly impacts agent response time.

---

## 10. Out of Scope

These are NOT addressed by this spec and require separate work:

| Area | Tests affected | Why out of scope |
|------|---------------|-----------------|
| **WebCrypto algorithms** (ML-DSA, KMAC) | 3 tests | Unrelated to DOM — requires algorithm implementation in crypto module |
| **WebDriver/testdriver** | 2 tests | Infrastructure for synthesizing trusted user input; Braille uses `handle_click()` instead |
| **CSS layout geometry** (offsetX/offsetY) | 1 test | Requires accurate `getBoundingClientRect()` from layout engine |
| **HTML insertion step ordering** | 1 test | Script execution timing during fragment insertion — HTML parser concern |
| **OpaqueRange** (tentative) | 1 test | Experimental API not in any finalized spec |
| **Cross-origin networking** | 1 test | Same-origin policy enforcement during iframe loading |
| **Navigation lifecycle** (beforeunload) | 1 test | Page lifecycle events during navigation |

Total: 10 tests are out of scope for this spec. The remaining 30 are addressed.

---

## 11. Implementation Guide

### What this replaces

This spec is a **ground-up reimplementation** of the DOM event system and document model. It supersedes:

| Existing code | Replaced by | Phase |
|--------------|------------|-------|
| `crates/engine/src/js/dom_bridge/event_dispatch.rs` — JS `__dispatch()` function | Rust event dispatch engine (Tier 1 + Tier 2) | Phase 2 |
| `_listeners`, `_captureKeys`, `_bubbleKeys`, `_winListeners` in IIFE closure | Rust `ListenerEntry` storage + callback Vec | Phase 2 |
| `Event`, `CustomEvent`, `MouseEvent` etc. in `js/globals/web_apis.rs` | Per-Realm Event constructors with Rust-backed state | Phase 2 |
| `document.URL`, `document.contentType` stubs in `js/dom_bridge/global_document.rs` | `DocumentData` struct with real metadata from HTTP responses | Phase 1 |
| `AbortController`/`AbortSignal` in `js/globals/web_apis.rs` | Rust `AbortSignalData` with JS wrappers | Phase 5 |
| Single `DomTree` arena for all content (including iframes) | Per-Document arena in `DocumentStore` | Phase 1 |
| Single QuickJS Context for all JS execution | Multi-Context with Realm registry | Phase 3 |
| `thread_local! TREE/STATE` in `js/dom_bridge/mod.rs` | `BrailleContext` per-Realm (already scaffolded in `js/context.rs`) | Phase 3 |

Code that is NOT replaced (carried forward):
- HTML parser (`crates/engine/src/html/`)
- CSS cascade (`crates/engine/src/css/`)
- Layout engine (`crates/engine/src/layout/`)
- Snapshot/a11y serialization (`crates/engine/src/a11y/`)
- DOM tree node types and arena (`crates/engine/src/dom/`) — structure preserved, scoped per Document
- Native tree operations (`__n_appendChild`, `__n_removeChild`, etc.) — still needed
- `Engine` public API (`handle_click`, `handle_type`, `snapshot`, etc.) — interface preserved

### Module structure for new code

```
crates/engine/src/
├── dom_v2/
│   ├── mod.rs              — public re-exports
│   ├── document.rs         — DocumentData, DocumentStore, DocumentId
│   ├── realm.rs            — RealmData, RealmStore, RealmId, template cloning
│   ├── event.rs            — EventData, EventState (dispatch flag, phases, propagation)
│   ├── dispatch.rs         — Tier 1 (JS fast path) + Tier 2 (Rust-driven) dispatch
│   ├── listeners.rs        — ListenerEntry, callback registry, deduplication
│   ├── activation.rs       — Click activation behavior (checkbox/radio/submit/link)
│   ├── abort.rs            — AbortSignalData, AbortController, AbortSignal.any/timeout
│   └── event_target.rs     — Standalone EventTarget constructor
```

### Coexistence during migration

The old and new code coexist. The migration is NOT a big-bang rewrite. Each phase:

1. Adds new code in `dom_v2/`
2. Wires it into the existing `Engine` (behind feature flag or runtime switch)
3. Passes the target WPT tests
4. Once stable, removes the old code path

**Feature flag:** `Engine::use_dom_v2: bool` (default false during development). When true, `dispatchEvent` routes through the new Rust dispatch engine instead of JS `__dispatch`. This allows running the ratchet against both paths.

**Gradual listener migration:** During Phase 2, listeners can be dual-stored (both JS closure maps AND Rust ListenerEntry). The Rust dispatch reads from Rust storage; the old JS dispatch reads from JS storage. Once Phase 2 is stable and all tests pass, remove the JS storage.

### Acceptance criteria per phase

#### Phase 1: DocumentData + FetchProvider extension

**First commit deliverable:**
- New file: `crates/engine/src/dom_v2/document.rs` with `DocumentData` struct
- `DomTree` gains `pub document_data: Option<DocumentData>` field
- `FetchProvider` trait extended with `fetch_with_metadata()` → `FetchResponse`
- Document JS getters (`document.URL`, `document.contentType`, `document.characterSet`, `document.compatMode`) read from `DocumentData`
- Loading code populates `DocumentData` from HTTP response headers

**Verification:** Run the ratchet. These tests must flip from FAIL to PASS:
- `wpt:dom/nodes/Document-URL.html`
- `wpt:dom/nodes/Document-contentType/contentType/contenttype_bmp.html`
- `wpt:dom/nodes/Document-contentType/contentType/contenttype_css.html`
- `wpt:dom/nodes/Document-contentType/contentType/contenttype_datauri_02.html`
- `wpt:dom/nodes/Document-contentType/contentType/contenttype_javascripturi.html`
- `wpt:dom/nodes/Document-contentType/contentType/contenttype_mimeheader_01.html`
- `wpt:dom/nodes/Document-contentType/contentType/xhr_responseType_document.html`
- `wpt:dom/nodes/Comment-constructor.html`

No existing PASS tests may regress.

#### Phase 2: Event dispatch state machine in Rust

**First commit deliverable:**
- New files: `dom_v2/event.rs`, `dom_v2/dispatch.rs`, `dom_v2/listeners.rs`, `dom_v2/activation.rs`, `dom_v2/event_target.rs`
- `EventData` struct with dispatch flag, propagation flags, phase, target, currentTarget, isTrusted
- `ListenerEntry` struct with callback_id, deduplication
- Rust dispatch function that builds propagation path + invokes listeners via JS callback
- `window.event` as native getter backed by Rust stack
- Activation behavior for checkbox/radio/submit with pre-activation rollback
- `EventTarget` constructable standalone
- `initEvent` no-op during dispatch
- isTrusted flipped to false on redispatch
- webkit-prefixed handler dedup

**Verification:** Run the ratchet. These tests must flip from FAIL to PASS:
- `wpt:dom/events/Event-dispatch-click.html`
- `wpt:dom/events/Event-dispatch-redispatch.html`
- `wpt:dom/events/Event-init-while-dispatching.html`
- `wpt:dom/events/EventListener-invoke-legacy.html`
- `wpt:dom/events/event-global-extra.window.js` (single-Realm subtests)
- `wpt:dom/events/webkit-animation-end-event.html`
- `wpt:dom/events/webkit-animation-start-event.html`
- `cargo:dom_bridge_react::capture_phase_listener_fires_on_root`

No existing PASS tests may regress.

#### Phase 3: Multi-Realm support

**First commit deliverable:**
- New files: `dom_v2/realm.rs`
- Template Realm creation (one-time prototype registration)
- Realm cloning from template (<1ms)
- Lazy Realm creation for iframes
- Per-Realm wrapper cache
- Per-Realm `window.event` stack
- Listener entries carry RealmId
- Cross-Realm dispatch (Tier 2 path)
- Per-Realm error reporting

**Verification:** Run the ratchet. These tests must flip from FAIL to PASS:
- `wpt:dom/events/Event-timestamp-cross-realm-getter.html`
- `wpt:dom/events/EventListener-handleEvent-cross-realm.html`
- `wpt:dom/events/EventListener-incumbent-global-1.sub.html`
- `wpt:dom/events/EventListener-incumbent-global-2.sub.html`
- `wpt:dom/events/event-global-extra.window.js` (cross-Realm subtests)
- `wpt:dom/events/event-global-is-still-set-when-reporting-exception-onerror.html`
- `wpt:dom/abort/reason-constructor.html`

No existing PASS tests may regress.

#### Phase 4: createHTMLDocument / DOMParser / createElementNS

**Verification:** These tests must flip from FAIL to PASS:
- `wpt:dom/nodes/DOMImplementation-createHTMLDocument.html`
- `wpt:dom/nodes/Document-createElement-namespace.html`
- `wpt:dom/nodes/Document-createElementNS.html`
- `wpt:dom/nodes/Comment-constructor.html` (cross-frame subtests)
- `wpt:dom/nodes/insertion-removing-steps/Node-appendChild-script-and-default-style-meta-from-fragment.tentative.html`

#### Phase 5: AbortSignal completion

**Verification:** These tests must flip from FAIL to PASS:
- `wpt:dom/abort/abort-signal-any.any.js`
- `wpt:dom/abort/event.any.js`
- `wpt:dom/abort/reason-constructor.html` (if not already fixed in Phase 3)
- `wpt:dom/abort/timeout.any.js`

### How to start (for a cold session)

1. Read this spec (SPEC-DOM-V2.md)
2. Read `CLAUDE.md` for project conventions (TDD, ratchet workflow, no shortcuts)
3. Check current HWM: `cargo run -p test-runner`
4. Pick Phase 1 or Phase 2 (they're independent)
5. Create `crates/engine/src/dom_v2/mod.rs` and the phase's files
6. Implement until the phase's target tests pass
7. Run full regression to confirm no breakage
8. Commit

---

## 12. Design Principles

1. **Rust owns the truth.** The DOM tree, event dispatch state, listener registry, and Document metadata live in Rust. JS is a view layer that calls into Rust via native functions and receives callbacks.

2. **Per-Realm JS isolation.** Each Realm has its own Context, prototypes, and wrapper cache. Cross-realm access creates wrappers in the accessing Realm's context.

3. **Events are state machines, not bags of properties.** The dispatch flag, propagation flags, and phase are internal Rust state. JS can read them (via getters) but cannot violate the state machine invariants.

4. **Listeners are Rust data.** Stored, deduplicated, and iterated in Rust. JS callbacks are referenced by ID into a per-Realm callback table. This eliminates the closure-scope IIFE pattern for listener storage.

5. **Documents are born with identity.** No Document exists without URL, content type, and Realm. These are set at creation time and immutable thereafter (URL can change for pushState, but that's a navigation concern).

6. **Adoption is explicit.** Nodes don't silently float between Documents. Every cross-Document node movement goes through the adopt algorithm, which updates ownerDocument recursively and fires adoption callbacks.

7. **Trusted is unforgeable.** `isTrusted` is set in Rust at event creation and cannot be modified from JS. No defineProperty, no prototype override, no proxy can change it.
