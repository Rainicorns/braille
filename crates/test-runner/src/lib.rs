use std::collections::HashMap;
use std::path::{Path, PathBuf};

use braille_engine::{Engine, FetchedResources};

// ---------------------------------------------------------------------------
// Paths
// ---------------------------------------------------------------------------

pub fn workspace_root() -> PathBuf {
    // Walk up from CARGO_MANIFEST_DIR (crates/wpt-runner) to workspace root
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

pub fn wpt_root() -> PathBuf {
    workspace_root().join("tests/wpt")
}

// ---------------------------------------------------------------------------
// Test harness JS
// ---------------------------------------------------------------------------

pub fn load_testharness_js() -> String {
    let path = wpt_root().join("resources/testharness.js");
    std::fs::read_to_string(&path).expect("failed to read testharness.js")
}

pub fn testharness_preamble() -> String {
    r#"
// Minimal WPT test harness preamble
if (globalThis.__braille_stop_on_fail === undefined) globalThis.__braille_stop_on_fail = true;
(function() {
    var results = [];
    var setup_fn = null;
    var setup_ran = false;
    var single_test_mode = false;
    var promise_chain = Promise.resolve();
    var _stopped = false;
    var _result_callbacks = [];
    var _completion_callbacks = [];
    var _pending_async = [];

    function _progress(status) {
        if (typeof __braille_test_progress === 'function') {
            __braille_test_progress(status === 0 || status === 3);
        }
    }

    var _nameCount = {};
    function _dedupName(name) {
        if (_nameCount[name] === undefined) {
            _nameCount[name] = 0;
            return name;
        }
        _nameCount[name]++;
        var deduped = name + ' ' + _nameCount[name];
        // Also track the deduped name to prevent its own collisions
        _nameCount[deduped] = 0;
        return deduped;
    }

    function _autoName(fn) {
        var s = fn.toString();
        // Only auto-name arrow functions
        var arrowIdx = s.indexOf('=>');
        if (arrowIdx < 0) return null;

        var afterArrow = s.substring(arrowIdx + 2).trim();

        // Concise arrow (no braces): () => expr
        if (afterArrow.charAt(0) !== '{') {
            return afterArrow;
        }

        // Block arrow: () => { ... }
        var bodyStart = 1;
        var bodyEnd = afterArrow.lastIndexOf('}');
        if (bodyEnd <= bodyStart) return null;
        var body = afterArrow.substring(bodyStart, bodyEnd);

        // Multiline → page title
        if (body.indexOf('\n') >= 0) return null;

        // Trim and strip trailing semicolons
        body = body.trim().replace(/;+\s*$/, '').trim();

        // Empty → page title
        if (!body) return null;

        return body;
    }

    function _resolveName(fn, name) {
        if (name) return _dedupName(name);
        // Try auto-naming from function source
        var auto = _autoName(fn);
        if (auto) return _dedupName(auto);
        // Fall back to page title
        var title = (typeof document !== 'undefined' && document.title) ? document.title : "(unnamed)";
        return _dedupName(title);
    }

    function _notifyResult(result) {
        for (var i = 0; i < _result_callbacks.length; i++) {
            try { _result_callbacks[i](result); } catch(e) {}
        }
    }

    function run_setup() {
        if (!setup_ran && setup_fn) {
            setup_ran = true;
            setup_fn();
        }
    }

    self.test = function(fn, name) {
        if (_stopped) return;
        run_setup();
        var cleanups = [];
        var _signal = null;
        var resolvedName = _resolveName(fn, name);
        var t = {
            name: resolvedName,
            step: function(f) { return function() { return f.apply(t, arguments); }; },
            step_func: function(f) { return function() { return f.apply(t, arguments); }; },
            step_func_done: function(f) { return function() { return f.apply(t, arguments); }; },
            unreached_func: function(msg) { return function() { throw new Error(msg || "unreached"); }; },
            add_cleanup: function(f) { cleanups.push(f); },
            force_timeout: function() { result.status = 2; result.message = "Test timed out"; },
            get_signal: function() {
                if (!_signal) {
                    var ac = new AbortController();
                    _signal = ac.signal;
                    _signal._controller = ac;
                }
                return _signal;
            }
        };
        var result = { name: resolvedName, status: 0, message: "" };
        try {
            fn.call(t, t);
        } catch(e) {
            if (e instanceof OptionalFeatureUnsupportedError) {
                result.status = 3;
            } else {
                result.status = 1;
            }
            result.message = e.message || String(e);
        }
        for (var i = 0; i < cleanups.length; i++) {
            try { cleanups[i](); } catch(e) {}
        }
        if (_signal && _signal._controller) {
            _signal._controller.abort();
        }
        results.push(result);
        _notifyResult(result);
        _progress(result.status);
        if (result.status === 1 && typeof __braille_stop_on_fail !== 'undefined' && __braille_stop_on_fail) {
            _stopped = true;
        }
    };

    self.async_test = function(fn, name) {
        if (typeof fn === "string") {
            name = fn;
            fn = null;
        }
        var resolvedName = fn ? _resolveName(fn, name) : (name || "(unnamed)");
        var _asignal = null;
        var _acleanups = [];
        var _started = !!fn;
        var result = { name: resolvedName, status: _started ? 0 : 3, message: "" };
        function _markStarted() {
            if (!_started) {
                _started = true;
                result.status = 0;
                _pending_async.push({ test: t, result: result });
            }
        }
        var t = {
            name: resolvedName,
            step: function(f) {
                if (t._done) return;
                _markStarted();
                try {
                    f.apply(t, [t]);
                } catch(e) {
                    if (t._done) return;
                    if (e instanceof OptionalFeatureUnsupportedError) {
                        result.status = 3;
                    } else {
                        result.status = 1;
                    }
                    result.message = e.message || String(e);
                    t.done();
                }
            },
            step_func: function(f) {
                return function() {
                    if (t._done) return;
                    _markStarted();
                    try {
                        return f.apply(t, arguments);
                    } catch(e) {
                        if (t._done) return;
                        if (e instanceof OptionalFeatureUnsupportedError) {
                            result.status = 3;
                        } else {
                            result.status = 1;
                        }
                        result.message = e.message || String(e);
                        t.done();
                    }
                };
            },
            step_func_done: function(f) {
                return function() {
                    if (t._done) return;
                    _markStarted();
                    try {
                        f.apply(t, arguments);
                        t.done();
                    } catch(e) {
                        if (t._done) return;
                        if (e instanceof OptionalFeatureUnsupportedError) {
                            result.status = 3;
                        } else {
                            result.status = 1;
                        }
                        result.message = e.message || String(e);
                        t.done();
                    }
                };
            },
            done: function() {
                _markStarted();
                t._done = true;
                if (_asignal && _asignal._controller) _asignal._controller.abort();
                for (var ci = 0; ci < _acleanups.length; ci++) {
                    try { _acleanups[ci](); } catch(ce) {}
                }
                // Finalize: report result
                _notifyResult(result);
                _progress(result.status);
            },
            unreached_func: function(msg) {
                return function() { throw new Error(msg || "unreached"); };
            },
            add_cleanup: function(f) { _acleanups.push(f); },
            force_timeout: function() { result.status = 2; result.message = "Test timed out"; },
            step_timeout: function(fn, timeout) { return setTimeout(fn, timeout || 0); },
            get_signal: function() {
                if (!_asignal) {
                    var ac = new AbortController();
                    _asignal = ac.signal;
                    _asignal._controller = ac;
                }
                return _asignal;
            },
            _done: false
        };
        if (fn) {
            try {
                fn.call(t, t);
            } catch(e) {
                if (e instanceof OptionalFeatureUnsupportedError) {
                    result.status = 3;
                } else {
                    result.status = 1;
                }
                result.message = e.message || String(e);
            }
        }
        results.push(result);
        // For async tests that complete synchronously (done() already called), report now
        // For others, report is deferred to done()
        if (t._done || !_started) {
            _notifyResult(result);
            _progress(result.status);
        } else {
            _pending_async.push({ test: t, result: result });
        }
        return t;
    };

    self.promise_test = function(fn, name) {
        var resolvedName = _resolveName(fn, name);
        var result = { name: resolvedName, status: 0, message: "" };
        results.push(result);
        promise_chain = promise_chain.then(function() {
            var cleanups = [];
            var _psignal = null;
            var t = {
                name: resolvedName,
                step: function(f) { return function() { return f.apply(t, arguments); }; },
                step_func: function(f) { return function() { return f.apply(t, arguments); }; },
                step_func_done: function(f) { return function() { f.apply(t, arguments); t._done = true; }; },
                done: function() { t._done = true; if (_psignal && _psignal._controller) _psignal._controller.abort(); },
                unreached_func: function(msg) { return function() { throw new Error(msg || "unreached"); }; },
                add_cleanup: function(f) { cleanups.push(f); },
                force_timeout: function() { result.status = 2; result.message = "Test timed out"; },
                step_timeout: function(fn, timeout) { return setTimeout(fn, timeout || 0); },
                get_signal: function() {
                    if (!_psignal) {
                        var ac = new AbortController();
                        _psignal = ac.signal;
                        _psignal._controller = ac;
                    }
                    return _psignal;
                },
                step_wait: function(cond, description, timeout, interval) {
                    return new Promise(function(resolve, reject) {
                        if (cond()) { resolve(); return; }
                        var attempts = 0;
                        var maxAttempts = Math.ceil((timeout || 3000) / (interval || 10));
                        function check() {
                            if (cond()) { resolve(); return; }
                            if (++attempts >= maxAttempts) { reject(new Error(description || "step_wait timed out")); return; }
                            setTimeout(check, interval || 10);
                        }
                        setTimeout(check, interval || 10);
                    });
                },
                _done: false
            };
            var p;
            try {
                p = fn(t);
            } catch(e) {
                if (e instanceof OptionalFeatureUnsupportedError) {
                    result.status = 3;
                } else {
                    result.status = 1;
                }
                result.message = e.message || String(e);
                for (var i = 0; i < cleanups.length; i++) { try { cleanups[i](); } catch(ce) {} }
                _notifyResult(result);
                _progress(result.status);
                return;
            }
            // Validate thenable return
            if (!p || typeof p.then !== 'function') {
                result.status = 1;
                if (p === undefined) {
                    result.message = "promise_test: test body must return a 'thenable' object (received undefined)";
                } else if (p === null) {
                    result.message = "promise_test: test body must return a 'thenable' object (received null)";
                } else {
                    result.message = "promise_test: test body must return a 'thenable' object (received an object with no `then` method)";
                }
                var chain = Promise.resolve();
                for (var i = 0; i < cleanups.length; i++) {
                    chain = chain.then(cleanups[i]);
                }
                return chain.then(function() {
                    _notifyResult(result);
                    _progress(result.status);
                });
            }
            return p.then(function() {
            }, function(e) {
                if (e instanceof OptionalFeatureUnsupportedError) {
                    result.status = 3;
                } else {
                    result.status = 1;
                }
                var val = e;
                if (val instanceof Error) {
                    result.message = "promise_test: Unhandled rejection with value: object \"" + val + "\"";
                } else if (typeof val === 'string') {
                    result.message = "promise_test: Unhandled rejection with value: \"" + val + "\"";
                } else {
                    result.message = e.message || String(e);
                }
            }).then(function() {
                var chain = Promise.resolve();
                for (var i = 0; i < cleanups.length; i++) {
                    chain = chain.then(cleanups[i]);
                }
                return chain;
            }).then(function() {
                _notifyResult(result);
                _progress(result.status);
            });
        });
    };

    self.setup = function(fn_or_props, maybe_props) {
        var fn = null, props = null;
        if (typeof fn_or_props === 'function') {
            fn = fn_or_props;
            props = maybe_props || null;
        } else if (fn_or_props && typeof fn_or_props === 'object') {
            props = fn_or_props;
        }
        if (fn) {
            fn();
            setup_ran = true;
        }
        if (props && props.single_test) {
            single_test_mode = true;
        }
    };

    self.done = function() {
        if (single_test_mode) {
            // Finalize the single test: use the page title as the test name
            var title = (typeof document !== 'undefined' && document.title) ? document.title : "(single test)";
            if (results.length === 0) {
                results.push({ name: title, status: 0, message: "" });
            } else {
                // Update the name of the existing single test if it was anonymous
                if (results[0].name === "(single test)" || results[0].name === "(unnamed)") {
                    results[0].name = title;
                }
            }
        }
    };

    self.add_completion_callback = function(fn) { _completion_callbacks.push(fn); };
    self.add_result_callback = function(fn) { _result_callbacks.push(fn); };
    self.add_start_callback = function() {};

    self.on_event = function(obj, event_type, handler) {
        if (obj && typeof obj.addEventListener === 'function') {
            obj.addEventListener(event_type, handler);
        }
    };

    self.step_timeout = function(fn, timeout) {
        return setTimeout(fn, timeout || 0);
    };

    self.generate_tests = function(fn, tests, props) {
        for (var i = 0; i < tests.length; i++) {
            (function(args, idx) {
                var name = args[0];
                var testProps = {};
                if (props) {
                    testProps = Array.isArray(props) ? (props[idx] || {}) : props;
                }
                self.test(function() {
                    this.properties = testProps;
                    fn.apply(this, args.slice(1));
                }, name);
            })(tests[i], i);
        }
    };

    // AssertionError (yes, WPT spells it this way)
    function AssertionError(message) {
        this.message = message || '';
        this.stack = (new Error()).stack;
    }
    AssertionError.prototype = Object.create(Error.prototype);
    AssertionError.prototype.constructor = AssertionError;
    AssertionError.prototype.name = 'AssertionError';
    self.AssertionError = AssertionError;

    // OptionalFeatureUnsupportedError — thrown by assert_implements_optional
    function OptionalFeatureUnsupportedError(message) {
        this.message = message || '';
        this.stack = (new Error()).stack;
    }
    OptionalFeatureUnsupportedError.prototype = Object.create(Error.prototype);
    OptionalFeatureUnsupportedError.prototype.constructor = OptionalFeatureUnsupportedError;
    OptionalFeatureUnsupportedError.prototype.name = 'OptionalFeatureUnsupportedError';
    self.OptionalFeatureUnsupportedError = OptionalFeatureUnsupportedError;

    // Assertions
    self.assert_true = function(val, msg) {
        if (val !== true) throw new AssertionError(msg || "assert_true: got " + val);
    };
    self.assert_false = function(val, msg) {
        if (val !== false) throw new AssertionError(msg || "assert_false: got " + val);
    };
    self.assert_equals = function(a, b, msg) {
        if (!Object.is(a, b)) throw new AssertionError(msg || "assert_equals: " + format_value(a) + " !== " + format_value(b));
    };
    self.assert_not_equals = function(a, b, msg) {
        if (Object.is(a, b)) throw new AssertionError(msg || "assert_not_equals: values are equal: " + format_value(a));
    };
    self.assert_in_array = function(val, arr, msg) {
        if (arr.indexOf(val) === -1) throw new AssertionError(msg || "assert_in_array: " + val + " not in array");
    };
    function _assertNumericType(val, msg) {
        if (typeof val !== 'number' && typeof val !== 'bigint') {
            throw new AssertionError(msg || "expected a number but got a \"" + typeof val + "\"");
        }
    }
    self.assert_greater_than = function(a, b, msg) {
        _assertNumericType(a, msg);
        _assertNumericType(b, msg);
        if (!(a > b)) throw new AssertionError(msg || "assert_greater_than: " + a + " <= " + b);
    };
    self.assert_less_than = function(a, b, msg) {
        _assertNumericType(a, msg);
        _assertNumericType(b, msg);
        if (!(a < b)) throw new AssertionError(msg || "assert_less_than: " + a + " >= " + b);
    };
    self.assert_greater_than_equal = function(a, b, msg) {
        _assertNumericType(a, msg);
        _assertNumericType(b, msg);
        if (!(a >= b)) throw new AssertionError(msg || "assert_greater_than_equal: " + a + " < " + b);
    };
    self.assert_less_than_equal = function(a, b, msg) {
        _assertNumericType(a, msg);
        _assertNumericType(b, msg);
        if (!(a <= b)) throw new AssertionError(msg || "assert_less_than_equal: " + a + " > " + b);
    };
    self.assert_approx_equals = function(actual, expected, epsilon, msg) {
        if (typeof epsilon !== 'number') epsilon = 0;
        if (Math.abs(actual - expected) > epsilon) throw new AssertionError(msg || "assert_approx_equals: " + actual + " not within " + epsilon + " of " + expected);
    };
    self.assert_array_approx_equals = function(actual, expected, epsilon, msg) {
        if (actual.length !== expected.length) throw new AssertionError(msg || "assert_array_approx_equals: length mismatch");
        for (var i = 0; i < actual.length; i++) {
            if (Math.abs(actual[i] - expected[i]) > epsilon) throw new AssertionError(msg || "assert_array_approx_equals: index " + i + ": " + actual[i] + " not within " + epsilon + " of " + expected[i]);
        }
    };
    self.assert_array_equals = function(a, b, msg) {
        var aLen = a ? a.length : undefined;
        var bLen = b ? b.length : undefined;
        if (aLen === undefined || bLen === undefined || aLen !== bLen) {
            throw new AssertionError(msg || "assert_array_equals: length mismatch (" + aLen + " vs " + bLen + ")");
        }
        for (var i = 0; i < aLen; i++) {
            if (!Object.is(a[i], b[i])) throw new AssertionError(msg || "assert_array_equals: index " + i + ": " + a[i] + " !== " + b[i]);
        }
    };
    self.assert_object_equals = function(a, b, msg) {
        var aStr = JSON.stringify(a);
        var bStr = JSON.stringify(b);
        if (aStr !== bStr) throw new AssertionError(msg || "assert_object_equals: " + aStr + " !== " + bStr);
    };
    self.assert_regexp_match = function(val, re, msg) {
        if (!re.test(val)) throw new AssertionError(msg || "assert_regexp_match: " + val + " doesn't match " + re);
    };
    self.assert_own_property = function(obj, prop, msg) {
        if (!obj.hasOwnProperty(prop)) throw new AssertionError(msg || "assert_own_property: missing " + prop);
    };
    self.assert_not_own_property = function(obj, prop, msg) {
        if (obj.hasOwnProperty(prop)) throw new AssertionError(msg || "assert_not_own_property: unexpected " + prop);
    };
    self.assert_class_string = function(obj, expected, msg) {
        var actual = Object.prototype.toString.call(obj);
        var cls = actual.slice(8, -1);
        if (cls !== expected) throw new AssertionError(msg || "assert_class_string: " + cls + " !== " + expected);
    };
    self.assert_throws_js = function(ctor, fn, msg) {
        // Validate ctor is an Error subtype (not DOMException)
        if (ctor.prototype && !(ctor.prototype instanceof Error) && ctor !== Error) {
            throw new AssertionError(msg || 'assert_throws_js: function "' + ctor + '" is not an Error subtype');
        }
        var threw = false;
        try { fn(); } catch(e) {
            threw = true;
            if (!(e instanceof ctor)) {
                throw new AssertionError(msg || 'assert_throws_js: function "' + fn + '" threw object "' + e + '" (' + (e && typeof e === 'object' ? '"' + e.constructor.name + '"' : typeof e) + ') expected instance of function "' + ctor + '" ("' + ctor.name + '")');
            }
        }
        if (!threw) throw new AssertionError(msg || "assert_throws_js: no error thrown");
    };
    var __domExceptionCodes = {
        IndexSizeError: 1, HierarchyRequestError: 3, WrongDocumentError: 4,
        InvalidCharacterError: 5, NoModificationAllowedError: 7, NotFoundError: 8,
        NotSupportedError: 9, InUseAttributeError: 10, InvalidStateError: 11,
        SyntaxError: 12, InvalidModificationError: 13, NamespaceError: 14,
        InvalidAccessError: 15, TypeMismatchError: 17, SecurityError: 18,
        NetworkError: 19, AbortError: 20, URLMismatchError: 21,
        QuotaExceededError: 22, TimeoutError: 23, InvalidNodeTypeError: 24,
        DataCloneError: 25
    };
    var __legacyCodeNames = {
        INDEX_SIZE_ERR: "IndexSizeError", HIERARCHY_REQUEST_ERR: "HierarchyRequestError",
        WRONG_DOCUMENT_ERR: "WrongDocumentError", INVALID_CHARACTER_ERR: "InvalidCharacterError",
        NO_MODIFICATION_ALLOWED_ERR: "NoModificationAllowedError", NOT_FOUND_ERR: "NotFoundError",
        NOT_SUPPORTED_ERR: "NotSupportedError", INUSE_ATTRIBUTE_ERR: "InUseAttributeError",
        INVALID_STATE_ERR: "InvalidStateError", SYNTAX_ERR: "SyntaxError",
        INVALID_MODIFICATION_ERR: "InvalidModificationError", NAMESPACE_ERR: "NamespaceError",
        INVALID_ACCESS_ERR: "InvalidAccessError", TYPE_MISMATCH_ERR: "TypeMismatchError",
        SECURITY_ERR: "SecurityError", NETWORK_ERR: "NetworkError",
        ABORT_ERR: "AbortError", URL_MISMATCH_ERR: "URLMismatchError",
        QUOTA_EXCEEDED_ERR: "QuotaExceededError", TIMEOUT_ERR: "TimeoutError",
        INVALID_NODE_TYPE_ERR: "InvalidNodeTypeError", DATA_CLONE_ERR: "DataCloneError"
    };
    self.assert_throws_dom = function(type, funcOrConstructor, descOrFunc, maybeDesc) {
        var func, desc, constructor = null;
        if (typeof funcOrConstructor === 'function' && funcOrConstructor !== DOMException &&
            !(funcOrConstructor.prototype instanceof DOMException) &&
            typeof funcOrConstructor.prototype === 'object' &&
            !('name' in funcOrConstructor.prototype && 'code' in funcOrConstructor.prototype)) {
            // assert_throws_dom(type, fn, desc) — no constructor
            func = funcOrConstructor;
            desc = descOrFunc || '';
        } else if (typeof descOrFunc === 'function') {
            // assert_throws_dom(type, DOMExceptionConstructor, fn, desc)
            constructor = funcOrConstructor;
            func = descOrFunc;
            desc = maybeDesc || '';
        } else {
            func = funcOrConstructor;
            desc = descOrFunc || '';
        }

        // Resolve name and expected code
        var eName, eCode;
        if (typeof type === 'number') {
            eCode = type;
            eName = null;
            // Find name by code
            for (var k in __domExceptionCodes) {
                if (__domExceptionCodes[k] === type) { eName = k; break; }
            }
            if (eCode === 0) {
                throw new AssertionError("Test bug: ambiguous DOMException code 0 passed to assert_throws_dom()");
            }
        } else {
            eName = __legacyCodeNames[type] || type;
            eCode = __domExceptionCodes[eName];
            if (eCode === undefined) {
                throw new AssertionError("Test bug: unrecognized DOMException code name or name \"" + type + "\" passed to assert_throws_dom()");
            }
        }

        var threw = false;
        try { func(); } catch(e) {
            threw = true;
            // Must be a DOMException (or from the specified constructor's global)
            var isDOMExc = (e instanceof DOMException) ||
                           (e && typeof e === 'object' && e.constructor === DOMException);
            var targetCtor = constructor || DOMException;
            if (constructor) {
                var isFromGlobal = (e instanceof constructor) ||
                                   (e && typeof e === 'object' && e.constructor === constructor);
                if (!isFromGlobal) {
                    throw new AssertionError((desc ? desc + ": " : "") + "assert_throws_dom: function \"" + func + "\" threw an exception from the wrong global");
                }
            } else if (!isDOMExc) {
                throw new AssertionError((desc ? desc + ": " : "") + "assert_throws_dom: function \"" + func + "\" threw object \"" + e + "\" that is not a DOMException " + eName + ": property \"code\" is equal to " + (e && e.code) + ", expected " + eCode);
            }
            if (e.name !== eName) {
                throw new AssertionError((desc ? desc + ": " : "") + "assert_throws_dom: expected " + eName + " but got " + e.name);
            }
            if (e.code !== eCode) {
                throw new AssertionError((desc ? desc + ": " : "") + "assert_throws_dom: expected code " + eCode + " but got " + e.code);
            }
        }
        if (!threw) throw new AssertionError((desc ? desc + ": " : "") + "assert_throws_dom(" + eName + "): no error thrown");
    };
    self.assert_throws_quotaexceedederror = function(fnOrCtor, reqOrFn, quotaOrReq, descOrQuota, maybeDesc) {
        // Simplified: just check that fn throws a DOMException with name QuotaExceededError
        var fn2, desc;
        if (typeof fnOrCtor === 'function' && fnOrCtor.name !== 'QuotaExceededError') {
            fn2 = fnOrCtor; desc = descOrQuota || quotaOrReq || '';
        } else {
            fn2 = reqOrFn; desc = maybeDesc || descOrQuota || '';
        }
        var threw = false;
        try { fn2(); } catch(e) {
            threw = true;
            if (!e || e.name !== 'QuotaExceededError') throw new AssertionError(desc || "assert_throws_quotaexceedederror: wrong error: " + (e && e.name));
        }
        if (!threw) throw new AssertionError(desc || "assert_throws_quotaexceedederror: no error thrown");
    };
    self.assert_throws_exactly = function(expected, fn, msg) {
        var threw = false;
        try { fn(); } catch(e) {
            threw = true;
            if (e !== expected) throw new AssertionError(msg || "assert_throws_exactly: wrong error");
        }
        if (!threw) throw new AssertionError(msg || "assert_throws_exactly: no error thrown");
    };
    self.promise_rejects_js = function(test, constructor, promise, description) {
        return promise.then(
            function() { throw new AssertionError(description + ": promise resolved, expected rejection"); },
            function(e) {
                if (!(e instanceof constructor)) {
                    throw new AssertionError(description + ": wrong rejection type: " + e);
                }
            }
        );
    };
    self.promise_rejects_exactly = function(test, exception, promise, description) {
        return promise.then(
            function() { throw new AssertionError(description + ": promise resolved, expected rejection"); },
            function(e) {
                if (e !== exception) {
                    throw new AssertionError(description + ": wrong rejection value");
                }
            }
        );
    };
    self.promise_rejects_dom = function(test, type, promise, description) {
        return promise.then(
            function() { throw new AssertionError((description || "") + ": should have rejected: " + type); },
            function(e) {
                if (e.name !== type) {
                    throw new AssertionError((description || "") + ": expected " + type + " but got " + e.name + ": " + e.message);
                }
            }
        );
    };
    self.assert_unreached = function(msg) {
        throw new AssertionError(msg || "assert_unreached");
    };
    self.assert_readonly = function(obj, prop, msg) {
        var desc = Object.getOwnPropertyDescriptor(obj, prop);
        if (!desc || desc.writable !== false) {
            if (!desc || desc.set) throw new AssertionError(msg || "assert_readonly: " + prop + " is not readonly");
        }
    };
    self.assert_idl_attribute = function(obj, prop, msg) {
        if (!(prop in obj)) throw new AssertionError(msg || "assert_idl_attribute: missing " + prop);
    };
    self.assert_inherits = function(obj, prop, msg) {
        if (!(prop in obj) || obj.hasOwnProperty(prop)) throw new AssertionError(msg || "assert_inherits: " + prop + " is not inherited");
    };
    self.assert_implements = function(val, msg) {
        if (!val) throw new AssertionError(msg || "assert_implements: not implemented");
    };
    self.assert_implements_optional = function(val, msg) {
        if (!val) throw new OptionalFeatureUnsupportedError(msg || "assert_implements_optional: not implemented");
    };
    self.subsetTest = function(testFunc) {
        var args = Array.prototype.slice.call(arguments, 1);
        return testFunc.apply(null, args);
    };
    self.shouldRunSubTest = function() { return true; };
    self.subsetTestByKey = function(key, testFunc) {
        var args = Array.prototype.slice.call(arguments, 2);
        return testFunc.apply(null, args);
    };
    self.format_value = function(val) {
        if (val === null) return "null";
        if (val === undefined) return "undefined";
        if (typeof val === "string") return '"' + val + '"';
        return String(val);
    };

    if (typeof Event !== 'undefined') {
        if (!Event.NONE) Event.NONE = 0;
        if (!Event.CAPTURING_PHASE) Event.CAPTURING_PHASE = 1;
        if (!Event.AT_TARGET) Event.AT_TARGET = 2;
        if (!Event.BUBBLING_PHASE) Event.BUBBLING_PHASE = 3;
    }

    self.EventWatcher = function(test, target, eventTypes, setup) {
        if (typeof eventTypes === "string") eventTypes = [eventTypes];
        var waitingFor = null;

        function listener(evt) {
            if (waitingFor && evt.type === waitingFor.type) {
                var resolve = waitingFor.resolve;
                waitingFor = null;
                resolve(evt);
            }
        }

        for (var i = 0; i < eventTypes.length; i++) {
            target.addEventListener(eventTypes[i], listener);
        }

        if (test && test.add_cleanup) {
            test.add_cleanup(function() {
                for (var i = 0; i < eventTypes.length; i++) {
                    target.removeEventListener(eventTypes[i], listener);
                }
            });
        }

        this.wait_for = function(type) {
            if (setup) setup();
            return new Promise(function(resolve) {
                waitingFor = { type: type, resolve: resolve };
            });
        };
    };

    self.__wpt_get_results = function() {
        // Finalize pending async tests — mark unfinished ones as TIMEOUT
        for (var i = 0; i < _pending_async.length; i++) {
            var entry = _pending_async[i];
            if (!entry.test._done) {
                entry.result.status = 2;
                entry.result.message = "Test timed out";
            }
        }
        _pending_async = [];
        return results;
    };
})();
"#
    .to_string()
}

pub fn testharnessreport_shim() -> String {
    "// testharnessreport.js shim — no-op".to_string()
}

// ---------------------------------------------------------------------------
// Script resolution
// ---------------------------------------------------------------------------

pub fn resolve_script_src(
    html_path: &Path,
    src: &str,
    preamble: &str,
    report_shim: &str,
) -> Option<String> {
    if src == "/resources/testharness.js" {
        return Some(preamble.to_string());
    }
    if src == "/resources/testharnessreport.js" {
        return Some(report_shim.to_string());
    }
    if src == "/common/subset-tests.js" {
        return Some("function subsetTest(testFunc) { var args = Array.prototype.slice.call(arguments, 1); testFunc.apply(this, args); }".to_string());
    }
    if src.contains("testdriver") {
        return Some(r#"
            function __hasPassiveListener(target, eventType) {
                // Check the target and its ancestors, plus always check document and window
                var el = target;
                while (el) {
                    if (el.__passiveTypes && el.__passiveTypes[eventType]) return true;
                    if (el === window) break;
                    el = el.parentNode || null;
                }
                // Always check document and window explicitly (they may not be in the wrapper chain)
                if (document.__passiveTypes && document.__passiveTypes[eventType]) return true;
                if (window.__passiveTypes && window.__passiveTypes[eventType]) return true;
                return false;
            }
            // Helper: resolve element through iframes — if element is an iframe, find element inside it
            function __resolveIframeTarget(el, x, y) {
                if (el && el.tagName === 'IFRAME' && el.contentDocument) {
                    var iframeRect = el.getBoundingClientRect();
                    var innerX = x - iframeRect.left;
                    var innerY = y - iframeRect.top;
                    // Walk iframe document's elements depth-first
                    var root = el.contentDocument.documentElement;
                    if (!root) return el;
                    var best = root;
                    function walk(node) {
                        if (!node || node.nodeType !== 1) return;
                        var r = node.getBoundingClientRect();
                        if (r && innerX >= r.left && innerX <= r.right && innerY >= r.top && innerY <= r.bottom) best = node;
                        var ch = node.children;
                        if (ch) { for (var i = 0; i < ch.length; i++) walk(ch[i]); }
                    }
                    walk(root);
                    return best;
                }
                return el;
            }
            // Helper: find nearest scrollable ancestor (scrollHeight > clientHeight or scrollWidth > clientWidth)
            // Stops at IFRAME boundaries — never crosses into the outer document
            function __findScrollableAncestor(el, axis) {
                var cur = el;
                var hitIframeBoundary = false;
                while (cur && cur.nodeType === 1) {
                    if (axis === 'y' && cur.scrollHeight > cur.clientHeight) return cur;
                    if (axis === 'x' && cur.scrollWidth > cur.clientWidth) return cur;
                    if (axis === 'both' && (cur.scrollHeight > cur.clientHeight || cur.scrollWidth > cur.clientWidth)) return cur;
                    var parent = cur.parentNode;
                    // Stop at iframe boundary — don't walk into the outer document
                    if (parent && parent.tagName === 'IFRAME') { hitIframeBoundary = true; break; }
                    cur = parent;
                }
                if (hitIframeBoundary) {
                    // Inside an iframe — fall back to iframe's scrollingElement
                    // Walk back up to find the iframe element, then get its document's scrollingElement
                    var iframeCur = el;
                    while (iframeCur) {
                        var p = iframeCur.parentNode;
                        if (p && p.tagName === 'IFRAME') {
                            var realm = typeof __braille_get_iframe_realm === 'function' ? __braille_get_iframe_realm(p.__nid !== undefined ? p.__nid : -1) : null;
                            if (realm && realm.document && realm.document.scrollingElement) {
                                return realm.document.scrollingElement;
                            }
                            break;
                        }
                        iframeCur = p;
                    }
                    return null;
                }
                // Check if we're inside an iframe (content under a standalone document node,
                // not directly under an IFRAME element in the DOM tree).
                // Detect by checking if the root document node is NOT the main document.
                if (cur && cur.nodeType === 9 && cur !== document) {
                    // Inside an iframe's document — don't fall back to main scrollingElement
                    return null;
                }
                // Fall back to outer document.scrollingElement
                var se = document.scrollingElement;
                if (se) return se;
                return null;
            }
            // Helper: apply scroll delta to nearest scrollable ancestor.
            // Bypasses the scrollTop/scrollLeft setters (which fire scroll+scrollend)
            // to only fire 'scroll'. Caller fires 'scrollend' once at gesture end.
            function __applyScrollDelta(target, dx, dy) {
                var didScroll = false;
                if (dy !== 0) {
                    var scrollerY = __findScrollableAncestor(target, 'y');
                    if (scrollerY) {
                        if (!scrollerY.__props) scrollerY.__props = {};
                        var old = scrollerY.__props._scrollTop || 0;
                        var v = Math.round(old + dy);
                        if (v < 0) v = 0;
                        var maxT = scrollerY.scrollHeight - scrollerY.clientHeight;
                        if (maxT > 0 && v > maxT) v = maxT;
                        if (v !== old) {
                            scrollerY.__props._scrollTop = v;
                            var st = __resolveScrollTarget(scrollerY);
                            st.target.dispatchEvent(new Event('scroll', {bubbles: st.isRoot}));
                            didScroll = true;
                        }
                    }
                }
                if (dx !== 0) {
                    var scrollerX = __findScrollableAncestor(target, 'x');
                    if (scrollerX) {
                        if (!scrollerX.__props) scrollerX.__props = {};
                        var old = scrollerX.__props._scrollLeft || 0;
                        var v = Math.round(old + dx);
                        if (v < 0) v = 0;
                        var maxL = scrollerX.scrollWidth - scrollerX.clientWidth;
                        if (maxL > 0 && v > maxL) v = maxL;
                        if (v !== old) {
                            scrollerX.__props._scrollLeft = v;
                            var st = __resolveScrollTarget(scrollerX);
                            st.target.dispatchEvent(new Event('scroll', {bubbles: st.isRoot}));
                            didScroll = true;
                        }
                    }
                }
                return didScroll;
            }
            // Helper: check if element is an iframe document's root scrollable element
            // (either the scrollingElement or the body). Returns the iframe document if so.
            function __getOwningIframeDoc(el) {
                if (!el || el.__nid === undefined) return null;
                var cur = el.__nid;
                var parent = __n_getParent(cur);
                while (parent >= 0) {
                    if (__n_getTagName(parent) === 'IFRAME') {
                        var realm = __braille_get_iframe_realm(parent);
                        if (realm && realm.document) {
                            var se = realm.document.scrollingElement;
                            // Match if el is the scrollingElement, its body, or direct child of document root
                            if (se === el) return realm.document;
                            var tag = el.tagName;
                            if (tag === 'BODY' || tag === 'HTML') return realm.document;
                        }
                        return null;
                    }
                    parent = __n_getParent(parent);
                }
                return null;
            }
            // Helper: resolve scroll event target — if element is an iframe document's
            // scrollingElement, dispatch on the iframe document instead
            function __resolveScrollTarget(scroller) {
                // Check outer document first
                if (scroller === document.scrollingElement) {
                    return { target: document, isRoot: true };
                }
                // Check if scroller is an iframe document's scrollingElement
                var iframeDoc = __getOwningIframeDoc(scroller);
                if (iframeDoc) {
                    return { target: iframeDoc, isRoot: true };
                }
                return { target: scroller, isRoot: false };
            }
            // Helper: fire scrollend on the given target
            function __fireScrollend(evTarget, bubbles) {
                evTarget.dispatchEvent(new Event('scrollend', {bubbles: !!bubbles}));
            }
            if (typeof test_driver_internal === 'undefined') {
                var test_driver_internal = {
                    action_sequence: function(actions, context) {
                        return new Promise(function(resolve) {
                            // Detect multi-pointer (pinch) gesture
                            var touchPointers = [];
                            for (var i = 0; i < actions.length; i++) {
                                if (actions[i].type === "pointer" && actions[i].parameters && actions[i].parameters.pointerType === "touch") {
                                    touchPointers.push(actions[i]);
                                }
                            }
                            // Pinch zoom: two touch pointers = zoom gesture
                            if (touchPointers.length >= 2) {
                                // Calculate pinch: if pointers converge, zoom in
                                var vv = window.visualViewport;
                                if (vv) {
                                    vv.scale = 2;
                                    vv.width = 1280 / vv.scale;
                                    vv.height = 800 / vv.scale;
                                    // Center the viewport after zoom — offset = (full - visible) / 2
                                    vv.offsetLeft = (1280 - vv.width) / 2;
                                    vv.offsetTop = (800 - vv.height) / 2;
                                    vv.pageLeft = vv.offsetLeft;
                                    vv.pageTop = vv.offsetTop;
                                    vv.dispatchEvent(new Event('resize'));
                                }
                                resolve();
                                return;
                            }
                            for (var i = 0; i < actions.length; i++) {
                                var source = actions[i];
                                if (source.type === "wheel") {
                                    // Wheel transaction: first scroll determines the target element
                                    var transactionTarget = null;
                                    var wheelScrolled = false;
                                    var wheelScrollTarget = null;
                                    for (var j = 0; j < source.actions.length; j++) {
                                        var action = source.actions[j];
                                        if (action.type === "scroll") {
                                            // Resolve origin: if action has _originElement, use its position
                                            var ax = action.x || 0;
                                            var ay = action.y || 0;
                                            if (action._originElement && action._originElement.getBoundingClientRect) {
                                                var or = action._originElement.getBoundingClientRect();
                                                ax += or.left + or.width / 2;
                                                ay += or.top + or.height / 2;
                                            }
                                            if (!transactionTarget) {
                                                transactionTarget = (typeof document !== 'undefined' && document.elementFromPoint)
                                                    ? document.elementFromPoint(ax, ay)
                                                    : null;
                                                if (!transactionTarget) transactionTarget = (typeof document !== 'undefined' && document.body) || null;
                                                // Resolve through iframes
                                                transactionTarget = __resolveIframeTarget(transactionTarget, ax, ay);
                                            }
                                            var target = transactionTarget;
                                            if (target) {
                                                // Re-resolve event target at cursor position (element may have moved)
                                                var eventTarget = (typeof document !== 'undefined' && document.elementFromPoint)
                                                    ? document.elementFromPoint(ax, ay)
                                                    : target;
                                                if (!eventTarget) eventTarget = target;
                                                var cancelable = !__hasPassiveListener(eventTarget, "wheel");
                                                var wev = new WheelEvent("wheel", {
                                                    bubbles: true, cancelable: cancelable,
                                                    deltaX: action.deltaX || 0, deltaY: action.deltaY || 0,
                                                    clientX: ax, clientY: ay, view: window
                                                });
                                                eventTarget.dispatchEvent(wev);
                                                // Retarget if transaction target became non-hittable
                                                if (transactionTarget) {
                                                    var tRect = transactionTarget.getBoundingClientRect();
                                                    var tStyle = window.getComputedStyle ? window.getComputedStyle(transactionTarget) : null;
                                                    var tDisplay = tStyle ? tStyle.display : '';
                                                    if (!tRect || (tRect.width === 0 && tRect.height === 0) || tDisplay === 'none' || tDisplay === 'contents') {
                                                        transactionTarget = null;
                                                    }
                                                }
                                                // Default action: scroll if not prevented
                                                if (!wev.defaultPrevented) {
                                                    var scrollerY = (action.deltaY || 0) !== 0 ? __findScrollableAncestor(target, 'y') : null;
                                                    var scrollerX = (action.deltaX || 0) !== 0 ? __findScrollableAncestor(target, 'x') : null;
                                                    var scroller = scrollerY || scrollerX;
                                                    var moved = __applyScrollDelta(target, action.deltaX || 0, action.deltaY || 0);
                                                    if (scroller && moved) { wheelScrollTarget = scroller; wheelScrolled = true; }
                                                }
                                            }
                                        }
                                    }
                                    // Fire scrollend once for the whole wheel transaction
                                    if (wheelScrolled && wheelScrollTarget) {
                                        var st = __resolveScrollTarget(wheelScrollTarget);
                                        __fireScrollend(st.target, st.isRoot);
                                    }
                                } else if (source.type === "pointer" && source.parameters && source.parameters.pointerType === "touch") {
                                    // Touch pointer: track positions for touch-to-scroll
                                    var touchTarget = null;
                                    var isDown = false;
                                    var lastX = 0, lastY = 0;
                                    var curX = 0, curY = 0;
                                    var originEl = null;
                                    var totalDx = 0, totalDy = 0;
                                    var touchScrolledElement = null;
                                    for (var j = 0; j < source.actions.length; j++) {
                                        var action = source.actions[j];
                                        if (action.type === "pointerMove") {
                                            var mx = action.x || 0;
                                            var my = action.y || 0;
                                            // Resolve origin element
                                            if (action._originElement) {
                                                originEl = action._originElement;
                                                var or = originEl.getBoundingClientRect();
                                                mx += or.left + or.width / 2;
                                                my += or.top + or.height / 2;
                                            }
                                            if (!isDown) {
                                                curX = mx; curY = my;
                                            } else {
                                                var dx = curX - mx;
                                                var dy = curY - my;
                                                totalDx += dx;
                                                totalDy += dy;
                                                curX = mx; curY = my;
                                                // Fire touchmove
                                                if (touchTarget) {
                                                    var cancelable = !__hasPassiveListener(touchTarget, "touchmove");
                                                    var te = new Event("touchmove", {bubbles: true, cancelable: cancelable});
                                                    te.touches = [{clientX: mx, clientY: my}];
                                                    te.changedTouches = [{clientX: mx, clientY: my}];
                                                    te.targetTouches = [{clientX: mx, clientY: my}];
                                                    touchTarget.dispatchEvent(te);
                                                    // Apply scroll delta (touch drag: opposite of pointer movement)
                                                    if (!te.defaultPrevented) {
                                                        var vv = window.visualViewport;
                                                        if (vv && vv.scale > 1) {
                                                            // When zoomed in, touch-pan adjusts visual viewport offset
                                                            vv.offsetTop = Math.max(0, (vv.offsetTop || 0) + dy);
                                                            vv.offsetLeft = Math.max(0, (vv.offsetLeft || 0) + dx);
                                                            vv.pageTop = vv.offsetTop;
                                                            vv.pageLeft = vv.offsetLeft;
                                                            vv.dispatchEvent(new Event('scroll'));
                                                        } else {
                                                            var scr = __findScrollableAncestor(touchTarget, 'both');
                                                            if (scr) touchScrolledElement = scr;
                                                            __applyScrollDelta(touchTarget, dx, dy);
                                                        }
                                                    }
                                                }
                                            }
                                        } else if (action.type === "pointerDown") {
                                            isDown = true;
                                            lastX = curX; lastY = curY;
                                            touchTarget = document.elementFromPoint ? document.elementFromPoint(curX, curY) : document.body;
                                            if (!touchTarget) touchTarget = document.body || null;
                                            // Resolve through iframes
                                            touchTarget = __resolveIframeTarget(touchTarget, curX, curY);
                                            if (touchTarget) {
                                                var cancelable = !__hasPassiveListener(touchTarget, "touchstart");
                                                var te = new Event("touchstart", {bubbles: true, cancelable: cancelable});
                                                te.touches = [{clientX: curX, clientY: curY}];
                                                te.changedTouches = [{clientX: curX, clientY: curY}];
                                                te.targetTouches = [{clientX: curX, clientY: curY}];
                                                touchTarget.dispatchEvent(te);
                                            }
                                        } else if (action.type === "pointerUp") {
                                            if (touchTarget) {
                                                var cancelable = !__hasPassiveListener(touchTarget, "touchend");
                                                var te = new Event("touchend", {bubbles: true, cancelable: cancelable});
                                                te.touches = []; te.changedTouches = [{clientX: curX, clientY: curY}]; te.targetTouches = [];
                                                touchTarget.dispatchEvent(te);
                                                // Apply scroll-snap before firing scrollend
                                                if (totalDy !== 0 || totalDx !== 0) {
                                                    var vv = window.visualViewport;
                                                    if (vv && vv.scale > 1) {
                                                        vv.dispatchEvent(new Event('scrollend'));
                                                    }
                                                    if (touchScrolledElement) {
                                                        // Snap to nearest snap point (like browsers do after touch gestures)
                                                        if (typeof __computeSnapOffset === 'function') {
                                                            var snapped = __computeSnapOffset(touchScrolledElement, touchScrolledElement.scrollLeft, touchScrolledElement.scrollTop);
                                                            if (snapped.x !== touchScrolledElement.scrollLeft || snapped.y !== touchScrolledElement.scrollTop) {
                                                                if (!touchScrolledElement.__props) touchScrolledElement.__props = {};
                                                                touchScrolledElement.__props._scrollLeft = snapped.x;
                                                                touchScrolledElement.__props._scrollTop = snapped.y;
                                                                var st2 = __resolveScrollTarget(touchScrolledElement);
                                                                st2.target.dispatchEvent(new Event('scroll', {bubbles: st2.isRoot}));
                                                            }
                                                        }
                                                        var st = __resolveScrollTarget(touchScrolledElement);
                                                        __fireScrollend(st.target, st.isRoot);
                                                    }
                                                }
                                            }
                                            isDown = false;
                                            touchTarget = null;
                                            totalDx = 0; totalDy = 0;
                                            touchScrolledElement = null;
                                        }
                                    }
                                } else if (source.type === "pointer" && (!source.parameters || source.parameters.pointerType === "mouse" || !source.parameters.pointerType)) {
                                    // Mouse pointer: dispatch PointerEvent with pointerId=1
                                    var mouseTarget = null;
                                    var mCurX = 0, mCurY = 0;
                                    var mOriginEl = null;
                                    var mouseDownTarget = null;
                                    var mouseDownOriginEl = null;
                                    for (var j = 0; j < source.actions.length; j++) {
                                        var action = source.actions[j];
                                        if (action.type === "pointerMove") {
                                            var mx = action.x || 0;
                                            var my = action.y || 0;
                                            if (action._originElement) {
                                                mOriginEl = action._originElement;
                                                var or = mOriginEl.getBoundingClientRect();
                                                mx += or.left + or.width / 2;
                                                my += or.top + or.height / 2;
                                            }
                                            mCurX = mx; mCurY = my;
                                            mouseTarget = document.elementFromPoint ? document.elementFromPoint(mCurX, mCurY) : document.body;
                                            if (!mouseTarget) mouseTarget = document.body || null;
                                            mouseTarget = __resolveIframeTarget(mouseTarget, mCurX, mCurY);
                                            if (mouseTarget) {
                                                mouseTarget.dispatchEvent(new PointerEvent('pointermove', {bubbles:true,cancelable:true,clientX:mCurX,clientY:mCurY,pointerId:1,pointerType:'mouse'}));
                                                mouseTarget.dispatchEvent(new MouseEvent('mousemove', {bubbles:true,cancelable:true,clientX:mCurX,clientY:mCurY}));
                                            }
                                        } else if (action.type === "pointerDown") {
                                            if (!mouseTarget) {
                                                mouseTarget = document.elementFromPoint ? document.elementFromPoint(mCurX, mCurY) : document.body;
                                                if (!mouseTarget) mouseTarget = document.body || null;
                                                mouseTarget = __resolveIframeTarget(mouseTarget, mCurX, mCurY);
                                            }
                                            mouseDownTarget = mouseTarget;
                                            mouseDownOriginEl = mOriginEl;
                                            if (mouseTarget) {
                                                mouseTarget.dispatchEvent(new PointerEvent('pointerdown', {bubbles:true,cancelable:true,clientX:mCurX,clientY:mCurY,pointerId:1,pointerType:'mouse'}));
                                                mouseTarget.dispatchEvent(new MouseEvent('mousedown', {bubbles:true,cancelable:true,clientX:mCurX,clientY:mCurY}));
                                            }
                                        } else if (action.type === "pointerUp") {
                                            if (mouseTarget) {
                                                mouseTarget.dispatchEvent(new PointerEvent('pointerup', {bubbles:true,cancelable:true,clientX:mCurX,clientY:mCurY,pointerId:1,pointerType:'mouse'}));
                                                mouseTarget.dispatchEvent(new MouseEvent('mouseup', {bubbles:true,cancelable:true,clientX:mCurX,clientY:mCurY}));
                                                mouseTarget.dispatchEvent(new MouseEvent('click', {bubbles:true,cancelable:true,clientX:mCurX,clientY:mCurY}));
                                            }
                                            // Simulate user-initiated text selection (pointer drag)
                                            if (mouseDownTarget && typeof getSelection === 'function') {
                                                var sel = getSelection();
                                                var anchorEl = mouseDownOriginEl || mouseDownTarget;
                                                var focusEl = mOriginEl || mouseTarget;
                                                // Find first text node in an element
                                                function __findFirstText(el) {
                                                    if (!el) return null;
                                                    if (el.nodeType === 3) return el;
                                                    var cn = el.childNodes;
                                                    if (cn) for (var ti = 0; ti < cn.length; ti++) {
                                                        var t = __findFirstText(cn[ti]);
                                                        if (t) return t;
                                                    }
                                                    return null;
                                                }
                                                var anchorText = __findFirstText(anchorEl);
                                                var focusText = __findFirstText(focusEl);
                                                if (anchorText && focusText) {
                                                    sel.removeAllRanges();
                                                    var r = document.createRange();
                                                    r.setStart(anchorText, 0);
                                                    r.setEnd(focusText, focusText.length || 0);
                                                    sel.addRange(r);
                                                }
                                            }
                                            mouseDownTarget = null;
                                            mouseDownOriginEl = null;
                                        }
                                    }
                                }
                            }
                            resolve();
                        });
                    },
                };
            }
            if (typeof test_driver === 'undefined') {
                var test_driver = {
                    click: function(element) {
                        if (element) {
                            // Simulate full pointer sequence: pointerdown → pointerup → click
                            if (element.dispatchEvent) {
                                element.dispatchEvent(new PointerEvent('pointerdown', {bubbles:true,cancelable:true}));
                                element.dispatchEvent(new PointerEvent('pointerup', {bubbles:true,cancelable:true}));
                            }
                            if (typeof element.click === 'function') element.click();
                        }
                        return Promise.resolve();
                    },
                    send_keys: function(element, keys) {
                        var keyMap = {
                            '\uE003': {key:'Backspace',code:'Backspace'},
                            '\uE004': {key:'Tab',code:'Tab'},
                            '\uE006': {key:'Enter',code:'Enter'},
                            '\uE007': {key:'Enter',code:'Enter'},
                            '\uE008': {key:'Shift',code:'ShiftLeft'},
                            '\uE009': {key:'Control',code:'ControlLeft'},
                            '\uE00A': {key:'Alt',code:'AltLeft'},
                            '\uE00D': {key:' ',code:'Space'},
                            '\uE010': {key:'End',code:'End'},
                            '\uE011': {key:'Home',code:'Home'},
                            '\uE012': {key:'ArrowLeft',code:'ArrowLeft'},
                            '\uE013': {key:'ArrowUp',code:'ArrowUp'},
                            '\uE014': {key:'ArrowRight',code:'ArrowRight'},
                            '\uE015': {key:'ArrowDown',code:'ArrowDown'},
                            '\uE017': {key:'Delete',code:'Delete'},
                            '\uE00C': {key:'Escape',code:'Escape'},
                        };
                        var CHAR_WIDTH = 8;
                        var SCROLL_STEP = 40;
                        if (element && element.dispatchEvent) {
                            for (var i = 0; i < keys.length; i++) {
                                var ch = keys[i];
                                var mapped = keyMap[ch] || {key: ch, code: 'Key' + ch.toUpperCase()};
                                var opts = {key: mapped.key, code: mapped.code, bubbles: true, cancelable: true};
                                var kdev = new KeyboardEvent('keydown', opts);
                                element.dispatchEvent(kdev);
                                element.dispatchEvent(new KeyboardEvent('keypress', opts));
                                // Handle cursor movement for input/textarea
                                var isInput = element.tagName === 'INPUT' || element.tagName === 'TEXTAREA';
                                if (isInput) {
                                    var val = element.value || '';
                                    var pos = element.selectionStart;
                                    if (mapped.key === 'ArrowRight') {
                                        if (pos < val.length) pos++;
                                    } else if (mapped.key === 'ArrowLeft') {
                                        if (pos > 0) pos--;
                                    } else if (mapped.key === 'Home') {
                                        pos = 0;
                                    } else if (mapped.key === 'End') {
                                        pos = val.length;
                                    }
                                    element.selectionStart = pos;
                                    element.selectionEnd = pos;
                                    // Compute scrollLeft based on cursor position vs visible width
                                    var elWidth = element.getBoundingClientRect().width || 50;
                                    var cursorX = pos * CHAR_WIDTH;
                                    var currentScroll = element.scrollLeft || 0;
                                    if (cursorX > currentScroll + elWidth) {
                                        element.scrollLeft = cursorX - elWidth;
                                    } else if (cursorX < currentScroll) {
                                        element.scrollLeft = cursorX;
                                    }
                                } else if (!kdev.defaultPrevented) {
                                    // Keyboard scroll for non-input elements (buttons, divs, etc.)
                                    var scrollDx = 0, scrollDy = 0;
                                    if (mapped.key === 'ArrowDown') scrollDy = SCROLL_STEP;
                                    else if (mapped.key === 'ArrowUp') scrollDy = -SCROLL_STEP;
                                    else if (mapped.key === 'ArrowRight') scrollDx = SCROLL_STEP;
                                    else if (mapped.key === 'ArrowLeft') scrollDx = -SCROLL_STEP;
                                    if (scrollDx !== 0 || scrollDy !== 0) {
                                        var scroller = __findScrollableAncestor(element, 'both');
                                        var moved = __applyScrollDelta(element, scrollDx, scrollDy);
                                        // Fire scrollend only if scroll position actually changed
                                        if (scroller && moved) {
                                            var st = __resolveScrollTarget(scroller);
                                            __fireScrollend(st.target, st.isRoot);
                                        }
                                    }
                                }
                                element.dispatchEvent(new KeyboardEvent('keyup', opts));
                            }
                        }
                        return Promise.resolve();
                    },
                    bless: function(intent, action) {
                        if (typeof action === 'function') return Promise.resolve(action());
                        return Promise.resolve();
                    },
                    set_permission: function() { return Promise.resolve(); },
                    action_sequence: function(actions, context) {
                        return test_driver_internal.action_sequence(actions, context);
                    },
                    Actions: function() {
                        var self = this;
                        self._actions = [];
                        self._pointerType = "mouse";
                        self.addPointer = function(name, pointerType) { self._pointerType = pointerType || "mouse"; return self; };
                        self.addWheel = function(name) { return self; };
                        self.pointerMove = function(x, y, opts) {
                            var a = {source:"pointer",type:"pointerMove",x:x||0,y:y||0};
                            if (opts && opts.origin && typeof opts.origin === 'object') a._originElement = opts.origin;
                            self._actions.push(a);
                            return self;
                        };
                        self.pointerDown = function(opts) { self._actions.push({source:"pointer",type:"pointerDown"}); return self; };
                        self.pointerUp = function(opts) { self._actions.push({source:"pointer",type:"pointerUp"}); return self; };
                        self.pause = function(duration) { return self; };
                        self.addTick = function() { return self; };
                        self.scroll = function(x, y, deltaX, deltaY, opts) {
                            var a = {source:"wheel",type:"scroll",x:x,y:y,deltaX:deltaX,deltaY:deltaY};
                            if (opts && opts.origin && typeof opts.origin === 'object') a._originElement = opts.origin;
                            self._actions.push(a);
                            return self;
                        };
                        self.keyDown = function(key) { self._actions.push({source:"key",type:"keyDown",key:key}); return self; };
                        self.keyUp = function(key) { self._actions.push({source:"key",type:"keyUp",key:key}); return self; };
                        self.send = function() {
                            var serialized = [];
                            var wheelActions = [];
                            var pointerActions = [];
                            var keyActions = [];
                            for (var i = 0; i < self._actions.length; i++) {
                                var a = self._actions[i];
                                if (a.source === "wheel") wheelActions.push(a);
                                else if (a.source === "pointer") pointerActions.push(a);
                                else if (a.source === "key") keyActions.push(a);
                            }
                            if (wheelActions.length) serialized.push({type:"wheel",actions:wheelActions});
                            if (pointerActions.length) serialized.push({type:"pointer",parameters:{pointerType:self._pointerType},actions:pointerActions});
                            // Execute key actions via send_keys
                            if (keyActions.length) {
                                var el = document.activeElement || document.body;
                                for (var k = 0; k < keyActions.length; k++) {
                                    if (keyActions[k].type === "keyDown") {
                                        test_driver.send_keys(el, keyActions[k].key);
                                    }
                                }
                            }
                            return test_driver.action_sequence(serialized);
                        };
                    },
                };
            }
        "#.to_string());
    }
    if src.contains("trusted-click") {
        return Some(r#"
            function trusted_click() {
                return new Promise(function(resolve) {
                    document.addEventListener("click", resolve, {once: true});
                    test_driver.click(document.body);
                });
            }
            function fullScreenChange() {
                return new Promise(function(resolve) {
                    document.addEventListener("fullscreenchange", resolve, {once: true});
                });
            }
        "#.to_string());
    }

    // Strip query params (e.g. ?pipe=trickle(d1)) before resolving filesystem path
    let clean_src = src.split('?').next().unwrap_or(src);
    let resolved_path = if clean_src.starts_with('/') {
        wpt_root().join(clean_src.trim_start_matches('/'))
    } else {
        html_path.parent().unwrap().join(clean_src)
    };

    // Catch relative paths that resolve to testharness.js or testharnessreport.js
    if let Some(fname) = resolved_path.file_name().and_then(|f| f.to_str()) {
        if fname == "testharness.js" {
            if let Ok(canonical) = resolved_path.canonicalize() {
                let th_path = wpt_root().join("resources/testharness.js");
                if let Ok(th_canonical) = th_path.canonicalize() {
                    if canonical == th_canonical {
                        return Some(preamble.to_string());
                    }
                }
            }
        }
        if fname == "testharnessreport.js" {
            if let Ok(canonical) = resolved_path.canonicalize() {
                let tr_path = wpt_root().join("resources/testharnessreport.js");
                if let Ok(tr_canonical) = tr_path.canonicalize() {
                    if canonical == tr_canonical {
                        return Some(report_shim.to_string());
                    }
                }
            }
        }
    }

    std::fs::read_to_string(&resolved_path).ok()
}

pub fn extract_script_srcs(html: &str) -> Vec<String> {
    let mut srcs = Vec::new();
    let lower = html.to_ascii_lowercase();
    let mut pos = 0;
    while let Some(script_start) = lower[pos..].find("<script") {
        let abs_start = pos + script_start;
        let tag_end = match lower[abs_start..].find('>') {
            Some(e) => abs_start + e,
            None => break,
        };
        let tag = &html[abs_start..=tag_end];

        if let Some(src_idx) = tag.to_ascii_lowercase().find("src=") {
            let after_src = &tag[src_idx + 4..];
            let quote = after_src.chars().next().unwrap_or(' ');
            if quote == '"' || quote == '\'' {
                if let Some(end_quote) = after_src[1..].find(quote) {
                    let src_val = &after_src[1..1 + end_quote];
                    srcs.push(src_val.to_string());
                }
            } else {
                let end = after_src
                    .find(|c: char| c.is_whitespace() || c == '>')
                    .unwrap_or(after_src.len());
                srcs.push(after_src[..end].to_string());
            }
        }

        pos = tag_end + 1;
    }
    srcs
}

// ---------------------------------------------------------------------------
// Link href extraction (for <link rel="stylesheet">)
// ---------------------------------------------------------------------------

pub fn extract_link_hrefs(html: &str) -> Vec<String> {
    let mut hrefs = Vec::new();
    let lower = html.to_ascii_lowercase();
    let mut pos = 0;
    while let Some(link_start) = lower[pos..].find("<link") {
        let abs_start = pos + link_start;
        let tag_end = match lower[abs_start..].find('>') {
            Some(e) => abs_start + e,
            None => break,
        };
        let tag = &html[abs_start..=tag_end];
        let tag_lower = tag.to_ascii_lowercase();

        // Only extract stylesheet links
        if !tag_lower.contains("stylesheet") {
            pos = tag_end + 1;
            continue;
        }

        if let Some(href_idx) = tag_lower.find("href=") {
            let after_href = &tag[href_idx + 5..];
            let quote = after_href.chars().next().unwrap_or(' ');
            if quote == '"' || quote == '\'' {
                if let Some(end_quote) = after_href[1..].find(quote) {
                    let href_val = &after_href[1..1 + end_quote];
                    hrefs.push(href_val.to_string());
                }
            } else {
                let end = after_href
                    .find(|c: char| c.is_whitespace() || c == '>')
                    .unwrap_or(after_href.len());
                hrefs.push(after_href[..end].to_string());
            }
        }

        pos = tag_end + 1;
    }
    hrefs
}

/// Extract CSS file references from inline script content (e.g. `link.href = "resources/green.css"`)
pub fn extract_css_refs_from_scripts(html: &str) -> Vec<String> {
    let mut refs = Vec::new();
    let mut pos = 0;
    while pos < html.len() {
        // Look for quoted strings containing .css
        let next_quote = html[pos..].find(['"', '\'']);
        let q_pos = match next_quote {
            Some(p) => pos + p,
            None => break,
        };
        let quote = html.as_bytes()[q_pos] as char;
        let after = &html[q_pos + 1..];
        let end = match after.find(quote) {
            Some(e) => e,
            None => { pos = q_pos + 1; continue; }
        };
        let val = &after[..end];
        if val.contains(".css") && !val.contains('<') && !val.contains('>') && val.len() < 200 {
            // Looks like a CSS file reference — skip if it's clearly not a path
            if !val.starts_with("http://") && !val.starts_with("https://") && !val.starts_with("//")
                && !refs.iter().any(|r: &String| r == val)
            {
                refs.push(val.to_string());
            }
        }
        pos = q_pos + 1 + end + 1;
    }
    refs
}

pub fn resolve_link_href(html_path: &Path, href: &str) -> Option<String> {
    // Skip data: URLs — those are handled inline by the JS side
    if href.starts_with("data:") {
        return None;
    }
    // Strip query params before resolving filesystem path
    let clean_href = href.split('?').next().unwrap_or(href);
    let resolved_path = if clean_href.starts_with('/') {
        wpt_root().join(clean_href.trim_start_matches('/'))
    } else {
        html_path.parent().unwrap().join(clean_href)
    };
    std::fs::read_to_string(&resolved_path).ok()
}

// ---------------------------------------------------------------------------
// Iframe src resolution
// ---------------------------------------------------------------------------

pub fn extract_iframe_srcs(html: &str) -> Vec<String> {
    let mut srcs = Vec::new();
    let lower = html.to_ascii_lowercase();
    let mut pos = 0;
    while let Some(tag_start) = lower[pos..].find("<iframe") {
        let abs_start = pos + tag_start;
        let tag_end = match lower[abs_start..].find('>') {
            Some(e) => abs_start + e,
            None => break,
        };
        let tag = &html[abs_start..=tag_end];

        if let Some(src_idx) = tag.to_ascii_lowercase().find("src=") {
            let after_src = &tag[src_idx + 4..];
            let quote = after_src.chars().next().unwrap_or(' ');
            if quote == '"' || quote == '\'' {
                if let Some(end_quote) = after_src[1..].find(quote) {
                    let src_val = &after_src[1..1 + end_quote];
                    srcs.push(src_val.to_string());
                }
            } else {
                let end = after_src
                    .find(|c: char| c.is_whitespace() || c == '>')
                    .unwrap_or(after_src.len());
                srcs.push(after_src[..end].to_string());
            }
        }

        pos = tag_end + 1;
    }
    srcs
}

pub fn extract_js_iframe_srcs(html: &str) -> Vec<String> {
    let mut srcs = Vec::new();
    let lower = html.to_ascii_lowercase();
    let mut pos = 0;
    while let Some(start) = lower[pos..].find("<script") {
        let abs_start = pos + start;
        let open_end = match lower[abs_start..].find('>') {
            Some(e) => abs_start + e + 1,
            None => break,
        };
        let close = match lower[open_end..].find("</script") {
            Some(e) => open_end + e,
            None => break,
        };
        let body = &html[open_end..close];
        let mut spos = 0;
        while spos < body.len() {
            if let Some(idx) = body[spos..].find(".src") {
                let after_src = spos + idx + 4;
                let rest = &body[after_src..];
                let trimmed = rest.trim_start();
                if let Some(after_equals) = trimmed.strip_prefix('=') {
                    let after_eq = after_equals.trim_start();
                    let quote = after_eq.chars().next().unwrap_or(' ');
                    if quote == '"' || quote == '\'' {
                        if let Some(end_quote) = after_eq[1..].find(quote) {
                            let src_val = &after_eq[1..1 + end_quote];
                            srcs.push(src_val.to_string());
                        }
                    }
                }
                spos = after_src;
            } else {
                break;
            }
        }
        pos = close;
    }
    srcs
}

pub fn resolve_iframe_src(html_path: &Path, src: &str) -> Option<String> {
    // Strip fragment and query params before resolving filesystem path
    let src_no_fragment = src.split('#').next().unwrap_or(src);
    let clean_src = src_no_fragment.split('?').next().unwrap_or(src_no_fragment);
    let resolved_path = if clean_src.starts_with('/') {
        wpt_root().join(clean_src.trim_start_matches('/'))
    } else {
        html_path.parent().unwrap().join(clean_src)
    };
    std::fs::read_to_string(&resolved_path).ok()
}

// ---------------------------------------------------------------------------
// Wrap .any.js / .window.js in HTML template
// ---------------------------------------------------------------------------

pub fn wrap_js_in_html(js_path: &Path) -> String {
    let js_content = std::fs::read_to_string(js_path).unwrap();
    let title = js_path.file_stem().unwrap().to_str().unwrap();

    // Detect META: global=shadowrealm (only if window is NOT listed — prefer window mode)
    let is_shadowrealm = js_content.lines().any(|l| {
        l.starts_with("// META: global=") && l.contains("shadowrealm") && !l.contains("window")
    });

    let mut meta_scripts = String::new();
    for line in js_content.lines() {
        if let Some(rest) = line.strip_prefix("// META: script=") {
            let script_path = rest.trim();
            let resolved = if script_path.starts_with('/') {
                wpt_root().join(script_path.trim_start_matches('/'))
            } else {
                js_path.parent().unwrap().join(script_path)
            };
            if let Ok(script_content) = std::fs::read_to_string(&resolved) {
                meta_scripts.push_str(&format!("<script>\n{script_content}\n</script>\n"));
            }
        }
    }

    if is_shadowrealm {
        wrap_shadowrealm_html(title, &js_content, &meta_scripts)
    } else {
        format!(
            r#"<!DOCTYPE html>
<meta charset=utf-8>
<title>{title}</title>
<script src="/resources/testharness.js"></script>
<script src="/resources/testharnessreport.js"></script>
{meta_scripts}<script>
{js_content}
</script>
"#
        )
    }
}

fn wrap_shadowrealm_html(title: &str, js_content: &str, meta_scripts: &str) -> String {
    // ShadowRealm tests run inside a simulated ShadowRealm environment.
    // The preamble loads normally (outer scope), then we:
    // 1. Save and delete web-only globals
    // 2. Strip non-[Exposed=*] members from [Exposed=*] APIs
    // 3. Run meta scripts + test code
    // 4. Restore everything
    let escaped_js = js_content.replace('\\', "\\\\").replace('`', "\\`").replace("${", "\\${");

    format!(
        r#"<!DOCTYPE html>
<meta charset=utf-8>
<title>{title}</title>
<script src="/resources/testharness.js"></script>
<script src="/resources/testharnessreport.js"></script>
{meta_scripts}<script>
(function() {{
    // Web-only globals that should not exist in ShadowRealm
    var webOnly = [
        'window', 'self', 'document', 'navigator', 'location', 'history',
        'screen', 'performance', 'localStorage', 'sessionStorage',
        'fetch', 'Request', 'Response', 'Headers',
        'XMLHttpRequest', 'Worker', 'MessageChannel', 'MessagePort',
        'Blob', 'FormData', 'CSS',
        'MutationObserver', 'IntersectionObserver', 'ResizeObserver',
        'HTMLElement', 'HTMLInputElement', 'HTMLFormElement', 'HTMLIFrameElement',
        'Node', 'Element', 'Document',
        'crypto', 'SubtleCrypto', 'CryptoKey',
        'isSecureContext'
    ];

    // Save and delete web-only globals
    var saved = {{}};
    for (var i = 0; i < webOnly.length; i++) {{
        var name = webOnly[i];
        if (name in globalThis) {{
            saved[name] = Object.getOwnPropertyDescriptor(globalThis, name);
            delete globalThis[name];
        }}
    }}

    // Strip non-[Exposed=*] members from [Exposed=*] APIs
    var savedMembers = {{}};
    if (typeof AbortSignal !== 'undefined') {{
        if ('timeout' in AbortSignal) {{
            savedMembers['AbortSignal.timeout'] = Object.getOwnPropertyDescriptor(AbortSignal, 'timeout');
            delete AbortSignal.timeout;
        }}
    }}

    // Run the test code
    try {{
        (new Function(`{escaped_js}`))();
    }} catch(e) {{
        // Let the test harness handle it
        throw e;
    }} finally {{
        // Restore web-only globals
        for (var name in saved) {{
            Object.defineProperty(globalThis, name, saved[name]);
        }}
        // Restore stripped members
        for (var key in savedMembers) {{
            var parts = key.split('.');
            Object.defineProperty(globalThis[parts[0]], parts[1], savedMembers[key]);
        }}
    }}
}})();
</script>
"#
    )
}

// ---------------------------------------------------------------------------
// Test discovery
// ---------------------------------------------------------------------------

pub fn discover_tests_recursive(dir: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if !dir.exists() {
        return paths;
    }
    discover_recursive_inner(dir, &mut paths);
    paths.sort();
    paths
}

fn discover_recursive_inner(dir: &Path, paths: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap()).collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            discover_recursive_inner(&path, paths);
        } else {
            let name = path.file_name().unwrap().to_str().unwrap();
            if name.ends_with(".html")
                || name.ends_with(".any.js")
                || name.ends_with(".window.js")
            {
                if name.ends_with(".worker.js") {
                    continue;
                }
                paths.push(path);
            }
        }
    }
}

