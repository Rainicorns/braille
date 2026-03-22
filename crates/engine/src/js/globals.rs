use std::cell::RefCell;
use std::rc::Rc;

use rquickjs::prelude::Rest;
use rquickjs::{Ctx, Function, Object};

use crate::dom::tree::DomTree;

use super::state::EngineState;

/// Register all global objects and functions in the JS context.
pub fn register_all(ctx: &Ctx<'_>, tree: Rc<RefCell<DomTree>>, state: Rc<RefCell<EngineState>>) {
    register_console(ctx, Rc::clone(&state));
    register_timers(ctx, Rc::clone(&state));
    register_dom_stubs(ctx);
    register_fetch(ctx, Rc::clone(&state));
    super::crypto::register(ctx);
    super::dom_bridge::install(ctx, Rc::clone(&tree), Rc::clone(&state));
    register_css_object(ctx);
    super::intl::register_intl(ctx);
    register_intl_js(ctx);
}

fn register_console(ctx: &Ctx<'_>, state: Rc<RefCell<EngineState>>) {
    let console = Object::new(ctx.clone()).unwrap();

    let mk = |state: Rc<RefCell<EngineState>>, prefix: &'static str| {
        let state = Rc::clone(&state);
        Function::new(ctx.clone(), move |args: Rest<rquickjs::Value<'_>>| {
            let parts: Vec<String> = args.0.iter().map(|v| {
                if let Some(s) = v.as_string() {
                    s.to_string().unwrap_or_default()
                } else if v.is_null() {
                    "null".to_string()
                } else if v.is_undefined() {
                    "undefined".to_string()
                } else if let Some(b) = v.as_bool() {
                    b.to_string()
                } else if let Some(n) = v.as_int() {
                    n.to_string()
                } else if let Some(n) = v.as_float() {
                    format!("{n}")
                } else if v.is_error() {
                    // JS Error object — extract message
                    if let Some(obj) = v.as_object().cloned() {
                        let msg = obj.get::<_, String>("message").unwrap_or_default();
                        let name = obj.get::<_, String>("name").unwrap_or_else(|_| "Error".to_string());
                        format!("{name}: {msg}")
                    } else {
                        "Error".to_string()
                    }
                } else {
                    v.get::<String>().unwrap_or_else(|_| "[object]".to_string())
                }
            }).collect();
            let line = if prefix.is_empty() {
                parts.join(" ")
            } else {
                format!("[{}] {}", prefix, parts.join(" "))
            };
            state.borrow_mut().console_buffer.push(line);
        })
        .unwrap()
    };

    console.set("log", mk(Rc::clone(&state), "")).unwrap();
    console.set("info", mk(Rc::clone(&state), "info")).unwrap();
    console.set("warn", mk(Rc::clone(&state), "warn")).unwrap();
    console.set("error", mk(Rc::clone(&state), "error")).unwrap();
    console.set("debug", mk(Rc::clone(&state), "debug")).unwrap();
    // Stubs for methods that don't produce output
    let noop = Function::new(ctx.clone(), || {}).unwrap();
    console.set("trace", noop.clone()).unwrap();
    console.set("assert", noop.clone()).unwrap();
    console.set("count", noop.clone()).unwrap();
    console.set("time", noop.clone()).unwrap();
    console.set("timeEnd", noop.clone()).unwrap();
    console.set("group", noop.clone()).unwrap();
    console.set("groupEnd", noop.clone()).unwrap();
    console.set("table", noop).unwrap();

    ctx.globals().set("console", console).unwrap();
}

