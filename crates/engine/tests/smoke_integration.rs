//! Smoke integration tests that exercise the full DOM API surface in realistic
//! scenarios. Each test combines multiple APIs to simulate real-world framework
//! patterns and user workflows.
//!
//! These are true integration tests: they only use the public Engine API
//! (load_html, snapshot, handle_click, handle_type, handle_select, etc.)
//! and embed all JS inside <script> tags. Results are verified through
//! accessibility snapshots and Engine interaction methods.
//!
//! NOTE: The accessibility tree only renders elements with semantic roles
//! (heading, paragraph, list, listitem, link, button, input, form, nav, main,
//! etc.). Plain <div>/<span> text is not shown. We use <p> to display JS
//! results in the a11y snapshot.

use braille_engine::Engine;
use braille_wire::SnapMode;

// ---------------------------------------------------------------------------
// 1. React-like reconciler: build initial DOM, "re-render" by clearing and
//    rebuilding children, verify final state via snapshot
// ---------------------------------------------------------------------------

#[test]
fn react_like_reconciler_simulation() {
    let html = r#"
    <html><body>
      <div id="root"></div>
      <script>
        function render(items) {
            var root = document.getElementById("root");
            root.innerHTML = "";
            var ul = document.createElement("ul");
            for (var i = 0; i < items.length; i++) {
                var li = document.createElement("li");
                li.textContent = items[i];
                li.setAttribute("data-index", String(i));
                ul.appendChild(li);
            }
            root.appendChild(ul);
        }

        // First render
        render(["Apple", "Banana", "Cherry"]);

        // Verify first render via innerHTML
        var firstHTML = document.getElementById("root").innerHTML;
        var firstHadApple = firstHTML.indexOf("Apple") !== -1;

        // Re-render with new data (simulating React state change)
        render(["Date", "Elderberry", "Fig", "Grape"]);

        // Re-render to empty
        render([]);

        // Render one more time for snapshot
        render(["Final Item"]);

        // Store verification in a paragraph (visible in a11y tree)
        var p = document.createElement("p");
        p.textContent = "firstHadApple=" + firstHadApple;
        document.body.appendChild(p);
      </script>
    </body></html>"#;

    let mut engine = Engine::new();
    engine.load_html(html);
    let snap = engine.snapshot(SnapMode::Accessibility);

    // After final render, should see "Final Item"
    assert!(snap.contains("Final Item"), "should see final render: {}", snap);
    // Old items should not be in the snapshot as list items
    // (Note: "Apple" substring exists inside "firstHadApple", so check for the listitem form)
    assert!(!snap.contains("listitem \"Apple\""), "Apple listitem should be gone: {}", snap);
    assert!(!snap.contains("listitem \"Grape\""), "Grape listitem should be gone: {}", snap);
    // Verify first render did contain Apple
    assert!(snap.contains("firstHadApple=true"), "first render should have had Apple: {}", snap);
}

// ---------------------------------------------------------------------------
// 2. Svelte-like direct DOM: build DOM imperatively, add event listeners,
//    dispatch events, verify handlers ran
// ---------------------------------------------------------------------------

#[test]
fn svelte_like_direct_dom_manipulation() {
    let html = r#"
    <html><body>
      <div id="app"></div>
      <script>
        var log = [];
        var container = document.createElement("div");

        var display = document.createElement("p");
        display.setAttribute("id", "display");
        display.textContent = "Count: 0";
        container.appendChild(display);

        var incrementBtn = document.createElement("button");
        incrementBtn.textContent = "+1";
        container.appendChild(incrementBtn);

        var decrementBtn = document.createElement("button");
        decrementBtn.textContent = "-1";
        container.appendChild(decrementBtn);

        document.getElementById("app").appendChild(container);

        var count = 0;
        incrementBtn.addEventListener("click", function(e) {
            count++;
            display.textContent = "Count: " + count;
            log.push("inc:" + count);
        });
        decrementBtn.addEventListener("click", function(e) {
            count--;
            display.textContent = "Count: " + count;
            log.push("dec:" + count);
        });

        // Simulate clicks by dispatching events
        incrementBtn.dispatchEvent(new Event("click"));
        incrementBtn.dispatchEvent(new Event("click"));
        incrementBtn.dispatchEvent(new Event("click"));
        decrementBtn.dispatchEvent(new Event("click"));

        // Store the log in a paragraph
        var logP = document.createElement("p");
        logP.textContent = "LOG=" + log.join(",");
        document.body.appendChild(logP);
      </script>
    </body></html>"#;

    let mut engine = Engine::new();
    engine.load_html(html);
    let snap = engine.snapshot(SnapMode::Accessibility);

    assert!(snap.contains("Count: 2"), "counter should show 2: {}", snap);
    assert!(snap.contains("LOG=inc:1,inc:2,inc:3,dec:2"), "log should record events: {}", snap);
    assert!(snap.contains("+1"), "increment button: {}", snap);
    assert!(snap.contains("-1"), "decrement button: {}", snap);
}