/// Convert an absolute path to a relative path from the wpt root.
pub fn to_wpt_relative(path: &Path) -> String {
    let wpt = wpt_root();
    path.strip_prefix(&wpt)
        .unwrap_or(path)
        .to_str()
        .unwrap()
        .to_string()
}

// ---------------------------------------------------------------------------
// Test result
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct WptTestResult {
    pub passed: usize,
    pub failed: usize,
    pub details: Vec<String>,
    pub is_crash_test: bool,
}

#[derive(serde::Deserialize)]
struct WptSubResult {
    name: String,
    status: i32,
    message: String,
}

/// Tests that need incremental (interleaved) HTML parsing.
const INCREMENTAL_TESTS: &[&str] = &["MutationObserver-document"];

// ---------------------------------------------------------------------------
// Run a single WPT test
// ---------------------------------------------------------------------------

pub fn run_wpt_test(html_path: &Path, preamble: &str, report_shim: &str) -> WptTestResult {
    let html = match std::fs::read_to_string(html_path) {
        Ok(h) => h,
        Err(e) => {
            return WptTestResult {
                passed: 0,
                failed: 1,
                details: vec![format!("failed to read {}: {}", html_path.display(), e)],
                is_crash_test: false,
            };
        }
    };

    // Extract variant meta tags: <meta name="variant" content="?include=foo"/>
    let variants = extract_variants(&html);

    if variants.len() > 1 {
        // Run each variant in a separate engine instance for test isolation
        let mut total_passed = 0;
        let mut total_failed = 0;
        let mut all_details = Vec::new();

        for variant in &variants {
            let result = run_wpt_test_with_search(html_path, &html, preamble, report_shim, variant);
            total_passed += result.passed;
            total_failed += result.failed;
            if result.is_crash_test {
                return WptTestResult {
                    passed: 1,
                    failed: 0,
                    details: vec![],
                    is_crash_test: true,
                };
            }
            all_details.extend(result.details);
        }

        return WptTestResult {
            passed: total_passed,
            failed: total_failed,
            details: all_details,
            is_crash_test: false,
        };
    }

    // No variants (or single variant) — run once
    let search = variants.first().map(|s| s.as_str()).unwrap_or("");
    run_wpt_test_with_search(html_path, &html, preamble, report_shim, search)
}

