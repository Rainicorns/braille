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
