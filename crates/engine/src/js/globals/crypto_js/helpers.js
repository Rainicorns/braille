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
        'ml-dsa-44':'ML-DSA-44','ml-dsa-65':'ML-DSA-65','ml-dsa-87':'ML-DSA-87',
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
        if (typeof a === 'string') return {name: algoNameMap[asciiLower(a)] || a};
        var o = {};
        for (var k in a) o[k] = a[k];
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
    function b64urlDecode(s) {
        var b = s.replace(/-/g,'+').replace(/_/g,'/');
        while (b.length % 4) b += '=';
        var bin = atob(b), arr = new Uint8Array(bin.length);
        for(var i=0;i<bin.length;i++) arr[i]=bin.charCodeAt(i);
        return arr;
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