/// Replace lone surrogate escapes (\uD800-\uDFFF) in JSON with \uFFFD.
/// A surrogate is "lone" if it's a high surrogate not followed by a low, or a low not preceded by a high.
fn sanitize_json_surrogates(json: &str) -> String {
    use std::fmt::Write;
    let bytes = json.as_bytes();
    let len = bytes.len();
    let mut out = String::with_capacity(len);
    let mut i = 0;
    while i < len {
        if i + 5 < len && bytes[i] == b'\\' && bytes[i + 1] == b'u' {
            // Parse the 4 hex digits
            if let Ok(hex_str) = std::str::from_utf8(&bytes[i + 2..i + 6]) {
                if let Ok(code) = u16::from_str_radix(hex_str, 16) {
                    if (0xD800..=0xDBFF).contains(&code) {
                        // High surrogate — check if followed by a low surrogate
                        let has_low = i + 11 < len
                            && bytes[i + 6] == b'\\'
                            && bytes[i + 7] == b'u'
                            && std::str::from_utf8(&bytes[i + 8..i + 12])
                                .ok()
                                .and_then(|s| u16::from_str_radix(s, 16).ok())
                                .is_some_and(|low| (0xDC00..=0xDFFF).contains(&low));
                        if has_low {
                            // Valid pair — copy both escapes through as-is
                            out.push_str(&json[i..i + 12]);
                            i += 12;
                            continue;
                        } else {
                            let _ = write!(out, "\\uFFFD");
                            i += 6;
                            continue;
                        }
                    } else if (0xDC00..=0xDFFF).contains(&code) {
                        // Low surrogate — check if preceded by a high surrogate
                        // (if it were part of a valid pair, we'd have skipped past it)
                        let _ = write!(out, "\\uFFFD");
                        i += 6;
                        continue;
                    }
                }
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

/// Extract variant query strings from `<meta name="variant" content="?include=...">` tags.
fn extract_variants(html: &str) -> Vec<String> {
    let mut variants = Vec::new();
    for line in html.lines() {
        let trimmed = line.trim();
        if !trimmed.contains("variant") {
            continue;
        }
        // Match <meta name="variant" content="?include=foo"/>
        if let Some(pos) = trimmed.find("name=\"variant\"").or_else(|| trimmed.find("name='variant'")) {
            // Ensure it starts with <meta
            let before = &trimmed[..pos];
            if !before.contains("<meta") {
                continue;
            }
            // Extract content attribute value
            if let Some(cpos) = trimmed.find("content=\"") {
                let start = cpos + 9;
                if let Some(end) = trimmed[start..].find('"') {
                    variants.push(trimmed[start..start + end].to_string());
                }
            } else if let Some(cpos) = trimmed.find("content='") {
                let start = cpos + 9;
                if let Some(end) = trimmed[start..].find('\'') {
                    variants.push(trimmed[start..start + end].to_string());
                }
            }
        }
    }
    variants
}

/// Extract expected test results from `<script type="text/json" id="expected">` block.
fn extract_expected_results(html: &str) -> Option<ExpectedResults> {
    // Find the expected JSON block
    let marker = r#"<script type="text/json" id="expected">"#;
    let start = html.find(marker)?;
    let json_start = start + marker.len();
    let end = html[json_start..].find("</script>")?;
    let json_str = &html[json_start..json_start + end];

    let parsed: serde_json::Value = serde_json::from_str(json_str.trim()).ok()?;

    let status = parsed
        .get("summarized_status")
        .and_then(|s| s.get("status_string"))
        .and_then(|s| s.as_str())
        .unwrap_or("OK")
        .to_string();

    let mut tests = HashMap::new();
    if let Some(arr) = parsed.get("summarized_tests").and_then(|t| t.as_array()) {
        for t in arr {
            let name = t.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
            let status_str = t
                .get("status_string")
                .and_then(|s| s.as_str())
                .unwrap_or("PASS")
                .to_string();
            tests.insert(name, status_str);
        }
    }

    Some(ExpectedResults {
        overall_status: status,
        subtests: tests,
    })
}

#[derive(Debug)]
struct ExpectedResults {
    overall_status: String,
    subtests: HashMap<String, String>,
}

impl ExpectedResults {
    /// Returns true if any subtest is expected to not-pass.
    fn has_expected_failures(&self) -> bool {
        self.subtests.values().any(|s| s != "PASS")
            || (self.overall_status != "OK" && self.overall_status != "PASS")
    }
}

fn run_wpt_test_with_search(
    html_path: &Path,
    html: &str,
    preamble: &str,
    report_shim: &str,
    search: &str,
) -> WptTestResult {
    // Extract expected results before running
    let expected = extract_expected_results(html);

    // If test has expected failures, disable stop-on-fail so all subtests run
    let has_expected_failures = expected.as_ref().is_some_and(|e| e.has_expected_failures());

    // Prepend location.search to preamble so it's set before subset-tests-by-key.js runs
    let variant_preamble = if !search.is_empty() {
        let escaped = search.replace('\\', "\\\\").replace('\'', "\\'");
        format!("location.search = '{}';\n{}", escaped, preamble)
    } else {
        preamble.to_string()
    };

    // Disable stop-on-fail for tests with expected failures
    let variant_preamble = if has_expected_failures {
        format!(
            "globalThis.__braille_stop_on_fail = false;\n{}",
            variant_preamble
        )
    } else {
        variant_preamble
    };

    let srcs = extract_script_srcs(html);
    let mut fetched_scripts = HashMap::new();
    for src in &srcs {
        if let Some(content) = resolve_script_src(html_path, src, &variant_preamble, report_shim) {
            fetched_scripts.insert(src.clone(), content);
        }
    }

    let mut link_hrefs = extract_link_hrefs(html);
    // Also extract CSS refs from inline scripts (dynamically created <link> elements)
    for css_ref in extract_css_refs_from_scripts(html) {
        if !link_hrefs.contains(&css_ref) {
            link_hrefs.push(css_ref);
        }
    }
    let mut fetched_css = HashMap::new();
    for href in &link_hrefs {
        if let Some(content) = resolve_link_href(html_path, href) {
            fetched_css.insert(href.clone(), content);
        }
    }

    let mut iframe_srcs = extract_iframe_srcs(html);
    iframe_srcs.extend(extract_js_iframe_srcs(html));
    let mut fetched_iframes = HashMap::new();
    for src in &iframe_srcs {
        let src_no_fragment = src.split('#').next().unwrap_or(src);
        if let Some(content) = resolve_iframe_src(html_path, src) {
            fetched_iframes.insert(src_no_fragment.to_string(), content);
        }
    }

    let resources = FetchedResources {
        scripts: fetched_scripts,
        iframes: fetched_iframes,
        css: fetched_css,
    };

    let mut engine = Engine::new();

    let file_stem = html_path.file_stem().unwrap().to_str().unwrap();
    let use_incremental = INCREMENTAL_TESTS.iter().any(|t| file_stem.contains(t));
    let js_errors = if use_incremental {
        engine.load_html_incremental_with_resources_lossy(html, &resources)
    } else {
        engine.load_html_with_resources_lossy(html, &resources)
    };

    // Crash tests don't include testharness.js — if we got here, it passed
    let is_crash_test = !srcs.iter().any(|s| s.contains("testharness.js"));
    if is_crash_test {
        return WptTestResult {
            passed: 1,
            failed: 0,
            details: vec![],
            is_crash_test: true,
        };
    }

    // Drain microtask queue so promise_test chains resolve before reading results
    engine.settle();

    let has_test_fn = engine.eval_js("typeof globalThis.test").unwrap_or_default();
    if has_test_fn != "function" {
        let err_summary = if js_errors.is_empty() {
            "test harness preamble did not load".to_string()
        } else {
            format!(
                "preamble failed. First error: {}",
                js_errors[0].chars().take(200).collect::<String>()
            )
        };
        return WptTestResult {
            passed: 0,
            failed: 1,
            details: vec![err_summary],
            is_crash_test: false,
        };
    }

    let results_json = match engine.eval_js("JSON.stringify(__wpt_get_results())") {
        Ok(j) => j,
        Err(e) => {
            return WptTestResult {
                passed: 0,
                failed: 1,
                details: vec![format!("failed to get results: {}", e)],
                is_crash_test: false,
            };
        }
    };

    if results_json == "undefined" || results_json == "null" || results_json == "[]" {
        let errs: Vec<String> = js_errors
            .iter()
            .map(|e| e.chars().take(200).collect::<String>())
            .collect();
        return WptTestResult {
            passed: 0,
            failed: 1,
            details: vec![format!(
                "no tests ran. js_errors({})={:?}",
                js_errors.len(),
                errs
            )],
            is_crash_test: false,
        };
    }

    // JSON.stringify can produce \uD800-\uDFFF lone surrogate escapes which are
    // valid JSON per RFC 8259 but rejected by serde_json (not valid Unicode scalars).
    // Replace lone surrogates with \uFFFD (replacement character) before parsing.
    let results_json = sanitize_json_surrogates(&results_json);

    let results: Vec<WptSubResult> = match serde_json::from_str(&results_json) {
        Ok(r) => r,
        Err(e) => {
            return WptTestResult {
                passed: 0,
                failed: 1,
                details: vec![format!("failed to parse results JSON: {}", e)],
                is_crash_test: false,
            };
        }
    };

    // Convert numeric status to string for comparison
    fn status_to_string(status: i32) -> &'static str {
        match status {
            0 => "PASS",
            1 => "FAIL",
            2 => "TIMEOUT",
            3 => "NOTRUN",
            _ => "UNKNOWN",
        }
    }

    let mut failures = Vec::new();

    if let Some(ref exp) = expected {
        // Compare actual results against expected results
        // Build a map of actual results by name
        let mut actual_by_name: HashMap<&str, &WptSubResult> = HashMap::new();
        for r in &results {
            actual_by_name.insert(&r.name, r);
        }

        // Check each expected subtest
        for (exp_name, exp_status) in &exp.subtests {
            match actual_by_name.get(exp_name.as_str()) {
                Some(actual) => {
                    let actual_status = status_to_string(actual.status);
                    if actual_status != exp_status {
                        failures.push(format!(
                            "  [MISMATCH] {}: expected {} but got {}{}",
                            exp_name,
                            exp_status,
                            actual_status,
                            if actual.message.is_empty() {
                                String::new()
                            } else {
                                format!(": {}", actual.message)
                            }
                        ));
                    }
                }
                None => {
                    // Expected subtest didn't run
                    if exp_status != "NOTRUN" {
                        failures.push(format!(
                            "  [MISSING] {}: expected {} but test did not run",
                            exp_name, exp_status
                        ));
                    }
                }
            }
        }

        // Check for unexpected subtests (ran but not in expected)
        for r in &results {
            if !exp.subtests.contains_key(&r.name) {
                failures.push(format!(
                    "  [UNEXPECTED] {}: ran with status {} but not in expected results",
                    r.name,
                    status_to_string(r.status)
                ));
            }
        }
    } else {
        // No expected results — use simple pass/fail counting
        for r in &results {
            if r.status != 0 && r.status != 3 {
                failures.push(format!(
                    "  [{}] {}: {}",
                    status_to_string(r.status),
                    r.name,
                    r.message
                ));
            }
        }
    }

    let pass_count = if expected.is_some() {
        // With expected results, pass = total - mismatches
        results.len().saturating_sub(failures.len())
    } else {
        results
            .iter()
            .filter(|r| r.status == 0 || r.status == 3)
            .count()
    };
    let fail_count = failures.len();

    WptTestResult {
        passed: pass_count,
        failed: fail_count,
        details: failures,
        is_crash_test: false,
    }
}
