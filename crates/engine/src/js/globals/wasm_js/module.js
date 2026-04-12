// WebAssembly.Module wrapper

var Module = (function() {
    function Module(bufferSource) {
        if (!(this instanceof Module)) {
            throw new TypeError("WebAssembly.Module must be called with new");
        }
        var bytes = _toBytes(bufferSource);
        var id = __braille_wasm_compile(bytes);
        if (id < 0) {
            throw new CompileError(__braille_wasm_last_error());
        }
        Object.defineProperty(this, '__module_id', { value: id, writable: false, enumerable: false });
    }

    Module.exports = function(module) {
        if (!(module instanceof Module)) {
            throw new TypeError("WebAssembly.Module.exports requires a Module");
        }
        return JSON.parse(__braille_wasm_module_exports(module.__module_id));
    };

    Module.imports = function(module) {
        if (!(module instanceof Module)) {
            throw new TypeError("WebAssembly.Module.imports requires a Module");
        }
        return JSON.parse(__braille_wasm_module_imports(module.__module_id));
    };

    return Module;
})();
