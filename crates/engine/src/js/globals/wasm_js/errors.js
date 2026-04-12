// WebAssembly error types
// These extend Error and are used by the spec for typed error checking.

var CompileError = (function() {
    function CompileError(message) {
        var err = new Error(message);
        err.name = 'CompileError';
        Object.setPrototypeOf(err, CompileError.prototype);
        return err;
    }
    CompileError.prototype = Object.create(Error.prototype);
    CompileError.prototype.constructor = CompileError;
    CompileError.prototype.name = 'CompileError';
    return CompileError;
})();

var LinkError = (function() {
    function LinkError(message) {
        var err = new Error(message);
        err.name = 'LinkError';
        Object.setPrototypeOf(err, LinkError.prototype);
        return err;
    }
    LinkError.prototype = Object.create(Error.prototype);
    LinkError.prototype.constructor = LinkError;
    LinkError.prototype.name = 'LinkError';
    return LinkError;
})();

var RuntimeError = (function() {
    function RuntimeError(message) {
        var err = new Error(message);
        err.name = 'RuntimeError';
        Object.setPrototypeOf(err, RuntimeError.prototype);
        return err;
    }
    RuntimeError.prototype = Object.create(Error.prototype);
    RuntimeError.prototype.constructor = RuntimeError;
    RuntimeError.prototype.name = 'RuntimeError';
    return RuntimeError;
})();
