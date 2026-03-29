//! DOM bridge web API tests: React reconciler requirements, script IDL, CSS, Intl, crypto.
use braille_engine::Engine;
use braille_wire::{FetchResponseData, SnapMode};

fn engine_with_html(html: &str) -> Engine {
    let mut e = Engine::new();
    e.load_html(html);
    e
}

// =========================================================================
// Tier 4: DOM bridge completeness — React reconciler requirements
// =========================================================================

#[test]
fn childnodes_includes_text_nodes() {
    let mut e = engine_with_html("<html><body><div id='d'>hello<span>world</span></div></body></html>");
    let result = e.eval_js(
        "var d = document.getElementById('d'); d.childNodes.length"
    );
    assert_eq!(result.unwrap(), "2", "childNodes should include text + element");
    let result = e.eval_js("document.getElementById('d').childNodes[0].nodeType");
    assert_eq!(result.unwrap(), "3", "first childNode should be text (nodeType 3)");
}

#[test]
fn firstchild_lastchild() {
    let mut e = engine_with_html("<html><body><div id='d'><span>a</span><b>b</b></div></body></html>");
    let result = e.eval_js("document.getElementById('d').firstChild.tagName");
    assert_eq!(result.unwrap(), "SPAN");
    let result = e.eval_js("document.getElementById('d').lastChild.tagName");
    assert_eq!(result.unwrap(), "B");
}

#[test]
fn firstchild_lastchild_null() {
    let mut e = engine_with_html("<html><body><div id='d'></div></body></html>");
    let result = e.eval_js("document.getElementById('d').firstChild === null");
    assert_eq!(result.unwrap(), "true");
    let result = e.eval_js("document.getElementById('d').lastChild === null");
    assert_eq!(result.unwrap(), "true");
}

#[test]
fn nextsibling_previoussibling() {
    let mut e = engine_with_html("<html><body><div id='p'><span id='a'>a</span><b id='b'>b</b><i id='c'>c</i></div></body></html>");
    let result = e.eval_js("document.getElementById('a').nextSibling.tagName");
    assert_eq!(result.unwrap(), "B");
    let result = e.eval_js("document.getElementById('c').previousSibling.tagName");
    assert_eq!(result.unwrap(), "B");
    let result = e.eval_js("document.getElementById('a').previousSibling === null");
    assert_eq!(result.unwrap(), "true");
    let result = e.eval_js("document.getElementById('c').nextSibling === null");
    assert_eq!(result.unwrap(), "true");
}

#[test]
fn text_node_data_property() {
    let mut e = engine_with_html("<html><body><div id='d'>hello</div></body></html>");
    let result = e.eval_js("document.getElementById('d').firstChild.data");
    assert_eq!(result.unwrap(), "hello");
    // set data
    e.eval_js("document.getElementById('d').firstChild.data = 'world';").unwrap();
    let result = e.eval_js("document.getElementById('d').firstChild.data");
    assert_eq!(result.unwrap(), "world");
}

#[test]
fn nodevalue_for_text_and_comment() {
    let mut e = engine_with_html("<html><body><div id='d'>text</div></body></html>");
    // text node nodeValue
    let result = e.eval_js("document.getElementById('d').firstChild.nodeValue");
    assert_eq!(result.unwrap(), "text");
    // element nodeValue is null
    let result = e.eval_js("document.getElementById('d').nodeValue === null");
    assert_eq!(result.unwrap(), "true");
    // comment nodeValue
    let result = e.eval_js("document.createComment('hi').nodeValue");
    assert_eq!(result.unwrap(), "hi");
}

#[test]
fn clone_node_shallow() {
    let mut e = engine_with_html("<html><body><div id='d' class='x'><span>child</span></div></body></html>");
    let result = e.eval_js(
        "var orig = document.getElementById('d'); var cl = orig.cloneNode(false); cl.tagName + '|' + cl.getAttribute('class') + '|' + cl.childNodes.length"
    );
    assert_eq!(result.unwrap(), "DIV|x|0");
}

#[test]
fn clone_node_deep() {
    let mut e = engine_with_html("<html><body><div id='d'><span>child</span></div></body></html>");
    let result = e.eval_js(
        "var cl = document.getElementById('d').cloneNode(true); cl.childNodes.length + '|' + cl.firstChild.tagName"
    );
    assert_eq!(result.unwrap(), "1|SPAN");
}