// ---------------------------------------------------------------------------
// 3. Form workflow: create a form with inputs, set values via JS, read them
//    back, verify form.elements collection
// ---------------------------------------------------------------------------

#[test]
fn form_workflow_with_js_values() {
    let html = r##"
    <html><body>
      <form id="signup" action="/register" method="post">
        <input id="username" type="text" name="username" />
        <input id="email" type="email" name="email" />
        <input id="password" type="password" name="password" />
        <input id="agree" type="checkbox" name="agree" />
        <select id="role" name="role">
          <option value="user">User</option>
          <option value="admin">Admin</option>
          <option value="mod">Moderator</option>
        </select>
        <textarea id="bio" name="bio"></textarea>
        <button type="submit">Register</button>
      </form>
      <script>
        document.getElementById("username").value = "alice";
        document.getElementById("email").value = "alice@example.com";
        document.getElementById("password").value = "s3cret";
        document.getElementById("agree").checked = true;
        document.getElementById("role").value = "admin";
        document.getElementById("bio").value = "Hello, I am Alice.";

        // Verify form.elements and form properties
        var elems = document.getElementById("signup").elements;
        var tags = [];
        for (var i = 0; i < elems.length; i++) {
            tags.push(elems[i].tagName);
        }

        var p = document.createElement("p");
        p.textContent = "elems=" + elems.length + " tags=" + tags.join(",") +
            " action=" + document.getElementById("signup").action +
            " method=" + document.getElementById("signup").method;
        document.body.appendChild(p);
      </script>
    </body></html>"##;

    let mut engine = Engine::new();
    engine.load_html(html);
    let snap = engine.snapshot(SnapMode::Accessibility);

    // Verify the snapshot shows set values
    assert!(snap.contains("value=\"alice\""), "username should be alice: {}", snap);
    assert!(snap.contains("value=\"alice@example.com\""), "email value: {}", snap);
    assert!(snap.contains("value=\"s3cret\""), "password value: {}", snap);
    assert!(snap.contains("Hello, I am Alice."), "bio value: {}", snap);

    // Verify form properties from paragraph output
    assert!(snap.contains("elems=7"), "form.elements count: {}", snap);
    assert!(snap.contains("tags=INPUT,INPUT,INPUT,INPUT,SELECT,TEXTAREA,BUTTON"), "form element tags: {}", snap);
    assert!(snap.contains("action=/register"), "form action: {}", snap);
    assert!(snap.contains("method=post"), "form method: {}", snap);
}

// ---------------------------------------------------------------------------
// 4. Event delegation: add listener on parent, dispatch on child, verify
//    event bubbles up with correct target/currentTarget
// ---------------------------------------------------------------------------

#[test]
fn event_delegation_pattern() {
    let html = r#"
    <html><body>
      <ul id="list">
        <li id="item1">First</li>
        <li id="item2">Second</li>
        <li id="item3">Third</li>
      </ul>
      <script>
        var clickedItems = [];
        var targetTags = [];

        document.getElementById("list").addEventListener("click", function(e) {
            clickedItems.push(e.target.textContent);
            targetTags.push(e.target.tagName + "/" + e.currentTarget.tagName);
        });

        document.getElementById("item2").dispatchEvent(new Event("click", { bubbles: true }));
        document.getElementById("item1").dispatchEvent(new Event("click", { bubbles: true }));
        document.getElementById("item3").dispatchEvent(new Event("click", { bubbles: true }));

        var p = document.createElement("p");
        p.textContent = "clicked=" + clickedItems.join(",") + " tags=" + targetTags[0];
        document.body.appendChild(p);
      </script>
    </body></html>"#;

    let mut engine = Engine::new();
    engine.load_html(html);
    let snap = engine.snapshot(SnapMode::Accessibility);

    assert!(snap.contains("clicked=Second,First,Third"), "delegated handler: {}", snap);
    let snap_lower = snap.to_ascii_lowercase();
    assert!(snap_lower.contains("tags=li/ul"), "target/currentTarget tags: {}", snap);
}

// ---------------------------------------------------------------------------
// 5. Dynamic styling: set element.style properties, verify inline style
//    and getComputedStyle
// ---------------------------------------------------------------------------

