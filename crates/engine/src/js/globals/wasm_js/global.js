// WebAssembly.Global wrapper

var Global = (function() {
    function Global(descriptorOrId, value) {
        if (!(this instanceof Global)) {
            throw new TypeError("WebAssembly.Global must be called with new");
        }
        if (typeof descriptorOrId === 'number') {
            // Internal: wrap existing global by ID
            Object.defineProperty(this, '__global_id', { value: descriptorOrId, writable: false, enumerable: false });
            Object.defineProperty(this, '__valtype', { value: 'unknown', writable: true, enumerable: false });
            // Read type info
            var info = JSON.parse(__braille_wasm_global_type(descriptorOrId));
            this.__valtype = info.value || 'i32';
            return;
        }
        var descriptor = descriptorOrId;
        var valtype = descriptor.value || 'i32';
        var mutable = descriptor.mutable || false;
        var numValue = (value === undefined) ? 0 : (typeof value === 'bigint' ? Number(value) : Number(value));
        var id = __braille_wasm_global_new(valtype, mutable, numValue);
        if (id < 0) {
            throw new TypeError("failed to create WebAssembly.Global");
        }
        Object.defineProperty(this, '__global_id', { value: id, writable: false, enumerable: false });
        Object.defineProperty(this, '__valtype', { value: valtype, writable: false, enumerable: false });
    }

    Object.defineProperty(Global.prototype, 'value', {
        get: function() {
            var raw = __braille_wasm_global_get(this.__global_id);
            if (this.__valtype === 'i64') return BigInt(Math.trunc(raw));
            if (this.__valtype === 'i32') return raw | 0;
            return raw;
        },
        set: function(v) {
            var numVal = typeof v === 'bigint' ? Number(v) : Number(v);
            __braille_wasm_global_set(this.__global_id, numVal);
        },
        enumerable: true,
        configurable: true
    });

    Global.prototype.valueOf = function() {
        return this.value;
    };

    return Global;
})();