#[test]
fn replace_child() {
    let mut e = engine_with_html("<html><body><div id='p'><span id='old'>old</span></div></body></html>");
    e.eval_js("var p = document.getElementById('p'); var n = document.createElement('b'); n.textContent = 'new'; p.replaceChild(n, document.getElementById('old'));").unwrap();
    let snap = e.snapshot(SnapMode::Text);
    assert!(snap.contains("new"), "replaced child should appear: {snap}");
    assert!(!snap.contains("old"), "old child should be gone: {snap}");
}

#[test]
fn document_fragment_transfers_children() {
    let mut e = engine_with_html("<html><body><div id='target'></div></body></html>");
    e.eval_js(r#"
        var frag = document.createDocumentFragment();
        var a = document.createElement('span'); a.textContent = 'aaa';
        var b = document.createElement('span'); b.textContent = 'bbb';
        frag.appendChild(a);
        frag.appendChild(b);
        document.getElementById('target').appendChild(frag);
    "#).unwrap();
    let snap = e.snapshot(SnapMode::Text);
    assert!(snap.contains("aaa"), "fragment child a should appear: {snap}");
    assert!(snap.contains("bbb"), "fragment child b should appear: {snap}");
    // Fragment should now be empty
    let result = e.eval_js("frag.childNodes.length");
    assert_eq!(result.unwrap(), "0");
}

#[test]
fn innerhtml_getter() {
    let mut e = engine_with_html("<html><body><div id='d'><b>bold</b> text</div></body></html>");
    let result = e.eval_js("document.getElementById('d').innerHTML");
    let html = result.unwrap();
    assert!(html.contains("<b>bold</b>"), "innerHTML should contain <b>bold</b>, got: {html}");
    assert!(html.contains("text"), "innerHTML should contain text, got: {html}");
}

#[test]
fn matches_selector() {
    let mut e = engine_with_html("<html><body><div id='d' class='foo bar'></div></body></html>");
    let result = e.eval_js("document.getElementById('d').matches('.foo')");
    assert_eq!(result.unwrap(), "true");
    let result = e.eval_js("document.getElementById('d').matches('.baz')");
    assert_eq!(result.unwrap(), "false");
    let result = e.eval_js("document.getElementById('d').matches('div.bar')");
    assert_eq!(result.unwrap(), "true");
}

#[test]
fn has_child_nodes() {
    let mut e = engine_with_html("<html><body><div id='full'><span>x</span></div><div id='empty'></div></body></html>");
    let result = e.eval_js("document.getElementById('full').hasChildNodes()");
    assert_eq!(result.unwrap(), "true");
    let result = e.eval_js("document.getElementById('empty').hasChildNodes()");
    assert_eq!(result.unwrap(), "false");
}

// =========================================================================
// HTMLScriptElement IDL properties (noModule, async, defer)
// =========================================================================

#[test]
fn nomodule_in_check() {
    let mut e = engine_with_html("<html><body></body></html>");
    let result = e.eval_js("'noModule' in document.createElement('script')");
    assert_eq!(result.unwrap(), "true");
}

#[test]
fn nomodule_reflect() {
    let mut e = engine_with_html("<html><body></body></html>");
    let result = e.eval_js(r#"
        var s = document.createElement('script');
        s.noModule = true;
        var has = s.hasAttribute('nomodule');
        var get = s.noModule;
        s.noModule = false;
        var gone = !s.hasAttribute('nomodule');
        has + ',' + get + ',' + gone
    "#);
    assert_eq!(result.unwrap(), "true,true,true");
}

#[test]
fn async_defer_properties() {
    let mut e = engine_with_html("<html><body></body></html>");
    let result = e.eval_js(r#"
        var s = document.createElement('script');
        s.async = true;
        var a = s.hasAttribute('async');
        s.defer = true;
        var d = s.hasAttribute('defer');
        a + ',' + d
    "#);
    assert_eq!(result.unwrap(), "true,true");
}

#[test]
fn reversed_in_ol() {
    let mut e = engine_with_html("<html><body></body></html>");
    let result = e.eval_js("'reversed' in document.createElement('ol')");
    assert_eq!(result.unwrap(), "true");
}

#[test]
fn proton_browser_check() {
    // Exact check from ProtonMail's public-index.js module 33759
    let mut e = engine_with_html("<html><body></body></html>");
    let result = e.eval_js(r#"
        "reversed" in document.createElement("ol")
        && Object.fromEntries
        && "".trimStart
        && window.crypto.subtle
        ? 1 : 0
    "#);
    assert_eq!(result.unwrap(), "1");
}

// =========================================================================
// CSS.supports()
// =========================================================================

#[test]
fn css_supports_two_arg() {
    let mut e = engine_with_html("<html><body></body></html>");
    let result = e.eval_js("CSS.supports('display', 'flex')");
    assert_eq!(result.unwrap(), "true");
}

#[test]
fn css_supports_one_arg() {
    let mut e = engine_with_html("<html><body></body></html>");
    let result = e.eval_js("CSS.supports('(display: flex)')");
    assert_eq!(result.unwrap(), "true");
}

#[test]
fn css_supports_invalid() {
    let mut e = engine_with_html("<html><body></body></html>");
    let result = e.eval_js("CSS.supports('', '')");
    assert_eq!(result.unwrap(), "false");
}

#[test]
fn css_escape() {
    let mut e = engine_with_html("<html><body></body></html>");
    let result = e.eval_js("CSS.escape('foo.bar')");
    assert_eq!(result.unwrap(), r"foo\.bar");
}

// =========================================================================
// Intl
// =========================================================================

#[test]
fn intl_typeof() {
    let mut e = engine_with_html("<html><body></body></html>");
    let result = e.eval_js("typeof Intl === 'object'");
    assert_eq!(result.unwrap(), "true");
}

#[test]
fn intl_numberformat_basic() {
    let mut e = engine_with_html("<html><body></body></html>");
    let result = e.eval_js("new Intl.NumberFormat('en').format(1234.5)");
    assert_eq!(result.unwrap(), "1,234.5");
}

#[test]
fn intl_pluralrules() {
    let mut e = engine_with_html("<html><body></body></html>");
    let result = e.eval_js("new Intl.PluralRules('en').select(1)");
    assert_eq!(result.unwrap(), "one");
}

#[test]
fn intl_datetimeformat() {
    let mut e = engine_with_html("<html><body></body></html>");
    let result = e.eval_js("new Intl.DateTimeFormat('en').format(new Date(0))");
    let val = result.unwrap();
    assert!(!val.is_empty(), "DateTimeFormat should produce non-empty output");
}

// =========================================================================
// Dynamic script loading
// =========================================================================

#[test]
fn dynamic_script_load_fires_onload() {
    let mut e = engine_with_html("<html><head></head><body></body></html>");
    e.eval_js(r#"
        var s = document.createElement('script');
        s.src = 'https://example.com/chunk.js';
        window.__script_loaded = false;
        s.onload = function() { window.__script_loaded = true; };
        document.head.appendChild(s);
    "#).unwrap();

    // Should have a pending fetch for the script
    assert!(e.has_pending_fetches(), "should have pending fetch for script src");
    let pending = e.pending_fetches();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].url, "https://example.com/chunk.js");

    // Resolve the fetch with some JS code
    e.resolve_fetch(pending[0].id, &FetchResponseData {
        status: 200,
        status_text: "OK".to_string(),
        headers: vec![("content-type".to_string(), "application/javascript".to_string())],
        body: "window.__chunk_ran = 42;".to_string(),
        url: "https://example.com/chunk.js".to_string(),
        redirect_chain: vec![],
    });
    e.settle();

    // The script should have executed
    assert_eq!(e.eval_js("window.__chunk_ran").unwrap(), "42");
    // onload should have fired
    assert_eq!(e.eval_js("window.__script_loaded").unwrap(), "true");
}

#[test]
fn dynamic_script_eval_runs_code() {
    let mut e = engine_with_html("<html><head></head><body></body></html>");

    // Set up a global array that the "chunk" will push to (like webpack)
    e.eval_js("window.__chunks = []; window.__chunks.push = function(v) { Array.prototype.push.call(this, v); };").unwrap();

    e.eval_js(r#"
        var s = document.createElement('script');
        s.src = 'https://cdn.example.com/chunk.4599.js';
        document.head.appendChild(s);
    "#).unwrap();

    let pending = e.pending_fetches();
    assert_eq!(pending.len(), 1);

    // The chunk pushes data onto the global array (webpack pattern)
    e.resolve_fetch(pending[0].id, &FetchResponseData {
        status: 200,
        status_text: "OK".to_string(),
        headers: vec![],
        body: "window.__chunks.push([4599, {hello: 'world'}]);".to_string(),
        url: "https://cdn.example.com/chunk.4599.js".to_string(),
        redirect_chain: vec![],
    });
    e.settle();

    assert_eq!(e.eval_js("window.__chunks.length").unwrap(), "1");
    assert_eq!(e.eval_js("window.__chunks[0][0]").unwrap(), "4599");
}

#[test]
fn dynamic_script_insertbefore_also_loads() {
    let mut e = engine_with_html("<html><head><meta charset='utf-8'></head><body></body></html>");
    e.eval_js(r#"
        var s = document.createElement('script');
        s.src = 'https://example.com/insert.js';
        var meta = document.querySelector('meta');
        document.head.insertBefore(s, meta);
    "#).unwrap();

    assert!(e.has_pending_fetches(), "insertBefore should trigger script load");
    let pending = e.pending_fetches();
    assert_eq!(pending[0].url, "https://example.com/insert.js");
}

#[test]
fn dynamic_script_error_fires_onerror() {
    let mut e = engine_with_html("<html><head></head><body></body></html>");
    e.eval_js(r#"
        var s = document.createElement('script');
        s.src = 'https://example.com/missing.js';
        window.__script_error = false;
        s.onerror = function() { window.__script_error = true; };
        document.head.appendChild(s);
    "#).unwrap();

    let pending = e.pending_fetches();
    e.reject_fetch(pending[0].id, "Network error");
    e.settle();

    assert_eq!(e.eval_js("window.__script_error").unwrap(), "true");
}

#[test]
fn message_channel_settles() {
    // React's scheduler uses MessageChannel → setTimeout(0) → callback
    let mut e = engine_with_html("<html><body></body></html>");
    e.eval_js(r#"
        window.__mc_result = 'not fired';
        var ch = new MessageChannel();
        ch.port1.onmessage = function(ev) {
            window.__mc_result = 'fired: ' + ev.data;
        };
        ch.port2.postMessage('hello');
    "#).unwrap();

    // Before settle: the setTimeout(0) hasn't fired
    assert_eq!(e.eval_js("window.__mc_result").unwrap(), "not fired");

    // After settle: the timer fires, MessageChannel callback runs
    e.settle();
    assert_eq!(e.eval_js("window.__mc_result").unwrap(), "fired: hello");
}

#[test]
fn settle_fires_chained_timers() {
    // Verify that settle() processes cascading timers
    let mut e = engine_with_html("<html><body></body></html>");
    e.eval_js(r#"
        window.__chain = [];
        setTimeout(function() {
            __chain.push('A');
            setTimeout(function() {
                __chain.push('B');
                setTimeout(function() {
                    __chain.push('C');
                }, 0);
            }, 0);
        }, 0);
    "#).unwrap();
    e.settle();
    assert_eq!(e.eval_js("__chain.join(',')").unwrap(), "A,B,C");
}

// =========================================================================
// WebCrypto
// =========================================================================

#[test]
fn crypto_get_random_values() {
    let mut e = engine_with_html("<html><body></body></html>");
    let result = e.eval_js(r#"
        var a = new Uint8Array(16);
        crypto.getRandomValues(a);
        a.length + ',' + (a.some(function(x){ return x !== 0; }) ? 'random' : 'all-zero')
    "#);
    assert_eq!(result.unwrap(), "16,random");
}

#[test]
fn crypto_subtle_digest_sha256() {
    let mut e = engine_with_html("<html><body></body></html>");
    // SHA-256 of empty string = e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
    e.eval_js(r#"
        var p; crypto.subtle.digest('SHA-256', new Uint8Array(0)).then(function(b){
            var a = new Uint8Array(b), h = '';
            for(var i=0;i<a.length;i++) h += (a[i]<16?'0':'') + a[i].toString(16);
            p = h;
        });
    "#).unwrap();
    e.settle();
    assert_eq!(e.eval_js("p").unwrap(), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
}

#[test]
fn crypto_subtle_aes_gcm_roundtrip() {
    let mut e = engine_with_html("<html><body></body></html>");
    e.eval_js(r#"
        var result = 'pending';
        crypto.subtle.generateKey({name:'AES-GCM',length:256}, true, ['encrypt','decrypt'])
        .then(function(key) {
            var iv = crypto.getRandomValues(new Uint8Array(12));
            var data = new TextEncoder().encode('hello world');
            return crypto.subtle.encrypt({name:'AES-GCM',iv:iv}, key, data)
            .then(function(ct) {
                return crypto.subtle.decrypt({name:'AES-GCM',iv:iv}, key, ct);
            });
        })
        .then(function(pt) {
            result = new TextDecoder().decode(new Uint8Array(pt));
        })
        .catch(function(e) { result = 'ERROR: ' + e.message; });
    "#).unwrap();
    e.settle();
    let val = e.eval_js("result");
    assert_eq!(val.unwrap(), "hello world");
}

#[test]
fn crypto_subtle_hmac_sign_verify() {
    let mut e = engine_with_html("<html><body></body></html>");
    e.eval_js(r#"
        var result = 'pending';
        crypto.subtle.generateKey({name:'HMAC',hash:'SHA-256'}, false, ['sign','verify'])
        .then(function(key) {
            var data = new TextEncoder().encode('test message');
            return crypto.subtle.sign({name:'HMAC'}, key, data)
            .then(function(sig) {
                return crypto.subtle.verify({name:'HMAC'}, key, sig, data);
            });
        })
        .then(function(valid) { result = String(valid); })
        .catch(function(e) { result = 'ERROR: ' + e.message; });
    "#).unwrap();
    e.settle();
    assert_eq!(e.eval_js("result").unwrap(), "true");
}

#[test]
fn crypto_x25519_import_and_derive() {
    let mut e = engine_with_html("<html><body></body></html>");
    e.eval_js(r#"
        var result = 'pending';
        var pkcs8 = new Uint8Array([48, 46, 2, 1, 0, 48, 5, 6, 3, 43, 101, 110, 4, 34, 4, 32, 200, 131, 142, 118, 208, 87, 223, 183, 216, 201, 90, 105, 225, 56, 22, 10, 221, 99, 115, 253, 113, 164, 210, 118, 187, 86, 227, 168, 27, 100, 255, 97]);
        var spki = new Uint8Array([48, 42, 48, 5, 6, 3, 43, 101, 110, 3, 33, 0, 28, 242, 177, 230, 2, 46, 197, 55, 55, 30, 215, 245, 62, 84, 250, 17, 84, 216, 62, 152, 235, 100, 234, 81, 250, 229, 179, 48, 124, 254, 151, 6]);
        var expected = new Uint8Array([39, 104, 64, 157, 250, 185, 158, 194, 59, 140, 137, 185, 63, 245, 136, 2, 149, 247, 97, 118, 8, 143, 137, 228, 61, 254, 190, 126, 161, 149, 0, 8]);

        Promise.all([
            crypto.subtle.importKey("pkcs8", pkcs8, {name: "X25519"}, false, ["deriveBits", "deriveKey"]),
            crypto.subtle.importKey("spki", spki, {name: "X25519"}, false, [])
        ]).then(function(keys) {
            var privKey = keys[0];
            var pubKey = keys[1];
            result = 'imported: priv=' + privKey.type + ' pub=' + pubKey.type;
            return crypto.subtle.deriveBits({name: "X25519", public: pubKey}, privKey, 256);
        }).then(function(derived) {
            var a = new Uint8Array(derived);
            var match = true;
            for (var i = 0; i < expected.length; i++) {
                if (a[i] !== expected[i]) { match = false; break; }
            }
            result = match ? 'PASS' : 'FAIL: derived mismatch';
        }).catch(function(e) {
            result = 'ERROR: ' + e.name + ': ' + e.message;
        });
    "#).unwrap();
    e.settle();
    let val = e.eval_js("result").unwrap();
    assert_eq!(val, "PASS");
}

#[test]
fn crypto_cryptokey_class() {
    let mut e = engine_with_html("<html><body></body></html>");
    e.eval_js(r#"
        var result = 'pending';
        crypto.subtle.generateKey({name:'AES-GCM',length:256}, true, ['encrypt','decrypt'])
        .then(function(key) {
            var checks = [];
            checks.push('ctor=' + (key.constructor === CryptoKey));
            checks.push('type=' + key.type);
            checks.push('extractable=' + key.extractable);
            checks.push('algoName=' + key.algorithm.name);
            result = checks.join(',');
        });
    "#).unwrap();
    e.settle();
    assert_eq!(e.eval_js("result").unwrap(), "ctor=true,type=secret,extractable=true,algoName=AES-GCM");
}

#[test]
fn crypto_x25519_import_chain_debug() {
    let mut e = engine_with_html("<html><body></body></html>");
    e.eval_js(r#"
        var result = 'pending';
        var pkcs8 = new Uint8Array([48, 46, 2, 1, 0, 48, 5, 6, 3, 43, 101, 110, 4, 34, 4, 32, 200, 131, 142, 118, 208, 87, 223, 183, 216, 201, 90, 105, 225, 56, 22, 10, 221, 99, 115, 253, 113, 164, 210, 118, 187, 86, 227, 168, 27, 100, 255, 97]);
        var spki = new Uint8Array([48, 42, 48, 5, 6, 3, 43, 101, 110, 3, 33, 0, 28, 242, 177, 230, 2, 46, 197, 55, 55, 30, 215, 245, 62, 84, 250, 17, 84, 216, 62, 152, 235, 100, 234, 81, 250, 229, 179, 48, 124, 254, 151, 6]);
        var ecSPKI = new Uint8Array([48, 89, 48, 19, 6, 7, 42, 134, 72, 206, 61, 2, 1, 6, 8, 42, 134, 72, 206, 61, 3, 1, 7, 3, 66, 0, 4, 154, 116, 32, 120, 126, 95, 77, 105, 211, 232, 34, 114, 115, 1, 109, 56, 224, 71, 129, 133, 223, 127, 238, 156, 142, 103, 60, 202, 211, 79, 126, 128, 254, 49, 141, 182, 221, 107, 119, 218, 99, 32, 165, 246, 151, 89, 9, 68, 23, 177, 52, 239, 138, 139, 116, 193, 101, 4, 57, 198, 115, 0, 90, 61]);

        var subtle = crypto.subtle;
        var promises = [];
        var privateKeys = {};
        var publicKeys = {};
        var noDeriveBitsKeys = {};
        var ecdhPublicKeys = {};

        promises.push(
            subtle.importKey("pkcs8", pkcs8, {name: "X25519"}, false, ["deriveBits", "deriveKey"])
            .then(function(key) { privateKeys["X25519"] = key; result = 'pkcs8-ok'; },
                  function(err) { privateKeys["X25519"] = null; result = 'pkcs8-err:' + err.message; })
        );
        promises.push(
            subtle.importKey("pkcs8", pkcs8, {name: "X25519"}, false, ["deriveKey"])
            .then(function(key) { noDeriveBitsKeys["X25519"] = key; },
                  function(err) { noDeriveBitsKeys["X25519"] = null; })
        );
        promises.push(
            subtle.importKey("spki", spki, {name: "X25519"}, false, [])
            .then(function(key) { publicKeys["X25519"] = key; },
                  function(err) { publicKeys["X25519"] = null; result = 'spki-err:' + err.message; })
        );
        // ecSPKI as ECDH P-256 (not pushed to promises)
        subtle.importKey("spki", ecSPKI, {name: "ECDH", namedCurve: "P-256"}, false, [])
            .then(function(key) { ecdhPublicKeys["X25519"] = key; })
            .catch(function(err) { result = 'ecdh-spki-err:' + err.message; });

        Promise.all(promises).then(function() {
            result = 'all-resolved: priv=' + (privateKeys["X25519"] !== null) + ' pub=' + (publicKeys["X25519"] !== null);
        }).catch(function(err) {
            result = 'all-rejected: ' + err.message;
        });
    "#).unwrap();
    e.settle();
    let val = e.eval_js("result").unwrap();
    eprintln!("crypto_x25519_import_chain_debug result: {}", val);
    assert!(val.starts_with("all-resolved"), "Expected all-resolved but got: {}", val);
}

#[test]
fn crypto_x25519_wpt_pattern_debug() {
    // Reproduce the exact WPT test pattern that fails
    let mut e = engine_with_html("<html><body></body></html>");
    e.eval_js(r#"
        var results = [];
        self.promise_test = function(fn, name) {
            var result = { name: name || "(unnamed)", status: 0, message: "" };
            results.push(result);
            var cleanups = [];
            var t = {
                name: name || "(unnamed)",
                step_func: function(f) { return function() { return f.apply(t, arguments); }; },
                done: function() { t._done = true; },
                unreached_func: function(msg) { return function() { throw new Error(msg || "unreached"); }; },
                add_cleanup: function(f) { cleanups.push(f); },
                step_timeout: function(fn, timeout) { fn(); },
                _done: false
            };
            try {
                var p = fn(t);
                if (p && typeof p.then === 'function') {
                    p.then(function() {}, function(e) {
                        result.status = 1;
                        result.message = e.message || String(e);
                    });
                }
            } catch(e) {
                result.status = 1;
                result.message = e.message || String(e);
            }
            for (var i = 0; i < cleanups.length; i++) {
                try { cleanups[i](); } catch(ce) {}
            }
        };
        self.assert_true = function(val, msg) { if (val !== true) throw new Error(msg || "assert_true: got " + val); };
        self.assert_equals = function(a, b, msg) { if (a !== b) throw new Error(msg || "assert_equals: " + a + " !== " + b); };
        self.assert_unreached = function(msg) { throw new Error(msg || "assert_unreached"); };
        self.subsetTest = function(testFunc) { var args = Array.prototype.slice.call(arguments, 1); testFunc.apply(this, args); };

        // Now run the pattern from the WPT test
        function define_tests() {
            var subtle = crypto.subtle;
            var pkcs8 = {"X25519": new Uint8Array([48, 46, 2, 1, 0, 48, 5, 6, 3, 43, 101, 110, 4, 34, 4, 32, 200, 131, 142, 118, 208, 87, 223, 183, 216, 201, 90, 105, 225, 56, 22, 10, 221, 99, 115, 253, 113, 164, 210, 118, 187, 86, 227, 168, 27, 100, 255, 97])};
            var spki = {"X25519": new Uint8Array([48, 42, 48, 5, 6, 3, 43, 101, 110, 3, 33, 0, 28, 242, 177, 230, 2, 46, 197, 55, 55, 30, 215, 245, 62, 84, 250, 17, 84, 216, 62, 152, 235, 100, 234, 81, 250, 229, 179, 48, 124, 254, 151, 6])};
            var ecSPKI = new Uint8Array([48, 89, 48, 19, 6, 7, 42, 134, 72, 206, 61, 2, 1, 6, 8, 42, 134, 72, 206, 61, 3, 1, 7, 3, 66, 0, 4, 154, 116, 32, 120, 126, 95, 77, 105, 211, 232, 34, 114, 115, 1, 109, 56, 224, 71, 129, 133, 223, 127, 238, 156, 142, 103, 60, 202, 211, 79, 126, 128, 254, 49, 141, 182, 221, 107, 119, 218, 99, 32, 165, 246, 151, 89, 9, 68, 23, 177, 52, 239, 138, 139, 116, 193, 101, 4, 57, 198, 115, 0, 90, 61]);
            var algorithmName = "X25519";
            var sizes = {"X25519": 32};
            var derivations = {"X25519": new Uint8Array([39, 104, 64, 157, 250, 185, 158, 194, 59, 140, 137, 185, 63, 245, 136, 2, 149, 247, 97, 118, 8, 143, 137, 228, 61, 254, 190, 126, 161, 149, 0, 8])};

            return importKeys(pkcs8, spki, sizes)
            .then(function(r) {
                self.__debug_r = JSON.stringify(Object.keys(r));
                self.__debug_pub = r.publicKeys ? JSON.stringify(Object.keys(r.publicKeys)) : 'null';
                self.__debug_strict = (function() { return !this; })();
                publicKeys = r.publicKeys;
                self.__debug_pubAssigned = typeof publicKeys;
                privateKeys = r.privateKeys;
                noDeriveBitsKeys = r.noDeriveBitsKeys;
                ecdhKeys = r.ecdhKeys;

                promise_test(function(test) {
                    return subtle.deriveBits({name: algorithmName, public: publicKeys[algorithmName]}, privateKeys[algorithmName], 8 * sizes[algorithmName])
                    .then(function(derivation) {
                        var a = new Uint8Array(derivation);
                        var exp = derivations[algorithmName];
                        var ok = a.length === exp.length;
                        for (var i = 0; ok && i < a.length; i++) if (a[i] !== exp[i]) ok = false;
                        assert_true(ok, "Derived correct bits");
                    }, function(err) {
                        assert_unreached("deriveBits failed with error " + err.name + ": " + err.message);
                    });
                }, algorithmName + " good parameters");
            });

            function importKeys(pkcs8, spki, sizes) {
                var privateKeys = {};
                var publicKeys = {};
                var noDeriveBitsKeys = {};
                var ecdhPublicKeys = {};
                var promises = [];
                promises.push(subtle.importKey("pkcs8", pkcs8[algorithmName], {name: algorithmName}, false, ["deriveBits", "deriveKey"])
                    .then(function(key) { privateKeys[algorithmName] = key; }, function(err) { privateKeys[algorithmName] = null; }));
                promises.push(subtle.importKey("pkcs8", pkcs8[algorithmName], {name: algorithmName}, false, ["deriveKey"])
                    .then(function(key) { noDeriveBitsKeys[algorithmName] = key; }, function(err) { noDeriveBitsKeys[algorithmName] = null; }));
                promises.push(subtle.importKey("spki", spki[algorithmName], {name: algorithmName}, false, [])
                    .then(function(key) { publicKeys[algorithmName] = key; }, function(err) { publicKeys[algorithmName] = null; }));
                try {
                    subtle.importKey("spki", ecSPKI, {name: "ECDH", namedCurve: "P-256"}, false, [])
                        .then(function(key) { ecdhPublicKeys[algorithmName] = key; });
                } catch(ecdherr) {
                    // ignore - P-256 ECDH import failure shouldn't block test
                }
                return Promise.all(promises)
                    .then(function() { return {privateKeys: privateKeys, publicKeys: publicKeys, noDeriveBitsKeys: noDeriveBitsKeys, ecdhKeys: ecdhPublicKeys}; });
            }
        }

        promise_test(define_tests, 'setup - define tests');
    "#).unwrap();
    e.settle();
    // Debug output
    let debug_r = e.eval_js("typeof self.__debug_r !== 'undefined' ? self.__debug_r : 'NOT_SET'").unwrap();
    let debug_pub = e.eval_js("typeof self.__debug_pub !== 'undefined' ? self.__debug_pub : 'NOT_SET'").unwrap();
    let debug_strict = e.eval_js("typeof self.__debug_strict !== 'undefined' ? String(self.__debug_strict) : 'NOT_SET'").unwrap();
    let top_level_strict = e.eval_js("(function() { return !this; })()").unwrap();
    eprintln!("top_level_strict: {}", top_level_strict);
    let debug_assigned = e.eval_js("typeof self.__debug_pubAssigned !== 'undefined' ? self.__debug_pubAssigned : 'NOT_SET'").unwrap();
    eprintln!("strict: {}", debug_strict);
    eprintln!("pubAssigned: {}", debug_assigned);
    eprintln!("globalThis.publicKeys type: {}", e.eval_js("typeof globalThis.publicKeys").unwrap());
    eprintln!("debug_r: {}", debug_r);
    eprintln!("debug_pub: {}", debug_pub);
    // Check unhandled rejections
    let rejections = e.eval_js("typeof __braille_pending_rejections !== 'undefined' ? JSON.stringify(__braille_pending_rejections) : '[]'").unwrap();
    eprintln!("Pending rejections: {}", rejections);
    let val = e.eval_js("JSON.stringify(results)").unwrap();
    eprintln!("WPT pattern results: {}", val);
    let parsed: Vec<serde_json::Value> = serde_json::from_str(&val).unwrap();
    for r in &parsed {
        let status = r["status"].as_i64().unwrap();
        let name = r["name"].as_str().unwrap();
        let msg = r["message"].as_str().unwrap_or("");
        if status != 0 {
            eprintln!("  FAIL: {} — {}", name, msg);
        }
    }
    assert!(parsed.iter().all(|r| r["status"].as_i64() == Some(0)), "Not all tests passed");
}

#[test]
fn crypto_domexception() {
    let mut e = engine_with_html("<html><body></body></html>");
    e.eval_js(r#"
        var result = 'pending';
        crypto.subtle.generateKey({name:'FAKE_ALGO'}, true, ['encrypt'])
        .catch(function(e) {
            result = e.name + ',' + (e instanceof DOMException);
        });
    "#).unwrap();
    e.settle();
    assert_eq!(e.eval_js("result").unwrap(), "NotSupportedError,true");
}

#[test]
fn dynamic_script_no_src_does_not_fetch() {
    let mut e = engine_with_html("<html><head></head><body></body></html>");
    e.eval_js(r#"
        var s = document.createElement('script');
        s.textContent = 'window.__inline = 1';
        document.head.appendChild(s);
    "#).unwrap();

    assert!(!e.has_pending_fetches(), "inline script should not trigger fetch");
}

// =========================================================================
