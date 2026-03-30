        encrypt: function(algo, key, data) {
            var a = normalizeAlgo(algo);
            if (key.usages.indexOf('encrypt') === -1 && key.usages.indexOf('wrapKey') === -1) {
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
            if (key.usages.indexOf('decrypt') === -1 && key.usages.indexOf('unwrapKey') === -1) {
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
