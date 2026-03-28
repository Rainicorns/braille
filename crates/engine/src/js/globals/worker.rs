use std::cell::RefCell;
use std::rc::Rc;

use rquickjs::{Ctx, Function};

use crate::js::state::{EngineState, PendingWorkerMessage, PendingWorkerSpawn, PendingWorkerTerminate};

pub(super) fn register_worker(ctx: &Ctx<'_>, state: Rc<RefCell<EngineState>>) {
    // Native: push a worker spawn request, return a temporary JS-side worker index
    let state_spawn = Rc::clone(&state);
    let spawn_fn = Function::new(ctx.clone(), move |url: String| -> u32 {
        let mut st = state_spawn.borrow_mut();
        let idx = st.pending_worker_spawns.len() as u32;
        st.pending_worker_spawns.push(PendingWorkerSpawn { url });
        idx
    })
    .unwrap();
    ctx.globals().set("__braille_worker_spawn", spawn_fn).unwrap();

    // Native: push a postMessage to a worker
    let state_post = Rc::clone(&state);
    let post_fn = Function::new(ctx.clone(), move |worker_id: u64, data: String| {
        let mut st = state_post.borrow_mut();
        st.pending_worker_messages.push(PendingWorkerMessage {
            worker_id,
            data,
        });
    })
    .unwrap();
    ctx.globals().set("__braille_worker_post", post_fn).unwrap();

    // Native: push a terminate request
    let state_term = Rc::clone(&state);
    let term_fn = Function::new(ctx.clone(), move |worker_id: u64| {
        let mut st = state_term.borrow_mut();
        st.pending_worker_terminates
            .push(PendingWorkerTerminate { worker_id });
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
                    this._workerId = 0;  // 0 = not yet assigned by host
                    this._inline = false;

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

                    // data: URLs can be executed inline without host delegation
                    if (typeof resolvedUrl === 'string' && resolvedUrl.indexOf('data:') === 0) {
                        this._inline = true;
                        this._workerId = -1;
                        var workerSelf = this;
                        // Extract code from data: URL (data:text/javascript,CODE or data:text/javascript;base64,CODE)
                        var commaIdx = resolvedUrl.indexOf(',');
                        if (commaIdx >= 0) {
                            var meta = resolvedUrl.substring(5, commaIdx);
                            var payload = resolvedUrl.substring(commaIdx + 1);
                            var code;
                            if (meta.indexOf('base64') >= 0) {
                                code = atob(payload);
                            } else {
                                code = decodeURIComponent(payload);
                            }
                            // Execute in next microtask with worker-like scope
                            setTimeout(function() {
                                if (workerSelf._terminated) return;
                                var workerPostMessage = function(data) {
                                    if (workerSelf._terminated) return;
                                    // Deliver asynchronously like a real worker would
                                    setTimeout(function() {
                                        var event = { type: 'message', data: data, origin: '', lastEventId: '', source: null, ports: [] };
                                        workerSelf._dispatch('message', event);
                                    }, 0);
                                };
                                // Create a worker-like scope and execute
                                var workerScope = {
                                    postMessage: workerPostMessage,
                                    self: null,
                                    onmessage: null,
                                    _pendingMessages: []
                                };
                                workerScope.self = workerScope;
                                workerSelf._workerScope = workerScope;
                                var fn = new Function('postMessage', 'self', code);
                                fn(workerPostMessage, workerScope);
                            }, 0);
                        }
                        return;
                    }

                    this._tempId = nextTempId++;
                    pendingAssignments.push(this);
                    __braille_worker_spawn(resolvedUrl);
                }

                postMessage(data) {
                    if (this._terminated) return;
                    if (this._inline && this._workerScope) {
                        // Deliver to inline worker's onmessage
                        var scope = this._workerScope;
                        if (scope.onmessage) {
                            var event = { type: 'message', data: data, origin: '', lastEventId: '', source: null, ports: [] };
                            setTimeout(function() { scope.onmessage(event); }, 0);
                        }
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
