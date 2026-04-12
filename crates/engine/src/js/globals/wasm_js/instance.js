// WebAssembly.Instance wrapper

// Global array for import function callbacks (used by Rust import bridge)
var __braille_wasm_import_fns = [];
var __braille_wasm_next_fn_id = 0;

function _resolveImports(module, importObject) {
    if (!importObject) return '[]';
    var moduleImports = JSON.parse(__braille_wasm_module_imports(module.__module_id));
    var resolved = [];
    for (var i = 0; i < moduleImports.length; i++) {
        var imp = moduleImports[i];
        var modObj = importObject[imp.module];
        if (!modObj) {
            throw new LinkError('import object field "' + imp.module + '" is not an object');
        }
        var val = modObj[imp.name];
        if (val === undefined) {
            throw new LinkError('import object does not provide "' + imp.module + '"."' + imp.name + '"');
        }
        var desc = { module: imp.module, name: imp.name };
        if (imp.kind === 'function') {
            if (typeof val !== 'function') {
                throw new LinkError('import "' + imp.module + '"."' + imp.name + '" is not a function');
            }
            var fn_id = __braille_wasm_next_fn_id++;
            __braille_wasm_import_fns[fn_id] = val;
            desc.fn_id = fn_id;
        } else if (imp.kind === 'memory') {
            if (!(val instanceof Memory)) {
                throw new LinkError('import "' + imp.module + '"."' + imp.name + '" is not a Memory');
            }
            desc.mem_id = val.__mem_id;
        } else if (imp.kind === 'table') {
            if (!(val instanceof Table)) {
                throw new LinkError('import "' + imp.module + '"."' + imp.name + '" is not a Table');
            }
            desc.table_id = val.__table_id;
        } else if (imp.kind === 'global') {
            if (val instanceof Global) {
                desc.global_id = val.__global_id;
            } else {
                // Numeric value — create a const global from it
                var valtype = imp.valtype || 'i32';
                var numVal = typeof val === 'bigint' ? Number(val) : (typeof val === 'number' ? val : 0);
                var gid = __braille_wasm_global_new(valtype, false, numVal);
                if (gid < 0) {
                    throw new LinkError('failed to create global for import "' + imp.module + '"."' + imp.name + '"');
                }
                desc.global_id = gid;
            }
        }
        resolved.push(desc);
    }
    return JSON.stringify(resolved);
}

var Instance = (function() {
    function Instance(module, importObject) {
        if (!(this instanceof Instance)) {
            throw new TypeError("WebAssembly.Instance must be called with new");
        }
        if (!(module instanceof Module)) {
            throw new TypeError("first argument must be a WebAssembly.Module");
        }
        var importsJson = _resolveImports(module, importObject);
        var id = __braille_wasm_instantiate(module.__module_id, importsJson);
        if (id < 0) {
            var err = __braille_wasm_last_error();
            // Determine error type based on message
            if (err.indexOf('incompatible import type') >= 0 || err.indexOf('unknown import') >= 0) {
                throw new LinkError(err);
            }
            throw new RuntimeError(err);
        }
        Object.defineProperty(this, '__instance_id', { value: id, writable: false, enumerable: false });

        // Build exports object
        var exportIds = JSON.parse(__braille_wasm_instance_export_ids(id));
        var exportsObj = Object.create(null);
        for (var name in exportIds) {
            var info = exportIds[name];
            if (info.kind === 'function') {
                (function(exportName) {
                    exportsObj[exportName] = function() {
                        var args = [];
                        for (var i = 0; i < arguments.length; i++) {
                            var a = arguments[i];
                            var type = 'f64';
                            var value = 0;
                            if (typeof a === 'bigint') {
                                type = 'i64';
                                value = Number(a);
                            } else if (typeof a === 'number') {
                                value = a;
                            }
                            args.push({type: type, value: value});
                        }
                        return __braille_wasm_call(id, exportName, JSON.stringify(args));
                    };
                })(name);
            } else if (info.kind === 'memory') {
                exportsObj[name] = new Memory(info.id);
            } else if (info.kind === 'table') {
                exportsObj[name] = new Table(info.id);
            } else if (info.kind === 'global') {
                exportsObj[name] = new Global(info.id);
            }
        }
        Object.freeze(exportsObj);
        Object.defineProperty(this, 'exports', { value: exportsObj, writable: false, enumerable: true });
    }
    return Instance;
})();
