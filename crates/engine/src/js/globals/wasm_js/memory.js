// WebAssembly.Memory wrapper

var Memory = (function() {
    function Memory(descriptorOrId) {
        if (!(this instanceof Memory)) {
            throw new TypeError("WebAssembly.Memory must be called with new");
        }
        if (typeof descriptorOrId === 'number') {
            // Internal: wrap existing memory by ID
            Object.defineProperty(this, '__mem_id', { value: descriptorOrId, writable: false, enumerable: false });
            Object.defineProperty(this, '__buffer', { value: null, writable: true, enumerable: false });
            return;
        }
        var descriptor = descriptorOrId;
        var initial = descriptor.initial;
        if (initial === undefined) {
            throw new TypeError("initial is required");
        }
        var maximum = descriptor.maximum !== undefined ? descriptor.maximum : -1;
        var id = __braille_wasm_memory_new(initial, maximum);
        if (id < 0) {
            throw new RangeError("failed to create WebAssembly.Memory");
        }
        Object.defineProperty(this, '__mem_id', { value: id, writable: false, enumerable: false });
        Object.defineProperty(this, '__buffer', { value: null, writable: true, enumerable: false });
    }

    Object.defineProperty(Memory.prototype, 'buffer', {
        get: function() {
            // Create a new ArrayBuffer backed by a copy of the memory data.
            // Not zero-copy yet — good enough for correctness, optimize later.
            var byteLength = __braille_wasm_memory_byte_length(this.__mem_id);
            var data = __braille_wasm_memory_read(this.__mem_id, 0, byteLength);
            var ab = new ArrayBuffer(byteLength);
            var view = new Uint8Array(ab);
            // data is returned as a Uint8Array-like from Rust Vec<u8>
            if (data && data.length) {
                for (var i = 0; i < data.length; i++) {
                    view[i] = data[i];
                }
            }
            return ab;
        },
        enumerable: true,
        configurable: true
    });

    Memory.prototype.grow = function(delta) {
        var oldPages = __braille_wasm_memory_grow(this.__mem_id, delta);
        if (oldPages < 0) {
            throw new RangeError("failed to grow memory");
        }
        this.__buffer = null;
        return oldPages;
    };

    return Memory;
})();
