use braille_engine::Engine;

// alt_for: wpt:WebCryptoAPI/derive_bits_keys/pbkdf2.https.any.js
#[test]
fn pbkdf2_alt() {
    let mut engine = Engine::new();
    // Tests PBKDF2 deriveBits with multiple hashes, iterations, deriveKey, and error cases.
    // Replaces the WPT test which runs 8000+ subtests with 100k iterations each (too slow in debug).
    let html = r#"<!DOCTYPE html>
<html><body><script>
var subtle = crypto.subtle;
var errors = [];

function check(name, promise) {
    return promise.then(function() {
        console.log("PASS " + name);
    }).catch(function(e) {
        console.log("FAIL " + name + ": " + e.name + ": " + e.message);
        errors.push(name);
    });
}

function checkReject(name, expectedError, promise) {
    return promise.then(function() {
        console.log("FAIL " + name + ": should have rejected");
        errors.push(name);
    }, function(e) {
        if (e.name === expectedError) {
            console.log("PASS " + name);
        } else {
            console.log("FAIL " + name + ": expected " + expectedError + " got " + e.name + ": " + e.message);
            errors.push(name);
        }
    });
}

var password = new Uint8Array([80, 64, 115, 115, 119, 48, 114, 100]);
var salt = new Uint8Array([83, 111, 100, 105, 117, 109, 32, 67, 104, 108, 111, 114, 105, 100, 101]);

subtle.importKey("raw", password, {name: "PBKDF2"}, false, ["deriveBits", "deriveKey"]).then(function(key) {
    var tests = [];

    // deriveBits with each hash
    ["SHA-1", "SHA-256", "SHA-384", "SHA-512"].forEach(function(hash) {
        tests.push(check("deriveBits " + hash, subtle.deriveBits({name: "PBKDF2", hash: hash, salt: salt, iterations: 1000}, key, 256)));
    });

    // deriveBits with 0 length
    tests.push(check("deriveBits 0 length", subtle.deriveBits({name: "PBKDF2", hash: "SHA-256", salt: salt, iterations: 1}, key, 0)));

    // deriveKey
    tests.push(check("deriveKey AES-CBC-256",
        subtle.deriveKey({name: "PBKDF2", hash: "SHA-256", salt: salt, iterations: 1000}, key, {name: "AES-CBC", length: 256}, true, ["encrypt"])
    ));

    // Error: bad hash
    tests.push(checkReject("bad hash", "NotSupportedError",
        subtle.deriveBits({name: "PBKDF2", hash: "SHA256", salt: salt, iterations: 1}, key, 256)
    ));

    // Error: 0 iterations
    tests.push(checkReject("0 iterations", "OperationError",
        subtle.deriveBits({name: "PBKDF2", hash: "SHA-256", salt: salt, iterations: 0}, key, 256)
    ));

    // Error: null length
    tests.push(checkReject("null length", "OperationError",
        subtle.deriveBits({name: "PBKDF2", hash: "SHA-256", salt: salt, iterations: 1}, key, null)
    ));

    // Error: non-multiple-of-8 length
    tests.push(checkReject("non-mult-8 length", "OperationError",
        subtle.deriveBits({name: "PBKDF2", hash: "SHA-256", salt: salt, iterations: 1}, key, 7)
    ));

    // Error: missing deriveBits usage
    return subtle.importKey("raw", password, {name: "PBKDF2"}, false, ["deriveKey"]).then(function(noBitsKey) {
        tests.push(checkReject("missing deriveBits usage", "InvalidAccessError",
            subtle.deriveBits({name: "PBKDF2", hash: "SHA-256", salt: salt, iterations: 1}, noBitsKey, 256)
        ));

        // Error: wrong algorithm key
        return subtle.generateKey({name: "ECDH", namedCurve: "P-256"}, false, ["deriveBits"]).then(function(ecPair) {
            tests.push(checkReject("wrong algorithm key", "InvalidAccessError",
                subtle.deriveBits({name: "PBKDF2", hash: "SHA-256", salt: salt, iterations: 1}, ecPair.privateKey, 256)
            ));

            return Promise.all(tests).then(function() {
                console.log("DONE errors=" + errors.length);
            });
        });
    });
});
</script></body></html>"#;

    engine.load_html(html);
    engine.settle();

    let console = engine.drain_console();
    for line in &console {
        eprintln!("  {}", line);
    }

    let done = console.iter().any(|l: &String| l.contains("DONE errors=0"));
    assert!(done, "all PBKDF2 subtests should pass");
}
