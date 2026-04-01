//! Tests for Element.children HTMLCollection — named access, ownKeys, namespace handling.
use braille_engine::Engine;

fn engine_with_html(html: &str) -> Engine {
    let mut e = Engine::new();
    e.load_html(html);
    e
}

#[test]
fn children_own_property_names_with_non_html_namespace() {
    // Exact WPT test setup: setup() runs first, then test functions
    let mut e = engine_with_html(r#"<!DOCTYPE html>
<html><body>
<div id="log"></div>
<div id="test"><img><img id="foo"><img id="foo"><img name="bar"></div>
</body></html>"#);

    // setup() — same as WPT
    e.eval_js(r#"
        var container = document.getElementById("test");
        var child = document.createElementNS("", "img");
        child.setAttribute("id", "baz");
        container.appendChild(child);
        child = document.createElementNS("", "img");
        child.setAttribute("name", "qux");
        container.appendChild(child);
        "setup done"
    "#).unwrap();

    // Subtest 1: item("foo") returns the IDless first element
    let r1 = e.eval_js(r#"
        var container = document.getElementById("test");
        var result = container.children.item("foo");
        (result instanceof Element) + "|" + !result.hasAttribute("id")
    "#).unwrap();
    eprintln!("subtest1: {}", r1);
    assert_eq!(r1, "true|true");

    // Subtest 2: for..in enumerable own + getOwnPropertyNames
    let r2 = e.eval_js(r#"
        var container = document.getElementById("test");
        var list = container.children;
        var result = [];
        for (var p in list) {
            if (list.hasOwnProperty(p)) {
                result.push(p);
            }
        }
        var enumerable = JSON.stringify(result);
        var ownNames = JSON.stringify(Object.getOwnPropertyNames(list));

        // Debug: check each child's getAttribute for name/id
        var debug = [];
        for (var i = 0; i < list.length; i++) {
            var el = list[i];
            debug.push(i + ":id=" + el.getAttribute("id") + ",name=" + el.getAttribute("name") + ",ns=" + el.namespaceURI + ",hasGetAttr=" + (typeof el.getAttribute));
        }
        enumerable + "|" + ownNames + "|" + debug.join(";")
    "#).unwrap();
    eprintln!("subtest2: {}", r2);

    // Enumerable own properties should be indices only
    assert!(r2.starts_with(r#"["0","1","2","3","4","5"]|"#), "enumerable own = indices only");
    // getOwnPropertyNames should include named items
    assert!(r2.contains(r#"["0","1","2","3","4","5","foo","bar","baz"]"#), "ownPropertyNames includes named");
}