#[test]
fn dynamic_styling_and_computed_style() {
    let html = r##"
    <html><body>
      <style>
        #box { color: blue; display: block; }
      </style>
      <div id="box">Styled Box</div>
      <script>
        var box = document.getElementById("box");

        box.style.setProperty("color", "red");
        box.style.setProperty("font-size", "24px");
        box.style.setProperty("margin", "10px");

        var results = [];
        results.push("color=" + box.style.getPropertyValue("color"));
        results.push("fontSize=" + box.style.getPropertyValue("font-size"));
        results.push("margin=" + box.style.getPropertyValue("margin"));
        results.push("length=" + box.style.length);
        results.push("item0=" + box.style.item(0));

        var cs = getComputedStyle(box);
        var computedDisplay = cs.getPropertyValue("display");
        results.push("hasComputed=" + (typeof computedDisplay === "string" ? "yes" : "no"));

        box.style.setProperty("display", "none");
        results.push("inlineDisplay=" + box.style.getPropertyValue("display"));

        var oldMargin = box.style.removeProperty("margin");
        results.push("removedMargin=" + oldMargin);

        var p = document.createElement("p");
        p.textContent = results.join("|");
        document.body.appendChild(p);
      </script>
    </body></html>"##;

    let mut engine = Engine::new();
    engine.load_html(html);
    let snap = engine.snapshot(SnapMode::Accessibility);

    assert!(snap.contains("color=red"), "inline color: {}", snap);
    assert!(snap.contains("fontSize=24px"), "inline font-size: {}", snap);
    assert!(snap.contains("margin=10px"), "inline margin: {}", snap);
    assert!(snap.contains("length=3"), "style length: {}", snap);
    assert!(snap.contains("item0=color"), "style item 0: {}", snap);
    assert!(snap.contains("hasComputed=yes"), "computed style accessible: {}", snap);
    assert!(snap.contains("inlineDisplay=none"), "inline display: {}", snap);
    assert!(snap.contains("removedMargin=10px"), "removed margin: {}", snap);
}

// ---------------------------------------------------------------------------
// 6. classList toggle workflow
// ---------------------------------------------------------------------------

#[test]
fn class_list_toggle_workflow() {
    let html = r#"
    <html><body>
      <div id="card" class="card">
        <h2>Card Title</h2>
      </div>
      <script>
        var card = document.getElementById("card");
        var results = [];

        card.classList.add("active", "highlighted");
        results.push("afterAdd=" + card.className);

        var t1 = card.classList.toggle("highlighted");
        results.push("toggleOff=" + t1 + ":" + card.className);

        var t2 = card.classList.toggle("featured");
        results.push("toggleOn=" + t2 + ":" + card.className);

        card.classList.remove("active");
        results.push("afterRemove=" + card.className);

        results.push("hasCard=" + card.classList.contains("card"));
        results.push("hasFeatured=" + card.classList.contains("featured"));
        results.push("hasActive=" + card.classList.contains("active"));

        var found = document.querySelector(".card");
        results.push("foundTag=" + found.tagName);

        var p = document.createElement("p");
        p.textContent = results.join("|");
        document.body.appendChild(p);
      </script>
    </body></html>"#;

    let mut engine = Engine::new();
    engine.load_html(html);
    let snap = engine.snapshot(SnapMode::Accessibility);

    assert!(snap.contains("afterAdd=card active highlighted"), "after add: {}", snap);
    assert!(snap.contains("toggleOff=false:card active"), "toggle off: {}", snap);
    assert!(snap.contains("toggleOn=true:card active featured"), "toggle on: {}", snap);
    assert!(snap.contains("afterRemove=card featured"), "after remove: {}", snap);
    assert!(snap.contains("hasCard=true"), "contains card: {}", snap);
    assert!(snap.contains("hasFeatured=true"), "contains featured: {}", snap);
    assert!(snap.contains("hasActive=false"), "not contains active: {}", snap);
    assert!(snap.contains("foundTag=DIV"), "querySelector: {}", snap);
}

// ---------------------------------------------------------------------------
// 7. innerHTML round-trip
// ---------------------------------------------------------------------------

#[test]
fn inner_html_round_trip() {
    let html = r#"
    <html><body>
      <div id="container"></div>
      <script>
        var c = document.getElementById("container");
        c.innerHTML = '<h1>Welcome</h1><p>Hello world</p>';

        var h1 = document.querySelector("h1");
        h1.textContent = "Modified Welcome";

        var p = document.querySelector("p");
        p.textContent = "Modified paragraph";

        var inner = c.innerHTML;
        var hasModified = inner.indexOf("Modified Welcome") !== -1;

        c.insertAdjacentHTML("beforeend", "<p>Added paragraph</p>");

        var resultP = document.createElement("p");
        resultP.textContent = "roundTrip=" + hasModified;
        document.body.appendChild(resultP);
      </script>
    </body></html>"#;

    let mut engine = Engine::new();
    engine.load_html(html);
    let snap = engine.snapshot(SnapMode::Accessibility);

    assert!(snap.contains("Modified Welcome"), "modified heading: {}", snap);
    assert!(snap.contains("Modified paragraph"), "modified paragraph: {}", snap);
    assert!(snap.contains("Added paragraph"), "insertAdjacentHTML: {}", snap);
    assert!(snap.contains("roundTrip=true"), "innerHTML reflects changes: {}", snap);
}

// ---------------------------------------------------------------------------
// 8. Dataset manipulation
// ---------------------------------------------------------------------------

