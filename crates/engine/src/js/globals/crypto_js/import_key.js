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
                    var privNeedsUsages = {'ECDSA':1,'ECDH':1,'Ed25519':1,'Ed448':1,'X25519':1,'X448':1,'RSA-OAEP':1,'RSA-PSS':1,'RSASSA-PKCS1-v1_5':1,'ML-KEM-512':1,'ML-KEM-768':1,'ML-KEM-1024':1,'ML-DSA-44':1,'ML-DSA-65':1,'ML-DSA-87':1};
                    if (privNeedsUsages[name]) {
                        return Promise.reject(new DOMException('usages cannot be empty for private keys', 'SyntaxError'));
                    }
                }
            }
            // Parse JWK once for all validation steps
            var _jwk = (format === 'jwk') ? (typeof keyData === 'string' ? JSON.parse(keyData) : keyData) : null;
            if (_jwk && _jwk.d && usages && usages.length === 0) {
                var privNeedsUsages2 = {'ECDSA':1,'ECDH':1,'Ed25519':1,'Ed448':1,'X25519':1,'X448':1,'RSA-OAEP':1,'RSA-PSS':1,'RSASSA-PKCS1-v1_5':1};
                if (privNeedsUsages2[name]) {
                    return Promise.reject(new DOMException('usages cannot be empty for private keys', 'SyntaxError'));
                }
            }

            // Validate usages against valid operations for the algorithm + key type
            if (usages && usages.length > 0) {
                var isPrivFormat = (format === 'pkcs8' || format === 'raw-seed');
                var isPubFormat = (format === 'spki' || format === 'raw' || format === 'raw-public');
                var isJwkPriv = _jwk && !!_jwk.d;
                // Valid usages per algorithm for public vs private
                var pubUsagesMap = {
                    'ECDH':[],'ECDSA':['verify'],'Ed25519':['verify'],'Ed448':['verify'],
                    'X25519':[],'X448':[],
                    'RSA-OAEP':['encrypt','wrapKey'],'RSA-PSS':['verify'],'RSASSA-PKCS1-v1_5':['verify'],
                    'ML-DSA-44':['verify'],'ML-DSA-65':['verify'],'ML-DSA-87':['verify']
                };
                var privUsagesMap = {
                    'ECDH':['deriveKey','deriveBits'],'ECDSA':['sign'],'Ed25519':['sign'],'Ed448':['sign'],
                    'X25519':['deriveKey','deriveBits'],'X448':['deriveKey','deriveBits'],
                    'RSA-OAEP':['decrypt','unwrapKey'],'RSA-PSS':['sign'],'RSASSA-PKCS1-v1_5':['sign'],
                    'ML-KEM-512':['decapsulateBits','decapsulateKey'],'ML-KEM-768':['decapsulateBits','decapsulateKey'],'ML-KEM-1024':['decapsulateBits','decapsulateKey'],
                    'ML-DSA-44':['sign'],'ML-DSA-65':['sign'],'ML-DSA-87':['sign']
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
                if (name === 'ML-DSA-44' || name === 'ML-DSA-65' || name === 'ML-DSA-87') {
                    var imported = __braille_crypto_mldsa_pkcs8_import(name, Array.from(derBytes));
                    if (imported.length === 0) {
                        return Promise.reject(new DOMException('invalid ML-DSA PKCS8 key data', 'DataError'));
                    }
                    return Promise.resolve(mkKey('private', {name: name}, extractable, usages, {privateKeyBytes: imported[0], publicKeyBytes: imported[1]}));
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
                if (name === 'ML-DSA-44' || name === 'ML-DSA-65' || name === 'ML-DSA-87') {
                    var vkBytes = __braille_crypto_mldsa_spki_import(name, Array.from(derBytes));
                    if (vkBytes.length === 0) {
                        return Promise.reject(new DOMException('invalid ML-DSA SPKI key data', 'DataError'));
                    }
                    return Promise.resolve(mkKey('public', {name: name}, extractable, usages, {publicKeyBytes: vkBytes}));
                }
                return Promise.reject(new DOMException('importKey spki for ' + name + ' not supported', 'NotSupportedError'));
            }

            // JWK import
            if (format === 'jwk') {
                var jwk = _jwk;
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
                        if (b64urlDecode(jwk.x).length !== okpKeySize) {
                            return Promise.reject(new DOMException(name + ' JWK x has incorrect size', 'DataError'));
                        }
                        if (jwk.d && b64urlDecode(jwk.d).length !== okpKeySize) {
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
                    var pubBytes = Array.from(b64urlDecode(jwk.x));
                    var okpJwkAlg = jwk.alg || null;
                    if (jwk.d) {
                        var privBytes = Array.from(b64urlDecode(jwk.d));
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
