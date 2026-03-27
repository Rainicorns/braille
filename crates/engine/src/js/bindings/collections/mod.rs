//! JS bindings for NodeList and HTMLCollection interfaces.
//!
//! NodeList: returned by childNodes (live) and querySelectorAll (static).
//! HTMLCollection: returned by children, getElementsByTagName, getElementsByClassName (live).
//!
//! Both use JS Proxy objects to support live bracket-index access ([0], [1], etc.)
//! as well as `length`, `item()`, and iterator methods.
//!
//! IMPORTANT: Proxy creation uses pre-built factory functions (stored in per-realm state)
//! instead of context.eval(). This avoids a Boa bug where eval() inside native functions
//! can corrupt the calling scope's variable environment (index-out-of-bounds in DefInitVar).

mod domstringmap;
mod htmlcollection;
mod namednodemap;
mod nodelist;
mod register;

pub(crate) use domstringmap::create_live_domstringmap;
pub(crate) use htmlcollection::{
    create_live_htmlcollection, create_live_htmlcollection_by_class,
    create_live_htmlcollection_by_tag, create_live_htmlcollection_by_tag_name_ns,
};
pub(crate) use namednodemap::create_live_namednodemap;
pub(crate) use nodelist::{create_live_nodelist, create_static_nodelist};
pub(crate) use register::register_collections;

#[cfg(test)]
mod tests {
    use crate::Engine;

    /// Verify that NodeList proxy works correctly even inside complex JS scopes.
    /// This was previously failing due to a Boa bug where context.eval() called from
    /// native functions corrupted the calling scope's variable environment.
    #[test]
    fn wpt_iterator_in_complex_scope() {
        let mut engine = Engine::new();

        engine.load_html(
            r#"<!DOCTYPE html>
<meta charset=utf-8>
<title>Debug</title>
<div id="test"><span>1</span><span>2</span></div>
"#,
        );

        // Run the full iterator test inside a try/catch (like the WPT harness does)
        let result = engine
            .eval_js(
                r#"
(function() {
    var result = { status: 0, message: "" };
    try {
        var node = document.createElement("div");
        var kid1 = document.createElement("p");
        var kid2 = document.createTextNode("hey");
        var kid3 = document.createElement("span");
        node.appendChild(kid1);
        node.appendChild(kid2);
        node.appendChild(kid3);

        var list = node.childNodes;

        // Spread
        var spread = [...list];
        if (spread.length !== 3) throw new Error("spread length: " + spread.length);
        if (spread[0] !== kid1) throw new Error("spread[0] wrong");

        // keys
        var keys = list.keys();
        if (keys instanceof Array) throw new Error("keys instanceof Array");
        keys = [...keys];
        if (keys.length !== 3 || keys[0] !== 0 || keys[1] !== 1 || keys[2] !== 2)
            throw new Error("keys wrong: " + JSON.stringify(keys));

        // values
        var values = list.values();
        values = [...values];
        if (values.length !== 3 || values[0] !== kid1)
            throw new Error("values wrong");

        // entries
        var entries = list.entries();
        entries = [...entries];
        if (entries.length !== 3) throw new Error("entries wrong");

        // forEach
        var cur = 0;
        var thisObj = {};
        list.forEach(function(value, key, listObj) {
            if (listObj !== list) throw new Error("listObj !== list");
            if (this !== thisObj) throw new Error("this !== thisObj");
            cur++;
        }, thisObj);
        if (cur !== 3) throw new Error("forEach count: " + cur);

        // Identity checks
        if (list[Symbol.iterator] !== Array.prototype[Symbol.iterator])
            throw new Error("Symbol.iterator identity");
        if (list.keys !== Array.prototype.keys)
            throw new Error("keys identity");
        if (list.forEach !== Array.prototype.forEach)
            throw new Error("forEach identity");

    } catch(e) {
        result.status = 1;
        result.message = e.message || String(e);
    }
    return JSON.stringify(result);
})()
        "#,
            )
            .unwrap();

        eprintln!("Result: {}", result);
        assert!(result.contains("\"status\":0"), "Test failed: {}", result);
    }

    #[test]
    fn htmlcollection_children_named_props() {
        let mut engine = Engine::new();
        engine.load_html(
            r#"<!DOCTYPE html>
<div id="test"><img><img id=foo><img id=foo><img name="bar"></div>"#,
        );
        let result = engine
            .eval_js(
                r#"
(function() {
    var container = document.getElementById("test");
    var child = document.createElementNS("", "img");
    child.setAttribute("id", "baz");
    container.appendChild(child);
    child = document.createElementNS("", "img");
    child.setAttribute("name", "qux");
    container.appendChild(child);

    var list = container.children;
    var errors = [];

    // children.length should be 6
    if (list.length !== 6) errors.push("length=" + list.length);

    // namespaceURI: parsed element = xhtml, createElementNS("") = null
    if (list[0].namespaceURI !== "http://www.w3.org/1999/xhtml")
        errors.push("parsed ns=" + list[0].namespaceURI);
    if (list[4].namespaceURI !== null)
        errors.push("createElementNS ns=" + list[4].namespaceURI);

    // for..in + hasOwnProperty should only yield numeric indices
    var forIn = [];
    for (var p in list) {
        if (list.hasOwnProperty(p)) forIn.push(p);
    }
    if (forIn.length !== 6) errors.push("forIn=" + JSON.stringify(forIn));

    // Object.getOwnPropertyNames should include named props (but not qux)
    var own = Object.getOwnPropertyNames(list);
    if (own.indexOf("foo") === -1) errors.push("missing foo in ownPropertyNames");
    if (own.indexOf("bar") === -1) errors.push("missing bar in ownPropertyNames");
    if (own.indexOf("baz") === -1) errors.push("missing baz in ownPropertyNames");
    if (own.indexOf("qux") !== -1) errors.push("qux should not be in ownPropertyNames");

    return errors.length === 0 ? "ok" : errors.join("; ");
})()
"#,
            )
            .unwrap();
        assert_eq!(result, "ok", "HTMLCollection test failed: {}", result);
    }
}
