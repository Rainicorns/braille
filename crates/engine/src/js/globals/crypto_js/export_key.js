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
                    // Per RFC 8037, Ed25519/Ed448 JWK alg is "EdDSA"
                    if (key.algorithm.name === 'Ed25519' || key.algorithm.name === 'Ed448') {
                        jwk.alg = 'EdDSA';
                    }
                    if (key.type === 'private' && key._privateKeyBytes) {
                        jwk.d = b64url(key._privateKeyBytes);
                    }
                    return Promise.resolve(jwk);
                }
            }
            return Promise.reject(new DOMException('exportKey format ' + format + ' not supported', 'NotSupportedError'));
        },