#[test]
fn dataset_manipulation() {
    let html = r#"
    <html><body>
      <div id="product" data-product-id="42" data-category="electronics" data-in-stock="true"></div>
      <script>
        var el = document.getElementById("product");
        var results = [];

        results.push("productId=" + el.dataset.productId);
        results.push("category=" + el.dataset.category);
        results.push("inStock=" + el.dataset.inStock);

        el.setAttribute("data-price", "99.99");
        el.setAttribute("data-discount-rate", "0.15");

        results.push("price=" + el.dataset.price);
        results.push("discountRate=" + el.dataset.discountRate);
        results.push("getAttrId=" + el.getAttribute("data-product-id"));

        el.removeAttribute("data-in-stock");
        var ds = el.dataset;
        results.push("removedInStock=" + (typeof ds.inStock === "undefined" ? "gone" : ds.inStock));

        var p = document.createElement("p");
        p.textContent = results.join("|");
        document.body.appendChild(p);
      </script>
    </body></html>"#;

    let mut engine = Engine::new();
    engine.load_html(html);
    let snap = engine.snapshot(SnapMode::Accessibility);

    assert!(snap.contains("productId=42"), "productId: {}", snap);
    assert!(snap.contains("category=electronics"), "category: {}", snap);
    assert!(snap.contains("inStock=true"), "inStock: {}", snap);
    assert!(snap.contains("price=99.99"), "price: {}", snap);
    assert!(snap.contains("discountRate=0.15"), "discountRate: {}", snap);
    assert!(snap.contains("getAttrId=42"), "getAttribute: {}", snap);
    assert!(snap.contains("removedInStock=gone"), "removed attr: {}", snap);
}

// ---------------------------------------------------------------------------
// 9. Select/option interaction
// ---------------------------------------------------------------------------

#[test]
fn select_option_interaction() {
    let html = r##"
    <html><body>
      <select id="colors">
        <option id="opt-r" value="red">Red</option>
        <option id="opt-g" value="green">Green</option>
        <option id="opt-b" value="blue" selected>Blue</option>
      </select>
      <script>
        var sel = document.getElementById("colors");
        var results = [];

        results.push("initVal=" + sel.value);
        results.push("initIdx=" + sel.selectedIndex);
        results.push("optLen=" + sel.options.length);
        results.push("optBtext=" + document.getElementById("opt-b").text);
        results.push("optBsel=" + document.getElementById("opt-b").selected);

        sel.selectedIndex = 0;
        results.push("idx0val=" + sel.value);
        results.push("idx0optR=" + document.getElementById("opt-r").selected);

        sel.value = "green";
        results.push("greenIdx=" + sel.selectedIndex);
        results.push("greenOptG=" + document.getElementById("opt-g").selected);

        document.getElementById("opt-g").text = "Lime Green";
        results.push("newText=" + document.getElementById("opt-g").text);

        document.getElementById("opt-r").value = "crimson";
        results.push("newVal=" + document.getElementById("opt-r").value);

        var p = document.createElement("p");
        p.textContent = results.join("|");
        document.body.appendChild(p);
      </script>
    </body></html>"##;

    let mut engine = Engine::new();
    engine.load_html(html);
    let snap = engine.snapshot(SnapMode::Accessibility);

    assert!(snap.contains("initVal=blue"), "initial value: {}", snap);
    assert!(snap.contains("initIdx=2"), "initial index: {}", snap);
    assert!(snap.contains("optLen=3"), "options length: {}", snap);
    assert!(snap.contains("optBtext=Blue"), "option text: {}", snap);
    assert!(snap.contains("optBsel=true"), "option selected: {}", snap);
    assert!(snap.contains("idx0val=red"), "after index 0: {}", snap);
    assert!(snap.contains("idx0optR=true"), "opt-r selected: {}", snap);
    assert!(snap.contains("greenIdx=1"), "after green: {}", snap);
    assert!(snap.contains("greenOptG=true"), "opt-g selected: {}", snap);
    assert!(snap.contains("newText=Lime Green"), "text changed: {}", snap);
    assert!(snap.contains("newVal=crimson"), "value changed: {}", snap);
}

// ---------------------------------------------------------------------------
// 10. DOM tree walking
// ---------------------------------------------------------------------------

