use rquickjs::Ctx;

pub(super) fn register_crypto(ctx: &Ctx<'_>) {
    ctx.eval::<(), _>(crypto_js()).unwrap();
}

fn crypto_js() -> &'static str {
    r#"
(function() {
    // ---- DOMException (if not already defined) ----
    if (typeof globalThis.DOMException === 'undefined') {
        globalThis.DOMException = (function() {
            var codeMap = {
                IndexSizeError: 1, DOMStringSizeError: 2, HierarchyRequestError: 3,
                WrongDocumentError: 4, InvalidCharacterError: 5, NoDataAllowedError: 6,
                NoModificationAllowedError: 7, NotFoundError: 8, NotSupportedError: 9,
                InUseAttributeError: 10, InvalidStateError: 11, SyntaxError: 12,
                InvalidModificationError: 13, NamespaceError: 14, InvalidAccessError: 15,
                TypeMismatchError: 17, SecurityError: 18, NetworkError: 19,
                AbortError: 20, URLMismatchError: 21, QuotaExceededError: 22,
                TimeoutError: 23, InvalidNodeTypeError: 24, DataCloneError: 25,
                EncodingError: 0, NotReadableError: 0, UnknownError: 0,
                ConstraintError: 0, DataError: 0, TransactionInactiveError: 0,
                ReadOnlyError: 0, VersionError: 0, OperationError: 0
            };
            function DOMException(message, name) {
                this.message = message || '';
                this.name = name || 'Error';
                this.code = codeMap[this.name] || 0;
                this.stack = (new Error()).stack;
            }
            DOMException.prototype = Object.create(Error.prototype);
            DOMException.prototype.constructor = DOMException;
            Object.defineProperty(DOMException.prototype, Symbol.toStringTag, {value: 'DOMException', configurable: true});
            for (var n in codeMap) {
                if (codeMap[n] > 0) {
                    DOMException[n] = codeMap[n];
                    DOMException.prototype[n] = codeMap[n];
                }
            }
            return DOMException;
        })();
    }

    // ---- CryptoKey class ----
    function CryptoKey() {
        throw new TypeError("Illegal constructor");
    }
    CryptoKey.prototype = Object.create(Object.prototype);
    CryptoKey.prototype.constructor = CryptoKey;
    Object.defineProperty(CryptoKey.prototype, Symbol.toStringTag, {value: 'CryptoKey', configurable: true});
    globalThis.CryptoKey = CryptoKey;

    function mkKey(type, algorithm, extractable, usages, internals) {
        var key = Object.create(CryptoKey.prototype);
        var frozenAlgo = Object.freeze(Object.assign({}, algorithm));
        if (frozenAlgo.hash && typeof frozenAlgo.hash === 'object') Object.freeze(frozenAlgo.hash);
        var frozenUsages = Object.freeze(Array.prototype.slice.call(usages || []));
        Object.defineProperties(key, {
            type: {value: type, enumerable: true},
            algorithm: {value: frozenAlgo, enumerable: true},
            extractable: {value: !!extractable, enumerable: true},
            usages: {value: frozenUsages, enumerable: true},
            _raw: {value: internals && internals.raw || null},
            _privateKeyBytes: {value: internals && internals.privateKeyBytes || null},
            _publicKeyBytes: {value: internals && internals.publicKeyBytes || null}
        });
        return key;
    }

    // ---- Helpers ----
    function toBytes(data) {
        if (data instanceof ArrayBuffer) return new Uint8Array(data);
        if (data instanceof Uint8Array) return data;
        if (ArrayBuffer.isView(data)) return new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
        return new Uint8Array(data);
    }
    var algoNameMap = {
        'aes-gcm':'AES-GCM','aes-cbc':'AES-CBC','aes-ctr':'AES-CTR',
        'hmac':'HMAC','pbkdf2':'PBKDF2','hkdf':'HKDF',
        'x25519':'X25519','ed25519':'Ed25519',
        'ecdh':'ECDH','ecdsa':'ECDSA',
        'argon2d':'Argon2d','argon2i':'Argon2i','argon2id':'Argon2id'
    };
    function normalizeAlgo(a) {
        var o = typeof a === 'string' ? {name:a} : Object.assign({}, a);
        var lower = o.name.toLowerCase();
        if (algoNameMap[lower]) o.name = algoNameMap[lower];
        return o;
    }
    function hashName(h) { var n = typeof h === 'string' ? h : (h && h.name) || h; return String(n).toUpperCase(); }
    function normName(name) { return name.toLowerCase(); }

    // ASN.1 helpers for PKCS8/SPKI (minimal, for known OIDs)
    // X25519 OID: 1.3.101.110 -> [43, 101, 110]
    // Ed25519 OID: 1.3.101.112 -> [43, 101, 112]
    // P-256 OID: 1.2.840.10045.3.1.7
    // P-384 OID: 1.3.132.0.34

    function extractX25519PrivateFromPkcs8(der) {
        // PKCS8 for X25519: 30 2e 02 01 00 30 05 06 03 2b6570 04 22 04 20 <32 bytes>
        // The 32-byte key is at offset 16
        var bytes = toBytes(der);
        return Array.from(bytes.slice(16, 48));
    }
    function extractX25519PublicFromSpki(der) {
        // SPKI for X25519: 30 2a 30 05 06 03 2b656e 03 21 00 <32 bytes>
        // The 32-byte key is at offset 12
        var bytes = toBytes(der);
        return Array.from(bytes.slice(12, 44));
    }
    function wrapX25519PrivateAsPkcs8(privBytes) {
        // SEQUENCE { INTEGER 0, SEQUENCE { OID 1.3.101.110 }, OCTET_STRING { OCTET_STRING { key } } }
        return [48, 46, 2, 1, 0, 48, 5, 6, 3, 43, 101, 110, 4, 34, 4, 32].concat(Array.from(privBytes));
    }
    function wrapX25519PublicAsSpki(pubBytes) {
        // SEQUENCE { SEQUENCE { OID 1.3.101.110 }, BIT_STRING { 0x00, key } }
        return [48, 42, 48, 5, 6, 3, 43, 101, 110, 3, 33, 0].concat(Array.from(pubBytes));
    }

    function extractEd25519PrivateFromPkcs8(der) {
        var bytes = toBytes(der);
        return Array.from(bytes.slice(16, 48));
    }
    function extractEd25519PublicFromSpki(der) {
        var bytes = toBytes(der);
        return Array.from(bytes.slice(12, 44));
    }

    function isCfrgAlgo(name) {
        return name === 'X25519' || name === 'Ed25519';
    }
    function isEcAlgo(name) {
        return name === 'ECDH' || name === 'ECDSA';
    }

    // ---- subtle ----
    var subtle = {
        digest: function(algo, data) {
            var h = hashName(algo), d = Array.from(toBytes(data));
            var result = __braille_crypto_digest(h, d);
            return Promise.resolve(new Uint8Array(result).buffer);
        },

        generateKey: function(algo, extractable, usages) {
            var a = normalizeAlgo(algo);
            var name = a.name;

            if (name === 'AES-GCM' || name === 'AES-CBC' || name === 'AES-CTR') {
                var len = (a.length || 256) / 8;
                var raw = __braille_crypto_get_random_bytes(len);
                return Promise.resolve(mkKey('secret', {name:name,length:a.length||256}, extractable, usages, {raw:raw}));
            }
            if (name === 'HMAC') {
                var hLen = {SHA1:20,'SHA-1':20,'SHA-256':32,'SHA-384':48,'SHA-512':64}[hashName(a.hash)] || 32;
                var raw = __braille_crypto_get_random_bytes(a.length ? a.length/8 : hLen);
                return Promise.resolve(mkKey('secret', {name:'HMAC',hash:{name:hashName(a.hash)},length:raw.length*8}, extractable, usages, {raw:raw}));
            }
            if (name === 'X25519') {
                var pair = __braille_crypto_x25519_generate();
                var pubKey = mkKey('public', {name:'X25519'}, true, [], {publicKeyBytes: pair[0]});
                var privKey = mkKey('private', {name:'X25519'}, extractable, usages, {privateKeyBytes: pair[1], publicKeyBytes: pair[0]});
                return Promise.resolve({publicKey: pubKey, privateKey: privKey});
            }
            if (name === 'Ed25519') {
                var pair = __braille_crypto_ed25519_generate();
                var pubKey = mkKey('public', {name:'Ed25519'}, true, ['verify'], {publicKeyBytes: pair[0]});
                var privKey = mkKey('private', {name:'Ed25519'}, extractable, ['sign'], {privateKeyBytes: pair[1], publicKeyBytes: pair[0]});
                return Promise.resolve({publicKey: pubKey, privateKey: privKey});
            }
            if (name === 'ECDH') {
                var curve = a.namedCurve;
                var pair = __braille_crypto_ecdh_generate(curve);
                var algoObj = {name: 'ECDH', namedCurve: curve};
                var pubKey = mkKey('public', algoObj, true, [], {publicKeyBytes: pair[0]});
                var privKey = mkKey('private', algoObj, extractable, usages, {privateKeyBytes: pair[1], publicKeyBytes: pair[0]});
                return Promise.resolve({publicKey: pubKey, privateKey: privKey});
            }
            if (name === 'ECDSA') {
                var curve = a.namedCurve;
                var pair = __braille_crypto_ecdh_generate(curve);
                var algoObj = {name: 'ECDSA', namedCurve: curve};
                var pubKey = mkKey('public', algoObj, true, ['verify'], {publicKeyBytes: pair[0]});
                var privKey = mkKey('private', algoObj, extractable, ['sign'], {privateKeyBytes: pair[1], publicKeyBytes: pair[0]});
                return Promise.resolve({publicKey: pubKey, privateKey: privKey});
            }
            return Promise.reject(new DOMException('generateKey for ' + name + ' not supported', 'NotSupportedError'));
        },

        importKey: function(format, keyData, algo, extractable, usages) {
            var a = normalizeAlgo(algo);
            var name = a.name;

            // Symmetric / KDF raw import
            if (format === 'raw' || format === 'raw-secret') {
                var raw = Array.from(toBytes(keyData));
                var algoObj = Object.assign({}, a);
                if (name === 'HMAC' && a.hash) algoObj = {name:'HMAC',hash:{name:hashName(a.hash)},length:raw.length*8};
                if (name === 'PBKDF2') algoObj = {name:'PBKDF2'};
                if (name === 'HKDF') algoObj = {name:'HKDF'};
                if (name === 'Argon2d' || name === 'Argon2i' || name === 'Argon2id') algoObj = {name: name};
                if (name === 'X25519') {
                    return Promise.resolve(mkKey('public', {name:'X25519'}, extractable, usages, {publicKeyBytes: raw}));
                }
                if (name === 'Ed25519') {
                    return Promise.resolve(mkKey('public', {name:'Ed25519'}, extractable, usages, {publicKeyBytes: raw}));
                }
                if (name === 'ECDH' || name === 'ECDSA') {
                    return Promise.resolve(mkKey('public', {name: name, namedCurve: a.namedCurve}, extractable, usages, {publicKeyBytes: raw}));
                }
                return Promise.resolve(mkKey('secret', algoObj, extractable, usages, {raw:raw}));
            }

            // PKCS8 import (private keys)
            if (format === 'pkcs8') {
                var derBytes = toBytes(keyData);
                if (name === 'X25519') {
                    var priv = extractX25519PrivateFromPkcs8(derBytes);
                    // Derive public key from private using native function
                    var pair = __braille_crypto_x25519_generate();
                    // We can't derive the public from the private easily in JS, but we can store just the private
                    // Actually, we need to compute the public key. We'll just do a dummy DH to get it...
                    // Instead, let's pass through native: generate ephemeral, but import with known private
                    // The simplest approach: store private bytes and compute public when needed
                    return Promise.resolve(mkKey('private', {name:'X25519'}, extractable, usages, {privateKeyBytes: priv}));
                }
                if (name === 'Ed25519') {
                    var priv = extractEd25519PrivateFromPkcs8(derBytes);
                    return Promise.resolve(mkKey('private', {name:'Ed25519'}, extractable, usages, {privateKeyBytes: priv}));
                }
                return Promise.reject(new DOMException('importKey pkcs8 for ' + name + ' not supported', 'NotSupportedError'));
            }

            // SPKI import (public keys)
            if (format === 'spki') {
                var derBytes = toBytes(keyData);
                if (name === 'X25519') {
                    var pub_bytes = extractX25519PublicFromSpki(derBytes);
                    return Promise.resolve(mkKey('public', {name:'X25519'}, extractable, usages, {publicKeyBytes: pub_bytes}));
                }
                if (name === 'Ed25519') {
                    var pub_bytes = extractEd25519PublicFromSpki(derBytes);
                    return Promise.resolve(mkKey('public', {name:'Ed25519'}, extractable, usages, {publicKeyBytes: pub_bytes}));
                }
                if (name === 'ECDH' || name === 'ECDSA') {
                    // For EC SPKI, extract the uncompressed point
                    // EC SPKI: SEQUENCE { SEQUENCE { OID ecPublicKey, OID namedCurve }, BIT_STRING { 0x00, 0x04, x, y } }
                    // Parse ASN.1 to find the BIT STRING content
                    var bytes = Array.from(derBytes);
                    // Find the BIT STRING (tag 0x03) - it's the last element in the outer SEQUENCE
                    var idx = 0;
                    // Skip outer SEQUENCE tag+len
                    idx = 2; // 30 xx
                    if (bytes[1] > 127) idx = 2 + (bytes[1] & 0x7f);
                    // Skip inner SEQUENCE (algorithm identifier)
                    var innerLen = bytes[idx + 1];
                    if (bytes[idx + 1] > 127) {
                        var numLenBytes = bytes[idx + 1] & 0x7f;
                        innerLen = 0;
                        for (var li = 0; li < numLenBytes; li++) innerLen = (innerLen << 8) | bytes[idx + 2 + li];
                        idx += 2 + numLenBytes + innerLen;
                    } else {
                        idx += 2 + innerLen;
                    }
                    // Now idx points to BIT STRING tag (0x03)
                    var bsLen = bytes[idx + 1];
                    var bsStart = idx + 2;
                    if (bytes[idx + 1] > 127) {
                        var numLenBytes2 = bytes[idx + 1] & 0x7f;
                        bsLen = 0;
                        for (var li2 = 0; li2 < numLenBytes2; li2++) bsLen = (bsLen << 8) | bytes[idx + 2 + li2];
                        bsStart = idx + 2 + numLenBytes2;
                    }
                    // Skip the 0x00 padding byte of BIT STRING
                    var pubBytes = bytes.slice(bsStart + 1, bsStart + bsLen);
                    var curve = a.namedCurve;
                    if (!curve) {
                        // Try to detect from OID in the SPKI
                        curve = 'P-256';
                    }
                    return Promise.resolve(mkKey('public', {name: name, namedCurve: curve}, extractable, usages, {publicKeyBytes: pubBytes}));
                }
                return Promise.reject(new DOMException('importKey spki for ' + name + ' not supported', 'NotSupportedError'));
            }

            // JWK import
            if (format === 'jwk') {
                var jwk = typeof keyData === 'string' ? JSON.parse(keyData) : keyData;
                if (jwk.k) {
                    var b64 = jwk.k.replace(/-/g,'+').replace(/_/g,'/');
                    while (b64.length % 4) b64 += '=';
                    var raw = Array.from((function(s){
                        var bin = atob(s), arr = new Uint8Array(bin.length);
                        for(var i=0;i<bin.length;i++) arr[i]=bin.charCodeAt(i);
                        return arr;
                    })(b64));
                    return Promise.resolve(mkKey('secret', a, extractable, usages, {raw:raw}));
                }
            }

            return Promise.reject(new DOMException('importKey format ' + format + ' for ' + name + ' not supported', 'NotSupportedError'));
        },

        exportKey: function(format, key) {
            if (format === 'raw') {
                if (key._raw) return Promise.resolve(new Uint8Array(key._raw).buffer);
                if (key._publicKeyBytes) return Promise.resolve(new Uint8Array(key._publicKeyBytes).buffer);
            }
            if (format === 'spki' && key.type === 'public') {
                if (key.algorithm.name === 'X25519' && key._publicKeyBytes) {
                    var spki = wrapX25519PublicAsSpki(key._publicKeyBytes);
                    return Promise.resolve(new Uint8Array(spki).buffer);
                }
            }
            if (format === 'pkcs8' && key.type === 'private') {
                if (key.algorithm.name === 'X25519' && key._privateKeyBytes) {
                    var pkcs8 = wrapX25519PrivateAsPkcs8(key._privateKeyBytes);
                    return Promise.resolve(new Uint8Array(pkcs8).buffer);
                }
            }
            if (format === 'jwk' && key._raw) {
                var b64url = (function(bytes){
                    var bin=''; for(var i=0;i<bytes.length;i++) bin+=String.fromCharCode(bytes[i]);
                    return btoa(bin).replace(/\+/g,'-').replace(/\//g,'_').replace(/=+$/,'');
                })(key._raw);
                return Promise.resolve({kty:'oct',k:b64url,alg:key.algorithm.name==='HMAC'?'HS256':'A256GCM',ext:key.extractable});
            }
            return Promise.reject(new DOMException('exportKey format ' + format + ' not supported', 'NotSupportedError'));
        },

        encrypt: function(algo, key, data) {
            var a = normalizeAlgo(algo);
            if (a.name === 'AES-GCM') {
                var iv = Array.from(toBytes(a.iv));
                var aad = a.additionalData ? Array.from(toBytes(a.additionalData)) : [];
                var pt = Array.from(toBytes(data));
                var result = __braille_crypto_aes_gcm_encrypt(key._raw, iv, pt, aad);
                return Promise.resolve(new Uint8Array(result).buffer);
            }
            if (a.name === 'AES-CBC') {
                var iv = Array.from(toBytes(a.iv));
                var pt = Array.from(toBytes(data));
                var result = __braille_crypto_aes_cbc_encrypt(key._raw, iv, pt);
                return Promise.resolve(new Uint8Array(result).buffer);
            }
            if (a.name === 'AES-CTR') {
                var counter = Array.from(toBytes(a.counter));
                var pt = Array.from(toBytes(data));
                var result = __braille_crypto_aes_ctr_encrypt(key._raw, counter, pt);
                return Promise.resolve(new Uint8Array(result).buffer);
            }
            return Promise.reject(new DOMException('encrypt ' + a.name + ' not supported', 'NotSupportedError'));
        },

        decrypt: function(algo, key, data) {
            var a = normalizeAlgo(algo);
            if (a.name === 'AES-GCM') {
                var iv = Array.from(toBytes(a.iv));
                var aad = a.additionalData ? Array.from(toBytes(a.additionalData)) : [];
                var ct = Array.from(toBytes(data));
                var result = __braille_crypto_aes_gcm_decrypt(key._raw, iv, ct, aad);
                return Promise.resolve(new Uint8Array(result).buffer);
            }
            if (a.name === 'AES-CBC') {
                var iv = Array.from(toBytes(a.iv));
                var ct = Array.from(toBytes(data));
                var result = __braille_crypto_aes_cbc_decrypt(key._raw, iv, ct);
                return Promise.resolve(new Uint8Array(result).buffer);
            }
            if (a.name === 'AES-CTR') {
                var counter = Array.from(toBytes(a.counter));
                var ct = Array.from(toBytes(data));
                var result = __braille_crypto_aes_ctr_decrypt(key._raw, counter, ct);
                return Promise.resolve(new Uint8Array(result).buffer);
            }
            return Promise.reject(new DOMException('decrypt ' + a.name + ' not supported', 'NotSupportedError'));
        },

        sign: function(algo, key, data) {
            var a = normalizeAlgo(algo);
            if (a.name === 'HMAC') {
                var h = hashName(key.algorithm && key.algorithm.hash);
                var result = __braille_crypto_hmac_sign(h, key._raw, Array.from(toBytes(data)));
                return Promise.resolve(new Uint8Array(result).buffer);
            }
            if (a.name === 'Ed25519') {
                var result = __braille_crypto_ed25519_sign(key._privateKeyBytes, Array.from(toBytes(data)));
                return Promise.resolve(new Uint8Array(result).buffer);
            }
            if (a.name === 'ECDSA') {
                var curve = key.algorithm.namedCurve;
                var h = hashName(a.hash);
                var result = __braille_crypto_ecdsa_sign(curve, h, key._privateKeyBytes, Array.from(toBytes(data)));
                return Promise.resolve(new Uint8Array(result).buffer);
            }
            return Promise.reject(new DOMException('sign ' + a.name + ' not supported', 'NotSupportedError'));
        },

        verify: function(algo, key, signature, data) {
            var a = normalizeAlgo(algo);
            if (a.name === 'HMAC') {
                var h = hashName(key.algorithm && key.algorithm.hash);
                var ok = __braille_crypto_hmac_verify(h, key._raw, Array.from(toBytes(signature)), Array.from(toBytes(data)));
                return Promise.resolve(ok);
            }
            if (a.name === 'Ed25519') {
                var ok = __braille_crypto_ed25519_verify(key._publicKeyBytes, Array.from(toBytes(signature)), Array.from(toBytes(data)));
                return Promise.resolve(ok);
            }
            if (a.name === 'ECDSA') {
                var curve = key.algorithm.namedCurve;
                var h = hashName(a.hash);
                var ok = __braille_crypto_ecdsa_verify(curve, h, key._publicKeyBytes, Array.from(toBytes(signature)), Array.from(toBytes(data)));
                return Promise.resolve(ok);
            }
            return Promise.reject(new DOMException('verify ' + a.name + ' not supported', 'NotSupportedError'));
        },

        deriveBits: function(algo, baseKey, length) {
            var a = normalizeAlgo(algo);
            if (a.name === 'PBKDF2') {
                var h = hashName(a.hash);
                var salt = Array.from(toBytes(a.salt));
                var result = __braille_crypto_pbkdf2(h, baseKey._raw, salt, a.iterations, length/8);
                return Promise.resolve(new Uint8Array(result).buffer);
            }
            if (a.name === 'HKDF') {
                var h = hashName(a.hash);
                var salt = a.salt ? Array.from(toBytes(a.salt)) : [];
                var info = a.info ? Array.from(toBytes(a.info)) : [];
                var result = __braille_crypto_hkdf(h, baseKey._raw, salt, info, length/8);
                return Promise.resolve(new Uint8Array(result).buffer);
            }
            if (a.name === 'Argon2d' || a.name === 'Argon2i' || a.name === 'Argon2id') {
                var variant = a.name === 'Argon2d' ? 'd' : (a.name === 'Argon2i' ? 'i' : 'id');
                var salt = a.nonce ? Array.from(toBytes(a.nonce)) : [];
                var secret = a.secretValue ? Array.from(toBytes(a.secretValue)) : [];
                var ad = a.associatedData ? Array.from(toBytes(a.associatedData)) : [];
                var memory = a.memory || 32;
                var passes = a.passes || 3;
                var parallelism = a.parallelism || 1;
                var result = __braille_crypto_argon2(variant, baseKey._raw, salt, [memory, passes, parallelism, length/8], secret, ad);
                return Promise.resolve(new Uint8Array(result).buffer);
            }
            if (a.name === 'X25519') {
                if (!a.public) {
                    return Promise.reject(new TypeError('X25519 deriveBits requires public property'));
                }
                if (!(a.public instanceof CryptoKey)) {
                    return Promise.reject(new TypeError('public property must be a CryptoKey'));
                }
                if (baseKey.type !== 'private') {
                    return Promise.reject(new DOMException('baseKey must be a private key', 'InvalidAccessError'));
                }
                if (a.public.type === 'private') {
                    return Promise.reject(new DOMException('public property must not be a private key', 'InvalidAccessError'));
                }
                if (a.public.type === 'secret') {
                    return Promise.reject(new DOMException('public property must be a public key', 'InvalidAccessError'));
                }
                if (a.public.algorithm.name !== 'X25519') {
                    return Promise.reject(new DOMException('Algorithm mismatch', 'InvalidAccessError'));
                }
                if (baseKey.usages.indexOf('deriveBits') === -1) {
                    return Promise.reject(new DOMException('baseKey usages do not include deriveBits', 'InvalidAccessError'));
                }
                var requestedBytes = length / 8;
                if (requestedBytes > 32) {
                    return Promise.reject(new DOMException('Requested too many bits', 'OperationError'));
                }
                var shared = __braille_crypto_x25519_derive_bits(baseKey._privateKeyBytes, a.public._publicKeyBytes);
                if (shared.length === 0) {
                    return Promise.reject(new DOMException('X25519 produced all-zero shared secret', 'OperationError'));
                }
                if (requestedBytes < 32) {
                    shared = shared.slice(0, requestedBytes);
                }
                return Promise.resolve(new Uint8Array(shared).buffer);
            }
            if (a.name === 'ECDH') {
                if (!a.public || !(a.public instanceof CryptoKey)) {
                    return Promise.reject(new TypeError('ECDH deriveBits requires public CryptoKey'));
                }
                if (baseKey.type !== 'private') {
                    return Promise.reject(new DOMException('baseKey must be a private key', 'InvalidAccessError'));
                }
                if (a.public.type !== 'public') {
                    return Promise.reject(new DOMException('public property must be a public key', 'InvalidAccessError'));
                }
                var curve = baseKey.algorithm.namedCurve;
                var shared = __braille_crypto_ecdh_derive(curve, baseKey._privateKeyBytes, a.public._publicKeyBytes);
                var requestedBytes = length / 8;
                if (requestedBytes > shared.length) {
                    return Promise.reject(new DOMException('Requested too many bits', 'OperationError'));
                }
                if (requestedBytes < shared.length) {
                    shared = shared.slice(0, requestedBytes);
                }
                return Promise.resolve(new Uint8Array(shared).buffer);
            }
            return Promise.reject(new DOMException('deriveBits ' + a.name + ' not supported', 'NotSupportedError'));
        },

        deriveKey: function(algo, baseKey, derivedKeyAlgo, extractable, usages) {
            var a = normalizeAlgo(algo);
            var dka = normalizeAlgo(derivedKeyAlgo);
            var bitLen = dka.length || 256;
            return subtle.deriveBits(a, baseKey, bitLen).then(function(bits) {
                return subtle.importKey('raw', bits, dka, extractable, usages);
            });
        },

        wrapKey: function(format, key, wrappingKey, wrapAlgorithm) {
            return subtle.exportKey(format, key).then(function(keyData) {
                if (format === 'jwk') {
                    var jsonStr = JSON.stringify(keyData);
                    var enc = new TextEncoder();
                    keyData = enc.encode(jsonStr).buffer;
                }
                return subtle.encrypt(wrapAlgorithm, wrappingKey, keyData);
            });
        },

        unwrapKey: function(format, wrappedKey, unwrappingKey, unwrapAlgorithm, unwrappedKeyAlgorithm, extractable, keyUsages) {
            return subtle.decrypt(unwrapAlgorithm, unwrappingKey, wrappedKey).then(function(keyData) {
                if (format === 'jwk') {
                    var dec = new TextDecoder();
                    keyData = JSON.parse(dec.decode(keyData));
                }
                return subtle.importKey(format, keyData, unwrappedKeyAlgorithm, extractable, keyUsages);
            });
        }
    };

    globalThis.crypto = {
        subtle: subtle,
        getRandomValues: function(arr) {
            var bytes = __braille_crypto_get_random_bytes(arr.length);
            for (var i = 0; i < arr.length; i++) arr[i] = bytes[i];
            return arr;
        },
        randomUUID: function() {
            var b = __braille_crypto_get_random_bytes(16);
            b[6] = (b[6] & 0x0f) | 0x40;
            b[8] = (b[8] & 0x3f) | 0x80;
            var h = ''; for (var i=0;i<16;i++) h += (b[i]<16?'0':'') + b[i].toString(16);
            return h.slice(0,8)+'-'+h.slice(8,12)+'-'+h.slice(12,16)+'-'+h.slice(16,20)+'-'+h.slice(20);
        }
    };
})();
"#
}
