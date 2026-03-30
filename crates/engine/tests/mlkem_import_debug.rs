use braille_engine::Engine;

fn eval_settled(js: &str) -> String {
    let mut e = Engine::new();
    e.load_html("<html><body></body></html>");
    e.eval_js(js).ok();
    e.settle();
    e.eval_js("__test_result").unwrap_or_else(|err| format!("ERROR: {}", err))
}

#[test]
fn mlkem_pkcs8_roundtrip() {
    let r = eval_settled(r#"
        (async function() {
            try {
                var kp = await crypto.subtle.generateKey('ML-KEM-512', true, ['decapsulateBits']);
                var pkcs8 = await crypto.subtle.exportKey('pkcs8', kp.privateKey);

                // Try importing our own export
                var imported = await crypto.subtle.importKey('pkcs8', pkcs8, 'ML-KEM-512', true, ['decapsulateBits']);
                __test_result = 'import=OK type=' + imported.type;
            } catch(e) {
                __test_result = 'ERROR: ' + e.message + ' | ' + (e.stack || '');
            }
        })();
    "#);
    eprintln!("mlkem_pkcs8_roundtrip: {}", r);
    assert!(r.contains("import=OK"), "Got: {}", r);
}

#[test]
fn mlkem_pkcs8_wpt_fixture() {
    // Use the exact bytes from the WPT test fixture for ML-KEM-512
    let r = eval_settled(r#"
        (async function() {
            try {
                var pkcs8 = new Uint8Array([
                    48, 84, 2, 1, 0, 48, 11, 6, 9, 96, 134, 72, 1, 101, 3, 4, 4, 1, 4, 66,
                    128, 64, 165, 38, 193, 164, 132, 122, 104, 173, 83, 214, 227, 31, 231,
                    183, 152, 228, 96, 140, 65, 23, 83, 131, 8, 205, 245, 192, 122, 226, 244,
                    94, 134, 109, 118, 60, 173, 203, 25, 75, 189, 144, 22, 37, 163, 53, 78,
                    149, 185, 80, 130, 235, 161, 49, 141, 160, 11, 40, 152, 18, 142, 30, 56,
                    252, 129, 211
                ]);
                var imported = await crypto.subtle.importKey('pkcs8', pkcs8.buffer, 'ML-KEM-512', true, ['decapsulateBits']);
                __test_result = 'import=OK type=' + imported.type;

                // Verify round-trip: export and re-import
                var exported = await crypto.subtle.exportKey('pkcs8', imported);
                var bytes = new Uint8Array(exported);
                var match = bytes.length === pkcs8.length;
                if (match) {
                    for (var i = 0; i < bytes.length; i++) {
                        if (bytes[i] !== pkcs8[i]) { match = false; break; }
                    }
                }
                __test_result += ' roundtrip=' + match;
            } catch(e) {
                __test_result = 'ERROR: ' + e.message + ' | ' + (e.stack || '');
            }
        })();
    "#);
    eprintln!("mlkem_pkcs8_wpt_fixture: {}", r);
    assert!(r.contains("import=OK"), "Got: {}", r);
    assert!(r.contains("roundtrip=true"), "Got: {}", r);
}
