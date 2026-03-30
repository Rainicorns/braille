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
                var wa = normalizeAlgo(wrapAlgorithm);
                if (wa.name === 'AES-KW') {
                    var result = __braille_crypto_aes_kw_wrap(wrappingKey._raw, Array.from(toBytes(keyData)));
                    return Promise.resolve(new Uint8Array(result).buffer);
                }
                return subtle.encrypt(wrapAlgorithm, wrappingKey, keyData);
            });
        },

        unwrapKey: function(format, wrappedKey, unwrappingKey, unwrapAlgorithm, unwrappedKeyAlgorithm, extractable, keyUsages) {
            var ua = normalizeAlgo(unwrapAlgorithm);
            var decryptPromise;
            if (ua.name === 'AES-KW') {
                var result = __braille_crypto_aes_kw_unwrap(unwrappingKey._raw, Array.from(toBytes(wrappedKey)));
                if (result[0][0] === 0) {
                    return Promise.reject(new DOMException('AES-KW unwrap failed', 'OperationError'));
                }
                decryptPromise = Promise.resolve(new Uint8Array(result[1]).buffer);
            } else {
                decryptPromise = subtle.decrypt(unwrapAlgorithm, unwrappingKey, wrappedKey);
            }
            return decryptPromise.then(function(keyData) {
                if (format === 'jwk') {
                    var dec = new TextDecoder();
                    keyData = JSON.parse(dec.decode(keyData));
                }
                return subtle.importKey(format, keyData, unwrappedKeyAlgorithm, extractable, keyUsages);
            });
        },
