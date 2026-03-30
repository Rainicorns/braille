    // SubtleCrypto.supports() static method
    var supportedOps = {
        'RSASSA-PKCS1-v1_5': {'generateKey':1,'importKey':1,'sign':1,'verify':1},
        'RSA-PSS': {'generateKey':1,'importKey':1,'sign':1,'verify':1},
        'RSA-OAEP': {'generateKey':1,'importKey':1,'encrypt':1,'decrypt':1},
        'ECDSA': {'generateKey':1,'importKey':1,'sign':1,'verify':1},
        'ECDH': {'generateKey':1,'importKey':1,'deriveBits':1},
        'Ed25519': {'generateKey':1,'importKey':1,'sign':1,'verify':1},
        'Ed448': {'generateKey':1,'importKey':1,'sign':1,'verify':1},
        'X25519': {'generateKey':1,'importKey':1,'deriveBits':1},
        'X448': {'generateKey':1,'importKey':1,'deriveBits':1},
        'AES-CBC': {'generateKey':1,'importKey':1,'encrypt':1,'decrypt':1},
        'AES-CTR': {'generateKey':1,'importKey':1,'encrypt':1,'decrypt':1},
        'AES-GCM': {'generateKey':1,'importKey':1,'encrypt':1,'decrypt':1},
        'AES-KW': {'generateKey':1,'importKey':1},
        'HMAC': {'generateKey':1,'importKey':1,'sign':1,'verify':1},
        'SHA-1': {'digest':1},
        'SHA-256': {'digest':1},
        'SHA-384': {'digest':1},
        'SHA-512': {'digest':1},
        'HKDF': {'importKey':1,'deriveBits':1},
        'PBKDF2': {'importKey':1,'deriveBits':1},
        'KMAC128': {'generateKey':1,'importKey':1,'sign':1,'verify':1},
        'KMAC256': {'generateKey':1,'importKey':1,'sign':1,'verify':1},
        'ML-KEM-512': {'generateKey':1,'importKey':1},
        'ML-KEM-768': {'generateKey':1,'importKey':1},
        'ML-KEM-1024': {'generateKey':1,'importKey':1},
        'ML-DSA-44': {'generateKey':1,'importKey':1,'sign':1,'verify':1},
        'ML-DSA-65': {'generateKey':1,'importKey':1,'sign':1,'verify':1},
        'ML-DSA-87': {'generateKey':1,'importKey':1,'sign':1,'verify':1}
    };
    var validOps = {'generateKey':1,'importKey':1,'sign':1,'verify':1,'encrypt':1,'decrypt':1,'deriveBits':1,'digest':1};
    var validAesLengths = {'128':1,'192':1,'256':1};
    var validHashes = {'SHA-1':1,'SHA-256':1,'SHA-384':1,'SHA-512':1};
    function SubtleCrypto() {}
    SubtleCrypto.supports = function(operation, algorithm) {
        if (!validOps[operation]) return false;
        var name;
        if (typeof algorithm === 'string') {
            name = normalizeAlgo({name: algorithm}).name;
        } else if (algorithm && typeof algorithm === 'object') {
            name = normalizeAlgo(algorithm).name;
            // Validate algorithm-specific parameters
            if (name.substring(0,3) === 'AES' && algorithm.length !== undefined) {
                if (!validAesLengths[String(algorithm.length)]) return false;
            }
            if (name === 'HMAC' && algorithm.hash !== undefined) {
                var h = typeof algorithm.hash === 'string' ? algorithm.hash : (algorithm.hash && algorithm.hash.name);
                if (h && !validHashes[h]) return false;
            }
        } else {
            return false;
        }
        var ops = supportedOps[name];
        if (!ops) return false;
        return !!ops[operation];
    };
    globalThis.SubtleCrypto = SubtleCrypto;
