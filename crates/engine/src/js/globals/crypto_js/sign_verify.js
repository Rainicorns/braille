        sign: function(algo, key, data) {
            var a = normalizeAlgo(algo);
            if (!key.usages || key.usages.indexOf('sign') < 0) {
                return Promise.reject(new DOMException('key usages do not include "sign"', 'InvalidAccessError'));
            }
            if (key.algorithm && key.algorithm.name !== a.name) {
                return Promise.reject(new DOMException('key algorithm mismatch', 'InvalidAccessError'));
            }
            if (a.name === 'HMAC') {
                var h = hashName(key.algorithm && key.algorithm.hash);
                var result = __braille_crypto_hmac_sign(h, key._raw, Array.from(toBytes(data)));
                return Promise.resolve(new Uint8Array(result).buffer);
            }
            if (a.name === 'Ed25519' || a.name === 'Ed448') {
                if (key.type !== 'private') {
                    return Promise.reject(new DOMException('key type must be private for sign', 'InvalidAccessError'));
                }
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
                if (key.type !== 'private') {
                    return Promise.reject(new DOMException('key type must be private for sign', 'InvalidAccessError'));
                }
                var curve = key.algorithm.namedCurve;
                var h = hashName(a.hash);
                var validHashes = {'SHA-1':1,'SHA-256':1,'SHA-384':1,'SHA-512':1};
                if (!validHashes[h]) {
                    return Promise.reject(new DOMException('Unrecognized hash: ' + h, 'NotSupportedError'));
                }
                var result = __braille_crypto_ecdsa_sign(curve, h, key._privateKeyBytes, Array.from(toBytes(data)));
                return Promise.resolve(new Uint8Array(result).buffer);
            }
            if (a.name === 'RSA-PSS') {
                if (key.type !== 'private') {
                    return Promise.reject(new DOMException('key type must be private for sign', 'InvalidAccessError'));
                }
                var h = key.algorithm.hash.name;
                var validHashes = {'SHA-1':1,'SHA-256':1,'SHA-384':1,'SHA-512':1};
                if (!validHashes[h]) {
                    return Promise.reject(new DOMException('Unrecognized hash: ' + h, 'NotSupportedError'));
                }
                var saltLen = a.saltLength || 0;
                var result = __braille_crypto_rsa_pss_sign(key._privateKeyBytes, h, saltLen, Array.from(toBytes(data)));
                return Promise.resolve(new Uint8Array(result).buffer);
            }
            if (a.name === 'RSASSA-PKCS1-v1_5') {
                if (key.type !== 'private') {
                    return Promise.reject(new DOMException('key type must be private for sign', 'InvalidAccessError'));
                }
                var h = key.algorithm.hash.name;
                var validHashes = {'SHA-1':1,'SHA-256':1,'SHA-384':1,'SHA-512':1};
                if (!validHashes[h]) {
                    return Promise.reject(new DOMException('Unrecognized hash: ' + h, 'NotSupportedError'));
                }
                var result = __braille_crypto_rsa_pkcs1_sign(key._privateKeyBytes, h, Array.from(toBytes(data)));
                return Promise.resolve(new Uint8Array(result).buffer);
            }
            return Promise.reject(new DOMException('sign ' + a.name + ' not supported', 'NotSupportedError'));
        },

        verify: function(algo, key, signature, data) {
            var a = normalizeAlgo(algo);
            if (!key.usages || key.usages.indexOf('verify') < 0) {
                return Promise.reject(new DOMException('key usages do not include "verify"', 'InvalidAccessError'));
            }
            if (key.algorithm && key.algorithm.name !== a.name) {
                return Promise.reject(new DOMException('key algorithm mismatch', 'InvalidAccessError'));
            }
            if (a.name === 'HMAC') {
                var h = hashName(key.algorithm && key.algorithm.hash);
                var ok = __braille_crypto_hmac_verify(h, key._raw, Array.from(toBytes(signature)), Array.from(toBytes(data)));
                return Promise.resolve(ok);
            }
            if (a.name === 'Ed25519' || a.name === 'Ed448') {
                if (key.type !== 'public') {
                    return Promise.reject(new DOMException('key type must be public for verify', 'InvalidAccessError'));
                }
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
                if (key.type !== 'public') {
                    return Promise.reject(new DOMException('key type must be public for verify', 'InvalidAccessError'));
                }
                var curve = key.algorithm.namedCurve;
                var h = hashName(a.hash);
                var validHashes = {'SHA-1':1,'SHA-256':1,'SHA-384':1,'SHA-512':1};
                if (!validHashes[h]) {
                    return Promise.reject(new DOMException('Unrecognized hash: ' + h, 'NotSupportedError'));
                }
                var ok = __braille_crypto_ecdsa_verify(curve, h, key._publicKeyBytes, Array.from(toBytes(signature)), Array.from(toBytes(data)));
                return Promise.resolve(ok);
            }
            if (a.name === 'RSA-PSS') {
                if (key.type !== 'public') {
                    return Promise.reject(new DOMException('key type must be public for verify', 'InvalidAccessError'));
                }
                var h = key.algorithm.hash.name;
                var validHashes = {'SHA-1':1,'SHA-256':1,'SHA-384':1,'SHA-512':1};
                if (!validHashes[h]) {
                    return Promise.reject(new DOMException('Unrecognized hash: ' + h, 'NotSupportedError'));
                }
                var saltLen = a.saltLength || 0;
                var ok = __braille_crypto_rsa_pss_verify(key._publicKeyBytes, h, saltLen, Array.from(toBytes(signature)), Array.from(toBytes(data)));
                return Promise.resolve(ok);
            }
            if (a.name === 'RSASSA-PKCS1-v1_5') {
                if (key.type !== 'public') {
                    return Promise.reject(new DOMException('key type must be public for verify', 'InvalidAccessError'));
                }
                var h = key.algorithm.hash.name;
                var validHashes = {'SHA-1':1,'SHA-256':1,'SHA-384':1,'SHA-512':1};
                if (!validHashes[h]) {
                    return Promise.reject(new DOMException('Unrecognized hash: ' + h, 'NotSupportedError'));
                }
                var ok = __braille_crypto_rsa_pkcs1_verify(key._publicKeyBytes, h, Array.from(toBytes(signature)), Array.from(toBytes(data)));
                return Promise.resolve(ok);
            }
            return Promise.reject(new DOMException('verify ' + a.name + ' not supported', 'NotSupportedError'));
        },
