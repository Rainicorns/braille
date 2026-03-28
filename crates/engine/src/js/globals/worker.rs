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
                            this._initInline(code);
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
                            this._initInline(code);
                            return;
                        }
                    }

                    // Fall back to host delegation
                    this._tempId = nextTempId++;
                    pendingAssignments.push(this);
                    __braille_worker_spawn(resolvedUrl);
                }

                _initInline(code) {
                    this._inline = true;
                    this._workerId = -1;
                    var workerSelf = this;

                    var workerPostMessage = function(data) {
                        if (workerSelf._terminated) return;
                        setTimeout(function() {
                            var event = { type: 'message', data: data, origin: '', lastEventId: '', source: null, ports: [] };
                            workerSelf._dispatch('message', event);
                        }, 0);
                    };

                    var workerScope = {
                        postMessage: workerPostMessage,
                        self: null,
                        onmessage: null,
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
                    this._workerScope = workerScope;

                    // Execute worker script in next microtask (like a real worker startup)
                    setTimeout(function() {
                        if (workerSelf._terminated) return;
                        var fn = new Function('postMessage', 'self', 'addEventListener', 'removeEventListener', 'importScripts', code);
                        fn(workerPostMessage, workerScope, workerScope.addEventListener, workerScope.removeEventListener, function(){});
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
        })();
    "#,
    )
    .unwrap();
}
