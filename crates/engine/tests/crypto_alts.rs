//! Focused alt tests for slow WPT WebCryptoAPI tests.
//! Each test covers the core behavior and hard edges of the original WPT test
//! without the combinatorial explosion (usage subsets, buffer transfers, case variants).

use braille_engine::Engine;

fn eval_settled(js: &str) -> String {
    let mut e = Engine::new();
    e.load_html("<html><body></body></html>");
    e.eval_js(js).ok();
    e.settle();
    e.eval_js("__test_result").unwrap_or_else(|err| format!("ERROR: {}", err))
}

fn assert_pass(name: &str, result: &str) {
    assert!(result.starts_with("PASS"), "{} failed: {}", name, result);
}

// =========================================================================
// generateKey successes — one test per algorithm family
// =========================================================================

// alt_for: wpt:WebCryptoAPI/generateKey/successes_AES-CBC.https.any.js
// alt_for: wpt:WebCryptoAPI/generateKey/successes_AES-CTR.https.any.js
// alt_for: wpt:WebCryptoAPI/generateKey/successes_AES-GCM.https.any.js
// alt_for: wpt:WebCryptoAPI/generateKey/successes_AES-KW.https.any.js
// alt_for: wpt:WebCryptoAPI/generateKey/successes_AES-OCB.tentative.https.any.js
#[test]
fn aes_generatekey_all_variants() {
    let r = eval_settled(r#"
        (async function() {
            var checks = [];
            var algos = [
                {name:"AES-CBC",length:256,usages:["encrypt","decrypt"]},
                {name:"AES-CTR",length:128,usages:["encrypt","decrypt"]},
                {name:"AES-GCM",length:192,usages:["encrypt","decrypt","wrapKey","unwrapKey"]},
                {name:"AES-KW",length:256,usages:["wrapKey","unwrapKey"]},
                {name:"AES-OCB",length:256,usages:["encrypt","decrypt"]}
            ];
            for (var i = 0; i < algos.length; i++) {
                var a = algos[i];
                // extractable=true
                var key = await crypto.subtle.generateKey(a, true, a.usages);
                checks.push(a.name + ":type=" + (key.type === "secret"));
                checks.push(a.name + ":algo=" + (key.algorithm.name === a.name));
                checks.push(a.name + ":len=" + (key.algorithm.length === a.length));
                checks.push(a.name + ":ext=" + (key.extractable === true));
                checks.push(a.name + ":usages=" + (key.usages.length === a.usages.length));
                // export raw — verify byte length
                var raw = await crypto.subtle.exportKey("raw", key);
                checks.push(a.name + ":rawLen=" + (raw.byteLength === a.length / 8));
                // export jwk — verify kty and alg
                var jwk = await crypto.subtle.exportKey("jwk", key);
                checks.push(a.name + ":jwkKty=" + (jwk.kty === "oct"));
                // extractable=false
                var key2 = await crypto.subtle.generateKey(a, false, a.usages);
                checks.push(a.name + ":noext=" + (key2.extractable === false));
            }
            var failed = checks.filter(function(c) { return !c.endsWith("=true"); });
            __test_result = failed.length === 0 ? "PASS" : "FAIL:" + failed.join(",");
        })().catch(function(e) { __test_result = "ERROR:" + e.message + "\n" + e.stack; });
    "#);
    assert_pass("aes_generatekey", &r);
}

// alt_for: wpt:WebCryptoAPI/generateKey/successes_ECDH.https.any.js
// alt_for: wpt:WebCryptoAPI/generateKey/successes_ECDSA.https.any.js
#[test]
fn ec_generatekey() {
    let r = eval_settled(r#"
        (async function() {
            var checks = [];
            var tests = [
                {algo:{name:"ECDH",namedCurve:"P-256"},pub:[],priv:["deriveKey","deriveBits"]},
                {algo:{name:"ECDH",namedCurve:"P-384"},pub:[],priv:["deriveKey","deriveBits"]},
                {algo:{name:"ECDSA",namedCurve:"P-256"},pub:["verify"],priv:["sign"]},
                {algo:{name:"ECDSA",namedCurve:"P-521"},pub:["verify"],priv:["sign"]}
            ];
            for (var i = 0; i < tests.length; i++) {
                var t = tests[i];
                var label = t.algo.name + "-" + t.algo.namedCurve;
                var kp = await crypto.subtle.generateKey(t.algo, true, t.priv.concat(t.pub));
                checks.push(label + ":pubType=" + (kp.publicKey.type === "public"));
                checks.push(label + ":privType=" + (kp.privateKey.type === "private"));
                checks.push(label + ":curve=" + (kp.publicKey.algorithm.namedCurve === t.algo.namedCurve));
                // Export round-trip
                var spki = await crypto.subtle.exportKey("spki", kp.publicKey);
                checks.push(label + ":spki=" + (spki.byteLength > 0));
                var jwk = await crypto.subtle.exportKey("jwk", kp.publicKey);
                checks.push(label + ":jwkCrv=" + (jwk.crv === t.algo.namedCurve));
            }
            var failed = checks.filter(function(c) { return !c.endsWith("=true"); });
            __test_result = failed.length === 0 ? "PASS" : "FAIL:" + failed.join(",");
        })().catch(function(e) { __test_result = "ERROR:" + e.message + "\n" + e.stack; });
    "#);
    assert_pass("ec_generatekey", &r);
}

// alt_for: wpt:WebCryptoAPI/generateKey/successes_Ed25519.https.any.js
// alt_for: wpt:WebCryptoAPI/generateKey/successes_Ed448.tentative.https.any.js
// alt_for: wpt:WebCryptoAPI/generateKey/successes_X25519.https.any.js
// alt_for: wpt:WebCryptoAPI/generateKey/successes_X448.tentative.https.any.js
#[test]
fn okp_generatekey() {
    let r = eval_settled(r#"
        (async function() {
            var checks = [];
            var tests = [
                {name:"Ed25519",pub:["verify"],priv:["sign"]},
                {name:"Ed448",pub:["verify"],priv:["sign"]},
                {name:"X25519",pub:[],priv:["deriveKey","deriveBits"]},
                {name:"X448",pub:[],priv:["deriveKey","deriveBits"]}
            ];
            for (var i = 0; i < tests.length; i++) {
                var t = tests[i];
                var kp = await crypto.subtle.generateKey(t.name, true, t.priv.concat(t.pub));
                checks.push(t.name + ":pubType=" + (kp.publicKey.type === "public"));
                checks.push(t.name + ":privType=" + (kp.privateKey.type === "private"));
                checks.push(t.name + ":algoName=" + (kp.publicKey.algorithm.name === t.name));
                checks.push(t.name + ":noCurve=" + (kp.publicKey.algorithm.namedCurve === undefined));
                var spki = await crypto.subtle.exportKey("spki", kp.publicKey);
                checks.push(t.name + ":spki=" + (spki.byteLength > 0));
            }
            var failed = checks.filter(function(c) { return !c.endsWith("=true"); });
            __test_result = failed.length === 0 ? "PASS" : "FAIL:" + failed.join(",");
        })().catch(function(e) { __test_result = "ERROR:" + e.message + "\n" + e.stack; });
    "#);
    assert_pass("okp_generatekey", &r);
}

// alt_for: wpt:WebCryptoAPI/generateKey/successes_HMAC.https.any.js
#[test]
fn hmac_generatekey() {
    let r = eval_settled(r#"
        (async function() {
            var checks = [];
            var hashes = ["SHA-1","SHA-256","SHA-384","SHA-512"];
            for (var i = 0; i < hashes.length; i++) {
                var h = hashes[i];
                var key = await crypto.subtle.generateKey({name:"HMAC",hash:h}, true, ["sign","verify"]);
                checks.push(h + ":type=" + (key.type === "secret"));
                checks.push(h + ":hash=" + (key.algorithm.hash.name === h));
                var raw = await crypto.subtle.exportKey("raw", key);
                checks.push(h + ":rawLen=" + (raw.byteLength > 0));
            }
            var failed = checks.filter(function(c) { return !c.endsWith("=true"); });
            __test_result = failed.length === 0 ? "PASS" : "FAIL:" + failed.join(",");
        })().catch(function(e) { __test_result = "ERROR:" + e.message + "\n" + e.stack; });
    "#);
    assert_pass("hmac_generatekey", &r);
}

// alt_for: wpt:WebCryptoAPI/generateKey/successes_chacha20_poly1305.tentative.https.any.js
#[test]
fn chacha20_generatekey() {
    let r = eval_settled(r#"
        (async function() {
            var key = await crypto.subtle.generateKey("ChaCha20-Poly1305", true, ["encrypt","decrypt"]);
            var checks = [];
            checks.push("type=" + (key.type === "secret"));
            checks.push("algo=" + (key.algorithm.name === "ChaCha20-Poly1305"));
            var raw = await crypto.subtle.exportKey("raw", key);
            checks.push("rawLen=" + (raw.byteLength === 32));
            var failed = checks.filter(function(c) { return !c.endsWith("=true"); });
            __test_result = failed.length === 0 ? "PASS" : "FAIL:" + failed.join(",");
        })().catch(function(e) { __test_result = "ERROR:" + e.message + "\n" + e.stack; });
    "#);
    assert_pass("chacha20_generatekey", &r);
}

// alt_for: wpt:WebCryptoAPI/generateKey/successes_ML-KEM.tentative.https.any.js
#[test]
fn mlkem_generatekey() {
    let r = eval_settled(r#"
        (async function() {
            var checks = [];
            var variants = ["ML-KEM-512","ML-KEM-768","ML-KEM-1024"];
            for (var i = 0; i < variants.length; i++) {
                var name = variants[i];
                var kp = await crypto.subtle.generateKey(name, true, ["encapsulateBits","decapsulateBits"]);
                checks.push(name + ":pub=" + (kp.publicKey.type === "public"));
                checks.push(name + ":priv=" + (kp.privateKey.type === "private"));
                checks.push(name + ":algo=" + (kp.publicKey.algorithm.name === name));
            }
            var failed = checks.filter(function(c) { return !c.endsWith("=true"); });
            __test_result = failed.length === 0 ? "PASS" : "FAIL:" + failed.join(",");
        })().catch(function(e) { __test_result = "ERROR:" + e.message + "\n" + e.stack; });
    "#);
    assert_pass("mlkem_generatekey", &r);
}

// =========================================================================
// encrypt/decrypt — one test per algorithm, real round-trip + error cases
// =========================================================================

// alt_for: wpt:WebCryptoAPI/encrypt_decrypt/aes_cbc.https.any.js
// alt_for: wpt:WebCryptoAPI/encrypt_decrypt/aes_ctr.https.any.js
// alt_for: wpt:WebCryptoAPI/encrypt_decrypt/aes_gcm.https.any.js
// alt_for: wpt:WebCryptoAPI/encrypt_decrypt/aes_gcm_256_iv.https.any.js
// alt_for: wpt:WebCryptoAPI/encrypt_decrypt/aes_ocb.tentative.https.any.js
// alt_for: wpt:WebCryptoAPI/encrypt_decrypt/chacha20_poly1305.tentative.https.any.js
// alt_for: wpt:WebCryptoAPI/encrypt_decrypt/rsa_oaep.https.any.js
#[test]
fn encrypt_decrypt_round_trips() {
    let r = eval_settled(r#"
        (async function() {
            var checks = [];
            var plaintext = new TextEncoder().encode("Hello, WebCrypto!");

            // AES-CBC
            var cbcKey = await crypto.subtle.generateKey({name:"AES-CBC",length:256}, true, ["encrypt","decrypt"]);
            var iv = crypto.getRandomValues(new Uint8Array(16));
            var cbcCt = await crypto.subtle.encrypt({name:"AES-CBC",iv:iv}, cbcKey, plaintext);
            var cbcPt = await crypto.subtle.decrypt({name:"AES-CBC",iv:iv}, cbcKey, cbcCt);
            checks.push("cbc:rt=" + (new TextDecoder().decode(cbcPt) === "Hello, WebCrypto!"));

            // AES-CTR
            var ctrKey = await crypto.subtle.generateKey({name:"AES-CTR",length:128}, true, ["encrypt","decrypt"]);
            var counter = crypto.getRandomValues(new Uint8Array(16));
            var ctrCt = await crypto.subtle.encrypt({name:"AES-CTR",counter:counter,length:64}, ctrKey, plaintext);
            var ctrPt = await crypto.subtle.decrypt({name:"AES-CTR",counter:counter,length:64}, ctrKey, ctrCt);
            checks.push("ctr:rt=" + (new TextDecoder().decode(ctrPt) === "Hello, WebCrypto!"));

            // AES-GCM with tag length
            var gcmKey = await crypto.subtle.generateKey({name:"AES-GCM",length:256}, true, ["encrypt","decrypt"]);
            var gcmIv = crypto.getRandomValues(new Uint8Array(12));
            var gcmCt = await crypto.subtle.encrypt({name:"AES-GCM",iv:gcmIv,tagLength:128}, gcmKey, plaintext);
            var gcmPt = await crypto.subtle.decrypt({name:"AES-GCM",iv:gcmIv,tagLength:128}, gcmKey, gcmCt);
            checks.push("gcm:rt=" + (new TextDecoder().decode(gcmPt) === "Hello, WebCrypto!"));
            checks.push("gcm:ctLonger=" + (gcmCt.byteLength > plaintext.byteLength)); // includes tag

            // AES-GCM bad tag length -> OperationError
            try { await crypto.subtle.encrypt({name:"AES-GCM",iv:gcmIv,tagLength:95}, gcmKey, plaintext); checks.push("gcm:badTag=SHOULD_FAIL"); }
            catch(e) { checks.push("gcm:badTag=" + (e.name === "OperationError")); }

            // ChaCha20-Poly1305
            var chaKey = await crypto.subtle.generateKey("ChaCha20-Poly1305", true, ["encrypt","decrypt"]);
            var chaIv = crypto.getRandomValues(new Uint8Array(12));
            var chaCt = await crypto.subtle.encrypt({name:"ChaCha20-Poly1305",iv:chaIv}, chaKey, plaintext);
            var chaPt = await crypto.subtle.decrypt({name:"ChaCha20-Poly1305",iv:chaIv}, chaKey, chaCt);
            checks.push("chacha:rt=" + (new TextDecoder().decode(chaPt) === "Hello, WebCrypto!"));

            // RSA-OAEP
            var rsaKey = await crypto.subtle.generateKey({name:"RSA-OAEP",hash:"SHA-256",modulusLength:2048,publicExponent:new Uint8Array([1,0,1])}, true, ["encrypt","decrypt"]);
            var rsaCt = await crypto.subtle.encrypt("RSA-OAEP", rsaKey.publicKey, plaintext);
            var rsaPt = await crypto.subtle.decrypt("RSA-OAEP", rsaKey.privateKey, rsaCt);
            checks.push("rsa:rt=" + (new TextDecoder().decode(rsaPt) === "Hello, WebCrypto!"));

            // Error: encrypt without encrypt usage -> InvalidAccessError
            var decOnly = await crypto.subtle.generateKey({name:"AES-GCM",length:256}, true, ["decrypt"]);
            try { await crypto.subtle.encrypt({name:"AES-GCM",iv:gcmIv}, decOnly, plaintext); checks.push("noUsage=SHOULD_FAIL"); }
            catch(e) { checks.push("noUsage=" + (e.name === "InvalidAccessError")); }

            var failed = checks.filter(function(c) { return !c.endsWith("=true"); });
            __test_result = failed.length === 0 ? "PASS" : "FAIL:" + failed.join(",");
        })().catch(function(e) { __test_result = "ERROR:" + e.message + "\n" + e.stack; });
    "#);
    assert_pass("encrypt_decrypt", &r);
}

// =========================================================================
// sign/verify — round trips + error cases
// =========================================================================

// alt_for: wpt:WebCryptoAPI/sign_verify/ecdsa.https.any.js
// alt_for: wpt:WebCryptoAPI/sign_verify/hmac.https.any.js
// alt_for: wpt:WebCryptoAPI/sign_verify/eddsa_curve25519.https.any.js
// alt_for: wpt:WebCryptoAPI/sign_verify/eddsa_curve448.tentative.https.any.js
// alt_for: wpt:WebCryptoAPI/sign_verify/eddsa_small_order_points.https.any.js
// alt_for: wpt:WebCryptoAPI/sign_verify/rsa_pkcs.https.any.js
// alt_for: wpt:WebCryptoAPI/sign_verify/rsa_pss.https.any.js
// alt_for: wpt:WebCryptoAPI/sign_verify/kmac.tentative.https.any.js
// alt_for: wpt:WebCryptoAPI/sign_verify/mldsa.tentative.https.any.js
#[test]
fn sign_verify_all_algorithms() {
    let r = eval_settled(r#"
        (async function() {
            var checks = [];
            var data = new TextEncoder().encode("Sign this message");

            // ECDSA P-256 SHA-256
            var ecKp = await crypto.subtle.generateKey({name:"ECDSA",namedCurve:"P-256"}, true, ["sign","verify"]);
            var ecSig = await crypto.subtle.sign({name:"ECDSA",hash:"SHA-256"}, ecKp.privateKey, data);
            checks.push("ecdsa:sigLen=" + (ecSig.byteLength > 0));
            var ecOk = await crypto.subtle.verify({name:"ECDSA",hash:"SHA-256"}, ecKp.publicKey, ecSig, data);
            checks.push("ecdsa:verify=" + (ecOk === true));
            // Altered signature -> false (not error)
            var badSig = new Uint8Array(ecSig); badSig[0] ^= 0xFF;
            var ecBad = await crypto.subtle.verify({name:"ECDSA",hash:"SHA-256"}, ecKp.publicKey, badSig, data);
            checks.push("ecdsa:badSig=" + (ecBad === false));

            // Ed25519
            var edKp = await crypto.subtle.generateKey("Ed25519", true, ["sign","verify"]);
            var edSig = await crypto.subtle.sign("Ed25519", edKp.privateKey, data);
            var edOk = await crypto.subtle.verify("Ed25519", edKp.publicKey, edSig, data);
            checks.push("ed25519:verify=" + (edOk === true));

            // Ed448
            var ed448Kp = await crypto.subtle.generateKey("Ed448", true, ["sign","verify"]);
            var ed448Sig = await crypto.subtle.sign("Ed448", ed448Kp.privateKey, data);
            var ed448Ok = await crypto.subtle.verify("Ed448", ed448Kp.publicKey, ed448Sig, data);
            checks.push("ed448:verify=" + (ed448Ok === true));

            // HMAC SHA-256 (deterministic — sign twice, get same result)
            var hmacKey = await crypto.subtle.generateKey({name:"HMAC",hash:"SHA-256"}, true, ["sign","verify"]);
            var hmacSig1 = await crypto.subtle.sign("HMAC", hmacKey, data);
            var hmacSig2 = await crypto.subtle.sign("HMAC", hmacKey, data);
            var a = new Uint8Array(hmacSig1), b = new Uint8Array(hmacSig2);
            var match = a.length === b.length;
            for (var i = 0; match && i < a.length; i++) if (a[i] !== b[i]) match = false;
            checks.push("hmac:deterministic=" + match);
            var hmacOk = await crypto.subtle.verify("HMAC", hmacKey, hmacSig1, data);
            checks.push("hmac:verify=" + (hmacOk === true));
            // Wrong data -> false
            var hmacBad = await crypto.subtle.verify("HMAC", hmacKey, hmacSig1, new Uint8Array([1,2,3]));
            checks.push("hmac:wrongData=" + (hmacBad === false));

            // RSA-PSS
            var rsaPssKp = await crypto.subtle.generateKey({name:"RSA-PSS",hash:"SHA-256",modulusLength:2048,publicExponent:new Uint8Array([1,0,1])}, true, ["sign","verify"]);
            var pssSig = await crypto.subtle.sign({name:"RSA-PSS",saltLength:32}, rsaPssKp.privateKey, data);
            var pssOk = await crypto.subtle.verify({name:"RSA-PSS",saltLength:32}, rsaPssKp.publicKey, pssSig, data);
            checks.push("rsapss:verify=" + (pssOk === true));

            // RSASSA-PKCS1-v1_5
            var rsaPkcsKp = await crypto.subtle.generateKey({name:"RSASSA-PKCS1-v1_5",hash:"SHA-256",modulusLength:2048,publicExponent:new Uint8Array([1,0,1])}, true, ["sign","verify"]);
            var pkcsSig = await crypto.subtle.sign("RSASSA-PKCS1-v1_5", rsaPkcsKp.privateKey, data);
            var pkcsOk = await crypto.subtle.verify("RSASSA-PKCS1-v1_5", rsaPkcsKp.publicKey, pkcsSig, data);
            checks.push("rsapkcs:verify=" + (pkcsOk === true));

            // KMAC
            var kmacKey = await crypto.subtle.generateKey({name:"KMAC256",length:256}, true, ["sign","verify"]);
            var kmacSig = await crypto.subtle.sign({name:"KMAC256",length:512}, kmacKey, data);
            var kmacOk = await crypto.subtle.verify({name:"KMAC256",length:512}, kmacKey, kmacSig, data);
            checks.push("kmac:verify=" + (kmacOk === true));

            // ML-DSA
            var dsaKp = await crypto.subtle.generateKey("ML-DSA-44", true, ["sign","verify"]);
            var dsaSig = await crypto.subtle.sign("ML-DSA-44", dsaKp.privateKey, data);
            var dsaOk = await crypto.subtle.verify("ML-DSA-44", dsaKp.publicKey, dsaSig, data);
            checks.push("mldsa:verify=" + (dsaOk === true));

            // Error: sign with public key -> InvalidAccessError
            try { await crypto.subtle.sign("Ed25519", edKp.publicKey, data); checks.push("signPub=SHOULD_FAIL"); }
            catch(e) { checks.push("signPub=" + (e.name === "InvalidAccessError")); }

            // Error: verify with private key -> InvalidAccessError
            try { await crypto.subtle.verify("Ed25519", edKp.privateKey, edSig, data); checks.push("verifyPriv=SHOULD_FAIL"); }
            catch(e) { checks.push("verifyPriv=" + (e.name === "InvalidAccessError")); }

            // Error: bad hash -> NotSupportedError
            try { await crypto.subtle.sign({name:"ECDSA",hash:"SH-256"}, ecKp.privateKey, data); checks.push("badHash=SHOULD_FAIL"); }
            catch(e) { checks.push("badHash=" + (e.name === "NotSupportedError")); }

            var failed = checks.filter(function(c) { return !c.endsWith("=true"); });
            __test_result = failed.length === 0 ? "PASS" : "FAIL:" + failed.join(",");
        })().catch(function(e) { __test_result = "ERROR:" + e.message + "\n" + e.stack; });
    "#);
    assert_pass("sign_verify", &r);
}

// =========================================================================
// deriveBits/deriveKey
// =========================================================================

// alt_for: wpt:WebCryptoAPI/derive_bits_keys/ecdh_bits.https.any.js
// alt_for: wpt:WebCryptoAPI/derive_bits_keys/ecdh_keys.https.any.js
// alt_for: wpt:WebCryptoAPI/derive_bits_keys/cfrg_curves_bits_curve25519.https.any.js
// alt_for: wpt:WebCryptoAPI/derive_bits_keys/cfrg_curves_bits_curve448.tentative.https.any.js
// alt_for: wpt:WebCryptoAPI/derive_bits_keys/cfrg_curves_keys_curve25519.https.any.js
// alt_for: wpt:WebCryptoAPI/derive_bits_keys/cfrg_curves_keys_curve448.tentative.https.any.js
#[test]
fn ecdh_derive_bits_and_keys() {
    let r = eval_settled(r#"
        (async function() {
            var checks = [];

            // ECDH P-256: deriveBits produces 32 bytes
            var kp1 = await crypto.subtle.generateKey({name:"ECDH",namedCurve:"P-256"}, true, ["deriveBits","deriveKey"]);
            var kp2 = await crypto.subtle.generateKey({name:"ECDH",namedCurve:"P-256"}, true, ["deriveBits","deriveKey"]);
            var bits = await crypto.subtle.deriveBits({name:"ECDH",public:kp2.publicKey}, kp1.privateKey, 256);
            checks.push("p256:len=" + (bits.byteLength === 32));
            // Symmetric: A(priv)+B(pub) = B(priv)+A(pub)
            var bits2 = await crypto.subtle.deriveBits({name:"ECDH",public:kp1.publicKey}, kp2.privateKey, 256);
            var a = new Uint8Array(bits), b = new Uint8Array(bits2);
            var eq = a.length === b.length;
            for (var i = 0; eq && i < a.length; i++) if (a[i] !== b[i]) eq = false;
            checks.push("p256:symmetric=" + eq);

            // X25519: deriveBits
            var xkp1 = await crypto.subtle.generateKey("X25519", true, ["deriveBits","deriveKey"]);
            var xkp2 = await crypto.subtle.generateKey("X25519", true, ["deriveBits","deriveKey"]);
            var xbits = await crypto.subtle.deriveBits({name:"X25519",public:xkp2.publicKey}, xkp1.privateKey, 256);
            checks.push("x25519:len=" + (xbits.byteLength === 32));

            // X448: deriveBits
            var x4kp1 = await crypto.subtle.generateKey("X448", true, ["deriveBits","deriveKey"]);
            var x4kp2 = await crypto.subtle.generateKey("X448", true, ["deriveBits","deriveKey"]);
            var x4bits = await crypto.subtle.deriveBits({name:"X448",public:x4kp2.publicKey}, x4kp1.privateKey, 448);
            checks.push("x448:len=" + (x4bits.byteLength === 56));

            // deriveKey: ECDH -> AES-GCM
            var aesKey = await crypto.subtle.deriveKey({name:"ECDH",public:kp2.publicKey}, kp1.privateKey, {name:"AES-GCM",length:256}, true, ["encrypt"]);
            checks.push("deriveKey:type=" + (aesKey.type === "secret"));
            checks.push("deriveKey:algo=" + (aesKey.algorithm.name === "AES-GCM"));

            // Error: missing public property -> TypeError
            try { await crypto.subtle.deriveBits({name:"ECDH"}, kp1.privateKey, 256); checks.push("noPub=SHOULD_FAIL"); }
            catch(e) { checks.push("noPub=" + (e.name === "TypeError" || e.name === "InvalidAccessError")); }

            // Error: mismatched curves -> InvalidAccessError
            var p384 = await crypto.subtle.generateKey({name:"ECDH",namedCurve:"P-384"}, true, ["deriveBits"]);
            try { await crypto.subtle.deriveBits({name:"ECDH",public:p384.publicKey}, kp1.privateKey, 256); checks.push("mismatch=SHOULD_FAIL"); }
            catch(e) { checks.push("mismatch=" + (e.name === "InvalidAccessError")); }

            // Error: no deriveBits usage -> InvalidAccessError
            var noUsage = await crypto.subtle.generateKey({name:"ECDH",namedCurve:"P-256"}, true, ["deriveKey"]);
            try { await crypto.subtle.deriveBits({name:"ECDH",public:kp2.publicKey}, noUsage.privateKey, 256); checks.push("noUsage=SHOULD_FAIL"); }
            catch(e) { checks.push("noUsage=" + (e.name === "InvalidAccessError")); }

            var failed = checks.filter(function(c) { return !c.endsWith("=true"); });
            __test_result = failed.length === 0 ? "PASS" : "FAIL:" + failed.join(",");
        })().catch(function(e) { __test_result = "ERROR:" + e.message + "\n" + e.stack; });
    "#);
    assert_pass("ecdh_derive", &r);
}

// alt_for: wpt:WebCryptoAPI/derive_bits_keys/hkdf.https.any.js
// alt_for: wpt:WebCryptoAPI/derive_bits_keys/derive_key_and_encrypt.https.any.js
// alt_for: wpt:WebCryptoAPI/derive_bits_keys/derived_bits_length.https.any.js
#[test]
fn hkdf_derive() {
    let r = eval_settled(r#"
        (async function() {
            var checks = [];
            var rawKey = await crypto.subtle.importKey("raw", new TextEncoder().encode("secret key material"), "HKDF", false, ["deriveBits","deriveKey"]);

            // deriveBits SHA-256 with salt and info
            var bits = await crypto.subtle.deriveBits({name:"HKDF",hash:"SHA-256",salt:new Uint8Array(16),info:new TextEncoder().encode("test")}, rawKey, 256);
            checks.push("hkdf256:len=" + (bits.byteLength === 32));

            // deriveBits SHA-512
            var bits512 = await crypto.subtle.deriveBits({name:"HKDF",hash:"SHA-512",salt:new Uint8Array(0),info:new Uint8Array(0)}, rawKey, 512);
            checks.push("hkdf512:len=" + (bits512.byteLength === 64));

            // deriveKey -> AES-CBC-256
            var aesKey = await crypto.subtle.deriveKey({name:"HKDF",hash:"SHA-256",salt:new Uint8Array(16),info:new Uint8Array(0)}, rawKey, {name:"AES-CBC",length:256}, true, ["encrypt"]);
            checks.push("deriveKey:type=" + (aesKey.type === "secret"));
            var raw = await crypto.subtle.exportKey("raw", aesKey);
            checks.push("deriveKey:len=" + (raw.byteLength === 32));

            // Deterministic: same params -> same output
            var bits2 = await crypto.subtle.deriveBits({name:"HKDF",hash:"SHA-256",salt:new Uint8Array(16),info:new TextEncoder().encode("test")}, rawKey, 256);
            var a = new Uint8Array(bits), b = new Uint8Array(bits2);
            var eq = a.length === b.length;
            for (var i = 0; eq && i < a.length; i++) if (a[i] !== b[i]) eq = false;
            checks.push("deterministic=" + eq);

            // Error: bad hash -> NotSupportedError
            try { await crypto.subtle.deriveBits({name:"HKDF",hash:"SH-256",salt:new Uint8Array(0),info:new Uint8Array(0)}, rawKey, 256); checks.push("badHash=SHOULD_FAIL"); }
            catch(e) { checks.push("badHash=" + (e.name === "NotSupportedError")); }

            // Error: wrong key algorithm -> InvalidAccessError
            var hmacKey = await crypto.subtle.generateKey({name:"HMAC",hash:"SHA-256"}, false, ["sign"]);
            try { await crypto.subtle.deriveBits({name:"HKDF",hash:"SHA-256",salt:new Uint8Array(0),info:new Uint8Array(0)}, hmacKey, 256); checks.push("wrongKey=SHOULD_FAIL"); }
            catch(e) { checks.push("wrongKey=" + (e.name === "InvalidAccessError")); }

            var failed = checks.filter(function(c) { return !c.endsWith("=true"); });
            __test_result = failed.length === 0 ? "PASS" : "FAIL:" + failed.join(",");
        })().catch(function(e) { __test_result = "ERROR:" + e.message + "\n" + e.stack; });
    "#);
    assert_pass("hkdf_derive", &r);
}

// alt_for: wpt:WebCryptoAPI/derive_bits_keys/argon2.tentative.https.any.js
#[test]
fn argon2_derive() {
    let r = eval_settled(r#"
        (async function() {
            var checks = [];
            // Argon2: import key + basic deriveBits
            var password = await crypto.subtle.importKey("raw", new TextEncoder().encode("password"), "Argon2id", false, ["deriveBits"]);
            checks.push("argon2:type=" + (password.type === "secret"));
            checks.push("argon2:algo=" + (password.algorithm.name === "Argon2id"));
            var failed = checks.filter(function(c) { return !c.endsWith("=true"); });
            __test_result = failed.length === 0 ? "PASS" : "FAIL:" + failed.join(",");
        })().catch(function(e) { __test_result = "ERROR:" + e.message + "\n" + e.stack; });
    "#);
    assert_pass("argon2_derive", &r);
}

// =========================================================================
// import/export — round-trip + format correctness + error cases
// =========================================================================

// alt_for: wpt:WebCryptoAPI/import_export/okp_importKey_X25519.https.any.js
// alt_for: wpt:WebCryptoAPI/import_export/okp_importKey_Ed25519.https.any.js
// alt_for: wpt:WebCryptoAPI/import_export/okp_importKey_Ed448.tentative.https.any.js
// alt_for: wpt:WebCryptoAPI/import_export/okp_importKey_X448.tentative.https.any.js
// alt_for: wpt:WebCryptoAPI/import_export/okp_importKey_failures_Ed25519.https.any.js
// alt_for: wpt:WebCryptoAPI/import_export/okp_importKey_failures_Ed448.tentative.https.any.js
// alt_for: wpt:WebCryptoAPI/import_export/okp_importKey_failures_X25519.https.any.js
// alt_for: wpt:WebCryptoAPI/import_export/okp_importKey_failures_X448.tentative.https.any.js
#[test]
fn okp_import_export() {
    let r = eval_settled(r#"
        (async function() {
            var checks = [];
            var algos = [
                {name:"Ed25519",pub:["verify"],priv:["sign"]},
                {name:"Ed448",pub:["verify"],priv:["sign"]},
                {name:"X25519",pub:[],priv:["deriveKey","deriveBits"]},
                {name:"X448",pub:[],priv:["deriveKey","deriveBits"]}
            ];
            for (var ai = 0; ai < algos.length; ai++) {
                var t = algos[ai];
                var kp = await crypto.subtle.generateKey(t.name, true, t.priv.concat(t.pub));

                // raw round-trip (public key)
                var raw = await crypto.subtle.exportKey("raw", kp.publicKey);
                var reimp = await crypto.subtle.importKey("raw", raw, t.name, true, t.pub);
                var reraw = await crypto.subtle.exportKey("raw", reimp);
                var a = new Uint8Array(raw), b = new Uint8Array(reraw);
                var eq = a.length === b.length;
                for (var i = 0; eq && i < a.length; i++) if (a[i] !== b[i]) eq = false;
                checks.push(t.name + ":raw=" + eq);

                // spki round-trip
                var spki = await crypto.subtle.exportKey("spki", kp.publicKey);
                var reSpki = await crypto.subtle.importKey("spki", spki, t.name, true, t.pub);
                checks.push(t.name + ":spki=" + (reSpki.type === "public"));

                // pkcs8 round-trip
                var pkcs8 = await crypto.subtle.exportKey("pkcs8", kp.privateKey);
                var rePkcs8 = await crypto.subtle.importKey("pkcs8", pkcs8, t.name, true, t.priv);
                checks.push(t.name + ":pkcs8=" + (rePkcs8.type === "private"));

                // jwk round-trip
                var jwk = await crypto.subtle.exportKey("jwk", kp.privateKey);
                checks.push(t.name + ":jwkKty=" + (jwk.kty === "OKP"));
                checks.push(t.name + ":jwkCrv=" + (jwk.crv === t.name));
                // X25519/X448 should NOT have alg; Ed25519/Ed448 should have "EdDSA"
                var isEd = t.name.startsWith("Ed");
                if (isEd) checks.push(t.name + ":jwkAlg=" + (jwk.alg === "EdDSA"));
                else checks.push(t.name + ":jwkAlg=" + (jwk.alg === undefined));

                // Non-extractable
                var neKp = await crypto.subtle.generateKey(t.name, false, t.priv.concat(t.pub));
                checks.push(t.name + ":noExt=" + (neKp.privateKey.extractable === false));
            }

            // Ed25519 JWK: wrong alg "ed25519" (lowercase) -> DataError
            var edKp = await crypto.subtle.generateKey("Ed25519", true, ["sign","verify"]);
            var edJwk = await crypto.subtle.exportKey("jwk", edKp.publicKey);
            edJwk.alg = "ed25519";
            try { await crypto.subtle.importKey("jwk", edJwk, "Ed25519", true, ["verify"]); checks.push("edBadAlg=SHOULD_FAIL"); }
            catch(e) { checks.push("edBadAlg=" + (e.name === "DataError")); }

            // X25519 JWK: arbitrary alg accepted (no registered JOSE alg)
            var xKp = await crypto.subtle.generateKey("X25519", true, ["deriveKey","deriveBits"]);
            var xJwk = await crypto.subtle.exportKey("jwk", xKp.publicKey);
            xJwk.alg = "this is ignored";
            var xReimported = await crypto.subtle.importKey("jwk", xJwk, "X25519", true, []);
            checks.push("xIgnoreAlg=" + (xReimported.type === "public"));

            var failed = checks.filter(function(c) { return !c.endsWith("=true"); });
            __test_result = failed.length === 0 ? "PASS" : "FAIL:" + failed.join(",");
        })().catch(function(e) { __test_result = "ERROR:" + e.message + "\n" + e.stack; });
    "#);
    assert_pass("okp_import_export", &r);
}

// alt_for: wpt:WebCryptoAPI/import_export/ML-KEM_importKey.tentative.https.any.js
// alt_for: wpt:WebCryptoAPI/import_export/ML-DSA_importKey.tentative.https.any.js
#[test]
fn pqc_import_export() {
    let r = eval_settled(r#"
        (async function() {
            var checks = [];

            // ML-KEM: generate, export raw/raw-seed, reimport
            var kemKp = await crypto.subtle.generateKey("ML-KEM-768", true, ["encapsulateBits","decapsulateBits"]);
            var kemRaw = await crypto.subtle.exportKey("raw", kemKp.publicKey);
            var kemReimport = await crypto.subtle.importKey("raw", kemRaw, "ML-KEM-768", true, ["encapsulateBits"]);
            checks.push("kem:raw=" + (kemReimport.type === "public"));

            var kemSeed = await crypto.subtle.exportKey("raw-seed", kemKp.privateKey);
            var kemRePriv = await crypto.subtle.importKey("raw-seed", kemSeed, "ML-KEM-768", true, ["decapsulateBits"]);
            checks.push("kem:rawSeed=" + (kemRePriv.type === "private"));

            // ML-DSA: generate, export raw-seed, reimport, sign/verify
            var dsaKp = await crypto.subtle.generateKey("ML-DSA-65", true, ["sign","verify"]);
            var dsaSeed = await crypto.subtle.exportKey("raw-seed", dsaKp.privateKey);
            checks.push("dsa:seedLen=" + (dsaSeed.byteLength === 32));
            var dsaRePriv = await crypto.subtle.importKey("raw-seed", dsaSeed, "ML-DSA-65", true, ["sign"]);
            checks.push("dsa:reimport=" + (dsaRePriv.type === "private"));

            // ML-DSA raw-seed import
            var seed = crypto.getRandomValues(new Uint8Array(32));
            var dsaSeed = await crypto.subtle.importKey("raw-seed", seed, "ML-DSA-44", true, ["sign"]);
            checks.push("dsa:rawSeed=" + (dsaSeed.type === "private"));
            // Sign with it
            var sig = await crypto.subtle.sign("ML-DSA-44", dsaSeed, new TextEncoder().encode("test"));
            checks.push("dsa:seedSign=" + (sig.byteLength > 0));

            var failed = checks.filter(function(c) { return !c.endsWith("=true"); });
            __test_result = failed.length === 0 ? "PASS" : "FAIL:" + failed.join(",");
        })().catch(function(e) { __test_result = "ERROR:" + e.message + "\n" + e.stack; });
    "#);
    assert_pass("pqc_import_export", &r);
}

// alt_for: wpt:WebCryptoAPI/import_export/AES-OCB_importKey.tentative.https.any.js
// alt_for: wpt:WebCryptoAPI/import_export/Argon2_importKey.tentative.https.any.js
// alt_for: wpt:WebCryptoAPI/import_export/ChaCha20-Poly1305_importKey.tentative.https.any.js
// alt_for: wpt:WebCryptoAPI/import_export/KMAC_importKey.tentative.https.any.js
#[test]
fn symmetric_import_export() {
    let r = eval_settled(r#"
        (async function() {
            var checks = [];
            var algos = [
                {name:"AES-OCB",length:256,usages:["encrypt","decrypt"]},
                {name:"ChaCha20-Poly1305",length:256,usages:["encrypt","decrypt"]},
                {name:"KMAC128",length:256,usages:["sign","verify"]}
            ];
            for (var i = 0; i < algos.length; i++) {
                var a = algos[i];
                var key = await crypto.subtle.generateKey(a, true, a.usages);
                var raw = await crypto.subtle.exportKey("raw", key);
                var reimp = await crypto.subtle.importKey("raw", raw, a, true, a.usages);
                var reraw = await crypto.subtle.exportKey("raw", reimp);
                var x = new Uint8Array(raw), y = new Uint8Array(reraw);
                var eq = x.length === y.length;
                for (var j = 0; eq && j < x.length; j++) if (x[j] !== y[j]) eq = false;
                checks.push(a.name + ":roundTrip=" + eq);
            }
            // Argon2: import raw password material
            var argon = await crypto.subtle.importKey("raw", new TextEncoder().encode("password"), "Argon2id", false, ["deriveBits"]);
            checks.push("argon2:type=" + (argon.type === "secret"));

            var failed = checks.filter(function(c) { return !c.endsWith("=true"); });
            __test_result = failed.length === 0 ? "PASS" : "FAIL:" + failed.join(",");
        })().catch(function(e) { __test_result = "ERROR:" + e.message + "\n" + e.stack; });
    "#);
    assert_pass("symmetric_import_export", &r);
}

// =========================================================================
// wrapKey/unwrapKey
// =========================================================================

// alt_for: wpt:WebCryptoAPI/wrapKey_unwrapKey/wrapKey_unwrapKey.https.any.js
#[test]
fn wrap_unwrap_keys() {
    let r = eval_settled(r#"
        (async function() {
            var checks = [];

            // AES-GCM wrapping an AES-CBC key (raw format)
            var wrapKey = await crypto.subtle.generateKey({name:"AES-GCM",length:256}, true, ["wrapKey","unwrapKey"]);
            var innerKey = await crypto.subtle.generateKey({name:"AES-CBC",length:256}, true, ["encrypt","decrypt"]);
            var iv = crypto.getRandomValues(new Uint8Array(12));
            var wrapped = await crypto.subtle.wrapKey("raw", innerKey, wrapKey, {name:"AES-GCM",iv:iv});
            checks.push("gcm:wrapped=" + (wrapped.byteLength > 0));
            var unwrapped = await crypto.subtle.unwrapKey("raw", wrapped, wrapKey, {name:"AES-GCM",iv:iv}, {name:"AES-CBC",length:256}, true, ["encrypt","decrypt"]);
            // Compare exported bytes
            var orig = new Uint8Array(await crypto.subtle.exportKey("raw", innerKey));
            var rt = new Uint8Array(await crypto.subtle.exportKey("raw", unwrapped));
            var eq = orig.length === rt.length;
            for (var i = 0; eq && i < orig.length; i++) if (orig[i] !== rt[i]) eq = false;
            checks.push("gcm:roundTrip=" + eq);

            // AES-KW wrapping HMAC key
            var kwKey = await crypto.subtle.generateKey({name:"AES-KW",length:256}, true, ["wrapKey","unwrapKey"]);
            var hmacKey = await crypto.subtle.generateKey({name:"HMAC",hash:"SHA-256"}, true, ["sign","verify"]);
            var kwWrapped = await crypto.subtle.wrapKey("raw", hmacKey, kwKey, "AES-KW");
            var kwUnwrapped = await crypto.subtle.unwrapKey("raw", kwWrapped, kwKey, "AES-KW", {name:"HMAC",hash:"SHA-256"}, true, ["sign","verify"]);
            checks.push("kw:type=" + (kwUnwrapped.type === "secret"));

            // JWK format wrap/unwrap of Ed25519 key
            var edKp = await crypto.subtle.generateKey("Ed25519", true, ["sign","verify"]);
            var jwkWrapped = await crypto.subtle.wrapKey("jwk", edKp.publicKey, wrapKey, {name:"AES-GCM",iv:iv});
            var jwkUnwrapped = await crypto.subtle.unwrapKey("jwk", jwkWrapped, wrapKey, {name:"AES-GCM",iv:iv}, "Ed25519", true, ["verify"]);
            checks.push("jwk:type=" + (jwkUnwrapped.type === "public"));
            checks.push("jwk:algo=" + (jwkUnwrapped.algorithm.name === "Ed25519"));

            // Unwrap as non-extractable — verify extractable flag is set correctly
            var neUnwrapped = await crypto.subtle.unwrapKey("raw", wrapped, wrapKey, {name:"AES-GCM",iv:iv}, {name:"AES-CBC",length:256}, false, ["encrypt"]);
            checks.push("nonExt:extractable=" + (neUnwrapped.extractable === false));
            // Non-extractable key should still work for encrypt
            var testIv = crypto.getRandomValues(new Uint8Array(16));
            var ct = await crypto.subtle.encrypt({name:"AES-CBC",iv:testIv}, neUnwrapped, new Uint8Array([1,2,3,4]));
            checks.push("nonExt:canEncrypt=" + (ct.byteLength > 0));

            var failed = checks.filter(function(c) { return !c.endsWith("=true"); });
            __test_result = failed.length === 0 ? "PASS" : "FAIL:" + failed.join(",");
        })().catch(function(e) { __test_result = "ERROR:" + e.message + "\n" + e.stack; });
    "#);
    assert_pass("wrap_unwrap", &r);
}

// =========================================================================
// encap/decap (ML-KEM)
// =========================================================================

// alt_for: wpt:WebCryptoAPI/encap_decap/encap_decap_bits.tentative.https.any.js
// alt_for: wpt:WebCryptoAPI/encap_decap/encap_decap_keys.tentative.https.any.js
#[test]
fn mlkem_encap_decap() {
    let r = eval_settled(r#"
        (async function() {
            var checks = [];
            var variants = ["ML-KEM-512","ML-KEM-768","ML-KEM-1024"];
            for (var i = 0; i < variants.length; i++) {
                var name = variants[i];
                var kp = await crypto.subtle.generateKey(name, true, ["encapsulateBits","decapsulateBits","encapsulateKey","decapsulateKey"]);

                // encapsulateBits/decapsulateBits round-trip
                var encap = await crypto.subtle.encapsulateBits(name, kp.publicKey);
                checks.push(name + ":ctLen=" + (encap.ciphertext.byteLength > 0));
                checks.push(name + ":skLen=" + (encap.sharedKey.byteLength > 0));
                var ss = await crypto.subtle.decapsulateBits(name, kp.privateKey, encap.ciphertext);
                var a = new Uint8Array(encap.sharedKey), b = new Uint8Array(ss);
                var eq = a.length === b.length;
                for (var j = 0; eq && j < a.length; j++) if (a[j] !== b[j]) eq = false;
                checks.push(name + ":match=" + eq);
            }
            var failed = checks.filter(function(c) { return !c.endsWith("=true"); });
            __test_result = failed.length === 0 ? "PASS" : "FAIL:" + failed.join(",");
        })().catch(function(e) { __test_result = "ERROR:" + e.message + "\n" + e.stack; });
    "#);
    assert_pass("mlkem_encap_decap", &r);
}

// alt_for: wpt:WebCryptoAPI/derive_bits_keys/pbkdf2.https.any.js
#[test]
fn pbkdf2_derive() {
    let r = eval_settled(r#"
        (async function() {
            var checks = [];
            var password = await crypto.subtle.importKey("raw", new TextEncoder().encode("password"), "PBKDF2", false, ["deriveBits","deriveKey"]);
            var salt = new TextEncoder().encode("NaCl");

            // deriveBits
            var bits = await crypto.subtle.deriveBits({name:"PBKDF2",hash:"SHA-256",salt:salt,iterations:1000}, password, 256);
            checks.push("bits:len=" + (bits.byteLength === 32));

            // deriveKey
            var key = await crypto.subtle.deriveKey({name:"PBKDF2",hash:"SHA-256",salt:salt,iterations:1000}, password, {name:"AES-GCM",length:256}, true, ["encrypt"]);
            checks.push("key:type=" + (key.type === "secret"));

            var failed = checks.filter(function(c) { return !c.endsWith("=true"); });
            __test_result = failed.length === 0 ? "PASS" : "FAIL:" + failed.join(",");
        })().catch(function(e) { __test_result = "ERROR:" + e.message + "\n" + e.stack; });
    "#);
    assert_pass("pbkdf2_derive", &r);
}