#[test]
fn dom_tree_walking() {
    // NOTE: No whitespace between elements to avoid text nodes messing up
    // firstChild/lastChild/nextSibling/previousSibling traversal
    let html = r#"<html><body><div id="root"><div id="a"><span id="a1">First</span><span id="a2">Second</span></div><div id="b"><span id="b1">Third</span></div></div><script>
        var results = [];

        results.push("rootFirstId=" + document.getElementById("root").firstChild.getAttribute("id"));
        results.push("aFirst=" + document.getElementById("a").firstChild.getAttribute("id"));
        results.push("aLast=" + document.getElementById("a").lastChild.getAttribute("id"));
        results.push("a1Next=" + document.getElementById("a1").nextSibling.getAttribute("id"));
        results.push("a2Prev=" + document.getElementById("a2").previousSibling.getAttribute("id"));
        results.push("a1Parent=" + document.getElementById("a1").parentNode.getAttribute("id"));
        results.push("aParent=" + document.getElementById("a").parentNode.getAttribute("id"));
        results.push("aNext=" + document.getElementById("a").nextSibling.getAttribute("id"));
        results.push("bPrev=" + document.getElementById("b").previousSibling.getAttribute("id"));
        results.push("rootChildren=" + document.getElementById("root").children.length);
        results.push("nodeType=" + document.getElementById("a").nodeType);
        results.push("tagName=" + document.getElementById("a").tagName);

        // Full tree walk using children array (element-only)
        var texts = [];
        function walk(node) {
            if (!node) return;
            if (node.nodeType === 1) {
                // Check for text content in leaf elements
                var child = node.firstChild;
                while (child) {
                    if (child.nodeType === 3) {
                        var t = child.textContent.trim();
                        if (t) texts.push(t);
                    } else if (child.nodeType === 1) {
                        walk(child);
                    }
                    child = child.nextSibling;
                }
            }
        }
        walk(document.getElementById("root"));
        results.push("walked=" + texts.join(","));

        var p = document.createElement("p");
        p.textContent = results.join("|");
        document.body.appendChild(p);
      </script></body></html>"#;

    let mut engine = Engine::new();
    engine.load_html(html);
    let snap = engine.snapshot(SnapMode::Accessibility);

    assert!(snap.contains("rootFirstId=a"), "root first child: {}", snap);
    assert!(snap.contains("aFirst=a1"), "#a first child: {}", snap);
    assert!(snap.contains("aLast=a2"), "#a last child: {}", snap);
    assert!(snap.contains("a1Next=a2"), "#a1 next sibling: {}", snap);
    assert!(snap.contains("a2Prev=a1"), "#a2 prev sibling: {}", snap);
    assert!(snap.contains("a1Parent=a"), "#a1 parent: {}", snap);
    assert!(snap.contains("aParent=root"), "#a parent: {}", snap);
    assert!(snap.contains("aNext=b"), "#a next sibling: {}", snap);
    assert!(snap.contains("bPrev=a"), "#b prev sibling: {}", snap);
    assert!(snap.contains("rootChildren=2"), "root children: {}", snap);
    assert!(snap.contains("nodeType=1"), "element nodeType: {}", snap);
    assert!(snap.contains("tagName=DIV"), "tagName: {}", snap);
    assert!(snap.contains("walked=First,Second,Third"), "tree walk: {}", snap);
}

// ---------------------------------------------------------------------------
// 11. Event listener once + removeEventListener
// ---------------------------------------------------------------------------

#[test]
fn event_listener_once_and_remove() {
    let html = r#"
    <html><body>
      <div id="emitter"></div>
      <script>
        var received = [];
        var emitter = document.getElementById("emitter");

        emitter.addEventListener("data", function(e) {
            received.push("once");
        }, { once: true });

        emitter.addEventListener("data", function(e) {
            received.push("regular");
        });

        // First dispatch: both fire
        emitter.dispatchEvent(new Event("data"));
        var afterFirst = received.join(",");

        // Second dispatch: only regular fires
        received = [];
        emitter.dispatchEvent(new Event("data"));
        var afterSecond = received.join(",");

        var p = document.createElement("p");
        p.textContent = "first=" + afterFirst + " second=" + afterSecond;
        document.body.appendChild(p);
      </script>
    </body></html>"#;

    let mut engine = Engine::new();
    engine.load_html(html);
    let snap = engine.snapshot(SnapMode::Accessibility);

    assert!(snap.contains("first=once,regular"), "first dispatch: {}", snap);
    assert!(snap.contains("second=regular"), "second dispatch (once removed): {}", snap);
}

// ---------------------------------------------------------------------------
// 12. Complex query selector workflow
// ---------------------------------------------------------------------------

#[test]
fn query_selector_complex_workflow() {
    let html = r#"
    <html><body>
      <div class="card" id="card1">
        <h2 class="title">Card One</h2>
        <p class="body">First card body</p>
      </div>
      <div class="card" id="card2">
        <h2 class="title">Card Two</h2>
        <p class="body">Second card body</p>
      </div>
      <div class="card" id="card3">
        <h2 class="title">Card Three</h2>
        <p class="body">Third card body</p>
      </div>
      <script>
        var results = [];
        results.push("first=" + document.querySelector(".card").getAttribute("id"));
        results.push("all=" + document.querySelectorAll(".card").length);
        results.push("firstTitle=" + document.querySelector(".card .title").textContent);
        results.push("card2Body=" + document.getElementById("card2").querySelector(".body").textContent);
        results.push("titleCount=" + document.getElementsByClassName("title").length);
        results.push("h2Count=" + document.getElementsByTagName("h2").length);

        var p = document.createElement("p");
        p.textContent = results.join("|");
        document.body.appendChild(p);
      </script>
    </body></html>"#;

    let mut engine = Engine::new();
    engine.load_html(html);
    let snap = engine.snapshot(SnapMode::Accessibility);

    assert!(snap.contains("first=card1"), "querySelector: {}", snap);
    assert!(snap.contains("all=3"), "querySelectorAll: {}", snap);
    assert!(snap.contains("firstTitle=Card One"), "descendant selector: {}", snap);
    assert!(snap.contains("card2Body=Second card body"), "scoped querySelector: {}", snap);
    assert!(snap.contains("titleCount=3"), "getElementsByClassName: {}", snap);
    assert!(snap.contains("h2Count=3"), "getElementsByTagName: {}", snap);
}

