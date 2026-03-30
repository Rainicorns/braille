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
