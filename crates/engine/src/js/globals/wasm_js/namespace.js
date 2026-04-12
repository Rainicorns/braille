// WebAssembly namespace — ties everything together

function _toBytes(bufferSource) {
    var view;
    if (bufferSource instanceof ArrayBuffer) {
        view = new Uint8Array(bufferSource);
    } else if (ArrayBuffer.isView(bufferSource)) {
        view = new Uint8Array(bufferSource.buffer, bufferSource.byteOffset, bufferSource.byteLength);
    } else {
        throw new TypeError("expected ArrayBuffer or typed array");
    }
    // Convert to plain array for Rust Vec<u8> compatibility
    var arr = [];
    for (var i = 0; i < view.length; i++) arr.push(view[i]);
    return arr;
}

var WebAssembly = {
    CompileError: CompileError,
    LinkError: LinkError,
    RuntimeError: RuntimeError,
    Module: Module,
    Instance: Instance,
    Memory: Memory,
    Table: Table,
    Global: Global,

    validate: function(bytes) {
        var data = _toBytes(bytes);
        return __braille_wasm_validate(data);
    },

    compile: function(bytes) {
        return new Promise(function(resolve, reject) {
            var data = _toBytes(bytes);
            var id = __braille_wasm_compile(data);
            if (id < 0) {
                reject(new CompileError(__braille_wasm_last_error()));
            } else {
                // Create Module wrapper without re-compiling (cache hit is fast but avoid the overhead)
                var m = Object.create(Module.prototype);
                Object.defineProperty(m, '__module_id', { value: id, writable: false, enumerable: false });
                resolve(m);
            }
        });
    },

    instantiate: function(moduleOrBytes, importObject) {
        if (moduleOrBytes instanceof Module) {
            // instantiate(module, imports) -> Promise<Instance>
            return new Promise(function(resolve, reject) {
                try {
                    var inst = new Instance(moduleOrBytes, importObject);
                    resolve(inst);
                } catch(e) {
                    reject(e);
                }
            });
        }
        // instantiate(bytes, imports) -> Promise<{module, instance}>
        return WebAssembly.compile(moduleOrBytes).then(function(mod) {
            return new Promise(function(resolve, reject) {
                try {
                    var inst = new Instance(mod, importObject);
                    resolve({ module: mod, instance: inst });
                } catch(e) {
                    reject(e);
                }
            });
        });
    }
};

// Make WebAssembly globally available
globalThis.WebAssembly = WebAssembly;