// ---------------------------------------------------------------------------
// 13. Mutation methods: insertBefore, replaceChild, removeChild, cloneNode
// ---------------------------------------------------------------------------

#[test]
fn dom_mutation_methods() {
    let html = r#"
    <html><body>
      <ul id="list">
        <li id="item1">First</li>
        <li id="item2">Second</li>
        <li id="item3">Third</li>
      </ul>
      <script>
        function getOrder() {
            var result = [];
            var list = document.getElementById("list");
            var child = list.firstChild;
            while (child) {
                if (child.nodeType === 1) result.push(child.textContent);
                child = child.nextSibling;
            }
            return result.join(",");
        }

        var results = [];
        results.push("initial=" + getOrder());

        // insertBefore
        var list = document.getElementById("list");
        var newItem = document.createElement("li");
        newItem.textContent = "OneHalf";
        list.insertBefore(newItem, document.getElementById("item2"));
        results.push("insert=" + getOrder());

        // removeChild
        list.removeChild(document.getElementById("item1"));
        results.push("remove=" + getOrder());

        // replaceChild
        var rep = document.createElement("li");
        rep.textContent = "Replacement";
        list.replaceChild(rep, document.getElementById("item2"));
        results.push("replace=" + getOrder());

        // cloneNode
        var clone = document.getElementById("item3").cloneNode(true);
        clone.textContent = "Clone";
        list.appendChild(clone);
        results.push("clone=" + getOrder());

        var p = document.createElement("p");
        p.textContent = results.join("|");
        document.body.appendChild(p);
      </script>
    </body></html>"#;

    let mut engine = Engine::new();
    engine.load_html(html);
    let snap = engine.snapshot(SnapMode::Accessibility);

    assert!(snap.contains("initial=First,Second,Third"), "initial: {}", snap);
    assert!(snap.contains("insert=First,OneHalf,Second,Third"), "after insert: {}", snap);
    assert!(snap.contains("remove=OneHalf,Second,Third"), "after remove: {}", snap);
    assert!(snap.contains("replace=OneHalf,Replacement,Third"), "after replace: {}", snap);
    assert!(snap.contains("clone=OneHalf,Replacement,Third,Clone"), "after clone: {}", snap);
}

// ---------------------------------------------------------------------------
// 14. HTMLElement properties
// ---------------------------------------------------------------------------

#[test]
fn html_element_properties() {
    let html = r#"
    <html><body>
      <div id="box"></div>
      <input id="inp" type="text" />
      <a id="link" href="/page">Link</a>
      <script>
        var results = [];
        results.push("divTab=" + document.getElementById("box").tabIndex);
        results.push("inputTab=" + document.getElementById("inp").tabIndex);
        results.push("linkTab=" + document.getElementById("link").tabIndex);

        document.getElementById("box").tabIndex = 3;
        results.push("divTabAfter=" + document.getElementById("box").tabIndex);

        var box = document.getElementById("box");
        box.title = "My tooltip";
        box.lang = "en-US";
        box.dir = "ltr";
        results.push("title=" + box.title);
        results.push("lang=" + box.lang);
        results.push("dir=" + box.dir);
        results.push("titleAttr=" + box.getAttribute("title"));

        var rect = box.getBoundingClientRect();
        results.push("rectW=" + rect.width);
        results.push("rectH=" + rect.height);

        document.getElementById("inp").focus();
        document.getElementById("inp").blur();
        results.push("focusBlur=ok");

        var clickLog = [];
        box.addEventListener("click", function(e) { clickLog.push(e.type); });
        box.click();
        results.push("clickEvent=" + clickLog[0]);

        var p = document.createElement("p");
        p.textContent = results.join("|");
        document.body.appendChild(p);
      </script>
    </body></html>"#;

    let mut engine = Engine::new();
    engine.load_html(html);
    let snap = engine.snapshot(SnapMode::Accessibility);

    assert!(snap.contains("divTab=-1"), "div tabIndex: {}", snap);
    assert!(snap.contains("inputTab=0"), "input tabIndex: {}", snap);
    assert!(snap.contains("linkTab=0"), "link tabIndex: {}", snap);
    assert!(snap.contains("divTabAfter=3"), "set tabIndex: {}", snap);
    assert!(snap.contains("title=My tooltip"), "title: {}", snap);
    assert!(snap.contains("lang=en-US"), "lang: {}", snap);
    assert!(snap.contains("dir=ltr"), "dir: {}", snap);
    assert!(snap.contains("titleAttr=My tooltip"), "title attr: {}", snap);
    assert!(snap.contains("rectW=0"), "rect width: {}", snap);
    assert!(snap.contains("rectH=0"), "rect height: {}", snap);
    assert!(snap.contains("focusBlur=ok"), "focus/blur: {}", snap);
    assert!(snap.contains("clickEvent=click"), "click event: {}", snap);
}

