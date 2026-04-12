//! Integration tests for document.evaluate() / XPathResult API.
use braille_engine::Engine;

fn engine_with_html(html: &str) -> Engine {
    let mut e = Engine::new();
    e.load_html(html);
    e
}

#[test]
fn xpath_result_constants_exist() {
    let mut e = engine_with_html("<html><body></body></html>");
    let r = e.eval_js("XPathResult.FIRST_ORDERED_NODE_TYPE").unwrap();
    assert_eq!(r, "9");
    let r = e.eval_js("XPathResult.ANY_TYPE").unwrap();
    assert_eq!(r, "0");
    let r = e.eval_js("XPathResult.ORDERED_NODE_SNAPSHOT_TYPE").unwrap();
    assert_eq!(r, "7");
}

#[test]
fn xpath_single_node_value_null_when_no_match() {
    let mut e = engine_with_html(r#"<html><body><div id="d"><span id="s"></span></div></body></html>"#);
    let r = e.eval_js(r#"
        var div = document.getElementById('d');
        var result = document.evaluate('//non-span', div, null, XPathResult.FIRST_ORDERED_NODE_TYPE, null);
        result.singleNodeValue === null
    "#).unwrap();
    assert_eq!(r, "true");
}

#[test]
fn xpath_single_node_value_not_null_when_match() {
    let mut e = engine_with_html(r#"<html><body><div id="d"><span id="s"></span></div></body></html>"#);
    let r = e.eval_js(r#"
        var div = document.getElementById('d');
        var result = document.evaluate('//span', div, null, XPathResult.FIRST_ORDERED_NODE_TYPE, null);
        result.singleNodeValue !== null
    "#).unwrap();
    assert_eq!(r, "true");
}

#[test]
fn xpath_single_node_value_returns_correct_element() {
    let mut e = engine_with_html(r#"<html><body><div id="d"><span id="s">hello</span></div></body></html>"#);
    let r = e.eval_js(r#"
        var div = document.getElementById('d');
        var result = document.evaluate('//span', div, null, XPathResult.FIRST_ORDERED_NODE_TYPE, null);
        result.singleNodeValue.id
    "#).unwrap();
    assert_eq!(r, "s");
}

#[test]
fn xpath_snapshot_type() {
    let mut e = engine_with_html(r#"<html><body><ul><li>a</li><li>b</li><li>c</li></ul></body></html>"#);
    let r = e.eval_js(r#"
        var result = document.evaluate('//li', document, null, XPathResult.ORDERED_NODE_SNAPSHOT_TYPE, null);
        result.snapshotLength
    "#).unwrap();
    assert_eq!(r, "3");
    let r = e.eval_js(r#"
        var result = document.evaluate('//li', document, null, XPathResult.ORDERED_NODE_SNAPSHOT_TYPE, null);
        result.snapshotItem(1).textContent
    "#).unwrap();
    assert_eq!(r, "b");
}

#[test]
fn xpath_iterator_type() {
    let mut e = engine_with_html(r#"<html><body><p>1</p><p>2</p><p>3</p></body></html>"#);
    let r = e.eval_js(r#"
        var result = document.evaluate('//p', document, null, XPathResult.ORDERED_NODE_ITERATOR_TYPE, null);
        var items = [];
        var node;
        while ((node = result.iterateNext()) !== null) {
            items.push(node.textContent);
        }
        items.join(',')
    "#).unwrap();
    assert_eq!(r, "1,2,3");
}

#[test]
fn xpath_boolean_type() {
    let mut e = engine_with_html(r#"<html><body><span></span></body></html>"#);
    let r = e.eval_js(r#"
        document.evaluate('//span', document, null, XPathResult.BOOLEAN_TYPE, null).booleanValue
    "#).unwrap();
    assert_eq!(r, "true");
    let r = e.eval_js(r#"
        document.evaluate('//nonexistent', document, null, XPathResult.BOOLEAN_TYPE, null).booleanValue
    "#).unwrap();
    assert_eq!(r, "false");
}

#[test]
fn xpath_string_type() {
    let mut e = engine_with_html(r#"<html><body><div id="t">hello world</div></body></html>"#);
    let r = e.eval_js(r#"
        document.evaluate('//div', document, null, XPathResult.STRING_TYPE, null).stringValue
    "#).unwrap();
    assert_eq!(r, "hello world");
}

#[test]
fn xpath_attribute_predicate() {
    let mut e = engine_with_html(r#"<html><body><a href="x">link</a><a>no href</a></body></html>"#);
    let r = e.eval_js(r#"
        var result = document.evaluate('//a[@href]', document, null, XPathResult.ORDERED_NODE_SNAPSHOT_TYPE, null);
        result.snapshotLength
    "#).unwrap();
    assert_eq!(r, "1");
}

#[test]
fn xpath_attribute_value_predicate() {
    let mut e = engine_with_html(r#"<html><body><div class="a">1</div><div class="b">2</div></body></html>"#);
    let r = e.eval_js(r#"
        var result = document.evaluate("//div[@class='b']", document, null, XPathResult.FIRST_ORDERED_NODE_TYPE, null);
        result.singleNodeValue.textContent
    "#).unwrap();
    assert_eq!(r, "2");
}

#[test]
fn xpath_wildcard() {
    let mut e = engine_with_html(r#"<html><body><div><span>a</span><b>b</b></div></body></html>"#);
    let r = e.eval_js(r#"
        var div = document.querySelector('div');
        var result = document.evaluate('//*', div, null, XPathResult.ORDERED_NODE_SNAPSHOT_TYPE, null);
        result.snapshotLength
    "#).unwrap();
    assert_eq!(r, "2");
}

#[test]
fn xpath_contains_predicate() {
    let mut e = engine_with_html(r#"<html><body><div class="foo-bar">1</div><div class="baz">2</div></body></html>"#);
    let r = e.eval_js(r#"
        var result = document.evaluate("//div[contains(@class, 'foo')]", document, null, XPathResult.FIRST_ORDERED_NODE_TYPE, null);
        result.singleNodeValue.textContent
    "#).unwrap();
    assert_eq!(r, "1");
}

#[test]
fn xpath_text_predicate() {
    let mut e = engine_with_html(r#"<html><body><p>hello</p><p>world</p></body></html>"#);
    let r = e.eval_js(r#"
        var result = document.evaluate("//p[text()='world']", document, null, XPathResult.FIRST_ORDERED_NODE_TYPE, null);
        result.singleNodeValue.textContent
    "#).unwrap();
    assert_eq!(r, "world");
}
