use rquickjs::{Ctx, Function};

use crate::js::dom_bridge::with_state_mut;
use crate::js::state::{PendingWorkerMessage, PendingWorkerSpawn, PendingWorkerTerminate};

pub(super) fn register_worker(ctx: &Ctx<'_>) {
    // Native: push a worker spawn request, return a temporary JS-side worker index
    let spawn_fn = Function::new(ctx.clone(), move |url: String| -> u32 {
        with_state_mut(|st| {
            let idx = st.pending_worker_spawns.len() as u32;
            st.pending_worker_spawns.push(PendingWorkerSpawn { url });
            idx
        })
    })
    .unwrap();
    ctx.globals().set("__braille_worker_spawn", spawn_fn).unwrap();

    // Native: push a postMessage to a worker
    let post_fn = Function::new(ctx.clone(), move |worker_id: u64, data: String| {
        with_state_mut(|st| {
            st.pending_worker_messages.push(PendingWorkerMessage {
                worker_id,
                data,
            });
        });
    })
    .unwrap();
    ctx.globals().set("__braille_worker_post", post_fn).unwrap();

    // Native: push a terminate request
    let term_fn = Function::new(ctx.clone(), move |worker_id: u64| {
        with_state_mut(|st| {
            st.pending_worker_terminates
                .push(PendingWorkerTerminate { worker_id });
        });
    })
    .unwrap();
    ctx.globals()
        .set("__braille_worker_terminate", term_fn)
        .unwrap();

    // JS-side Worker class that delegates to native functions
    ctx.eval::<(), _>(
        r#"
        (function() {
            var workerRegistry = {};
            var nextTempId = 1;
            var pendingAssignments = [];

            globalThis.Worker = class Worker {
                constructor(url) {
                    this.onmessage = null;
                    this.onerror = null;
                    this._listeners = {};
                    this._terminated = false;
                    this._workerId = 0;
                    this._inline = false;
                    this._workerScope = null;

                    // Resolve relative URLs
                    var resolvedUrl = url;
                    if (typeof url === 'string') {
                        if (url.charAt(0) === '/' && url.charAt(1) !== '/') {
                            resolvedUrl = location.origin + url;
                        } else if (url.charAt(0) === '/' && url.charAt(1) === '/') {
                            resolvedUrl = location.protocol + url;
                        } else if (!/^https?:\/\//.test(url) && !/^data:/.test(url) && !/^blob:/.test(url)) {
                            resolvedUrl = location.origin + location.pathname.replace(/[^\/]*$/, '') + url;
                        }
                    }

                    // data: URLs — extract code and run inline
                    if (typeof resolvedUrl === 'string' && resolvedUrl.indexOf('data:') === 0) {
                        var commaIdx = resolvedUrl.indexOf(',');
                        if (commaIdx >= 0) {
                            var meta = resolvedUrl.substring(5, commaIdx);
                            var payload = resolvedUrl.substring(commaIdx + 1);
                            var code = meta.indexOf('base64') >= 0 ? atob(payload) : decodeURIComponent(payload);
                            this._initInline(code, resolvedUrl);
                        }
                        return;
                    }

                    // Pre-fetched scripts — run inline without host delegation
                    var scripts = globalThis.__braille_worker_scripts;
                    if (scripts) {
                        // Try resolved URL, original URL, and pathname
                        var code = scripts[resolvedUrl] || scripts[url];
                        if (!code) {
                            try { code = scripts[new URL(resolvedUrl).pathname]; } catch(e) {}
                        }
                        if (code) {
                            this._initInline(code, resolvedUrl || url);
                            return;
                        }
                    }

                    // Fall back to host delegation
                    this._tempId = nextTempId++;
                    pendingAssignments.push(this);
                    __braille_worker_spawn(resolvedUrl);
                }

                _initInline(code, scriptUrl) {
                    this._inline = true;
                    this._workerId = -1;
                    var workerSelf = this;
                    // Derive worker location from script URL
                    var workerLocation = {};
                    try {
                        var u = new URL(scriptUrl || '', (typeof location !== 'undefined' && location.href) || 'http://localhost');
                        workerLocation = { href: u.href, origin: u.origin, protocol: u.protocol, host: u.host, hostname: u.hostname, port: u.port, pathname: u.pathname, search: u.search, hash: u.hash };
                    } catch(e) {
                        workerLocation = (typeof location !== 'undefined') ? location : {};
                    }

                    var workerPostMessage = function(data) {
                        if (workerSelf._terminated) return;
                        setTimeout(function() {
                            var event = { type: 'message', data: data, origin: '', lastEventId: '', source: null, ports: [] };
                            workerSelf._dispatch('message', event);
                        }, 0);
                    };

                    // Worker-scoped setTimeout that catches errors and routes to onerror
                    var _realSetTimeout = setTimeout;
                    var workerSetTimeout = function(fn, delay) {
                        var args = [];
                        for (var i = 2; i < arguments.length; i++) args.push(arguments[i]);
                        return _realSetTimeout(function() {
                            if (workerSelf._terminated) return;
                            try {
                                if (typeof fn === 'function') fn.apply(null, args);
                                else if (typeof fn === 'string') (0, eval)(fn);
                            } catch(e) {
                                // Fire onerror on worker scope (per spec: ErrorEvent with message)
                                var msg = (e && e.message) ? e.message : String(e);
                                if (typeof workerScope.onerror === 'function') {
                                    workerScope.onerror(msg, '', 0, 0, e);
                                }
                                var ls = workerScope._listeners['error'];
                                if (ls) {
                                    var errEvt = new Event('error');
                                    errEvt.message = msg;
                                    errEvt.error = e;
                                    var s = ls.slice();
                                    for (var j = 0; j < s.length; j++) s[j](errEvt);
                                }
                                // Propagate to parent Worker object
                                var parentErr = new Event('error');
                                parentErr.message = msg;
                                parentErr.error = e;
                                workerSelf._dispatch('error', parentErr);
                            }
                        }, delay || 0);
                    };
                    var workerSetInterval = function(fn, delay) {
                        var args = [];
                        for (var i = 2; i < arguments.length; i++) args.push(arguments[i]);
                        return setInterval(function() {
                            if (workerSelf._terminated) return;
                            try {
                                if (typeof fn === 'function') fn.apply(null, args);
                            } catch(e) {
                                var msg = (e && e.message) ? e.message : String(e);
                                if (typeof workerScope.onerror === 'function') {
                                    workerScope.onerror(msg, '', 0, 0, e);
                                }
                            }
                        }, delay || 0);
                    };

                    var workerScope = {
                        postMessage: workerPostMessage,
                        self: null,
                        onmessage: null,
                        onerror: null,
                        document: undefined,
                        window: undefined,
                        globalThis: null,
                        location: workerLocation,
                        setTimeout: workerSetTimeout,
                        setInterval: workerSetInterval,
                        clearTimeout: clearTimeout,
                        clearInterval: clearInterval,
                        _listeners: {},
                        addEventListener: function(type, handler) {
                            if (!workerScope._listeners[type]) workerScope._listeners[type] = [];
                            workerScope._listeners[type].push(handler);
                        },
                        removeEventListener: function(type, handler) {
                            if (workerScope._listeners[type]) {
                                workerScope._listeners[type] = workerScope._listeners[type].filter(function(f) { return f !== handler; });
                            }
                        },
                        _dispatch: function(type, event) {
                            if (workerScope['on' + type]) workerScope['on' + type](event);
                            var ls = workerScope._listeners[type];
                            if (ls) { var s = ls.slice(); for (var i = 0; i < s.length; i++) s[i](event); }
                        }
                    };
                    workerScope.self = workerScope;
                    workerScope.globalThis = workerScope;
                    this._workerScope = workerScope;

                    // Execute worker code with(self) so importScripts globals are visible
                    function execInWorker(c) {
                        var fn = new Function('postMessage','self','addEventListener','removeEventListener','importScripts','setTimeout','setInterval',
                            'with(self){\n' + c + '\n}');
                        fn(workerPostMessage, workerScope, workerScope.addEventListener, workerScope.removeEventListener, importScriptsFn, workerSetTimeout, workerSetInterval);
                    }
                    var importScriptsFn = function() {
                        var scripts = globalThis.__braille_worker_scripts;
                        if (!scripts) return;
                        for (var ai = 0; ai < arguments.length; ai++) {
                            var surl = arguments[ai];
                            var scode = scripts[surl] || scripts[surl.replace(/^.*:\/\/[^\/]+/, '')];
                            if (scode) execInWorker(scode);
                        }
                    };

                    // Execute worker script in next microtask (like a real worker startup)
                    setTimeout(function() {
                        if (workerSelf._terminated) return;
                        try {
                            execInWorker(code);
                        } catch(e) {
                            // Fire onerror on worker scope (IDL handler)
                            var msg = (e && e.message) ? e.message : String(e);
                            if (typeof workerScope.onerror === 'function') {
                                workerScope.onerror(msg, '', 0, 0, e);
                            }
                            // Fire error event listeners on worker scope
                            var scopeErr = new Event('error');
                            scopeErr.message = msg;
                            scopeErr.error = e;
                            var ls = workerScope._listeners['error'];
                            if (ls) {
                                var s = ls.slice();
                                for (var li = 0; li < s.length; li++) s[li](scopeErr);
                            }
                            // Propagate error to parent Worker object
                            var errEvent = new Event('error');
                            errEvent.message = msg;
                            errEvent.error = e;
                            workerSelf._dispatch('error', errEvent);
                        }
                    }, 0);
                }

                postMessage(data) {
                    if (this._terminated) return;
                    if (this._inline && this._workerScope) {
                        var scope = this._workerScope;
                        var event = { type: 'message', data: data, origin: '', lastEventId: '', source: null, ports: [] };
                        setTimeout(function() { scope._dispatch('message', event); }, 0);
                        return;
                    }
                    var serialized = (typeof data === 'string') ? data : JSON.stringify(data);
                    if (this._workerId > 0) {
                        __braille_worker_post(this._workerId, serialized);
                    }
                }

                terminate() {
                    if (this._terminated) return;
                    this._terminated = true;
                    if (this._inline) return;
                    if (this._workerId > 0) {
                        __braille_worker_terminate(this._workerId);
                    }
                }

                addEventListener(type, cb) {
                    if (!this._listeners[type]) this._listeners[type] = [];
                    this._listeners[type].push(cb);
                }

                removeEventListener(type, cb) {
                    if (this._listeners[type]) {
                        this._listeners[type] = this._listeners[type].filter(function(f) { return f !== cb; });
                    }
                }

                _dispatch(type, event) {
                    if (this['on' + type]) {
                        this['on' + type](event);
                    }
                    var listeners = this._listeners[type];
                    if (listeners) {
                        for (var i = 0; i < listeners.length; i++) {
                            listeners[i](event);
                        }
                    }
                }
            };

            // Called by the engine REPL when the host assigns a real worker_id
            globalThis.__braille_assign_worker_id = function(workerId) {
                var worker = pendingAssignments.shift();
                if (worker) {
                    worker._workerId = workerId;
                    workerRegistry[workerId] = worker;
                }
            };

            // Called by the engine REPL when a worker sends a message back
            globalThis.__braille_deliver_worker_message = function(workerId, data) {
                var worker = workerRegistry[workerId];
                if (worker && !worker._terminated) {
                    var parsed = data;
                    try { parsed = JSON.parse(data); } catch(e) {}
                    var event = { type: 'message', data: parsed, origin: '', lastEventId: '', source: null, ports: [] };
                    worker._dispatch('message', event);
                }
            };

            // Called by the engine REPL when a worker encounters an error
            globalThis.__braille_deliver_worker_error = function(workerId, errorMsg) {
                var worker = workerRegistry[workerId];
                if (worker && !worker._terminated) {
                    var event = new Event('error');
                    event.message = errorMsg;
                    worker._dispatch('error', event);
                }
            };
            // SharedWorker: like Worker but communicates via MessagePort
            globalThis.SharedWorker = class SharedWorker {
                constructor(url, name) {
                    this.onerror = null;
                    this._listeners = {};

                    // Create port pair
                    var outerPort = {
                        onmessage: null,
                        _listeners: {},
                        _started: false,
                        _queue: [],
                        postMessage: function(data) {
                            // Send to inner port
                            setTimeout(function() {
                                var ev = { type: 'message', data: data, origin: '', lastEventId: '', source: null, ports: [] };
                                if (innerPort.onmessage) innerPort.onmessage(ev);
                                var ls = innerPort._listeners['message'];
                                if (ls) { var s = ls.slice(); for (var i = 0; i < s.length; i++) s[i](ev); }
                            }, 0);
                        },
                        addEventListener: function(type, handler) {
                            if (!outerPort._listeners[type]) outerPort._listeners[type] = [];
                            outerPort._listeners[type].push(handler);
                            if (type === 'message') outerPort._started = true;
                        },
                        removeEventListener: function(type, handler) {
                            if (outerPort._listeners[type]) {
                                outerPort._listeners[type] = outerPort._listeners[type].filter(function(f) { return f !== handler; });
                            }
                        },
                        start: function() {
                            outerPort._started = true;
                            // Flush queued messages
                            var q = outerPort._queue.splice(0);
                            for (var i = 0; i < q.length; i++) {
                                var ev = q[i];
                                if (outerPort.onmessage) outerPort.onmessage(ev);
                                var ls = outerPort._listeners['message'];
                                if (ls) { var s = ls.slice(); for (var j = 0; j < s.length; j++) s[j](ev); }
                            }
                        },
                        close: function() {}
                    };

                    var innerPort = {
                        onmessage: null,
                        _listeners: {},
                        postMessage: function(data) {
                            var ev = { type: 'message', data: data, origin: '', lastEventId: '', source: null, ports: [] };
                            if (outerPort._started) {
                                setTimeout(function() {
                                    if (outerPort.onmessage) outerPort.onmessage(ev);
                                    var ls = outerPort._listeners['message'];
                                    if (ls) { var s = ls.slice(); for (var i = 0; i < s.length; i++) s[i](ev); }
                                }, 0);
                            } else {
                                outerPort._queue.push(ev);
                            }
                        },
                        addEventListener: function(type, handler) {
                            if (!innerPort._listeners[type]) innerPort._listeners[type] = [];
                            innerPort._listeners[type].push(handler);
                        },
                        removeEventListener: function() {},
                        start: function() {},
                        close: function() {}
                    };

                    this.port = outerPort;
                    var sharedSelf = this;

                    // Resolve URL
                    var resolvedUrl = url;
                    if (typeof url === 'string' && !/^https?:\/\//.test(url) && !/^data:/.test(url) && !/^blob:/.test(url)) {
                        if (url.charAt(0) === '/') {
                            resolvedUrl = (typeof location !== 'undefined' ? location.origin : '') + url;
                        } else {
                            resolvedUrl = (typeof location !== 'undefined' ? location.origin + location.pathname.replace(/[^\/]*$/, '') : '') + url;
                        }
                    }

                    // Find code from pre-fetched scripts
                    var code = null;
                    var scripts = globalThis.__braille_worker_scripts;
                    if (scripts) {
                        code = scripts[resolvedUrl] || scripts[url];
                        if (!code) {
                            try { code = scripts[new URL(resolvedUrl).pathname]; } catch(e) {}
                        }
                    }

                    if (code) {
                        setTimeout(function() {
                            // postMessage for SharedWorker: routes through the inner port to outer port
                            var sharedPostMessage = function(data) {
                                innerPort.postMessage(data);
                            };

                            var workerScope = {
                                self: null,
                                onconnect: null,
                                document: undefined,
                                window: undefined,
                                globalThis: null,
                                postMessage: sharedPostMessage,
                                setTimeout: setTimeout,
                                setInterval: setInterval,
                                clearTimeout: clearTimeout,
                                clearInterval: clearInterval,
                                _listeners: {},
                                addEventListener: function(type, handler) {
                                    if (!workerScope._listeners[type]) workerScope._listeners[type] = [];
                                    workerScope._listeners[type].push(handler);
                                },
                                removeEventListener: function() {},
                                close: function() {},
                                location: (typeof location !== 'undefined') ? location : {}
                            };
                            workerScope.self = workerScope;
                            workerScope.globalThis = workerScope;

                            var importScriptsFn = function() {
                                var sc = globalThis.__braille_worker_scripts;
                                if (!sc) return;
                                for (var ai = 0; ai < arguments.length; ai++) {
                                    var surl = arguments[ai];
                                    var scode = sc[surl] || sc[surl.replace(/^.*:\/\/[^\/]+/, '')];
                                    if (scode) {
                                        var fn2 = new Function('self','addEventListener','removeEventListener','importScripts','postMessage','setTimeout','setInterval',
                                            'with(self){\n' + scode + '\n}');
                                        fn2(workerScope, workerScope.addEventListener, workerScope.removeEventListener, importScriptsFn, sharedPostMessage, setTimeout, setInterval);
                                    }
                                }
                            };

                            try {
                                var fn = new Function('self','addEventListener','removeEventListener','importScripts','postMessage','setTimeout','setInterval',
                                    'with(self){\n' + code + '\n}');
                                fn(workerScope, workerScope.addEventListener, workerScope.removeEventListener, importScriptsFn, sharedPostMessage, setTimeout, setInterval);
                            } catch(e) {
                                var errEvent = new Event('error');
                                errEvent.message = (e && e.message) ? e.message : String(e);
                                errEvent.error = e;
                                if (sharedSelf.onerror) sharedSelf.onerror(errEvent);
                                return;
                            }

                            // Fire connect event with port
                            var connectEvent = { type: 'connect', ports: [innerPort], source: innerPort };
                            if (typeof workerScope.onconnect === 'function') {
                                workerScope.onconnect(connectEvent);
                            }
                            var cls = workerScope._listeners['connect'];
                            if (cls) { for (var ci = 0; ci < cls.length; ci++) cls[ci](connectEvent); }
                        }, 0);
                    }
                }

                addEventListener(type, cb) {
                    if (!this._listeners[type]) this._listeners[type] = [];
                    this._listeners[type].push(cb);
                }
                removeEventListener(type, cb) {
                    if (this._listeners[type]) {
                        this._listeners[type] = this._listeners[type].filter(function(f) { return f !== cb; });
                    }
                }
            };
        })();
    "#,
    )
    .unwrap();
}