// ---------------------------------------------------------------------------
// 15. Anchor/form properties, hidden, createTextNode
// ---------------------------------------------------------------------------

#[test]
fn anchor_form_hidden_and_text_node() {
    let html = r#"
    <html><body>
      <a id="link" href="https://example.com">Example</a>
      <form id="form" action="/api" method="post">
        <input id="field" name="q" />
      </form>
      <div id="toggler"></div>
      <script>
        var results = [];

        results.push("href=" + document.getElementById("link").href);
        document.getElementById("link").href = "https://new.example.com";
        results.push("newHref=" + document.getElementById("link").href);

        results.push("action=" + document.getElementById("form").action);
        results.push("method=" + document.getElementById("form").method);

        document.getElementById("form").action = "/new-api";
        document.getElementById("form").method = "get";
        results.push("newAction=" + document.getElementById("form").action);
        results.push("newMethod=" + document.getElementById("form").method);

        var toggler = document.getElementById("toggler");
        results.push("hidden1=" + toggler.hidden);
        toggler.hidden = true;
        results.push("hidden2=" + toggler.hidden);
        results.push("hasHidden=" + toggler.hasAttribute("hidden"));
        toggler.hidden = false;
        results.push("hidden3=" + toggler.hidden);

        var text = document.createTextNode(" - appended");
        document.getElementById("link").appendChild(text);
        results.push("linkText=" + document.getElementById("link").textContent);

        var p = document.createElement("p");
        p.textContent = results.join("|");
        document.body.appendChild(p);
      </script>
    </body></html>"#;

    let mut engine = Engine::new();
    engine.load_html(html);
    let snap = engine.snapshot(SnapMode::Accessibility);

    assert!(snap.contains("href=https://example.com"), "initial href: {}", snap);
    assert!(snap.contains("newHref=https://new.example.com"), "updated href: {}", snap);
    assert!(snap.contains("action=/api"), "initial action: {}", snap);
    assert!(snap.contains("method=post"), "initial method: {}", snap);
    assert!(snap.contains("newAction=/new-api"), "updated action: {}", snap);
    assert!(snap.contains("newMethod=get"), "updated method: {}", snap);
    assert!(snap.contains("hidden1=false"), "initially not hidden: {}", snap);
    assert!(snap.contains("hidden2=true"), "set hidden: {}", snap);
    assert!(snap.contains("hasHidden=true"), "hasAttribute hidden: {}", snap);
    assert!(snap.contains("hidden3=false"), "unset hidden: {}", snap);
    assert!(snap.contains("linkText=Example - appended"), "createTextNode: {}", snap);
}

// ---------------------------------------------------------------------------
// 16. Window.document and console
// ---------------------------------------------------------------------------

#[test]
fn window_document_and_console() {
    let html = r#"
    <html><body>
      <div id="target"></div>
      <script>
        console.log("hello from script");
        console.warn("warning message");

        var el = window.document.createElement("p");
        el.textContent = "Created via window.document";
        window.document.getElementById("target").appendChild(el);
      </script>
    </body></html>"#;

    let mut engine = Engine::new();
    engine.load_html(html);
    let snap = engine.snapshot(SnapMode::Accessibility);

    assert!(snap.contains("Created via window.document"), "window.document works: {}", snap);
}

// ---------------------------------------------------------------------------
// 17. Full page lifecycle: load, interact, re-load
// ---------------------------------------------------------------------------

#[test]
fn full_page_lifecycle() {
    let html1 = r#"
    <html><body>
      <h1>Todo App</h1>
      <input id="input" type="text" />
      <button id="add">Add</button>
      <ul id="todos"></ul>
      <script>
        function addTodo(text) {
            var ul = document.getElementById("todos");
            var li = document.createElement("li");
            li.textContent = text;
            ul.appendChild(li);
        }
        addTodo("Buy milk");
        addTodo("Walk the dog");
        addTodo("Write tests");
      </script>
    </body></html>"#;

    let mut engine = Engine::new();
    engine.load_html(html1);

    let snap1 = engine.snapshot(SnapMode::Accessibility);
    assert!(snap1.contains("Buy milk"), "todo 1: {}", snap1);
    assert!(snap1.contains("Walk the dog"), "todo 2: {}", snap1);
    assert!(snap1.contains("Write tests"), "todo 3: {}", snap1);

    // Type into the input
    engine.handle_type("@e1", "New todo item").unwrap();
    let snap2 = engine.snapshot(SnapMode::Accessibility);
    assert!(snap2.contains("New todo item"), "typed text: {}", snap2);

    // Navigate to a new page
    let html2 = r#"
    <html><body>
      <h1>Settings</h1>
      <form>
        <input type="text" name="name" />
        <button>Save</button>
      </form>
    </body></html>"#;

    engine.load_html(html2);
    let snap3 = engine.snapshot(SnapMode::Accessibility);

    assert!(!snap3.contains("Todo"), "old content gone: {}", snap3);
    assert!(!snap3.contains("Buy milk"), "old todos gone: {}", snap3);
    assert!(snap3.contains("Settings"), "new heading: {}", snap3);
}

