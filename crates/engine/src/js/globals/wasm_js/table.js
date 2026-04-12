// WebAssembly.Table wrapper

var Table = (function() {
    function Table(descriptorOrId) {
        if (!(this instanceof Table)) {
            throw new TypeError("WebAssembly.Table must be called with new");
        }
        if (typeof descriptorOrId === 'number') {
            // Internal: wrap existing table by ID
            Object.defineProperty(this, '__table_id', { value: descriptorOrId, writable: false, enumerable: false });
            return;
        }
        var descriptor = descriptorOrId;
        var element = descriptor.element;
        if (!element) {
            throw new TypeError("element is required");
        }
        var initial = descriptor.initial;
        if (initial === undefined) {
            throw new TypeError("initial is required");
        }
        var maximum = descriptor.maximum !== undefined ? descriptor.maximum : -1;
        var id = __braille_wasm_table_new(element, initial, maximum);
        if (id < 0) {
            throw new RangeError("failed to create WebAssembly.Table");
        }
        Object.defineProperty(this, '__table_id', { value: id, writable: false, enumerable: false });
    }

    Object.defineProperty(Table.prototype, 'length', {
        get: function() {
            return __braille_wasm_table_size(this.__table_id);
        },
        enumerable: true,
        configurable: true
    });

    Table.prototype.get = function(index) {
        // Stub — returns null for now
        return null;
    };

    Table.prototype.set = function(index, value) {
        // Stub
    };

    Table.prototype.grow = function(delta) {
        var element = 'funcref';
        var oldSize = __braille_wasm_table_grow(this.__table_id, delta, element);
        if (oldSize < 0) {
            throw new RangeError("failed to grow table");
        }
        return oldSize;
    };

    return Table;
})();