fn register_timers(ctx: &Ctx<'_>, state: Rc<RefCell<EngineState>>) {
    // setTimeout/setInterval: JS wrapper stores callback functions, Rust tracks timing
    {
        let state_st = Rc::clone(&state);
        let register_timer = Function::new(ctx.clone(), move |delay: f64, is_interval: bool| -> u32 {
            let mut st = state_st.borrow_mut();
            let id = st.next_timer_id;
            st.next_timer_id += 1;
            let current_time = st.timer_current_time_ms;
            st.timer_entries.insert(id, super::state::TimerEntry {
                id,
                callback_code: format!("__braille_fire_timer({id})"),
                delay_ms: delay.max(0.0) as u64,
                registered_at: current_time,
                is_interval,
            });
            id
        }).unwrap();
        ctx.globals().set("__braille_register_timer", register_timer).unwrap();

        let state_ct = Rc::clone(&state);
        let clear_timer = Function::new(ctx.clone(), move |id: u32| {
            state_ct.borrow_mut().timer_entries.remove(&id);
        }).unwrap();
        ctx.globals().set("__braille_clear_timer", clear_timer).unwrap();

        ctx.eval::<(), _>(r#"
            (function() {
                var _cbs = {};
                globalThis.setTimeout = function(cb, delay) {
                    var id = __braille_register_timer(delay || 0, false);
                    if (typeof cb === 'function') _cbs[id] = cb;
                    else _cbs[id] = function() { eval(cb); };
                    return id;
                };
                globalThis.setInterval = function(cb, delay) {
                    var id = __braille_register_timer(delay || 0, true);
                    if (typeof cb === 'function') _cbs[id] = cb;
                    else _cbs[id] = function() { eval(cb); };
                    return id;
                };
                globalThis.clearTimeout = function(id) { delete _cbs[id]; __braille_clear_timer(id); };
                globalThis.clearInterval = function(id) { delete _cbs[id]; __braille_clear_timer(id); };
                globalThis.__braille_timer_errors = [];
                globalThis.__braille_fire_timer = function(id) {
                    if (_cbs[id]) {
                        try { _cbs[id](); }
                        catch(e) { __braille_timer_errors.push('timer ' + id + ': ' + (e instanceof Error ? e.message + '\n' + (e.stack || '') : String(e))); }
                    }
                };
            })();
        "#).unwrap();
    }

    // clearTimeout / clearInterval
    {
        let state = Rc::clone(&state);
        let clear = Function::new(ctx.clone(), move |id: u32| {
            state.borrow_mut().timer_entries.remove(&id);
        }).unwrap();
        ctx.globals().set("clearTimeout", clear.clone()).unwrap();
        ctx.globals().set("clearInterval", clear).unwrap();
    }
}

fn register_fetch(ctx: &Ctx<'_>, state: Rc<RefCell<EngineState>>) {
    // fetch() queues a PendingFetch and returns a Promise
    let state_fetch = Rc::clone(&state);
    let fetch_setup = Function::new(ctx.clone(), move |url: String, method: String, headers_json: String, body: rquickjs::Value<'_>| -> u64 {
        let headers: Vec<(String, String)> = serde_json::from_str(&headers_json).unwrap_or_default();
        let body_str = if body.is_null() || body.is_undefined() {
            None
        } else {
            let s = body.as_string().map(|s| s.to_string().unwrap_or_default()).unwrap_or_default();
            if s.is_empty() { None } else { Some(s) }
        };

        let mut st = state_fetch.borrow_mut();
        let id = st.next_fetch_id;
        st.next_fetch_id += 1;

        st.pending_fetches.push(super::state::PendingFetch {
            id,
            url,
            method,
            headers,
            body: body_str,
            resolve_id: 0, // Will be set from JS side
            reject_id: 0,
        });

        id
    }).unwrap();

    ctx.globals().set("__braille_fetch_setup", fetch_setup).unwrap();

    // Install the JS-side fetch() wrapper
    ctx.eval::<(), _>(r#"
        globalThis.__braille_fetch_resolvers = {};
        globalThis.__braille_fetch_rejecters = {};
        globalThis.__braille_next_resolver_id = 1;

        globalThis.fetch = function(input, init) {
            var url = typeof input === 'string' ? input : (input && input.url ? input.url : String(input));
            // Resolve relative URLs against the page origin
            if (url.charAt(0) === '/' && url.charAt(1) !== '/') {
                url = location.origin + url;
            } else if (url.charAt(0) === '/' && url.charAt(1) === '/') {
                url = location.protocol + url;
            } else if (!/^https?:\/\//.test(url)) {
                url = location.origin + location.pathname.replace(/[^\/]*$/, '') + url;
            }
            var method = (init && init.method) ? init.method : 'GET';
            var headers = (init && init.headers) ? JSON.stringify(Object.entries(init.headers)) : '[]';
            var body = (init && init.body != null) ? String(init.body) : null;

            var id = __braille_fetch_setup(url, method, headers, body);

            return new Promise(function(resolve, reject) {
                var rid = __braille_next_resolver_id++;
                __braille_fetch_resolvers[rid] = resolve;
                __braille_fetch_rejecters[rid] = reject;
                // Store the resolver ID on the pending fetch (via global update)
                if (typeof __braille_set_fetch_resolver === 'function') {
                    __braille_set_fetch_resolver(id, rid);
                }
            });
        };

        globalThis.__braille_resolve_fetch = function(rid, responseJson) {
            var resolve = __braille_fetch_resolvers[rid];
            if (resolve) {
                delete __braille_fetch_resolvers[rid];
                delete __braille_fetch_rejecters[rid];
                var data = JSON.parse(responseJson);
                var headers = {};
                if (data.headers) {
                    for (var i = 0; i < data.headers.length; i++) {
                        headers[data.headers[i][0].toLowerCase()] = data.headers[i][1];
                    }
                }
                var response = {
                    ok: data.status >= 200 && data.status < 300,
                    status: data.status,
                    statusText: data.status_text,
                    url: data.url,
                    headers: {
                        get: function(name) { return headers[name.toLowerCase()] || null; },
                        has: function(name) { return name.toLowerCase() in headers; },
                        forEach: function(cb) { for (var k in headers) cb(headers[k], k); },
                    },
                    text: function() { return Promise.resolve(data.body); },
                    json: function() { return Promise.resolve(JSON.parse(data.body)); },
                };
                resolve(response);
            }
        };

        globalThis.__braille_reject_fetch = function(rid, error) {
            var reject = __braille_fetch_rejecters[rid];
            if (reject) {
                delete __braille_fetch_resolvers[rid];
                delete __braille_fetch_rejecters[rid];
                reject(new Error(error));
            }
        };
    "#).unwrap();

    // Link the resolver IDs back to pending fetches
    let state2 = Rc::clone(&state);
    let set_resolver = Function::new(ctx.clone(), move |fetch_id: u64, resolver_id: u32| {
        let mut st = state2.borrow_mut();
        if let Some(pf) = st.pending_fetches.iter_mut().find(|pf| pf.id == fetch_id) {
            pf.resolve_id = resolver_id;
            pf.reject_id = resolver_id;
        }
    }).unwrap();
    ctx.globals().set("__braille_set_fetch_resolver", set_resolver).unwrap();
}

fn register_dom_stubs(ctx: &Ctx<'_>) {
    // Comprehensive DOM/Web API stubs so real-world JS doesn't crash on missing globals.
    // These are JS-level stubs that provide the right shape but no real DOM integration.
    // Critical DOM operations (createElement, appendChild, etc.) are backed by native
    // Rust functions that operate on the real DomTree.
    ctx.eval::<(), _>(r#"
        globalThis.window = globalThis;
        globalThis.self = globalThis;
        globalThis.document = { nodeType: 9, nodeName: '#document', readyState: 'complete', cookie: '', title: '', defaultView: globalThis };

        // Event classes
        globalThis.Event = globalThis.Event || class Event {
            constructor(type, opts) {
                this.type = type;
                this.bubbles = (opts && opts.bubbles) || false;
                this.cancelable = (opts && opts.cancelable) || false;
                this.composed = (opts && opts.composed) || false;
                this.defaultPrevented = false;
                this.target = null;
                this.currentTarget = null;
                this.eventPhase = 0;
                this.isTrusted = false;
                this.timeStamp = 0;
                this._stopPropagation = false;
                this._stopImmediate = false;
            }
            preventDefault() { this.defaultPrevented = true; }
            stopPropagation() { this._stopPropagation = true; }
            stopImmediatePropagation() { this._stopImmediate = true; this._stopPropagation = true; }
        };
        globalThis.CustomEvent = class CustomEvent extends Event {
            constructor(type, opts) { super(type, opts); this.detail = (opts && opts.detail) || null; }
        };
        globalThis.MouseEvent = class MouseEvent extends Event {
            constructor(type, opts) {
                super(type, opts);
                this.button = (opts && opts.button) || 0;
                this.clientX = (opts && opts.clientX) || 0;
                this.clientY = (opts && opts.clientY) || 0;
            }
        };
        globalThis.KeyboardEvent = class KeyboardEvent extends Event {
            constructor(type, opts) {
                super(type, opts);
                this.key = (opts && opts.key) || '';
                this.code = (opts && opts.code) || '';
            }
        };
        globalThis.FocusEvent = class FocusEvent extends Event {
            constructor(type, opts) { super(type, opts); this.relatedTarget = (opts && opts.relatedTarget) || null; }
        };
        globalThis.InputEvent = class InputEvent extends Event {
            constructor(type, opts) { super(type, opts); this.data = (opts && opts.data) || null; this.inputType = (opts && opts.inputType) || ''; }
        };
        globalThis.UIEvent = class UIEvent extends Event {
            constructor(type, opts) { super(type, opts); this.detail = (opts && opts.detail) || 0; }
        };
        globalThis.AnimationEvent = class AnimationEvent extends Event { constructor(t,o){super(t,o);} };
        globalThis.TransitionEvent = class TransitionEvent extends Event { constructor(t,o){super(t,o);} };
        globalThis.WheelEvent = class WheelEvent extends MouseEvent { constructor(t,o){super(t,o);} };
        globalThis.CompositionEvent = class CompositionEvent extends UIEvent { constructor(t,o){super(t,o);} };
        globalThis.ErrorEvent = class ErrorEvent extends Event { constructor(t,o){super(t,o);this.message=o&&o.message||'';this.filename=o&&o.filename||'';} };

        // Navigator
        globalThis.navigator = {
            userAgent: 'Mozilla/5.0 (compatible; Braille/0.1)',
            language: 'en-US',
            languages: ['en-US'],
            platform: 'Linux',
            onLine: true,
            cookieEnabled: true,
            maxTouchPoints: 0,
            hardwareConcurrency: 1,
            clipboard: { writeText: function() { return Promise.resolve(); } },
            mediaDevices: {},
            serviceWorker: { register: function() { return Promise.resolve(); } },
            permissions: { query: function() { return Promise.resolve({state:'granted'}); } },
            sendBeacon: function() { return true; },
        };

        // Location — setting href parses the URL and updates all components
        globalThis.location = (function() {
            var loc = {
                _href: 'about:blank', protocol: 'https:', hostname: 'localhost',
                pathname: '/', search: '', hash: '', origin: 'https://localhost',
                host: 'localhost', port: '',
                assign: function(url) { loc.href = url; },
                replace: function(url) { loc.href = url; },
                reload: function() {},
                toString: function() { return loc.href; },
            };
            Object.defineProperty(loc, 'href', {
                get: function() { return loc._href; },
                set: function(v) {
                    loc._href = v;
                    // Parse URL components
                    var m = String(v).match(/^(https?:)\/\/([^/:]+)(?::(\d+))?(\/[^?#]*)?(\?[^#]*)?(#.*)?$/);
                    if (m) {
                        loc.protocol = m[1];
                        loc.hostname = m[2];
                        loc.port = m[3] || '';
                        loc.host = loc.port ? loc.hostname + ':' + loc.port : loc.hostname;
                        loc.pathname = m[4] || '/';
                        loc.search = m[5] || '';
                        loc.hash = m[6] || '';
                        loc.origin = loc.protocol + '//' + loc.host;
                    }
                },
                configurable: true, enumerable: true,
            });
            return loc;
        })();

        // History
        globalThis.history = {
            pushState: function(s,t,u) { if(u) location.href = u; },
            replaceState: function(s,t,u) { if(u) location.href = u; },
            back: function(){}, forward: function(){}, go: function(){},
            state: null, length: 1,
        };

        // Storage
        function makeStorage() {
            var data = {};
            return {
                getItem: function(k) { return k in data ? data[k] : null; },
                setItem: function(k,v) { data[k] = String(v); },
                removeItem: function(k) { delete data[k]; },
                clear: function() { data = {}; },
                key: function(i) { var keys = Object.keys(data); return i < keys.length ? keys[i] : null; },
                get length() { return Object.keys(data).length; },
            };
        }
        globalThis.localStorage = makeStorage();
        globalThis.sessionStorage = makeStorage();

        // Geometry/display stubs
        globalThis.getComputedStyle = function() { return new Proxy({}, { get: function(t,p) { return ''; } }); };
        globalThis.matchMedia = function(q) { return { matches: false, media: q, addListener: function(){}, removeListener: function(){}, addEventListener: function(){}, removeEventListener: function(){} }; };
        globalThis.requestAnimationFrame = function(cb) { return setTimeout(cb, 0); };
        globalThis.cancelAnimationFrame = function(id) { clearTimeout(id); };
        globalThis.requestIdleCallback = function(cb) { return setTimeout(cb, 0); };
        globalThis.cancelIdleCallback = function(id) { clearTimeout(id); };
        globalThis.getSelection = function() { return { rangeCount: 0, removeAllRanges: function(){}, addRange: function(){} }; };

        // MessageChannel — React 18 scheduler uses this for async rendering
        globalThis.MessageChannel = class MessageChannel {
            constructor() {
                var self = this;
                this.port1 = {
                    onmessage: null,
                    postMessage: function(msg) {
                        if (self.port2.onmessage) setTimeout(function() { self.port2.onmessage({data: msg}); }, 0);
                    },
                    close: function() {},
                    addEventListener: function() {},
                    removeEventListener: function() {},
                };
                this.port2 = {
                    onmessage: null,
                    postMessage: function(msg) {
                        if (self.port1.onmessage) setTimeout(function() { self.port1.onmessage({data: msg}); }, 0);
                    },
                    close: function() {},
                    addEventListener: function() {},
                    removeEventListener: function() {},
                };
            }
        };

        // Observer stubs
        globalThis.ResizeObserver = class { observe(){} unobserve(){} disconnect(){} };
        globalThis.IntersectionObserver = class { constructor(cb,opts){} observe(){} unobserve(){} disconnect(){} };
        globalThis.MutationObserver = class { constructor(cb){this._cb=cb;} observe(){} disconnect(){} takeRecords(){return [];} };

        // Performance
        globalThis.performance = {
            now: function() { return 0; },
            mark: function(){}, measure: function(){},
            getEntriesByType: function(){return [];}, getEntriesByName: function(){return [];},
            timing: { navigationStart: 0 },
        };

        // URL
        globalThis.URL = class URL {
            constructor(u, base) {
                if (base && !u.match(/^https?:\/\//)) {
                    if (u.startsWith('/')) { var m = base.match(/^(https?:\/\/[^\/]+)/); u = (m?m[1]:'') + u; }
                    else { u = base.replace(/[^\/]*$/, '') + u; }
                }
                this.href = u;
                var m = u.match(/^(https?):\/\/([^\/\?#]+)(\/[^?#]*)?(\?[^#]*)?(#.*)?$/);
                this.protocol = m ? m[1]+':' : '';
                this.host = m ? m[2] : '';
                this.hostname = this.host.replace(/:\d+$/, '');
                this.port = (this.host.match(/:(\d+)$/) || ['',''])[1];
                this.pathname = m ? (m[3]||'/') : '/';
                this.search = m ? (m[4]||'') : '';
                this.hash = m ? (m[5]||'') : '';
                this.origin = this.protocol + '//' + this.host;
                this.searchParams = new URLSearchParams(this.search);
            }
            toString() { return this.href; }
            toJSON() { return this.href; }
        };
        // URLSearchParams — spec-compliant including 2-arg delete(name, value)
        globalThis.URLSearchParams = class URLSearchParams {
            constructor(init) {
                this._entries = [];
                if (init) {
                    var s = String(init).replace(/^\?/,'');
                    if (s) s.split('&').forEach(function(p) {
                        var eq = p.indexOf('=');
                        if (eq < 0) this._entries.push([decodeURIComponent(p), '']);
                        else this._entries.push([decodeURIComponent(p.substring(0,eq)), decodeURIComponent(p.substring(eq+1))]);
                    }.bind(this));
                }
            }
            get(n) { var e=this._entries.find(function(e){return e[0]===n;}); return e?e[1]:null; }
            getAll(n) { return this._entries.filter(function(e){return e[0]===n;}).map(function(e){return e[1];}); }
            has(n,v) { return arguments.length > 1 ? this._entries.some(function(e){return e[0]===n && e[1]===v;}) : this._entries.some(function(e){return e[0]===n;}); }
            set(n,v) { var found=false; this._entries=this._entries.filter(function(e){if(e[0]===n){if(!found){e[1]=String(v);found=true;return true;}return false;}return true;}); if(!found) this._entries.push([n,String(v)]); }
            append(n,v) { this._entries.push([n,String(v)]); }
            delete(n,v) { if (arguments.length > 1) { this._entries=this._entries.filter(function(e){return !(e[0]===n && e[1]===String(v));}); } else { this._entries=this._entries.filter(function(e){return e[0]!==n;}); } }
            sort() { this._entries.sort(function(a,b){return a[0]<b[0]?-1:a[0]>b[0]?1:0;}); }
            toString() { return this._entries.map(function(e){return encodeURIComponent(e[0])+'='+encodeURIComponent(e[1]);}).join('&'); }
            forEach(cb) { this._entries.forEach(function(e){cb(e[1],e[0]);}); }
            keys() { return this._entries.map(function(e){return e[0];})[Symbol.iterator](); }
            values() { return this._entries.map(function(e){return e[1];})[Symbol.iterator](); }
            entries() { return this._entries[Symbol.iterator](); }
            get size() { return this._entries.length; }
            [Symbol.iterator]() { return this.entries(); }
        };

        // Encoding
        globalThis.TextEncoder = class TextEncoder { encode(s) { return new Uint8Array(Array.from(s||'').map(function(c){return c.charCodeAt(0);})); } };
        globalThis.TextDecoder = class TextDecoder { decode(buf) { if(!buf)return''; return Array.from(new Uint8Array(buf)).map(function(b){return String.fromCharCode(b);}).join(''); } };
        globalThis.btoa = globalThis.btoa || function(s) { return s; };
        globalThis.atob = globalThis.atob || function(s) { return s; };

        // Crypto — real WebCrypto backed by ring (native functions registered in crypto.rs)
        globalThis.crypto = (function() {
            function toBytes(data) {
                if (data instanceof ArrayBuffer) return new Uint8Array(data);
                if (data instanceof Uint8Array) return data;
                if (ArrayBuffer.isView(data)) return new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
                return new Uint8Array(data);
            }
            function normalizeAlgo(a) { return typeof a === 'string' ? {name:a} : a; }
            function hashName(h) { var n = typeof h === 'string' ? h : (h && h.name) || h; return String(n).toUpperCase(); }

            var subtle = {
                digest: function(algo, data) {
                    var h = hashName(algo), d = Array.from(toBytes(data));
                    var result = __braille_crypto_digest(h, d);
                    return Promise.resolve(new Uint8Array(result).buffer);
                },
                generateKey: function(algo, extractable, usages) {
                    var a = normalizeAlgo(algo);
                    if (a.name === 'AES-GCM' || a.name === 'AES-CBC' || a.name === 'AES-CTR') {
                        var len = (a.length || 256) / 8;
                        var raw = __braille_crypto_get_random_bytes(len);
                        var key = {type:'secret', algorithm:{name:a.name,length:a.length||256}, extractable:!!extractable, usages:usages||[], _raw:raw};
                        return Promise.resolve(key);
                    }
                    if (a.name === 'HMAC') {
                        var hLen = {SHA1:20,'SHA-1':20,'SHA-256':32,'SHA-384':48,'SHA-512':64}[hashName(a.hash)] || 32;
                        var raw = __braille_crypto_get_random_bytes(a.length ? a.length/8 : hLen);
                        var key = {type:'secret', algorithm:{name:'HMAC',hash:{name:hashName(a.hash)},length:raw.length*8}, extractable:!!extractable, usages:usages||[], _raw:raw};
                        return Promise.resolve(key);
                    }
                    return Promise.reject(new Error('NotSupportedError: generateKey for ' + a.name));
                },
                importKey: function(format, keyData, algo, extractable, usages) {
                    var a = normalizeAlgo(algo);
                    if (format === 'raw') {
                        var raw = Array.from(toBytes(keyData));
                        var key = {type:'secret', algorithm:a, extractable:!!extractable, usages:usages||[], _raw:raw};
                        if (a.name === 'HMAC' && a.hash) key.algorithm = {name:'HMAC',hash:{name:hashName(a.hash)},length:raw.length*8};
                        if (a.name === 'PBKDF2') key.algorithm = {name:'PBKDF2'};
                        return Promise.resolve(key);
                    }
                    if (format === 'jwk') {
                        var jwk = typeof keyData === 'string' ? JSON.parse(keyData) : keyData;
                        if (jwk.k) {
                            var b64 = jwk.k.replace(/-/g,'+').replace(/_/g,'/');
                            while (b64.length % 4) b64 += '=';
                            var raw = Array.from(Uint8Array.fromBase64 ? Uint8Array.fromBase64(b64) : (function(s){
                                var bin = atob(s), arr = new Uint8Array(bin.length);
                                for(var i=0;i<bin.length;i++) arr[i]=bin.charCodeAt(i);
                                return arr;
                            })(b64));
                            var key = {type:'secret', algorithm:a, extractable:!!extractable, usages:usages||[], _raw:raw};
                            return Promise.resolve(key);
                        }
                    }
                    return Promise.reject(new Error('NotSupportedError: importKey format ' + format));
                },
                exportKey: function(format, key) {
                    if (format === 'raw' && key._raw) return Promise.resolve(new Uint8Array(key._raw).buffer);
                    if (format === 'jwk' && key._raw) {
                        var b64url = (function(bytes){
                            var bin=''; for(var i=0;i<bytes.length;i++) bin+=String.fromCharCode(bytes[i]);
                            return btoa(bin).replace(/\+/g,'-').replace(/\//g,'_').replace(/=+$/,'');
                        })(key._raw);
                        return Promise.resolve({kty:'oct',k:b64url,alg:key.algorithm.name==='HMAC'?'HS256':'A256GCM',ext:key.extractable});
                    }
                    return Promise.reject(new Error('NotSupportedError: exportKey format ' + format));
                },
                encrypt: function(algo, key, data) {
                    var a = normalizeAlgo(algo);
                    if (a.name === 'AES-GCM') {
                        var iv = Array.from(toBytes(a.iv));
                        var aad = a.additionalData ? Array.from(toBytes(a.additionalData)) : [];
                        var pt = Array.from(toBytes(data));
                        var result = __braille_crypto_aes_gcm_encrypt(key._raw, iv, pt, aad);
                        return Promise.resolve(new Uint8Array(result).buffer);
                    }
                    return Promise.reject(new Error('NotSupportedError: encrypt ' + a.name));
                },
                decrypt: function(algo, key, data) {
                    var a = normalizeAlgo(algo);
                    if (a.name === 'AES-GCM') {
                        var iv = Array.from(toBytes(a.iv));
                        var aad = a.additionalData ? Array.from(toBytes(a.additionalData)) : [];
                        var ct = Array.from(toBytes(data));
                        var result = __braille_crypto_aes_gcm_decrypt(key._raw, iv, ct, aad);
                        return Promise.resolve(new Uint8Array(result).buffer);
                    }
                    return Promise.reject(new Error('NotSupportedError: decrypt ' + a.name));
                },
                sign: function(algo, key, data) {
                    var a = normalizeAlgo(algo);
                    if (a.name === 'HMAC') {
                        var h = hashName(key.algorithm && key.algorithm.hash);
                        var result = __braille_crypto_hmac_sign(h, key._raw, Array.from(toBytes(data)));
                        return Promise.resolve(new Uint8Array(result).buffer);
                    }
                    return Promise.reject(new Error('NotSupportedError: sign ' + a.name));
                },
                verify: function(algo, key, signature, data) {
                    var a = normalizeAlgo(algo);
                    if (a.name === 'HMAC') {
                        var h = hashName(key.algorithm && key.algorithm.hash);
                        var ok = __braille_crypto_hmac_verify(h, key._raw, Array.from(toBytes(signature)), Array.from(toBytes(data)));
                        return Promise.resolve(ok);
                    }
                    return Promise.reject(new Error('NotSupportedError: verify ' + a.name));
                },
                deriveBits: function(algo, baseKey, length) {
                    var a = normalizeAlgo(algo);
                    if (a.name === 'PBKDF2') {
                        var h = hashName(a.hash);
                        var salt = Array.from(toBytes(a.salt));
                        var result = __braille_crypto_pbkdf2(h, baseKey._raw, salt, a.iterations, length/8);
                        return Promise.resolve(new Uint8Array(result).buffer);
                    }
                    return Promise.reject(new Error('NotSupportedError: deriveBits ' + a.name));
                },
                deriveKey: function(algo, baseKey, derivedKeyAlgo, extractable, usages) {
                    var a = normalizeAlgo(algo);
                    var dka = normalizeAlgo(derivedKeyAlgo);
                    var bitLen = dka.length || 256;
                    return subtle.deriveBits(a, baseKey, bitLen).then(function(bits) {
                        return subtle.importKey('raw', bits, dka, extractable, usages);
                    });
                },
            };

            return {
                subtle: subtle,
                getRandomValues: function(arr) {
                    var bytes = __braille_crypto_get_random_bytes(arr.length);
                    for (var i = 0; i < arr.length; i++) arr[i] = bytes[i];
                    return arr;
                },
                randomUUID: function() {
                    var b = __braille_crypto_get_random_bytes(16);
                    b[6] = (b[6] & 0x0f) | 0x40;
                    b[8] = (b[8] & 0x3f) | 0x80;
                    var h = ''; for (var i=0;i<16;i++) h += (b[i]<16?'0':'') + b[i].toString(16);
                    return h.slice(0,8)+'-'+h.slice(8,12)+'-'+h.slice(12,16)+'-'+h.slice(16,20)+'-'+h.slice(20);
                },
            };
        })();

        // Misc stubs
        globalThis.AbortController = class AbortController { constructor(){this.signal={aborted:false,reason:undefined,addEventListener:function(){},removeEventListener:function(){},onabort:null};} abort(r){this.signal.aborted=true;this.signal.reason=r;} };
        globalThis.AbortSignal = { abort: function(r){var s={aborted:true,reason:r,addEventListener:function(){},removeEventListener:function(){}};return s;}, timeout: function(){return{aborted:false,addEventListener:function(){},removeEventListener:function(){}};} };
        globalThis.XMLHttpRequest = class XMLHttpRequest { open(){} send(){} setRequestHeader(){} addEventListener(){} removeEventListener(){} };
        globalThis.DOMParser = class DOMParser { parseFromString(s,t) { return document; } };
        globalThis.HTMLElement = class HTMLElement {};
        globalThis.HTMLIFrameElement = class HTMLIFrameElement extends HTMLElement {};
        globalThis.HTMLInputElement = class HTMLInputElement extends HTMLElement {};
        globalThis.HTMLTextAreaElement = class HTMLTextAreaElement extends HTMLElement {};
        globalThis.HTMLSelectElement = class HTMLSelectElement extends HTMLElement {};
        globalThis.HTMLFormElement = class HTMLFormElement extends HTMLElement {};
        globalThis.HTMLAnchorElement = class HTMLAnchorElement extends HTMLElement {};
        globalThis.HTMLImageElement = class HTMLImageElement extends HTMLElement {};
        globalThis.HTMLButtonElement = class HTMLButtonElement extends HTMLElement {};
        globalThis.HTMLOptionElement = class HTMLOptionElement extends HTMLElement {};
        globalThis.Element = class Element {};
        globalThis.Node = class Node {};
        globalThis.DocumentFragment = class DocumentFragment {};
        globalThis.ShadowRoot = class ShadowRoot {};
        globalThis.CSSStyleSheet = class CSSStyleSheet { insertRule(){return 0;} deleteRule(){} get cssRules(){return [];} };
        globalThis.FormData = class FormData {
            constructor() { this._entries = []; }
            append(n,v) { this._entries.push([n,String(v)]); }
            get(n) { var e=this._entries.find(function(e){return e[0]===n;}); return e?e[1]:null; }
            getAll(n) { return this._entries.filter(function(e){return e[0]===n;}).map(function(e){return e[1];}); }
            has(n) { return this._entries.some(function(e){return e[0]===n;}); }
            set(n,v) { this.delete(n); this.append(n,v); }
            delete(n) { this._entries=this._entries.filter(function(e){return e[0]!==n;}); }
            entries() { return this._entries[Symbol.iterator](); }
            keys() { return this._entries.map(function(e){return e[0];})[Symbol.iterator](); }
            values() { return this._entries.map(function(e){return e[1];})[Symbol.iterator](); }
            forEach(cb) { this._entries.forEach(function(e){cb(e[1],e[0]);}); }
            [Symbol.iterator]() { return this.entries(); }
        };
        globalThis.queueMicrotask = function(cb) { Promise.resolve().then(cb); };
        globalThis.structuredClone = globalThis.structuredClone || function(v) { return JSON.parse(JSON.stringify(v)); };
        globalThis.WeakRef = globalThis.WeakRef || class WeakRef { constructor(t){this._t=t;} deref(){return this._t;} };
        globalThis.FinalizationRegistry = globalThis.FinalizationRegistry || class FinalizationRegistry { register(){} };

        // Analytics stubs
        globalThis.dataLayer = [];
        globalThis.ga = function(){};
        globalThis.gtag = function(){};
    "#).unwrap();
}

// register_dom_stubs already includes document stub; dom_bridge::install overrides with real bindings.
// This function is kept for reference but unused.
#[allow(dead_code)]
fn _register_document_stub(ctx: &Ctx<'_>) {
    ctx.eval::<(), _>(r#"
        globalThis.document = {
            createElement: function(tag) { return { nodeName: tag.toUpperCase(), nodeType: 1, tagName: tag.toUpperCase(), children: [], childNodes: [], parentNode: null, style: {}, className: '', classList: { add:function(){}, remove:function(){}, contains:function(){return false;}, toggle:function(){} }, dataset: {}, attributes: [], setAttribute: function(){}, getAttribute: function(){return null;}, removeAttribute: function(){}, hasAttribute: function(){return false;}, addEventListener: function(){}, removeEventListener: function(){}, appendChild: function(c){this.childNodes.push(c);this.children.push(c);c.parentNode=this;return c;}, removeChild: function(c){var i=this.childNodes.indexOf(c);if(i>=0)this.childNodes.splice(i,1);i=this.children.indexOf(c);if(i>=0)this.children.splice(i,1);c.parentNode=null;return c;}, insertBefore: function(n,r){var i=this.childNodes.indexOf(r);if(i>=0){this.childNodes.splice(i,0,n);this.children.splice(i,0,n);}else{this.childNodes.push(n);this.children.push(n);}n.parentNode=this;return n;}, cloneNode: function(){return document.createElement(this.tagName||'div');}, contains: function(){return false;}, querySelector: function(){return null;}, querySelectorAll: function(){return [];}, getElementsByTagName: function(){return [];}, getElementsByClassName: function(){return [];}, innerHTML: '', textContent: '', outerHTML: '', getBoundingClientRect: function(){return{top:0,left:0,width:0,height:0,right:0,bottom:0};}, dispatchEvent: function(){return true;}, ownerDocument: null, id: '', }; },
            createElementNS: function(ns, tag) { var el = document.createElement(tag); el.namespaceURI = ns; return el; },
            createTextNode: function(t) { return { nodeType: 3, textContent: t, nodeName: '#text', parentNode: null, data: t }; },
            createComment: function(t) { return { nodeType: 8, textContent: t, nodeName: '#comment', parentNode: null, data: t }; },
            createDocumentFragment: function() { return { nodeType: 11, childNodes: [], children: [], appendChild: function(c){this.childNodes.push(c);this.children.push(c);c.parentNode=this;return c;}, querySelector: function(){return null;}, querySelectorAll: function(){return [];} }; },
            createRange: function() { return { setStart:function(){}, setEnd:function(){}, commonAncestorContainer: null, collapsed: true, selectNodeContents: function(){} }; },
            createTreeWalker: function() { return { nextNode: function(){return null;}, currentNode: null }; },
            getElementById: function() { return null; },
            getElementsByTagName: function() { return []; },
            getElementsByClassName: function() { return []; },
            querySelector: function() { return null; },
            querySelectorAll: function() { return []; },
            addEventListener: function() {},
            removeEventListener: function() {},
            head: { appendChild: function(c){return c;}, children: [], querySelectorAll: function(){return [];}, style: {} },
            body: { appendChild: function(c){return c;}, children: [], classList: {add:function(){},remove:function(){},contains:function(){return false;}}, style: {}, setAttribute: function(){}, getAttribute: function(){return null;}, addEventListener: function(){}, removeEventListener: function(){} },
            documentElement: { appendChild: function(c){return c;}, style: {}, setAttribute: function(){}, getAttribute: function(){return null;}, classList: {add:function(){},remove:function(){},contains:function(){return false;}} },
            title: '',
            cookie: '',
            readyState: 'complete',
            location: location,
            defaultView: globalThis,
            implementation: { createHTMLDocument: function(t) { return document; } },
            createEvent: function(t) { return new Event(t); },
            nodeType: 9,
            nodeName: '#document',
        };
    "#).unwrap();
}

// js_value_to_string removed — console uses Rest<String> which auto-converts

fn register_css_object(ctx: &Ctx<'_>) {
    // CSS global with supports() backed by native __n_cssSupports, plus CSS.escape()
    ctx.eval::<(), _>(r#"
        globalThis.CSS = {
            supports: function(propOrCondition, value) {
                if (arguments.length >= 2) {
                    return __n_cssSupports(String(propOrCondition) + ': ' + String(value));
                }
                var cond = String(propOrCondition).trim();
                // Strip outer parens: "(display: flex)" -> "display: flex"
                if (cond.charAt(0) === '(' && cond.charAt(cond.length - 1) === ')') {
                    cond = cond.substring(1, cond.length - 1).trim();
                }
                return __n_cssSupports(cond);
            },
            escape: function(value) {
                var s = String(value);
                var result = '';
                for (var i = 0; i < s.length; i++) {
                    var ch = s.charCodeAt(i);
                    if (ch === 0) { result += '\uFFFD'; continue; }
                    if ((ch >= 1 && ch <= 31) || ch === 127) { result += '\\' + ch.toString(16) + ' '; continue; }
                    if (i === 0 && ch >= 48 && ch <= 57) { result += '\\' + ch.toString(16) + ' '; continue; }
                    if (i === 0 && ch === 45 && s.length === 1) { result += '\\-'; continue; }
                    if (ch === 45 || ch === 95 || (ch >= 48 && ch <= 57) || (ch >= 65 && ch <= 90) || (ch >= 97 && ch <= 122) || ch >= 128) {
                        result += s.charAt(i); continue;
                    }
                    result += '\\' + s.charAt(i);
                }
                return result;
            }
        };
    "#).unwrap();
}

fn register_intl_js(ctx: &Ctx<'_>) {
    // Intl object with constructors backed by native __n_intlFormatDate/__n_intlFormatNumber
    ctx.eval::<(), _>(r#"
        globalThis.Intl = {
            DateTimeFormat: function DateTimeFormat(locales, opts) {
                if (!(this instanceof DateTimeFormat)) return new DateTimeFormat(locales, opts);
                this._opts = opts || {};
            },
            NumberFormat: function NumberFormat(locales, opts) {
                if (!(this instanceof NumberFormat)) return new NumberFormat(locales, opts);
                this._opts = opts || {};
            },
            Collator: function Collator(locales, opts) {
                if (!(this instanceof Collator)) return new Collator(locales, opts);
                this._opts = opts || {};
            },
            PluralRules: function PluralRules(locales, opts) {
                if (!(this instanceof PluralRules)) return new PluralRules(locales, opts);
                this._opts = opts || {};
            },
            RelativeTimeFormat: function RelativeTimeFormat(locales, opts) {
                if (!(this instanceof RelativeTimeFormat)) return new RelativeTimeFormat(locales, opts);
                this._opts = opts || {};
            },
            getCanonicalLocales: function(locales) { return ['en-US']; },
        };

        Intl.DateTimeFormat.prototype.format = function(date) {
            var ts = (date instanceof Date) ? date.getTime() : Number(date);
            return __n_intlFormatDate(ts, JSON.stringify(this._opts));
        };
        Intl.DateTimeFormat.prototype.resolvedOptions = function() {
            var r = { locale: 'en-US', calendar: 'gregory', numberingSystem: 'latn', timeZone: 'UTC' };
            var o = this._opts;
            if (o.year) r.year = o.year;
            if (o.month) r.month = o.month;
            if (o.day) r.day = o.day;
            if (o.hour) r.hour = o.hour;
            if (o.minute) r.minute = o.minute;
            if (o.second) r.second = o.second;
            if (o.weekday) r.weekday = o.weekday;
            return r;
        };
        Intl.DateTimeFormat.supportedLocalesOf = function() { return ['en-US']; };

        Intl.NumberFormat.prototype.format = function(n) {
            return __n_intlFormatNumber(Number(n), JSON.stringify(this._opts));
        };
        Intl.NumberFormat.prototype.resolvedOptions = function() {
            return { locale: 'en-US', numberingSystem: 'latn', style: this._opts.style || 'decimal', minimumFractionDigits: 0, maximumFractionDigits: 3 };
        };
        Intl.NumberFormat.supportedLocalesOf = function() { return ['en-US']; };

        Intl.Collator.prototype.compare = function(a, b) {
            a = String(a); b = String(b);
            if (a < b) return -1;
            if (a > b) return 1;
            return 0;
        };
        Intl.Collator.prototype.resolvedOptions = function() {
            return { locale: 'en-US', usage: 'sort', sensitivity: 'variant', collation: 'default' };
        };
        Intl.Collator.supportedLocalesOf = function() { return ['en-US']; };

        Intl.PluralRules.prototype.select = function(n) {
            return n === 1 ? 'one' : 'other';
        };
        Intl.PluralRules.prototype.resolvedOptions = function() {
            return { locale: 'en-US', type: 'cardinal', pluralCategories: ['one', 'other'] };
        };
        Intl.PluralRules.supportedLocalesOf = function() { return ['en-US']; };

        Intl.RelativeTimeFormat.prototype.format = function(value, unit) {
            var v = Math.abs(value);
            var u = String(unit).replace(/s$/, '');
            var label = v === 1 ? u : u + 's';
            if (value < 0) return v + ' ' + label + ' ago';
            if (value > 0) return 'in ' + v + ' ' + label;
            return 'now';
        };
        Intl.RelativeTimeFormat.prototype.resolvedOptions = function() {
            return { locale: 'en-US', style: 'long', numeric: 'always' };
        };
        Intl.RelativeTimeFormat.supportedLocalesOf = function() { return ['en-US']; };
    "#).unwrap();
}