// ---------------------------------------------------------------------------
// 18. CSS display:none hides elements from accessibility tree
// ---------------------------------------------------------------------------

#[test]
fn css_display_none_hides_from_a11y_tree() {
    let html = r##"
    <html><body>
      <style>
        .hidden { display: none; }
        .invisible { visibility: hidden; }
      </style>
      <h1>Visible Heading</h1>
      <p>Visible paragraph</p>
      <div class="hidden">
        <h2>Hidden Heading</h2>
        <p>This should not appear</p>
      </div>
      <div class="invisible">
        <h2>Invisible Heading</h2>
      </div>
      <p>Also visible</p>
    </body></html>"##;

    let mut engine = Engine::new();
    engine.load_html(html);
    let snap = engine.snapshot(SnapMode::Accessibility);

    assert!(snap.contains("Visible Heading"), "visible heading: {}", snap);
    assert!(snap.contains("Visible paragraph"), "visible paragraph: {}", snap);
    assert!(snap.contains("Also visible"), "also visible: {}", snap);
    assert!(!snap.contains("Hidden Heading"), "display:none: {}", snap);
    assert!(!snap.contains("This should not appear"), "display:none content: {}", snap);
    assert!(!snap.contains("Invisible Heading"), "visibility:hidden: {}", snap);
}

// ---------------------------------------------------------------------------
// 19. Input properties comprehensive
// ---------------------------------------------------------------------------

#[test]
fn input_properties_comprehensive() {
    let html = r#"
    <html><body>
      <input id="inp" />
      <script>
        var inp = document.getElementById("inp");
        var results = [];

        results.push("defaultType=" + inp.type);
        inp.type = "email";
        results.push("newType=" + inp.type);

        results.push("defaultDisabled=" + inp.disabled);
        inp.disabled = true;
        results.push("disabledTrue=" + inp.disabled);
        inp.disabled = false;
        results.push("disabledFalse=" + inp.disabled);

        results.push("defaultName=" + (inp.name === "" ? "empty" : inp.name));
        inp.name = "email_field";
        results.push("newName=" + inp.name);

        inp.placeholder = "Enter email";
        results.push("placeholder=" + inp.placeholder);

        inp.value = "test@example.com";
        results.push("value=" + inp.value);
        results.push("valueAttr=" + inp.getAttribute("value"));

        inp.type = "checkbox";
        results.push("defaultChecked=" + inp.checked);
        inp.checked = true;
        results.push("checkedTrue=" + inp.checked);

        var p = document.createElement("p");
        p.textContent = results.join("|");
        document.body.appendChild(p);
      </script>
    </body></html>"#;

    let mut engine = Engine::new();
    engine.load_html(html);
    let snap = engine.snapshot(SnapMode::Accessibility);

    assert!(snap.contains("defaultType=text"), "default type: {}", snap);
    assert!(snap.contains("newType=email"), "set type: {}", snap);
    assert!(snap.contains("defaultDisabled=false"), "default disabled: {}", snap);
    assert!(snap.contains("disabledTrue=true"), "set disabled: {}", snap);
    assert!(snap.contains("disabledFalse=false"), "unset disabled: {}", snap);
    assert!(snap.contains("defaultName=empty"), "default name: {}", snap);
    assert!(snap.contains("newName=email_field"), "set name: {}", snap);
    assert!(snap.contains("placeholder=Enter email"), "placeholder: {}", snap);
    assert!(snap.contains("value=test@example.com"), "value: {}", snap);
    assert!(snap.contains("valueAttr=test@example.com"), "value attr: {}", snap);
    assert!(snap.contains("defaultChecked=false"), "default checked: {}", snap);
    assert!(snap.contains("checkedTrue=true"), "set checked: {}", snap);
}

// ---------------------------------------------------------------------------
// 20. Event preventDefault and dispatchEvent return value
// ---------------------------------------------------------------------------

#[test]
fn event_prevent_default_and_return_value() {
    let html = r#"
    <html><body>
      <button id="btn">Submit</button>
      <script>
        var btn = document.getElementById("btn");

        btn.addEventListener("submit", function(e) { e.preventDefault(); });
        btn.addEventListener("info", function(e) { e.preventDefault(); });

        var r1 = btn.dispatchEvent(new Event("submit", { cancelable: true }));
        var r2 = btn.dispatchEvent(new Event("info", { cancelable: false }));
        var r3 = btn.dispatchEvent(new Event("noop"));

        var p = document.createElement("p");
        p.textContent = "cancelable=" + r1 + " nonCancelable=" + r2 + " noListeners=" + r3;
        document.body.appendChild(p);
      </script>
    </body></html>"#;

    let mut engine = Engine::new();
    engine.load_html(html);
    let snap = engine.snapshot(SnapMode::Accessibility);

    assert!(snap.contains("cancelable=false"), "cancelable+preventDefault: {}", snap);
    assert!(snap.contains("nonCancelable=true"), "non-cancelable: {}", snap);
    assert!(snap.contains("noListeners=true"), "no listeners: {}", snap);
}
