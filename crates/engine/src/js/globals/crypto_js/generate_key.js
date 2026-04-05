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
            if (name === 'KMAC128' || name === 'KMAC256') {
                var defaultLen = (name === 'KMAC128') ? 256 : 512;
                var keyLenBits = a.length || defaultLen;
                var raw = __braille_crypto_get_random_bytes(keyLenBits / 8);
                return Promise.resolve(mkKey('secret', {name:name,length:keyLenBits}, extractable, usages, {raw:raw}));
            }
            if (name === 'ML-DSA-44' || name === 'ML-DSA-65' || name === 'ML-DSA-87') {
                var seed = __braille_crypto_get_random_bytes(32);
                var vkBytes = __braille_crypto_mldsa_from_seed(name, Array.from(new Uint8Array(seed)));
                var algoObj = {name: name};
                var pubUsages = usages.filter(function(u){return u==='verify';});
                var privUsages = usages.filter(function(u){return u==='sign';});
                var pubKey = mkKey('public', algoObj, true, pubUsages, {publicKeyBytes: vkBytes});
                var privKey = mkKey('private', algoObj, extractable, privUsages, {privateKeyBytes: Array.from(new Uint8Array(seed)), publicKeyBytes: vkBytes});
                return Promise.resolve({publicKey: pubKey, privateKey: privKey});
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
