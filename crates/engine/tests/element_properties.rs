//! Tests for HTML element property reflections.
//! Each test verifies that element-specific properties correctly reflect attributes.

use braille_engine::Engine;

fn engine_with_html(html: &str) -> Engine {
    let mut e = Engine::new();
    e.load_html(html);
    e
}

// ---------------------------------------------------------------------------
// HTMLAnchorElement
// ---------------------------------------------------------------------------

#[test]
fn anchor_href_reflects_attribute() {
    let mut e = engine_with_html(r#"<html><body><a id="a" href="http://example.com/page">link</a></body></html>"#);
    let result = e.eval_js(r#"
        var a = document.getElementById('a');
        var r1 = a.href === 'http://example.com/page';
        a.href = 'http://other.com';
        var r2 = a.getAttribute('href') === 'http://other.com';
        r1 + ',' + r2;
    "#).unwrap();
    assert_eq!(result, "true,true");
}

#[test]
fn anchor_target_rel_download() {
    let mut e = engine_with_html(r#"<html><body><a id="a" target="_blank" rel="noopener" download="file.pdf">dl</a></body></html>"#);
    let result = e.eval_js(r#"
        var a = document.getElementById('a');
        a.target + ',' + a.rel + ',' + a.download;
    "#).unwrap();
    assert_eq!(result, "_blank,noopener,file.pdf");
}

#[test]
fn anchor_text_is_textcontent() {
    let mut e = engine_with_html(r#"<html><body><a id="a">Hello World</a></body></html>"#);
    let result = e.eval_js(r#"
        var a = document.getElementById('a');
        a.text;
    "#).unwrap();
    assert_eq!(result, "Hello World");
}

#[test]
fn anchor_url_decomposition() {
    let mut e = engine_with_html(r#"<html><body><a id="a" href="https://example.com:8080/path?q=1#frag">link</a></body></html>"#);
    let result = e.eval_js(r#"
        var a = document.getElementById('a');
        [a.protocol, a.hostname, a.port, a.pathname, a.search, a.hash, a.host, a.origin].join('|');
    "#).unwrap();
    assert_eq!(result, "https:|example.com|8080|/path|?q=1|#frag|example.com:8080|https://example.com:8080");
}

// ---------------------------------------------------------------------------
// HTMLImageElement
// ---------------------------------------------------------------------------

#[test]
fn img_src_and_alt_reflect() {
    let mut e = engine_with_html(r#"<html><body><img id="i" src="photo.jpg" alt="A photo"></body></html>"#);
    let result = e.eval_js(r#"
        var img = document.getElementById('i');
        img.src + ',' + img.alt;
    "#).unwrap();
    assert_eq!(result, "photo.jpg,A photo");
}

#[test]
fn img_complete_is_true() {
    let mut e = engine_with_html(r#"<html><body><img id="i" src="x.png"></body></html>"#);
    let result = e.eval_js(r#"
        document.getElementById('i').complete;
    "#).unwrap();
    assert_eq!(result, "true");
}

#[test]
fn img_natural_dimensions_zero() {
    let mut e = engine_with_html(r#"<html><body><img id="i" src="x.png"></body></html>"#);
    let result = e.eval_js(r#"
        var img = document.getElementById('i');
        img.naturalWidth + ',' + img.naturalHeight;
    "#).unwrap();
    assert_eq!(result, "0,0");
}

#[test]
fn img_width_height_reflect() {
    let mut e = engine_with_html(r#"<html><body><img id="i" width="100" height="200"></body></html>"#);
    let result = e.eval_js(r#"
        var img = document.getElementById('i');
        img.width + ',' + img.height;
    "#).unwrap();
    assert_eq!(result, "100,200");
}

// ---------------------------------------------------------------------------
// HTMLButtonElement
// ---------------------------------------------------------------------------

#[test]
fn button_type_defaults_to_submit() {
    let mut e = engine_with_html(r#"<html><body><button id="b">Click</button></body></html>"#);
    let result = e.eval_js(r#"
        document.getElementById('b').type;
    "#).unwrap();
    assert_eq!(result, "submit");
}

#[test]
fn button_disabled_reflects() {
    let mut e = engine_with_html(r#"<html><body><button id="b" disabled>Click</button></body></html>"#);
    let result = e.eval_js(r#"
        var b = document.getElementById('b');
        var r1 = b.disabled === true;
        b.disabled = false;
        var r2 = b.hasAttribute('disabled') === false;
        r1 + ',' + r2;
    "#).unwrap();
    assert_eq!(result, "true,true");
}

#[test]
fn button_name_value() {
    let mut e = engine_with_html(r#"<html><body><button id="b" name="btn" value="go">Go</button></body></html>"#);
    let result = e.eval_js(r#"
        var b = document.getElementById('b');
        b.name + ',' + b.value;
    "#).unwrap();
    assert_eq!(result, "btn,go");
}

// ---------------------------------------------------------------------------
// HTMLFormElement
// ---------------------------------------------------------------------------

#[test]
fn form_method_and_action() {
    let mut e = engine_with_html(r#"<html><body><form id="f" action="/submit" method="POST"></form></body></html>"#);
    let result = e.eval_js(r#"
        var f = document.getElementById('f');
        f.action + ',' + f.method;
    "#).unwrap();
    assert_eq!(result, "/submit,post");
}

#[test]
fn form_elements_returns_controls() {
    let mut e = engine_with_html(r#"<html><body><form id="f"><input name="a"><select name="b"></select><textarea name="c"></textarea></form></body></html>"#);
    let result = e.eval_js(r#"
        var f = document.getElementById('f');
        f.elements.length + ',' + f.length;
    "#).unwrap();
    assert_eq!(result, "3,3");
}

#[test]
fn form_enctype_default() {
    let mut e = engine_with_html(r#"<html><body><form id="f"></form></body></html>"#);
    let result = e.eval_js(r#"
        document.getElementById('f').enctype;
    "#).unwrap();
    assert_eq!(result, "application/x-www-form-urlencoded");
}

// ---------------------------------------------------------------------------
// HTMLLabelElement
// ---------------------------------------------------------------------------

#[test]
fn label_html_for_and_control() {
    let mut e = engine_with_html(r#"<html><body><label id="l" for="inp">Name</label><input id="inp"></body></html>"#);
    let result = e.eval_js(r#"
        var l = document.getElementById('l');
        var r1 = l.htmlFor === 'inp';
        var r2 = l.control === document.getElementById('inp');
        r1 + ',' + r2;
    "#).unwrap();
    assert_eq!(result, "true,true");
}

#[test]
fn label_implicit_control() {
    let mut e = engine_with_html(r#"<html><body><label id="l"><input id="inp"></label></body></html>"#);
    let result = e.eval_js(r#"
        var l = document.getElementById('l');
        l.control === document.getElementById('inp');
    "#).unwrap();
    assert_eq!(result, "true");
}

// ---------------------------------------------------------------------------
// HTMLInputElement (additional properties)
// ---------------------------------------------------------------------------

#[test]
fn input_type_name_disabled() {
    let mut e = engine_with_html(r#"<html><body><input id="i" type="email" name="user" disabled></body></html>"#);
    let result = e.eval_js(r#"
        var i = document.getElementById('i');
        i.type + ',' + i.name + ',' + i.disabled;
    "#).unwrap();
    assert_eq!(result, "email,user,true");
}

#[test]
fn input_type_defaults_to_text() {
    let mut e = engine_with_html(r#"<html><body><input id="i"></body></html>"#);
    let result = e.eval_js(r#"
        document.getElementById('i').type;
    "#).unwrap();
    assert_eq!(result, "text");
}

#[test]
fn input_placeholder_required() {
    let mut e = engine_with_html(r#"<html><body><input id="i" placeholder="Enter..." required></body></html>"#);
    let result = e.eval_js(r#"
        var i = document.getElementById('i');
        i.placeholder + ',' + i.required;
    "#).unwrap();
    assert_eq!(result, "Enter...,true");
}

// ---------------------------------------------------------------------------
// HTMLTextAreaElement
// ---------------------------------------------------------------------------

#[test]
fn textarea_name_rows_cols() {
    let mut e = engine_with_html(r#"<html><body><textarea id="t" name="bio" rows="5" cols="40"></textarea></body></html>"#);
    let result = e.eval_js(r#"
        var t = document.getElementById('t');
        t.name + ',' + t.rows + ',' + t.cols;
    "#).unwrap();
    assert_eq!(result, "bio,5,40");
}

#[test]
fn textarea_defaults() {
    let mut e = engine_with_html(r#"<html><body><textarea id="t"></textarea></body></html>"#);
    let result = e.eval_js(r#"
        var t = document.getElementById('t');
        t.rows + ',' + t.cols;
    "#).unwrap();
    assert_eq!(result, "2,20");
}

// ---------------------------------------------------------------------------
// HTMLCanvasElement
// ---------------------------------------------------------------------------

#[test]
fn canvas_get_context_returns_stub() {
    let mut e = engine_with_html(r#"<html><body><canvas id="c"></canvas></body></html>"#);
    let result = e.eval_js(r#"
        var c = document.getElementById('c');
        var ctx = c.getContext('2d');
        var r1 = ctx !== null;
        var r2 = typeof ctx.fillRect === 'function';
        var r3 = c.getContext('webgl') === null;
        r1 + ',' + r2 + ',' + r3;
    "#).unwrap();
    assert_eq!(result, "true,true,true");
}

#[test]
fn canvas_width_height_defaults() {
    let mut e = engine_with_html(r#"<html><body><canvas id="c"></canvas></body></html>"#);
    let result = e.eval_js(r#"
        var c = document.getElementById('c');
        c.width + ',' + c.height;
    "#).unwrap();
    assert_eq!(result, "300,150");
}

#[test]
fn canvas_to_data_url() {
    let mut e = engine_with_html(r#"<html><body><canvas id="c"></canvas></body></html>"#);
    let result = e.eval_js(r#"
        document.getElementById('c').toDataURL().indexOf('data:') === 0;
    "#).unwrap();
    assert_eq!(result, "true");
}
