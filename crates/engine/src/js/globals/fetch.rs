use std::cell::RefCell;
use std::rc::Rc;

use rquickjs::{Ctx, Function};

use crate::js::state::EngineState;

pub(super) fn register_fetch(ctx: &Ctx<'_>, state: Rc<RefCell<EngineState>>) {
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

        st.pending_fetches.push(crate::js::state::PendingFetch {
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

    // Install Headers, Request constructors and fetch() wrapper
    ctx.eval::<(), _>(r#"
        // --- Headers constructor ---
        globalThis.Headers = class Headers {
            constructor(init) {
                this._map = {};
                if (init instanceof Headers) {
                    init.forEach(function(v, k) { this.append(k, v); }.bind(this));
                } else if (Array.isArray(init)) {
                    for (var i = 0; i < init.length; i++) this.append(init[i][0], init[i][1]);
                } else if (init && typeof init === 'object') {
                    var keys = Object.keys(init);
                    for (var i = 0; i < keys.length; i++) this.append(keys[i], init[keys[i]]);
                }
            }
            append(name, value) { var k = name.toLowerCase(); if (this._map[k]) this._map[k] += ', ' + value; else this._map[k] = String(value); }
            set(name, value) { this._map[name.toLowerCase()] = String(value); }
            get(name) { var v = this._map[name.toLowerCase()]; return v !== undefined ? v : null; }
            has(name) { return name.toLowerCase() in this._map; }
            delete(name) { delete this._map[name.toLowerCase()]; }
            forEach(cb) { for (var k in this._map) cb(this._map[k], k, this); }
            entries() { var arr = []; for (var k in this._map) arr.push([k, this._map[k]]); return arr[Symbol.iterator](); }
            keys() { return Object.keys(this._map)[Symbol.iterator](); }
            values() { var arr = []; for (var k in this._map) arr.push(this._map[k]); return arr[Symbol.iterator](); }
            [Symbol.iterator]() { return this.entries(); }
        };

        // --- Request constructor ---
        globalThis.Request = class Request {
            constructor(input, init) {
                if (input instanceof Request) {
                    this.url = input.url;
                    this.method = input.method;
                    this.headers = new Headers(input.headers);
                    this.body = input.body;
                } else {
                    this.url = String(input);
                    this.method = 'GET';
                    this.headers = new Headers();
                    this.body = null;
                }
                if (init) {
                    if (init.method) this.method = init.method;
                    if (init.headers) this.headers = new Headers(init.headers);
                    if (init.body !== undefined) this.body = init.body;
                }
            }
            clone() { return new Request(this); }
        };

        // --- fetch() ---
        globalThis.__braille_fetch_resolvers = {};
        globalThis.__braille_fetch_rejecters = {};
        globalThis.__braille_next_resolver_id = 1;

        globalThis.fetch = function(input, init) {
            var url, method, headers, body;
            if (input instanceof Request) {
                url = input.url;
                method = (init && init.method) ? init.method : input.method;
                var h = (init && init.headers) ? new Headers(init.headers) : input.headers;
                var arr = [];
                h.forEach(function(v, k) { arr.push([k, String(v)]); });
                headers = JSON.stringify(arr);
                body = (init && init.body !== undefined) ? init.body : input.body;
            } else {
                url = typeof input === 'string' ? input : String(input);
                method = (init && init.method) ? init.method : 'GET';
                headers = '[]';
                if (init && init.headers) {
                    var h = init.headers;
                    if (typeof h.forEach === 'function') {
                        var arr = [];
                        h.forEach(function(v, k) { arr.push([k, String(v)]); });
                        headers = JSON.stringify(arr);
                    } else if (typeof h === 'object') {
                        headers = JSON.stringify(Object.entries(h).map(function(e) { return [e[0], String(e[1])]; }));
                    }
                }
                body = (init && init.body != null) ? String(init.body) : null;
            }

            // Resolve relative URLs against the page origin
            if (url.charAt(0) === '/' && url.charAt(1) !== '/') {
                url = location.origin + url;
            } else if (url.charAt(0) === '/' && url.charAt(1) === '/') {
                url = location.protocol + url;
            } else if (!/^https?:\/\//.test(url)) {
                url = location.origin + location.pathname.replace(/[^\/]*$/, '') + url;
            }

            var id = __braille_fetch_setup(url, method, headers, body);

            return new Promise(function(resolve, reject) {
                var rid = __braille_next_resolver_id++;
                __braille_fetch_resolvers[rid] = resolve;
                __braille_fetch_rejecters[rid] = reject;
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
                var hdrs = new Headers(data.headers);
                var bodyStr = data.body;
                var makeResponse = function(b) {
                    return {
                        ok: data.status >= 200 && data.status < 300,
                        status: data.status,
                        statusText: data.status_text,
                        url: data.url,
                        headers: hdrs,
                        _bodyUsed: false,
                        get bodyUsed() { return this._bodyUsed; },
                        text: function() { this._bodyUsed = true; return Promise.resolve(b); },
                        json: function() { this._bodyUsed = true; return Promise.resolve(JSON.parse(b)); },
                        arrayBuffer: function() {
                            this._bodyUsed = true;
                            var enc = new TextEncoder();
                            return Promise.resolve(enc.encode(b).buffer);
                        },
                        blob: function() {
                            this._bodyUsed = true;
                            var ct = hdrs.get('content-type') || '';
                            return Promise.resolve({ size: b.length, type: ct, text: function() { return Promise.resolve(b); }, arrayBuffer: function() { return Promise.resolve(new TextEncoder().encode(b).buffer); } });
                        },
                        body: new ReadableStream(b),
                        clone: function() { return makeResponse(b); },
                        type: 'basic',
                        redirected: false,
                    };
                };
                resolve(makeResponse(bodyStr));
            }
        };

        // Response constructor and static methods
        globalThis.Response = class Response {
            constructor(body, init) {
                var opts = init || {};
                this.status = opts.status || 200;
                this.statusText = opts.statusText || '';
                this.ok = this.status >= 200 && this.status < 300;
                this.headers = new Headers(opts.headers);
                this.url = '';
                this.type = 'basic';
                this.redirected = false;
                this._body = body != null ? String(body) : '';
                this._bodyUsed = false;
                this.body = new ReadableStream(this._body);
            }
            get bodyUsed() { return this._bodyUsed; }
            text() { this._bodyUsed = true; return Promise.resolve(this._body); }
            json() { this._bodyUsed = true; return Promise.resolve(JSON.parse(this._body)); }
            arrayBuffer() { this._bodyUsed = true; return Promise.resolve(new TextEncoder().encode(this._body).buffer); }
            blob() { this._bodyUsed = true; return Promise.resolve(new Blob([this._body], {type: this.headers.get('content-type') || ''})); }
            clone() { return new Response(this._body, {status: this.status, statusText: this.statusText, headers: this.headers}); }
            static redirect(url, status) { return new Response(null, {status: status || 302, headers: {Location: url}}); }
            static json(data, init) { var opts = init || {}; opts.headers = new Headers(opts.headers); opts.headers.set('content-type','application/json'); return new Response(JSON.stringify(data), opts); }
            static error() { return new Response(null, {status: 0}); }
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
