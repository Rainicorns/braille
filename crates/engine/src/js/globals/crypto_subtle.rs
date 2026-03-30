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
            _publicKeyBytes: {value: internals && internals.publicKeyBytes || null},
            _jwkAlg: {value: internals && internals.jwkAlg || null}
        });
        return key;
    }

    // ---- Helpers ----
    function toBytes(data) {
        if (data instanceof ArrayBuffer) {
            if (data.byteLength === 0) return new Uint8Array(0);
            return new Uint8Array(data);
        }
        if (ArrayBuffer.isView(data)) {
            if (data.byteLength === 0) return new Uint8Array(0);
            return new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
        }
        return new Uint8Array(data);
    }
    function toBytesSnapshot(data) {
        // Snapshot data into a plain array, tolerant of detached buffers.
        // Per spec, digest() reads algo first (triggering getters), then reads data.
        // If the buffer was detached during algo reading, we must see 0 bytes.
        // Note: Array.from(typedArray) throws on detached buffers in QuickJS,
        // but manual iteration via .length/.byteLength works fine (returns 0).
        if (ArrayBuffer.isView(data)) {
            var len = data.length;
            if (len === 0) return [];
            var arr = [];
            for (var i = 0; i < len; i++) arr.push(data[i]);
            return arr;
        }
        if (data instanceof ArrayBuffer) {
            if (data.byteLength === 0) return [];
            var view = new Uint8Array(data);
            var arr = [];
            for (var i = 0; i < view.length; i++) arr.push(view[i]);
            return arr;
        }
        return Array.from(toBytes(data));
    }
    var algoNameMap = {
        'aes-gcm':'AES-GCM','aes-cbc':'AES-CBC','aes-ctr':'AES-CTR',
        'hmac':'HMAC','pbkdf2':'PBKDF2','hkdf':'HKDF',
        'x25519':'X25519','x448':'X448','ed25519':'Ed25519',
        'ecdh':'ECDH','ecdsa':'ECDSA',
        'ed448':'Ed448',
        'argon2d':'Argon2d','argon2i':'Argon2i','argon2id':'Argon2id',
        'ml-kem-512':'ML-KEM-512','ml-kem-768':'ML-KEM-768','ml-kem-1024':'ML-KEM-1024',
        'aes-ocb':'AES-OCB',
        'chacha20-poly1305':'ChaCha20-Poly1305',
        'rsa-oaep':'RSA-OAEP','rsa-pss':'RSA-PSS','rsassa-pkcs1-v1_5':'RSASSA-PKCS1-v1_5',
        'aes-kw':'AES-KW',
        'kmac128':'KMAC128','kmac256':'KMAC256'
    };
    function asciiLower(s) {
        var out = '';
        for (var i = 0; i < s.length; i++) {
            var c = s.charCodeAt(i);
            out += (c >= 65 && c <= 90) ? String.fromCharCode(c + 32) : s[i];
        }
        return out;
    }
    function normalizeAlgo(a) {
        var o = typeof a === 'string' ? {name:a} : Object.assign({}, a);
        var lower = asciiLower(o.name);
        if (algoNameMap[lower]) o.name = algoNameMap[lower];
        return o;
    }
    function hashName(h) {
        var n = typeof h === 'string' ? h : (h && h.name) || h;
        var s = String(n);
        // Preserve casing for known algorithms that need it
        var upper = s.toUpperCase();
        // Map to canonical names used by Rust side
        var map = {'SHA-1':'SHA-1','SHA-256':'SHA-256','SHA-384':'SHA-384','SHA-512':'SHA-512',
            'SHA3-256':'SHA3-256','SHA3-384':'SHA3-384','SHA3-512':'SHA3-512',
            'CSHAKE128':'CSHAKE128','CSHAKE256':'CSHAKE256'};
        return map[upper] || upper;
    }
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

    function wrapEd25519PrivateAsPkcs8(privBytes) {
        return [48, 46, 2, 1, 0, 48, 5, 6, 3, 43, 101, 112, 4, 34, 4, 32].concat(Array.from(privBytes));
    }
    function wrapEd25519PublicAsSpki(pubBytes) {
        return [48, 42, 48, 5, 6, 3, 43, 101, 112, 3, 33, 0].concat(Array.from(pubBytes));
    }
    function b64url(bytes) {
        var bin=''; for(var i=0;i<bytes.length;i++) bin+=String.fromCharCode(typeof bytes[i]==='number'?bytes[i]:bytes[i]);
        return btoa(bin).replace(/\+/g,'-').replace(/\//g,'_').replace(/=+$/,'');
    }
    function isCfrgAlgo(name) {
        return name === 'X25519' || name === 'X448' || name === 'Ed25519' || name === 'Ed448';
    }
    function isEcAlgo(name) {
        return name === 'ECDH' || name === 'ECDSA';
    }
    function isRsaAlgo(name) {
        return name === 'RSA-OAEP' || name === 'RSA-PSS' || name === 'RSASSA-PKCS1-v1_5';
    }

    // ---- subtle ----
    var subtle = {
        digest: function(algo, data) {
            var a = typeof algo === 'string' ? {name: algo} : algo;
            var algoName = a.name;
            if (!algoName && typeof a !== 'string') {
                return Promise.reject(new TypeError('Missing algorithm name'));
            }
            var h = hashName(algoName || a);
            var validDigests = {'SHA-1':1,'SHA-256':1,'SHA-384':1,'SHA-512':1,'SHA3-256':1,'SHA3-384':1,'SHA3-512':1,'CSHAKE128':1,'CSHAKE256':1};
            if (!validDigests[h]) {
                return Promise.reject(new DOMException('digest algorithm ' + h + ' not supported', 'NotSupportedError'));
            }
            var d = toBytesSnapshot(data);
            var outputLen = a.length || 0;
            var result = __braille_crypto_digest(h, d, outputLen);
            return Promise.resolve(new Uint8Array(result).buffer);
        },

        generateKey: function(algo, extractable, usages) {
            // Step 1: Normalize algorithm — TypeError if no name
            if (typeof algo === 'object' && !algo.name) {
                return Promise.reject(new TypeError('Algorithm: name member is required'));
            }
            var a = normalizeAlgo(algo);
            var name = a.name;

            // Step 1b: Check algorithm is recognized for generateKey
            var genKeyAlgos = {
                'AES-GCM':1,'AES-CBC':1,'AES-CTR':1,'AES-KW':1,'AES-OCB':1,
                'HMAC':1,'ChaCha20-Poly1305':1,
                'RSA-OAEP':1,'RSA-PSS':1,'RSASSA-PKCS1-v1_5':1,
                'ECDH':1,'ECDSA':1,'Ed25519':1,'Ed448':1,'X25519':1,'X448':1,
                'ML-KEM-512':1,'ML-KEM-768':1,'ML-KEM-1024':1,
                'ML-DSA-44':1,'ML-DSA-65':1,'ML-DSA-87':1,
                'KMAC128':1,'KMAC256':1
            };
            if (!genKeyAlgos[name]) {
                return Promise.reject(new DOMException('Unrecognized algorithm name: ' + name, 'NotSupportedError'));
            }

            // Step 1c: For algorithms with hash, validate the hash is recognized
            if (isRsaAlgo(name) || name === 'HMAC') {
                if (a.hash) {
                    var hn = hashName(a.hash);
                    var validHashes = {'SHA-1':1,'SHA-256':1,'SHA-384':1,'SHA-512':1};
                    if (!validHashes[hn]) {
                        return Promise.reject(new DOMException('Unrecognized hash: ' + hn, 'NotSupportedError'));
                    }
                }
            }

            // Step 2: Validate usages — SyntaxError if any usage not valid for this algorithm
            var validUsagesMap = {
                'AES-GCM':['encrypt','decrypt','wrapKey','unwrapKey'],
                'AES-CBC':['encrypt','decrypt','wrapKey','unwrapKey'],
                'AES-CTR':['encrypt','decrypt','wrapKey','unwrapKey'],
                'AES-KW':['wrapKey','unwrapKey'],
                'AES-OCB':['encrypt','decrypt','wrapKey','unwrapKey'],
                'HMAC':['sign','verify'],
                'ChaCha20-Poly1305':['encrypt','decrypt','wrapKey','unwrapKey'],
                'RSA-OAEP':['encrypt','decrypt','wrapKey','unwrapKey'],
                'RSA-PSS':['sign','verify'],
                'RSASSA-PKCS1-v1_5':['sign','verify'],
                'ECDH':['deriveKey','deriveBits'],
                'ECDSA':['sign','verify'],
                'Ed25519':['sign','verify'],
                'Ed448':['sign','verify'],
                'X25519':['deriveKey','deriveBits'],
                'X448':['deriveKey','deriveBits'],
                'ML-KEM-512':['decapsulateBits','decapsulateKey','encapsulateBits','encapsulateKey'],
                'ML-KEM-768':['decapsulateBits','decapsulateKey','encapsulateBits','encapsulateKey'],
                'ML-KEM-1024':['decapsulateBits','decapsulateKey','encapsulateBits','encapsulateKey'],
                'ML-DSA-44':['sign','verify'],
                'ML-DSA-65':['sign','verify'],
                'ML-DSA-87':['sign','verify'],
                'KMAC128':['sign','verify'],
                'KMAC256':['sign','verify']
            };
            var validU = validUsagesMap[name] || [];
            if (usages && usages.length > 0) {
                for (var ui = 0; ui < usages.length; ui++) {
                    if (validU.indexOf(usages[ui]) === -1) {
                        return Promise.reject(new DOMException('Invalid key usage: ' + usages[ui], 'SyntaxError'));
                    }
                }
            }

            // Step 3: Validate algorithm-specific properties
            if (name.substring(0,3) === 'AES') {
                var aesLen = a.length;
                if (aesLen !== undefined && aesLen !== 128 && aesLen !== 192 && aesLen !== 256) {
                    return Promise.reject(new DOMException('AES key length must be 128, 192, or 256', 'OperationError'));
                }
            }
            if (isRsaAlgo(name)) {
                if (a.publicExponent) {
                    var pe = Array.from(a.publicExponent);
                    // Valid public exponents: 3 ([3]) or 65537 ([1,0,1])
                    var peValid = (pe.length === 1 && pe[0] === 3) || (pe.length === 3 && pe[0] === 1 && pe[1] === 0 && pe[2] === 1);
                    if (!peValid) {
                        return Promise.reject(new DOMException('Invalid RSA public exponent', 'OperationError'));
                    }
                }
            }
            if (name === 'ECDH' || name === 'ECDSA') {
                var validCurves = {'P-256':1,'P-384':1,'P-521':1};
                if (a.namedCurve && !validCurves[a.namedCurve]) {
                    return Promise.reject(new DOMException('Unsupported named curve: ' + a.namedCurve, 'NotSupportedError'));
                }
            }

            // Step 4: Empty usages — SyntaxError
            if (!usages || usages.length === 0) {
                return Promise.reject(new DOMException('usages cannot be empty', 'SyntaxError'));
            }

            if (name === 'AES-GCM' || name === 'AES-CBC' || name === 'AES-CTR' || name === 'AES-KW' || name === 'AES-OCB') {
                var len = (a.length || 256) / 8;
                var raw = __braille_crypto_get_random_bytes(len);
                return Promise.resolve(mkKey('secret', {name:name,length:a.length||256}, extractable, usages, {raw:raw}));
            }
            if (name === 'HMAC') {
                var h = hashName(a.hash);
                var blockSize = {'SHA-1':512,'SHA-256':512,'SHA-384':1024,'SHA-512':1024}[h] || 512;
                var keyLenBits = a.length || blockSize;
                var raw = __braille_crypto_get_random_bytes(keyLenBits / 8);
                return Promise.resolve(mkKey('secret', {name:'HMAC',hash:{name:h},length:keyLenBits}, extractable, usages, {raw:raw}));
            }
            if (name === 'X25519') {
                var pair = __braille_crypto_x25519_generate();
                var pubKey = mkKey('public', {name:'X25519'}, true, [], {publicKeyBytes: pair[0]});
                var privKey = mkKey('private', {name:'X25519'}, extractable, usages, {privateKeyBytes: pair[1], publicKeyBytes: pair[0]});
                return Promise.resolve({publicKey: pubKey, privateKey: privKey});
            }
            if (name === 'X448') {
                var pair = __braille_crypto_x448_generate();
                var pubKey = mkKey('public', {name:'X448'}, true, [], {publicKeyBytes: pair[0]});
                var privKey = mkKey('private', {name:'X448'}, extractable, usages, {privateKeyBytes: pair[1], publicKeyBytes: pair[0]});
                return Promise.resolve({publicKey: pubKey, privateKey: privKey});
            }
            if (name === 'Ed25519') {
                var pair = __braille_crypto_ed25519_generate();
                var pubUsages = usages.filter(function(u){return u==='verify';});
                var privUsages = usages.filter(function(u){return u==='sign';});
                var pubKey = mkKey('public', {name:'Ed25519'}, true, pubUsages, {publicKeyBytes: pair[0]});
                var privKey = mkKey('private', {name:'Ed25519'}, extractable, privUsages, {privateKeyBytes: pair[1], publicKeyBytes: pair[0]});
                return Promise.resolve({publicKey: pubKey, privateKey: privKey});
            }
            if (name === 'Ed448') {
                var pair = __braille_crypto_ed448_generate();
                var pubUsages = usages.filter(function(u){return u==='verify';});
                var privUsages = usages.filter(function(u){return u==='sign';});
                var pubKey = mkKey('public', {name:'Ed448'}, true, pubUsages, {publicKeyBytes: pair[0]});
                var privKey = mkKey('private', {name:'Ed448'}, extractable, privUsages, {privateKeyBytes: pair[1], publicKeyBytes: pair[0]});
                return Promise.resolve({publicKey: pubKey, privateKey: privKey});
            }
            if (name === 'ECDH') {
                var curve = a.namedCurve;
                var pair = __braille_crypto_ecdh_generate(curve);
                var algoObj = {name: 'ECDH', namedCurve: curve};
                var privUsages = usages.filter(function(u){return u==='deriveKey'||u==='deriveBits';});
                var pubKey = mkKey('public', algoObj, true, [], {publicKeyBytes: pair[0]});
                var privKey = mkKey('private', algoObj, extractable, privUsages, {privateKeyBytes: pair[1], publicKeyBytes: pair[0]});
                return Promise.resolve({publicKey: pubKey, privateKey: privKey});
            }
            if (name === 'ECDSA') {
                var curve = a.namedCurve;
                var pair = __braille_crypto_ecdh_generate(curve);
                var algoObj = {name: 'ECDSA', namedCurve: curve};
                var pubUsages = usages.filter(function(u){return u==='verify';});
                var privUsages = usages.filter(function(u){return u==='sign';});
                var pubKey = mkKey('public', algoObj, true, pubUsages, {publicKeyBytes: pair[0]});
                var privKey = mkKey('private', algoObj, extractable, privUsages, {privateKeyBytes: pair[1], publicKeyBytes: pair[0]});
                return Promise.resolve({publicKey: pubKey, privateKey: privKey});
            }
            if (isRsaAlgo(name)) {
                var modLen = a.modulusLength;
                var pubExp = a.publicExponent ? Array.from(a.publicExponent) : [1,0,1];
                var h = hashName(a.hash);
                var pair = __braille_crypto_rsa_generate(modLen, pubExp);
                var algoObj = {name: name, modulusLength: modLen, publicExponent: new Uint8Array(pubExp), hash: {name: h}};
                var pubUsages = usages.filter(function(u){return u==='encrypt'||u==='wrapKey'||u==='verify';});
                var privUsages = usages.filter(function(u){return u==='decrypt'||u==='unwrapKey'||u==='sign';});
                var pubKey = mkKey('public', algoObj, true, pubUsages, {publicKeyBytes: pair[0]});
                var privKey = mkKey('private', algoObj, extractable, privUsages, {privateKeyBytes: pair[1], publicKeyBytes: pair[0]});
                return Promise.resolve({publicKey: pubKey, privateKey: privKey});
            }
            if (name === 'ChaCha20-Poly1305') {
                var raw = __braille_crypto_get_random_bytes(32);
                return Promise.resolve(mkKey('secret', {name:'ChaCha20-Poly1305'}, extractable, usages, {raw:raw}));
            }
            if (name === 'ML-KEM-512' || name === 'ML-KEM-768' || name === 'ML-KEM-1024') {
                var pair = __braille_crypto_mlkem_generate(name);
                var algoObj = {name: name};
                var pubKey = mkKey('public', algoObj, true, usages.filter(function(u){return u==='encapsulateBits'||u==='encapsulateKey';}), {publicKeyBytes: pair[0]});
                var privUsages = usages.filter(function(u){return u==='decapsulateBits'||u==='decapsulateKey';});
                var privKey = mkKey('private', algoObj, extractable, privUsages, {privateKeyBytes: pair[1], publicKeyBytes: pair[0]});
                return Promise.resolve({publicKey: pubKey, privateKey: privKey});
            }
            return Promise.reject(new DOMException('generateKey for ' + name + ' not supported', 'NotSupportedError'));
        },

        importKey: function(format, keyData, algo, extractable, usages) {
            var a = normalizeAlgo(algo);
            var name = a.name;

            // Validate usages for secret/symmetric key imports
            // Secret keys with empty usages should fail with SyntaxError
            // (except for KDF algorithms like PBKDF2, HKDF which use deriveBits/deriveKey)
            if ((format === 'raw' || format === 'raw-secret' || format === 'jwk')) {
                var needsUsages = {'AES-GCM':1,'AES-CBC':1,'AES-CTR':1,'AES-KW':1,'AES-OCB':1,'HMAC':1,'ChaCha20-Poly1305':1,'Argon2d':1,'Argon2i':1,'Argon2id':1,'KMAC128':1,'KMAC256':1};
                // HKDF/PBKDF2 with extractable=false and empty usages = SyntaxError
                if ((name === 'PBKDF2' || name === 'HKDF') && !extractable && usages && usages.length === 0) {
                    return Promise.reject(new DOMException('usages cannot be empty for non-extractable keys', 'SyntaxError'));
                }
                if (needsUsages[name] && usages && usages.length === 0) {
                    return Promise.reject(new DOMException('usages cannot be empty for secret keys', 'SyntaxError'));
                }
            }

            // Empty usages for private key imports = SyntaxError
            // PKCS8 always imports private keys; JWK with 'd' is private
            if (format === 'pkcs8' || format === 'raw-seed') {
                if (usages && usages.length === 0) {
                    var privNeedsUsages = {'ECDSA':1,'ECDH':1,'Ed25519':1,'Ed448':1,'X25519':1,'X448':1,'RSA-OAEP':1,'RSA-PSS':1,'RSASSA-PKCS1-v1_5':1,'ML-KEM-512':1,'ML-KEM-768':1,'ML-KEM-1024':1};
                    if (privNeedsUsages[name]) {
                        return Promise.reject(new DOMException('usages cannot be empty for private keys', 'SyntaxError'));
                    }
                }
            }
            if (format === 'jwk') {
                var jwkCheck = typeof keyData === 'string' ? JSON.parse(keyData) : keyData;
                if (jwkCheck.d && usages && usages.length === 0) {
                    var privNeedsUsages2 = {'ECDSA':1,'ECDH':1,'Ed25519':1,'Ed448':1,'X25519':1,'X448':1,'RSA-OAEP':1,'RSA-PSS':1,'RSASSA-PKCS1-v1_5':1};
                    if (privNeedsUsages2[name]) {
                        return Promise.reject(new DOMException('usages cannot be empty for private keys', 'SyntaxError'));
                    }
                }
            }

            // Validate usages against valid operations for the algorithm + key type
            // Public key formats: spki, raw (for EC/OKP), jwk without 'd'
            // Private key formats: pkcs8, raw-seed, jwk with 'd'
            if (usages && usages.length > 0) {
                var isPrivFormat = (format === 'pkcs8' || format === 'raw-seed');
                var isPubFormat = (format === 'spki' || format === 'raw' || format === 'raw-public');
                var isJwkPriv = false;
                if (format === 'jwk') {
                    var jc = typeof keyData === 'string' ? JSON.parse(keyData) : keyData;
                    isJwkPriv = !!jc.d;
                }
                // Valid usages per algorithm for public vs private
                var pubUsagesMap = {
                    'ECDH':[],'ECDSA':['verify'],'Ed25519':['verify'],'Ed448':['verify'],
                    'X25519':[],'X448':[],
                    'RSA-OAEP':['encrypt','wrapKey'],'RSA-PSS':['verify'],'RSASSA-PKCS1-v1_5':['verify']
                };
                var privUsagesMap = {
                    'ECDH':['deriveKey','deriveBits'],'ECDSA':['sign'],'Ed25519':['sign'],'Ed448':['sign'],
                    'X25519':['deriveKey','deriveBits'],'X448':['deriveKey','deriveBits'],
                    'RSA-OAEP':['decrypt','unwrapKey'],'RSA-PSS':['sign'],'RSASSA-PKCS1-v1_5':['sign'],
                    'ML-KEM-512':['decapsulateBits','decapsulateKey'],'ML-KEM-768':['decapsulateBits','decapsulateKey'],'ML-KEM-1024':['decapsulateBits','decapsulateKey']
                };
                var validUsages = null;
                if (isPubFormat && pubUsagesMap[name] !== undefined) validUsages = pubUsagesMap[name];
                else if (isPrivFormat && privUsagesMap[name] !== undefined) validUsages = privUsagesMap[name];
                else if (format === 'jwk' && isJwkPriv && privUsagesMap[name] !== undefined) validUsages = privUsagesMap[name];
                else if (format === 'jwk' && !isJwkPriv && pubUsagesMap[name] !== undefined) validUsages = pubUsagesMap[name];

                if (validUsages !== null) {
                    for (var ui = 0; ui < usages.length; ui++) {
                        if (validUsages.indexOf(usages[ui]) === -1) {
                            return Promise.reject(new DOMException('Cannot create a key using the specified key usages.', 'SyntaxError'));
                        }
                    }
                }
            }

            // ML-KEM raw-seed import (64-byte seed → dk + ek)
            if (format === 'raw-seed') {
                if (name === 'ML-KEM-512' || name === 'ML-KEM-768' || name === 'ML-KEM-1024') {
                    var seedBytes = Array.from(toBytes(keyData));
                    var pair = __braille_crypto_mlkem_from_seed(name, seedBytes);
                    var algoObj = {name: name};
                    return Promise.resolve(mkKey('private', algoObj, extractable, usages, {privateKeyBytes: seedBytes, publicKeyBytes: pair[0]}));
                }
                return Promise.reject(new DOMException('importKey format raw-seed for ' + name + ' not supported', 'NotSupportedError'));
            }

            // ML-KEM raw-public import (public key bytes → public CryptoKey)
            if (format === 'raw' || format === 'raw-public') {
                if (name === 'ML-KEM-512' || name === 'ML-KEM-768' || name === 'ML-KEM-1024') {
                    var pubBytes = Array.from(toBytes(keyData));
                    return Promise.resolve(mkKey('public', {name: name}, extractable, usages, {publicKeyBytes: pubBytes}));
                }
            }

            // Symmetric / KDF raw import
            if (format === 'raw' || format === 'raw-secret') {
                var raw = Array.from(toBytes(keyData));
                var algoObj = Object.assign({}, a);
                if (name.substring(0,3) === 'AES') algoObj = {name:name,length:raw.length*8};
                if (name === 'ChaCha20-Poly1305') algoObj = {name:name};
                if (name === 'HMAC' && a.hash) algoObj = {name:'HMAC',hash:{name:hashName(a.hash)},length:raw.length*8};
                if (name === 'KMAC128' || name === 'KMAC256') algoObj = {name:name,length:raw.length*8};
                if (name === 'PBKDF2') algoObj = {name:'PBKDF2'};
                if (name === 'HKDF') algoObj = {name:'HKDF'};
                if (name === 'Argon2d' || name === 'Argon2i' || name === 'Argon2id') algoObj = {name: name};
                if (name === 'X25519' || name === 'Ed25519' || name === 'X448' || name === 'Ed448') {
                    var okpSizes = {'X25519':32,'Ed25519':32,'X448':56,'Ed448':57};
                    if (raw.length !== okpSizes[name]) {
                        return Promise.reject(new DOMException(name + ' raw key must be ' + okpSizes[name] + ' bytes', 'DataError'));
                    }
                    return Promise.resolve(mkKey('public', {name:name}, extractable, usages, {publicKeyBytes: raw}));
                }
                if (name === 'ECDH' || name === 'ECDSA') {
                    // Validate raw key length for the curve
                    var ecSizes = {'P-256':{uncompressed:65,compressed:33},'P-384':{uncompressed:97,compressed:49},'P-521':{uncompressed:133,compressed:67}};
                    var sizes = ecSizes[a.namedCurve];
                    if (sizes && raw.length !== sizes.uncompressed && raw.length !== sizes.compressed) {
                        return Promise.reject(new DOMException('EC raw key has incorrect length for ' + a.namedCurve, 'DataError'));
                    }
                    // Handle compressed points (02/03 prefix) by decompressing
                    var pubRaw = raw;
                    if (raw[0] === 0x02 || raw[0] === 0x03) {
                        pubRaw = __braille_crypto_ec_decompress(a.namedCurve, raw);
                        if (pubRaw.length === 0) {
                            return Promise.reject(new DOMException('Failed to decompress EC point', 'DataError'));
                        }
                    }
                    return Promise.resolve(mkKey('public', {name: name, namedCurve: a.namedCurve}, extractable, usages, {publicKeyBytes: pubRaw}));
                }
                return Promise.resolve(mkKey('secret', algoObj, extractable, usages, {raw:raw}));
            }

            // PKCS8 import (private keys)
            if (format === 'pkcs8') {
                var derBytes = toBytes(keyData);
                if (name === 'X25519') {
                    if (derBytes.length !== 48) {
                        return Promise.reject(new DOMException('X25519 PKCS8 must be 48 bytes', 'DataError'));
                    }
                    var priv = extractX25519PrivateFromPkcs8(derBytes);
                    return Promise.resolve(mkKey('private', {name:'X25519'}, extractable, usages, {privateKeyBytes: priv}));
                }
                if (name === 'X448') {
                    if (derBytes.length !== 72) {
                        return Promise.reject(new DOMException('X448 PKCS8 must be 72 bytes', 'DataError'));
                    }
                    var priv = Array.from(derBytes.slice(16, 72));
                    return Promise.resolve(mkKey('private', {name:'X448'}, extractable, usages, {privateKeyBytes: priv}));
                }
                if (name === 'Ed25519') {
                    if (derBytes.length !== 48) {
                        return Promise.reject(new DOMException('Ed25519 PKCS8 must be 48 bytes', 'DataError'));
                    }
                    var priv = extractEd25519PrivateFromPkcs8(derBytes);
                    return Promise.resolve(mkKey('private', {name:'Ed25519'}, extractable, usages, {privateKeyBytes: priv}));
                }
                if (name === 'ECDH' || name === 'ECDSA') {
                    var curve = a.namedCurve;
                    var imported = __braille_crypto_ec_pkcs8_import(curve, Array.from(derBytes));
                    if (imported.length === 0) {
                        return Promise.reject(new DOMException('invalid EC PKCS8 key data', 'DataError'));
                    }
                    return Promise.resolve(mkKey('private', {name: name, namedCurve: curve}, extractable, usages, {privateKeyBytes: imported[0], publicKeyBytes: imported[1]}));
                }
                if (isRsaAlgo(name)) {
                    var imported = __braille_crypto_rsa_pkcs8_import(Array.from(derBytes));
                    if (imported.length === 0) {
                        return Promise.reject(new DOMException('invalid RSA PKCS8 key data', 'DataError'));
                    }
                    var h = hashName(a.hash);
                    var modBits = (imported[2][0]<<24)|(imported[2][1]<<16)|(imported[2][2]<<8)|imported[2][3];
                    var algoObj = {name: name, modulusLength: modBits, publicExponent: new Uint8Array(imported[3]), hash: {name: h}};
                    return Promise.resolve(mkKey('private', algoObj, extractable, usages, {privateKeyBytes: imported[0], publicKeyBytes: imported[1]}));
                }
                if (name === 'ML-KEM-512' || name === 'ML-KEM-768' || name === 'ML-KEM-1024') {
                    // PKCS8 for ML-KEM: extract 64-byte seed from DER structure
                    var seed = __braille_crypto_mlkem_pkcs8_import(name, Array.from(derBytes));
                    var pair = __braille_crypto_mlkem_from_seed(name, seed);
                    return Promise.resolve(mkKey('private', {name: name}, extractable, usages, {privateKeyBytes: seed, publicKeyBytes: pair[0]}));
                }
                if (name === 'Ed448') {
                    if (derBytes.length !== 73) {
                        return Promise.reject(new DOMException('Ed448 PKCS8 must be 73 bytes', 'DataError'));
                    }
                    var priv = Array.from(derBytes.slice(16, 73));
                    return Promise.resolve(mkKey('private', {name:'Ed448'}, extractable, usages, {privateKeyBytes: priv}));
                }
                return Promise.reject(new DOMException('importKey pkcs8 for ' + name + ' not supported', 'NotSupportedError'));
            }

            // SPKI import (public keys)
            if (format === 'spki') {
                var derBytes = toBytes(keyData);
                if (name === 'X25519') {
                    if (derBytes.length !== 44) {
                        return Promise.reject(new DOMException('X25519 SPKI must be 44 bytes', 'DataError'));
                    }
                    var pub_bytes = extractX25519PublicFromSpki(derBytes);
                    return Promise.resolve(mkKey('public', {name:'X25519'}, extractable, usages, {publicKeyBytes: pub_bytes}));
                }
                if (name === 'X448') {
                    if (derBytes.length !== 68) {
                        return Promise.reject(new DOMException('X448 SPKI must be 68 bytes', 'DataError'));
                    }
                    var pub_bytes = Array.from(derBytes.slice(12, 68));
                    return Promise.resolve(mkKey('public', {name:'X448'}, extractable, usages, {publicKeyBytes: pub_bytes}));
                }
                if (name === 'Ed25519') {
                    if (derBytes.length !== 44) {
                        return Promise.reject(new DOMException('Ed25519 SPKI must be 44 bytes', 'DataError'));
                    }
                    var pub_bytes = extractEd25519PublicFromSpki(derBytes);
                    return Promise.resolve(mkKey('public', {name:'Ed25519'}, extractable, usages, {publicKeyBytes: pub_bytes}));
                }
                if (name === 'ECDH' || name === 'ECDSA') {
                    var imported = __braille_crypto_ec_spki_import(Array.from(derBytes));
                    if (imported.length === 0) {
                        return Promise.reject(new DOMException('invalid EC SPKI key data', 'DataError'));
                    }
                    var detectedCurve = '';
                    for (var ci = 0; ci < imported[0].length; ci++) detectedCurve += String.fromCharCode(imported[0][ci]);
                    var curve = a.namedCurve || detectedCurve;
                    if (a.namedCurve && detectedCurve !== a.namedCurve) {
                        return Promise.reject(new DOMException('SPKI curve ' + detectedCurve + ' does not match requested ' + a.namedCurve, 'DataError'));
                    }
                    return Promise.resolve(mkKey('public', {name: name, namedCurve: curve}, extractable, usages, {publicKeyBytes: imported[1]}));
                }
                if (isRsaAlgo(name)) {
                    var imported = __braille_crypto_rsa_spki_import(Array.from(derBytes));
                    if (imported.length === 0) {
                        return Promise.reject(new DOMException('invalid RSA SPKI key data', 'DataError'));
                    }
                    var h = hashName(a.hash);
                    var modBits = (imported[1][0]<<24)|(imported[1][1]<<16)|(imported[1][2]<<8)|imported[1][3];
                    var algoObj = {name: name, modulusLength: modBits, publicExponent: new Uint8Array(imported[2]), hash: {name: h}};
                    return Promise.resolve(mkKey('public', algoObj, extractable, usages, {publicKeyBytes: imported[0]}));
                }
                if (name === 'ML-KEM-512' || name === 'ML-KEM-768' || name === 'ML-KEM-1024') {
                    // SPKI for ML-KEM: extract public key bytes from DER structure
                    var pubBytes = __braille_crypto_mlkem_spki_import(name, Array.from(derBytes));
                    return Promise.resolve(mkKey('public', {name: name}, extractable, usages, {publicKeyBytes: pubBytes}));
                }
                if (name === 'Ed448') {
                    if (derBytes.length !== 69) {
                        return Promise.reject(new DOMException('Ed448 SPKI must be 69 bytes', 'DataError'));
                    }
                    var pub_bytes = Array.from(derBytes.slice(12, 69));
                    return Promise.resolve(mkKey('public', {name:'Ed448'}, extractable, usages, {publicKeyBytes: pub_bytes}));
                }
                return Promise.reject(new DOMException('importKey spki for ' + name + ' not supported', 'NotSupportedError'));
            }

            // JWK import
            if (format === 'jwk') {
                var jwk = typeof keyData === 'string' ? JSON.parse(keyData) : keyData;
                if (jwk.kty === 'oct' || jwk.k) {
                    var b64 = jwk.k.replace(/-/g,'+').replace(/_/g,'/');
                    while (b64.length % 4) b64 += '=';
                    var raw = Array.from((function(s){
                        var bin = atob(s), arr = new Uint8Array(bin.length);
                        for(var i=0;i<bin.length;i++) arr[i]=bin.charCodeAt(i);
                        return arr;
                    })(b64));
                    var algoObj = Object.assign({}, a);
                    if (name.substring(0,3) === 'AES') algoObj = {name:name,length:raw.length*8};
                    if (name === 'ChaCha20-Poly1305') algoObj = {name:name};
                    if (name === 'HMAC' && a.hash) algoObj = {name:'HMAC',hash:{name:hashName(a.hash)},length:raw.length*8};
                    if (name === 'KMAC128' || name === 'KMAC256') algoObj = {name:name,length:raw.length*8};
                    return Promise.resolve(mkKey('secret', algoObj, extractable, usages, {raw:raw}));
                }
                // JWK validation: check required fields and constraints
                if (isEcAlgo(name)) {
                    if (!jwk.kty) return Promise.reject(new DOMException('Missing JWK kty', 'DataError'));
                    if (jwk.kty !== 'EC') return Promise.reject(new DOMException("Invalid JWK kty: expected 'EC'", 'DataError'));
                    if (!jwk.crv) return Promise.reject(new DOMException('Missing JWK crv', 'DataError'));
                    if (!jwk.x) return Promise.reject(new DOMException('Missing JWK x', 'DataError'));
                    if (a.namedCurve && jwk.crv !== a.namedCurve) {
                        return Promise.reject(new DOMException('JWK curve does not match algorithm', 'DataError'));
                    }
                    if (jwk.use && jwk.use !== 'sig' && jwk.use !== 'enc') {
                        return Promise.reject(new DOMException("Invalid JWK use field", 'DataError'));
                    }
                    if (name === 'ECDSA' && jwk.use && jwk.use !== 'sig') {
                        return Promise.reject(new DOMException("JWK use must be 'sig' for ECDSA", 'DataError'));
                    }
                    if (name === 'ECDH' && jwk.use && jwk.use !== 'enc') {
                        return Promise.reject(new DOMException("JWK use must be 'enc' for ECDH", 'DataError'));
                    }
                    // Check ext field
                    if (jwk.ext === false && extractable) {
                        return Promise.reject(new DOMException('JWK ext is false but extractable requested', 'DataError'));
                    }
                }
                if (isCfrgAlgo(name)) {
                    if (!jwk.kty) return Promise.reject(new DOMException('Missing JWK kty', 'DataError'));
                    if (jwk.kty !== 'OKP') return Promise.reject(new DOMException("Invalid JWK kty: expected 'OKP'", 'DataError'));
                    if (!jwk.crv) return Promise.reject(new DOMException('Missing JWK crv', 'DataError'));
                    if (jwk.crv !== name) return Promise.reject(new DOMException('JWK crv does not match algorithm', 'DataError'));
                    if (!jwk.x) return Promise.reject(new DOMException('Missing JWK x', 'DataError'));
                    if (jwk.ext === false && extractable) {
                        return Promise.reject(new DOMException('JWK ext is false but extractable requested', 'DataError'));
                    }
                    // Validate alg: must be algorithm name or EdDSA (for Ed curves)
                    if (jwk.alg !== undefined) {
                        var validAlgs = [name];
                        if (name === 'Ed25519' || name === 'Ed448') validAlgs.push('EdDSA');
                        if (validAlgs.indexOf(jwk.alg) === -1) {
                            return Promise.reject(new DOMException("Invalid JWK alg: '" + jwk.alg + "'", 'DataError'));
                        }
                    }
                    // Validate use
                    if (jwk.use !== undefined) {
                        var expectedUse = (name === 'Ed25519' || name === 'Ed448') ? 'sig' : 'enc';
                        if (jwk.use !== expectedUse) {
                            return Promise.reject(new DOMException("Invalid JWK use for " + name, 'DataError'));
                        }
                    }
                    // Validate key sizes
                    var okpKeySize = {'X25519':32,'Ed25519':32,'X448':56,'Ed448':57}[name];
                    if (okpKeySize) {
                        // b64url decode and check size
                        function b64urlDecSize(s) {
                            var b = s.replace(/-/g,'+').replace(/_/g,'/');
                            while (b.length % 4) b += '=';
                            return atob(b).length;
                        }
                        if (b64urlDecSize(jwk.x) !== okpKeySize) {
                            return Promise.reject(new DOMException(name + ' JWK x has incorrect size', 'DataError'));
                        }
                        if (jwk.d && b64urlDecSize(jwk.d) !== okpKeySize) {
                            return Promise.reject(new DOMException(name + ' JWK d has incorrect size', 'DataError'));
                        }
                    }
                }
                if (isRsaAlgo(name)) {
                    if (!jwk.kty) return Promise.reject(new DOMException('Missing JWK kty', 'DataError'));
                    if (jwk.kty !== 'RSA') return Promise.reject(new DOMException("Invalid JWK kty: expected 'RSA'", 'DataError'));
                    if (jwk.ext === false && extractable) {
                        return Promise.reject(new DOMException('JWK ext is false but extractable requested', 'DataError'));
                    }
                }
                // EC JWK import
                if (jwk.kty === 'EC' && isEcAlgo(name)) {
                    var crv = jwk.crv || a.namedCurve;
                    if (a.namedCurve && jwk.crv && a.namedCurve !== jwk.crv) {
                        return Promise.reject(new DOMException('JWK curve does not match algorithm', 'DataError'));
                    }
                    var coordLen = {'P-256':32,'P-384':48,'P-521':66}[crv];
                    if (!coordLen) return Promise.reject(new DOMException('Unsupported curve: ' + crv, 'NotSupportedError'));
                    function b64urlDecode(s) {
                        var b = s.replace(/-/g,'+').replace(/_/g,'/');
                        while (b.length % 4) b += '=';
                        var bin = atob(b), arr = new Uint8Array(bin.length);
                        for(var i=0;i<bin.length;i++) arr[i]=bin.charCodeAt(i);
                        return arr;
                    }
                    var xBytes = b64urlDecode(jwk.x);
                    var yBytes = jwk.y ? b64urlDecode(jwk.y) : new Uint8Array(0);
                    // JWK coordinates must be exactly coordLen bytes
                    if (xBytes.length !== coordLen || (jwk.y && yBytes.length !== coordLen)) {
                        return Promise.reject(new DOMException('EC JWK coordinate has incorrect size for ' + crv, 'DataError'));
                    }
                    // Build uncompressed point: 04 || x || y
                    var pubBytes = [4];
                    for (var i=0;i<xBytes.length;i++) pubBytes.push(xBytes[i]);
                    for (var i=0;i<yBytes.length;i++) pubBytes.push(yBytes[i]);
                    if (jwk.d) {
                        var dBytes = b64urlDecode(jwk.d);
                        if (dBytes.length !== coordLen) {
                            return Promise.reject(new DOMException('EC JWK d has incorrect size for ' + crv, 'DataError'));
                        }
                        var privScalar = Array.from(dBytes);
                        return Promise.resolve(mkKey('private', {name:name,namedCurve:crv}, extractable, usages, {privateKeyBytes:privScalar, publicKeyBytes:pubBytes}));
                    }
                    return Promise.resolve(mkKey('public', {name:name,namedCurve:crv}, extractable, usages, {publicKeyBytes:pubBytes}));
                }
                // OKP JWK import (Ed25519, Ed448, X25519, X448)
                if (jwk.kty === 'OKP' && isCfrgAlgo(name)) {
                    function b64urlDec(s) {
                        var b = s.replace(/-/g,'+').replace(/_/g,'/');
                        while (b.length % 4) b += '=';
                        var bin = atob(b), arr = new Uint8Array(bin.length);
                        for(var i=0;i<bin.length;i++) arr[i]=bin.charCodeAt(i);
                        return arr;
                    }
                    var pubBytes = Array.from(b64urlDec(jwk.x));
                    var okpJwkAlg = jwk.alg || null;
                    if (jwk.d) {
                        var privBytes = Array.from(b64urlDec(jwk.d));
                        // Validate key pair: derive public from private and compare
                        var derivedPub = null;
                        if (name === 'Ed25519') derivedPub = __braille_crypto_ed25519_get_public(privBytes);
                        else if (name === 'Ed448') derivedPub = __braille_crypto_ed448_get_public(privBytes);
                        else if (name === 'X25519') derivedPub = __braille_crypto_x25519_get_public(privBytes);
                        else if (name === 'X448') derivedPub = __braille_crypto_x448_get_public(privBytes);
                        if (derivedPub) {
                            var mismatch = false;
                            if (derivedPub.length !== pubBytes.length) mismatch = true;
                            for (var ki = 0; !mismatch && ki < derivedPub.length; ki++) {
                                if (derivedPub[ki] !== pubBytes[ki]) mismatch = true;
                            }
                            if (mismatch) return Promise.reject(new DOMException(name + ' JWK key pair mismatch', 'DataError'));
                        }
                        return Promise.resolve(mkKey('private', {name:name}, extractable, usages, {privateKeyBytes:privBytes, publicKeyBytes:pubBytes, jwkAlg:okpJwkAlg}));
                    }
                    return Promise.resolve(mkKey('public', {name:name}, extractable, usages, {publicKeyBytes:pubBytes, jwkAlg:okpJwkAlg}));
                }
                // RSA JWK import
                if (jwk.kty === 'RSA' && isRsaAlgo(name)) {
                    function b64urlDecRsa(s) {
                        var b = s.replace(/-/g,'+').replace(/_/g,'/');
                        while (b.length % 4) b += '=';
                        var bin = atob(b), arr = new Uint8Array(bin.length);
                        for(var i=0;i<bin.length;i++) arr[i]=bin.charCodeAt(i);
                        return arr;
                    }
                    var h = hashName(a.hash);
                    if (jwk.d) {
                        // Private key JWK import: reconstruct PKCS8 DER via Rust
                        var imported = __braille_crypto_rsa_jwk_import(JSON.stringify(jwk));
                        var modBits = (imported[2][0]<<24)|(imported[2][1]<<16)|(imported[2][2]<<8)|imported[2][3];
                        var algoObj = {name:name, modulusLength:modBits, publicExponent:new Uint8Array(imported[3]), hash:{name:h}};
                        return Promise.resolve(mkKey('private', algoObj, extractable, usages, {privateKeyBytes:imported[0], publicKeyBytes:imported[1]}));
                    } else {
                        // Public key JWK import: reconstruct SPKI DER via Rust
                        var imported = __braille_crypto_rsa_jwk_pub_import(jwk.n, jwk.e);
                        var modBits = (imported[1][0]<<24)|(imported[1][1]<<16)|(imported[1][2]<<8)|imported[1][3];
                        var algoObj = {name:name, modulusLength:modBits, publicExponent:new Uint8Array(imported[2]), hash:{name:h}};
                        return Promise.resolve(mkKey('public', algoObj, extractable, usages, {publicKeyBytes:imported[0]}));
                    }
                }
            }

            return Promise.reject(new DOMException('importKey format ' + format + ' for ' + name + ' not supported', 'NotSupportedError'));
        },

        exportKey: function(format, key) {
            if (format === 'raw' || format === 'raw-secret' || format === 'raw-public') {
                if (key._raw) return Promise.resolve(new Uint8Array(key._raw).buffer);
                if (key._publicKeyBytes) return Promise.resolve(new Uint8Array(key._publicKeyBytes).buffer);
            }
            if (format === 'raw-seed') {
                if (key._privateKeyBytes) return Promise.resolve(new Uint8Array(key._privateKeyBytes).buffer);
            }
            if (format === 'spki' && key.type === 'public') {
                if (key.algorithm.name === 'X25519' && key._publicKeyBytes) {
                    var spki = wrapX25519PublicAsSpki(key._publicKeyBytes);
                    return Promise.resolve(new Uint8Array(spki).buffer);
                }
                if (key.algorithm.name === 'Ed25519' && key._publicKeyBytes) {
                    var spki = wrapEd25519PublicAsSpki(key._publicKeyBytes);
                    return Promise.resolve(new Uint8Array(spki).buffer);
                }
                if (key.algorithm.name === 'X448' && key._publicKeyBytes) {
                    // X448 SPKI: 30 42 30 05 06 03 2b6571 03 39 00 <56 bytes>
                    var spki = [48, 66, 48, 5, 6, 3, 43, 101, 111, 3, 57, 0].concat(Array.from(key._publicKeyBytes));
                    return Promise.resolve(new Uint8Array(spki).buffer);
                }
                if (key.algorithm.name === 'Ed448' && key._publicKeyBytes) {
                    // SPKI for Ed448: 30 43 30 05 06 03 2b6571 03 3a 00 <57 bytes>
                    var spki = [48, 67, 48, 5, 6, 3, 43, 101, 113, 3, 58, 0].concat(Array.from(key._publicKeyBytes));
                    return Promise.resolve(new Uint8Array(spki).buffer);
                }
                if (isRsaAlgo(key.algorithm.name) && key._publicKeyBytes) {
                    return Promise.resolve(new Uint8Array(key._publicKeyBytes).buffer);
                }
                if (isEcAlgo(key.algorithm.name) && key._publicKeyBytes) {
                    var der = __braille_crypto_ec_spki_export(key.algorithm.namedCurve, key._publicKeyBytes);
                    return Promise.resolve(new Uint8Array(der).buffer);
                }
                if (key.algorithm.name && key.algorithm.name.indexOf('ML-KEM') === 0 && key._publicKeyBytes) {
                    var spkiDer = __braille_crypto_mlkem_spki_export(key.algorithm.name, key._publicKeyBytes);
                    return Promise.resolve(new Uint8Array(spkiDer).buffer);
                }
            }
            if (format === 'pkcs8' && key.type === 'private') {
                if (key.algorithm.name === 'X25519' && key._privateKeyBytes) {
                    var pkcs8 = wrapX25519PrivateAsPkcs8(key._privateKeyBytes);
                    return Promise.resolve(new Uint8Array(pkcs8).buffer);
                }
                if (key.algorithm.name === 'Ed25519' && key._privateKeyBytes) {
                    var pkcs8 = wrapEd25519PrivateAsPkcs8(key._privateKeyBytes);
                    return Promise.resolve(new Uint8Array(pkcs8).buffer);
                }
                if (key.algorithm.name === 'X448' && key._privateKeyBytes) {
                    // X448 PKCS8: 30 46 02 01 00 30 05 06 03 2b656f 04 3a 04 38 <56 bytes>
                    var pkcs8 = [48, 70, 2, 1, 0, 48, 5, 6, 3, 43, 101, 111, 4, 58, 4, 56].concat(Array.from(key._privateKeyBytes));
                    return Promise.resolve(new Uint8Array(pkcs8).buffer);
                }
                if (key.algorithm.name === 'Ed448' && key._privateKeyBytes) {
                    // PKCS8 for Ed448: 30 47 02 01 00 30 05 06 03 2b6571 04 3b 04 39 <57 bytes>
                    var pkcs8 = [48, 71, 2, 1, 0, 48, 5, 6, 3, 43, 101, 113, 4, 59, 4, 57].concat(Array.from(key._privateKeyBytes));
                    return Promise.resolve(new Uint8Array(pkcs8).buffer);
                }
                if (isRsaAlgo(key.algorithm.name) && key._privateKeyBytes) {
                    return Promise.resolve(new Uint8Array(key._privateKeyBytes).buffer);
                }
                if (isEcAlgo(key.algorithm.name) && key._privateKeyBytes) {
                    var der = __braille_crypto_ec_pkcs8_export(key.algorithm.namedCurve, key._privateKeyBytes, key._publicKeyBytes);
                    return Promise.resolve(new Uint8Array(der).buffer);
                }
                if (key.algorithm.name && key.algorithm.name.indexOf('ML-KEM') === 0 && key._privateKeyBytes) {
                    var pkcs8Der = __braille_crypto_mlkem_pkcs8_export(key.algorithm.name, key._privateKeyBytes);
                    return Promise.resolve(new Uint8Array(pkcs8Der).buffer);
                }
            }
            if (format === 'jwk') {
                if (key._raw) {
                    var jwkAlg = '';
                    var klen = key._raw.length * 8;
                    var kname = key.algorithm.name;
                    if (kname === 'AES-GCM') jwkAlg = 'A' + klen + 'GCM';
                    else if (kname === 'AES-CBC') jwkAlg = 'A' + klen + 'CBC';
                    else if (kname === 'AES-CTR') jwkAlg = 'A' + klen + 'CTR';
                    else if (kname === 'AES-KW') jwkAlg = 'A' + klen + 'KW';
                    else if (kname === 'AES-OCB') jwkAlg = 'A' + klen + 'OCB';
                    else if (kname === 'HMAC') {
                        var hh = key.algorithm.hash && key.algorithm.hash.name;
                        jwkAlg = hh === 'SHA-1' ? 'HS1' : hh === 'SHA-256' ? 'HS256' : hh === 'SHA-384' ? 'HS384' : 'HS512';
                    }
                    else if (kname === 'KMAC128') jwkAlg = 'K128';
                    else if (kname === 'KMAC256') jwkAlg = 'K256';
                    var jwk = {kty:'oct',k:b64url(key._raw),ext:key.extractable,key_ops:Array.from(key.usages)};
                    if (jwkAlg) jwk.alg = jwkAlg;
                    return Promise.resolve(jwk);
                }
                if (isEcAlgo(key.algorithm.name) && key._publicKeyBytes) {
                    var privBytes = (key.type === 'private' && key._privateKeyBytes) ? key._privateKeyBytes : [];
                    var json = __braille_crypto_ec_jwk_export(key.algorithm.namedCurve, key._publicKeyBytes, privBytes);
                    var jwk = JSON.parse(json);
                    jwk.ext = key.extractable;
                    jwk.key_ops = Array.from(key.usages);
                    return Promise.resolve(jwk);
                }
                if (isRsaAlgo(key.algorithm.name) && key._publicKeyBytes) {
                    var privDer = (key.type === 'private' && key._privateKeyBytes) ? key._privateKeyBytes : [];
                    var json = __braille_crypto_rsa_jwk_export(key._publicKeyBytes, privDer);
                    var jwk = JSON.parse(json);
                    jwk.ext = key.extractable;
                    jwk.key_ops = Array.from(key.usages);
                    // Map algorithm + hash to JWK alg
                    var rsaHash = key.algorithm.hash && key.algorithm.hash.name;
                    if (key.algorithm.name === 'RSA-OAEP') {
                        jwk.alg = rsaHash === 'SHA-256' ? 'RSA-OAEP-256' : rsaHash === 'SHA-384' ? 'RSA-OAEP-384' : rsaHash === 'SHA-512' ? 'RSA-OAEP-512' : 'RSA-OAEP';
                    } else if (key.algorithm.name === 'RSA-PSS') {
                        jwk.alg = rsaHash === 'SHA-256' ? 'PS256' : rsaHash === 'SHA-384' ? 'PS384' : rsaHash === 'SHA-512' ? 'PS512' : 'PS1';
                    } else if (key.algorithm.name === 'RSASSA-PKCS1-v1_5') {
                        jwk.alg = rsaHash === 'SHA-256' ? 'RS256' : rsaHash === 'SHA-384' ? 'RS384' : rsaHash === 'SHA-512' ? 'RS512' : 'RS1';
                    }
                    return Promise.resolve(jwk);
                }
                if (isCfrgAlgo(key.algorithm.name) && key._publicKeyBytes) {
                    var jwk = {kty:'OKP',crv:key.algorithm.name,x:b64url(key._publicKeyBytes),ext:key.extractable,key_ops:Array.from(key.usages)};
                    // Per spec, Ed25519/Ed448 export alg = algorithm.name
                    if (key.algorithm.name === 'Ed25519' || key.algorithm.name === 'Ed448') {
                        jwk.alg = key.algorithm.name;
                    }
                    if (key.type === 'private' && key._privateKeyBytes) {
                        jwk.d = b64url(key._privateKeyBytes);
                    }
                    return Promise.resolve(jwk);
                }
            }
            return Promise.reject(new DOMException('exportKey format ' + format + ' not supported', 'NotSupportedError'));
        },

        encrypt: function(algo, key, data) {
            var a = normalizeAlgo(algo);
            if (key.usages.indexOf('encrypt') === -1) {
                return Promise.reject(new DOMException('key usages do not include encrypt', 'InvalidAccessError'));
            }
            if (key.algorithm.name !== a.name) {
                return Promise.reject(new DOMException('key algorithm does not match', 'InvalidAccessError'));
            }
            var pt = toBytesSnapshot(data);
            if (a.name === 'AES-GCM') {
                var iv = Array.from(toBytes(a.iv));
                var aad = a.additionalData ? Array.from(toBytes(a.additionalData)) : [];
                var tagLen = a.tagLength || 128;
                var validTags = {32:1,64:1,96:1,104:1,112:1,120:1,128:1};
                if (!validTags[tagLen]) {
                    return Promise.reject(new DOMException('tagLength must be 32, 64, 96, 104, 112, 120, or 128', 'OperationError'));
                }
                var result = __braille_crypto_aes_gcm_encrypt(key._raw, iv, pt, aad, tagLen);
                return Promise.resolve(new Uint8Array(result).buffer);
            }
            if (a.name === 'AES-CBC') {
                var iv = Array.from(toBytes(a.iv));
                if (iv.length !== 16) {
                    return Promise.reject(new DOMException('AES-CBC IV must be 16 bytes', 'OperationError'));
                }
                var result = __braille_crypto_aes_cbc_encrypt(key._raw, iv, pt);
                return Promise.resolve(new Uint8Array(result).buffer);
            }
            if (a.name === 'AES-CTR') {
                var counter = Array.from(toBytes(a.counter));
                var ctrLen = a.length;
                if (!ctrLen || ctrLen < 1 || ctrLen > 128) {
                    return Promise.reject(new DOMException('AES-CTR counter length must be between 1 and 128', 'OperationError'));
                }
                var result = __braille_crypto_aes_ctr_encrypt(key._raw, counter, pt);
                return Promise.resolve(new Uint8Array(result).buffer);
            }
            if (a.name === 'AES-OCB') {
                var iv = Array.from(toBytes(a.iv));
                if (iv.length === 0 || iv.length > 15) {
                    return Promise.reject(new DOMException('AES-OCB IV must be 1-15 bytes', 'OperationError'));
                }
                var aad = a.additionalData ? Array.from(toBytes(a.additionalData)) : [];
                var tagLen = a.tagLength || 128;
                var validTags = {64:1,96:1,128:1};
                if (!validTags[tagLen]) {
                    return Promise.reject(new DOMException('AES-OCB tagLength must be 64, 96, or 128', 'OperationError'));
                }
                var result = __braille_crypto_aes_ocb_encrypt(key._raw, iv, pt, aad, tagLen);
                return Promise.resolve(new Uint8Array(result).buffer);
            }
            if (a.name === 'ChaCha20-Poly1305') {
                var iv = Array.from(toBytes(a.iv));
                if (iv.length !== 12) {
                    return Promise.reject(new DOMException('ChaCha20-Poly1305 IV must be 12 bytes', 'OperationError'));
                }
                var aad = a.additionalData ? Array.from(toBytes(a.additionalData)) : [];
                var result = __braille_crypto_chacha20_encrypt(key._raw, iv, pt, aad);
                return Promise.resolve(new Uint8Array(result).buffer);
            }
            if (a.name === 'RSA-OAEP') {
                var label = a.label ? Array.from(toBytes(a.label)) : [];
                var h = key.algorithm.hash.name;
                var result = __braille_crypto_rsa_oaep_encrypt(key._publicKeyBytes, h, label, pt);
                if (result[0][0] === 0) {
                    return Promise.reject(new DOMException('RSA-OAEP encryption failed', 'OperationError'));
                }
                return Promise.resolve(new Uint8Array(result[1]).buffer);
            }
            return Promise.reject(new DOMException('encrypt ' + a.name + ' not supported', 'NotSupportedError'));
        },

        decrypt: function(algo, key, data) {
            var a = normalizeAlgo(algo);
            if (key.usages.indexOf('decrypt') === -1) {
                return Promise.reject(new DOMException('key usages do not include decrypt', 'InvalidAccessError'));
            }
            if (key.algorithm.name !== a.name) {
                return Promise.reject(new DOMException('key algorithm does not match', 'InvalidAccessError'));
            }
            var ct = toBytesSnapshot(data);
            if (a.name === 'AES-GCM') {
                var iv = Array.from(toBytes(a.iv));
                var aad = a.additionalData ? Array.from(toBytes(a.additionalData)) : [];
                var tagLen = a.tagLength || 128;
                var validTags = {32:1,64:1,96:1,104:1,112:1,120:1,128:1};
                if (!validTags[tagLen]) {
                    return Promise.reject(new DOMException('tagLength must be 32, 64, 96, 104, 112, 120, or 128', 'OperationError'));
                }
                var result = __braille_crypto_aes_gcm_decrypt(key._raw, iv, ct, aad, tagLen);
                if (result[0][0] === 0) {
                    return Promise.reject(new DOMException('AES-GCM decryption failed', 'OperationError'));
                }
                return Promise.resolve(new Uint8Array(result[1]).buffer);
            }
            if (a.name === 'AES-CBC') {
                var iv = Array.from(toBytes(a.iv));
                if (iv.length !== 16) {
                    return Promise.reject(new DOMException('AES-CBC IV must be 16 bytes', 'OperationError'));
                }
                var result = __braille_crypto_aes_cbc_decrypt(key._raw, iv, ct);
                if (result[0][0] === 0) {
                    return Promise.reject(new DOMException('AES-CBC decryption failed', 'OperationError'));
                }
                return Promise.resolve(new Uint8Array(result[1]).buffer);
            }
            if (a.name === 'AES-CTR') {
                var counter = Array.from(toBytes(a.counter));
                var ctrLen = a.length;
                if (!ctrLen || ctrLen < 1 || ctrLen > 128) {
                    return Promise.reject(new DOMException('AES-CTR counter length must be between 1 and 128', 'OperationError'));
                }
                var result = __braille_crypto_aes_ctr_decrypt(key._raw, counter, ct);
                return Promise.resolve(new Uint8Array(result).buffer);
            }
            if (a.name === 'AES-OCB') {
                var iv = Array.from(toBytes(a.iv));
                if (iv.length === 0 || iv.length > 15) {
                    return Promise.reject(new DOMException('AES-OCB IV must be 1-15 bytes', 'OperationError'));
                }
                var aad = a.additionalData ? Array.from(toBytes(a.additionalData)) : [];
                var tagLen = a.tagLength || 128;
                var validTags = {64:1,96:1,128:1};
                if (!validTags[tagLen]) {
                    return Promise.reject(new DOMException('AES-OCB tagLength must be 64, 96, or 128', 'OperationError'));
                }
                var result = __braille_crypto_aes_ocb_decrypt(key._raw, iv, ct, aad, tagLen);
                if (result[0][0] === 0) {
                    return Promise.reject(new DOMException('AES-OCB decryption failed', 'OperationError'));
                }
                return Promise.resolve(new Uint8Array(result[1]).buffer);
            }
            if (a.name === 'ChaCha20-Poly1305') {
                var iv = Array.from(toBytes(a.iv));
                if (iv.length !== 12) {
                    return Promise.reject(new DOMException('ChaCha20-Poly1305 IV must be 12 bytes', 'OperationError'));
                }
                var aad = a.additionalData ? Array.from(toBytes(a.additionalData)) : [];
                var result = __braille_crypto_chacha20_decrypt(key._raw, iv, ct, aad);
                if (result[0][0] === 0) {
                    return Promise.reject(new DOMException('ChaCha20-Poly1305 decryption failed', 'OperationError'));
                }
                return Promise.resolve(new Uint8Array(result[1]).buffer);
            }
            if (a.name === 'RSA-OAEP') {
                var label = a.label ? Array.from(toBytes(a.label)) : [];
                var h = key.algorithm.hash.name;
                var result = __braille_crypto_rsa_oaep_decrypt(key._privateKeyBytes, h, label, ct);
                if (result[0][0] === 0) {
                    return Promise.reject(new DOMException('RSA-OAEP decryption failed', 'OperationError'));
                }
                return Promise.resolve(new Uint8Array(result[1]).buffer);
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
            if (a.name === 'Ed448') {
                var result = __braille_crypto_ed448_sign(key._privateKeyBytes, Array.from(toBytes(data)));
                return Promise.resolve(new Uint8Array(result).buffer);
            }
            if (a.name === 'ECDSA') {
                var curve = key.algorithm.namedCurve;
                var h = hashName(a.hash);
                var result = __braille_crypto_ecdsa_sign(curve, h, key._privateKeyBytes, Array.from(toBytes(data)));
                return Promise.resolve(new Uint8Array(result).buffer);
            }
            if (a.name === 'RSA-PSS') {
                var h = key.algorithm.hash.name;
                var saltLen = a.saltLength || 0;
                var result = __braille_crypto_rsa_pss_sign(key._privateKeyBytes, h, saltLen, Array.from(toBytes(data)));
                return Promise.resolve(new Uint8Array(result).buffer);
            }
            if (a.name === 'RSASSA-PKCS1-v1_5') {
                var h = key.algorithm.hash.name;
                var result = __braille_crypto_rsa_pkcs1_sign(key._privateKeyBytes, h, Array.from(toBytes(data)));
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
            if (a.name === 'Ed448') {
                var ok = __braille_crypto_ed448_verify(key._publicKeyBytes, Array.from(toBytes(signature)), Array.from(toBytes(data)));
                return Promise.resolve(ok);
            }
            if (a.name === 'ECDSA') {
                var curve = key.algorithm.namedCurve;
                var h = hashName(a.hash);
                var ok = __braille_crypto_ecdsa_verify(curve, h, key._publicKeyBytes, Array.from(toBytes(signature)), Array.from(toBytes(data)));
                return Promise.resolve(ok);
            }
            if (a.name === 'RSA-PSS') {
                var h = key.algorithm.hash.name;
                var saltLen = a.saltLength || 0;
                var ok = __braille_crypto_rsa_pss_verify(key._publicKeyBytes, h, saltLen, Array.from(toBytes(signature)), Array.from(toBytes(data)));
                return Promise.resolve(ok);
            }
            if (a.name === 'RSASSA-PKCS1-v1_5') {
                var h = key.algorithm.hash.name;
                var ok = __braille_crypto_rsa_pkcs1_verify(key._publicKeyBytes, h, Array.from(toBytes(signature)), Array.from(toBytes(data)));
                return Promise.resolve(ok);
            }
            return Promise.reject(new DOMException('verify ' + a.name + ' not supported', 'NotSupportedError'));
        },

        deriveBits: function(algo, baseKey, length, _fromDeriveKey) {
            var a = normalizeAlgo(algo);
            if (a.name === 'PBKDF2') {
                if (baseKey.algorithm.name !== 'PBKDF2') {
                    return Promise.reject(new DOMException('baseKey algorithm does not match', 'InvalidAccessError'));
                }
                if (!_fromDeriveKey && baseKey.usages.indexOf('deriveBits') === -1) {
                    return Promise.reject(new DOMException('baseKey usages do not include deriveBits', 'InvalidAccessError'));
                }
                if (length === 0) return Promise.resolve(new ArrayBuffer(0));
                if (length === null || length === undefined || length % 8 !== 0) {
                    return Promise.reject(new DOMException('PBKDF2 requires length that is a multiple of 8', 'OperationError'));
                }
                var h = hashName(a.hash);
                var validHashes = ['SHA-1', 'SHA-256', 'SHA-384', 'SHA-512'];
                if (validHashes.indexOf(h) === -1) {
                    return Promise.reject(new DOMException('Unrecognized hash name: ' + h, 'NotSupportedError'));
                }
                if (!a.iterations || a.iterations === 0) {
                    return Promise.reject(new DOMException('iterations must be > 0', 'OperationError'));
                }
                var salt = Array.from(toBytes(a.salt));
                var result = __braille_crypto_pbkdf2(h, baseKey._raw, salt, a.iterations, length/8);
                return Promise.resolve(new Uint8Array(result).buffer);
            }
            if (a.name === 'HKDF') {
                if (baseKey.algorithm.name !== 'HKDF') {
                    return Promise.reject(new DOMException('baseKey algorithm does not match', 'InvalidAccessError'));
                }
                if (!_fromDeriveKey && baseKey.usages.indexOf('deriveBits') === -1) {
                    return Promise.reject(new DOMException('baseKey usages do not include deriveBits', 'InvalidAccessError'));
                }
                if (length === 0) return Promise.resolve(new ArrayBuffer(0));
                if (length === null || length === undefined || length % 8 !== 0) {
                    return Promise.reject(new DOMException('HKDF requires length that is a multiple of 8', 'OperationError'));
                }
                var h = hashName(a.hash);
                var validHashes = ['SHA-1', 'SHA-256', 'SHA-384', 'SHA-512'];
                if (validHashes.indexOf(h) === -1) {
                    return Promise.reject(new DOMException('Unrecognized hash name: ' + h, 'NotSupportedError'));
                }
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
                if (!_fromDeriveKey && baseKey.usages.indexOf('deriveBits') === -1) {
                    return Promise.reject(new DOMException('baseKey usages do not include deriveBits', 'InvalidAccessError'));
                }
                if (length === 0) {
                    return Promise.resolve(new ArrayBuffer(0));
                }
                if (length !== null && length !== undefined && length > 256) {
                    return Promise.reject(new DOMException('Requested too many bits', 'OperationError'));
                }
                var shared = __braille_crypto_x25519_derive_bits(baseKey._privateKeyBytes, a.public._publicKeyBytes);
                if (shared.length === 0) {
                    return Promise.reject(new DOMException('X25519 produced all-zero shared secret', 'OperationError'));
                }
                if (length === null || length === undefined) {
                    return Promise.resolve(new Uint8Array(shared).buffer);
                }
                var requestedBytes = Math.ceil(length / 8);
                shared = shared.slice(0, requestedBytes);
                if (length % 8 !== 0) {
                    shared[shared.length - 1] &= (0xFF << (8 - (length % 8)));
                }
                return Promise.resolve(new Uint8Array(shared).buffer);
            }
            if (a.name === 'X448') {
                if (!a.public) {
                    return Promise.reject(new TypeError('X448 deriveBits requires public property'));
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
                if (a.public.algorithm.name !== 'X448') {
                    return Promise.reject(new DOMException('Algorithm mismatch', 'InvalidAccessError'));
                }
                if (!_fromDeriveKey && baseKey.usages.indexOf('deriveBits') === -1) {
                    return Promise.reject(new DOMException('baseKey usages do not include deriveBits', 'InvalidAccessError'));
                }
                if (length === 0) {
                    return Promise.resolve(new ArrayBuffer(0));
                }
                if (length !== null && length !== undefined && length > 448) {
                    return Promise.reject(new DOMException('Requested too many bits', 'OperationError'));
                }
                var shared = __braille_crypto_x448_derive_bits(baseKey._privateKeyBytes, a.public._publicKeyBytes);
                if (shared.length === 0) {
                    return Promise.reject(new DOMException('X448 produced all-zero shared secret', 'OperationError'));
                }
                if (length === null || length === undefined) {
                    return Promise.resolve(new Uint8Array(shared).buffer);
                }
                var requestedBytes = Math.ceil(length / 8);
                shared = shared.slice(0, requestedBytes);
                if (length % 8 !== 0) {
                    shared[shared.length - 1] &= (0xFF << (8 - (length % 8)));
                }
                return Promise.resolve(new Uint8Array(shared).buffer);
            }
            if (a.name === 'ECDH') {
                if (!a.public || !(a.public instanceof CryptoKey)) {
                    return Promise.reject(new TypeError('ECDH deriveBits requires public CryptoKey'));
                }
                if (!_fromDeriveKey && baseKey.usages.indexOf('deriveBits') === -1) {
                    return Promise.reject(new DOMException('baseKey usages do not include deriveBits', 'InvalidAccessError'));
                }
                if (baseKey.type !== 'private') {
                    return Promise.reject(new DOMException('baseKey must be a private key', 'InvalidAccessError'));
                }
                if (a.public.type === 'private' || a.public.type === 'secret') {
                    return Promise.reject(new DOMException('public property must be a public key', 'InvalidAccessError'));
                }
                if (a.public.algorithm.name !== 'ECDH') {
                    return Promise.reject(new DOMException('public key algorithm must be ECDH', 'InvalidAccessError'));
                }
                if (a.public.algorithm.namedCurve !== baseKey.algorithm.namedCurve) {
                    return Promise.reject(new DOMException('public key curve does not match baseKey curve', 'InvalidAccessError'));
                }
                var curve = baseKey.algorithm.namedCurve;
                var curveSize = {'P-256': 32, 'P-384': 48, 'P-521': 66}[curve] || 32;
                var shared = __braille_crypto_ecdh_derive(curve, baseKey._privateKeyBytes, a.public._publicKeyBytes);
                if (length === 0) {
                    return Promise.resolve(new ArrayBuffer(0));
                }
                if (length === null || length === undefined) {
                    return Promise.resolve(new Uint8Array(shared).buffer);
                }
                if (length > curveSize * 8) {
                    return Promise.reject(new DOMException('Requested too many bits', 'OperationError'));
                }
                var requestedBytes = Math.ceil(length / 8);
                shared = shared.slice(0, requestedBytes);
                if (length % 8 !== 0) {
                    shared[shared.length - 1] &= (0xFF << (8 - (length % 8)));
                }
                return Promise.resolve(new Uint8Array(shared).buffer);
            }
            return Promise.reject(new DOMException('deriveBits ' + a.name + ' not supported', 'NotSupportedError'));
        },

        deriveKey: function(algo, baseKey, derivedKeyAlgo, extractable, usages) {
            var a = normalizeAlgo(algo);
            var dka = normalizeAlgo(derivedKeyAlgo);
            var bitLen = dka.length || 256;
            // deriveKey checks 'deriveKey' usage, not 'deriveBits'
            if (baseKey.usages.indexOf('deriveKey') === -1) {
                return Promise.reject(new DOMException('baseKey usages do not include deriveKey', 'InvalidAccessError'));
            }
            return subtle.deriveBits(a, baseKey, bitLen, true).then(function(bits) {
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
        },

        encapsulateBits: function(algo, publicKey) {
            var a = normalizeAlgo(algo);
            var name = a.name;
            if (name !== 'ML-KEM-512' && name !== 'ML-KEM-768' && name !== 'ML-KEM-1024') {
                return Promise.reject(new DOMException('encapsulateBits for ' + name + ' not supported', 'NotSupportedError'));
            }
            var result = __braille_crypto_mlkem_encapsulate(name, publicKey._publicKeyBytes);
            return Promise.resolve({
                ciphertext: new Uint8Array(result[0]).buffer,
                sharedKey: new Uint8Array(result[1]).buffer
            });
        },

        decapsulateBits: function(algo, privateKey, ciphertext) {
            var a = normalizeAlgo(algo);
            var name = a.name;
            if (name !== 'ML-KEM-512' && name !== 'ML-KEM-768' && name !== 'ML-KEM-1024') {
                return Promise.reject(new DOMException('decapsulateBits for ' + name + ' not supported', 'NotSupportedError'));
            }
            var ctBytes = Array.from(toBytes(ciphertext));
            var result = __braille_crypto_mlkem_decapsulate(name, privateKey._privateKeyBytes, ctBytes);
            return Promise.resolve(new Uint8Array(result).buffer);
        },

        encapsulateKey: function(algo, publicKey, derivedKeyAlgo, extractable, usages) {
            var a = normalizeAlgo(algo);
            var name = a.name;
            if (name !== 'ML-KEM-512' && name !== 'ML-KEM-768' && name !== 'ML-KEM-1024') {
                return Promise.reject(new DOMException('encapsulateKey for ' + name + ' not supported', 'NotSupportedError'));
            }
            var result = __braille_crypto_mlkem_encapsulate(name, publicKey._publicKeyBytes);
            var sharedBytes = result[1];
            var dka = normalizeAlgo(derivedKeyAlgo);
            return subtle.importKey('raw', new Uint8Array(sharedBytes), dka, extractable, usages).then(function(sharedKey) {
                return {
                    ciphertext: new Uint8Array(result[0]).buffer,
                    sharedKey: sharedKey
                };
            });
        },

        getPublicKey: function(privateKey, usages) {
            if (!(privateKey instanceof CryptoKey)) {
                return Promise.reject(new TypeError('key must be a CryptoKey'));
            }
            if (privateKey.type === 'public') {
                return Promise.reject(new DOMException('key must be a private key', 'InvalidAccessError'));
            }
            if (privateKey.type === 'secret') {
                return Promise.reject(new DOMException('getPublicKey not supported for symmetric keys', 'NotSupportedError'));
            }
            var name = privateKey.algorithm.name;
            // Validate usages for the algorithm's public key
            var validPubUsages = {
                'RSA-OAEP':['encrypt','wrapKey'],
                'RSA-PSS':['verify'],
                'RSASSA-PKCS1-v1_5':['verify'],
                'ECDH':[],
                'ECDSA':['verify'],
                'Ed25519':['verify'],
                'Ed448':['verify'],
                'X25519':[],
                'X448':[]
            };
            var validU = validPubUsages[name];
            if (validU === undefined) {
                return Promise.reject(new DOMException('getPublicKey not supported for ' + name, 'NotSupportedError'));
            }
            if (usages && usages.length > 0) {
                for (var ui = 0; ui < usages.length; ui++) {
                    if (validU.indexOf(usages[ui]) === -1) {
                        return Promise.reject(new DOMException('Invalid usage: ' + usages[ui], 'SyntaxError'));
                    }
                }
            }
            if (!privateKey._publicKeyBytes) {
                return Promise.reject(new DOMException('No public key available', 'OperationError'));
            }
            var pubKey = mkKey('public', privateKey.algorithm, true, usages || [], {publicKeyBytes: privateKey._publicKeyBytes});
            return Promise.resolve(pubKey);
        },

        decapsulateKey: function(algo, privateKey, ciphertext, derivedKeyAlgo, extractable, usages) {
            var a = normalizeAlgo(algo);
            var name = a.name;
            if (name !== 'ML-KEM-512' && name !== 'ML-KEM-768' && name !== 'ML-KEM-1024') {
                return Promise.reject(new DOMException('decapsulateKey for ' + name + ' not supported', 'NotSupportedError'));
            }
            var ctBytes = Array.from(toBytes(ciphertext));
            var result = __braille_crypto_mlkem_decapsulate(name, privateKey._privateKeyBytes, ctBytes);
            var dka = normalizeAlgo(derivedKeyAlgo);
            return subtle.importKey('raw', new Uint8Array(result), dka, extractable, usages);
        }
    };

    globalThis.crypto = {
        subtle: subtle,
        getRandomValues: function(arr) {
            if (arr instanceof Float32Array || arr instanceof Float64Array || (typeof Float16Array !== 'undefined' && arr instanceof Float16Array) || arr instanceof DataView) {
                throw new DOMException('The provided ArrayBufferView is not an integer typed array', 'TypeMismatchError');
            }
            if (arr.byteLength > 65536) {
                throw new DOMException('The ArrayBufferView\'s byte length exceeds the number of bytes of entropy available via this API (65536 bytes).', 'QuotaExceededError');
            }
            // Fill underlying buffer with random bytes, then let the typed array view interpret them
            var buf = new Uint8Array(arr.buffer, arr.byteOffset, arr.byteLength);
            var bytes = __braille_crypto_get_random_bytes(buf.length);
            for (var i = 0; i < buf.length; i++) buf[i] = bytes[i];
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
