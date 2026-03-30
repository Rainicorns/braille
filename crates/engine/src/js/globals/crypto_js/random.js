    globalThis.crypto = {
        subtle: subtle,
        getRandomValues: function(arr) {
            if (arr instanceof Float32Array || arr instanceof Float64Array || (typeof Float16Array !== 'undefined' && arr instanceof Float16Array) || arr instanceof DataView) {
                throw new DOMException('The provided ArrayBufferView is not an integer typed array', 'TypeMismatchError');
            }
            if (arr.byteLength > 65536) {
                throw new DOMException('The ArrayBufferView\'s byte length exceeds the number of bytes of entropy available via this API (65536 bytes).', 'QuotaExceededError');
            }
            // Fill underlying buffer with random bytes, then let the typed array view interpret them
            var buf = new Uint8Array(arr.buffer, arr.byteOffset, arr.byteLength);
            var bytes = __braille_crypto_get_random_bytes(buf.length);
            for (var i = 0; i < buf.length; i++) buf[i] = bytes[i];
            return arr;
        },
        randomUUID: function() {
            var b = __braille_crypto_get_random_bytes(16);
            b[6] = (b[6] & 0x0f) | 0x40;
            b[8] = (b[8] & 0x3f) | 0x80;
            var h = ''; for (var i=0;i<16;i++) h += (b[i]<16?'0':'') + b[i].toString(16);
            return h.slice(0,8)+'-'+h.slice(8,12)+'-'+h.slice(12,16)+'-'+h.slice(16,20)+'-'+h.slice(20);
        }
    };
