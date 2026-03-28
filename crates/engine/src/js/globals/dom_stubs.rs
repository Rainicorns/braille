use std::cell::RefCell;
use std::rc::Rc;

use rquickjs::{Ctx, Function};

use crate::js::state::EngineState;

pub(super) fn register_dom_stubs(ctx: &Ctx<'_>, state: Rc<RefCell<EngineState>>) {
    // Register __braille_navigate — called by location.href setter to signal pending navigation
    let nav_state = Rc::clone(&state);
    let navigate_fn = Function::new(ctx.clone(), move |url: String| {
        nav_state.borrow_mut().pending_navigation = Some(url);
    }).unwrap();
    ctx.globals().set("__braille_navigate", navigate_fn).unwrap();
    // Comprehensive DOM/Web API stubs so real-world JS doesn't crash on missing globals.
    // These are JS-level stubs that provide the right shape but no real DOM integration.
    // Critical DOM operations (createElement, appendChild, etc.) are backed by native
    // Rust functions that operate on the real DomTree.
    ctx.eval::<(), _>(r#"
        globalThis.window = globalThis;
        globalThis.self = globalThis;
        globalThis.isSecureContext = true;
        globalThis.document = { nodeType: 9, nodeName: '#document', readyState: 'complete', cookie: '', title: '', defaultView: globalThis };

        // Event classes
        globalThis.Event = globalThis.Event || class Event {
            constructor(type, opts) {
                if (arguments.length < 1) throw new TypeError("Failed to construct 'Event': 1 argument required, but only 0 present.");
                this.type = String(type);
                this.bubbles = (opts && opts.bubbles) || false;
                this.cancelable = (opts && opts.cancelable) || false;
                this.composed = (opts && opts.composed) || false;
                this.defaultPrevented = false;
                this.returnValue = true;
                this.target = null;
                this.currentTarget = null;
                this.srcElement = null;
                this.eventPhase = 0;
                this.isTrusted = false;
                this.timeStamp = Date.now();
                this._stopPropagation = false;
                this._stopImmediate = false;
                this._dispatching = false;
                this._initialized = true;
            }
            get cancelBubble() { return this._stopPropagation; }
            set cancelBubble(v) { if (v) this._stopPropagation = true; }
            preventDefault() { if (this.cancelable) { this.defaultPrevented = true; this.returnValue = false; } }
            stopPropagation() { this._stopPropagation = true; }
            stopImmediatePropagation() { this._stopImmediate = true; this._stopPropagation = true; }
            composedPath() { if (!this._dispatching && this.eventPhase === 0) return []; return this._path || []; }
            initEvent(type, bubbles, cancelable) {
                if (arguments.length < 1) throw new TypeError("Failed to execute 'initEvent' on 'Event': 1 argument required, but only 0 present.");
                if (this._dispatching) return;
                this._stopPropagation = false;
                this._stopImmediate = false;
                this.defaultPrevented = false;
                this.returnValue = true;
                this.isTrusted = false;
                this.target = null;
                this.srcElement = null;
                this.type = String(type);
                this.bubbles = !!bubbles;
                this.cancelable = !!cancelable;
                this._initialized = true;
            }
        };
        Event.NONE = 0;
        Event.CAPTURING_PHASE = 1;
        Event.AT_TARGET = 2;
        Event.BUBBLING_PHASE = 3;
        Event.prototype.NONE = 0;
        Event.prototype.CAPTURING_PHASE = 1;
        Event.prototype.AT_TARGET = 2;
        Event.prototype.BUBBLING_PHASE = 3;
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
        globalThis.PointerEvent = class PointerEvent extends MouseEvent {
            constructor(t,o){super(t,o);this.pointerId=(o&&o.pointerId)||0;this.width=(o&&o.width)||1;this.height=(o&&o.height)||1;this.pressure=(o&&o.pressure)||0;this.tiltX=(o&&o.tiltX)||0;this.tiltY=(o&&o.tiltY)||0;this.pointerType=(o&&o.pointerType)||'mouse';this.isPrimary=(o&&o.isPrimary)!==undefined?o.isPrimary:true;}
        };
        globalThis.TouchEvent = class TouchEvent extends UIEvent {
            constructor(t,o){super(t,o);this.touches=(o&&o.touches)||[];this.targetTouches=(o&&o.targetTouches)||[];this.changedTouches=(o&&o.changedTouches)||[];}
        };
        globalThis.Touch = class Touch {
            constructor(o){this.identifier=(o&&o.identifier)||0;this.target=(o&&o.target)||null;this.clientX=(o&&o.clientX)||0;this.clientY=(o&&o.clientY)||0;this.pageX=(o&&o.pageX)||0;this.pageY=(o&&o.pageY)||0;}
        };
        globalThis.ClipboardEvent = class ClipboardEvent extends Event {
            constructor(t,o){super(t,o);this.clipboardData=(o&&o.clipboardData)||{getData:function(){return '';},setData:function(){},types:[]};}
        };
        globalThis.DragEvent = class DragEvent extends MouseEvent {
            constructor(t,o){super(t,o);this.dataTransfer=(o&&o.dataTransfer)||{getData:function(){return '';},setData:function(){},setDragImage:function(){},dropEffect:'none',effectAllowed:'all',types:[],files:[]};}
        };
        globalThis.PopStateEvent = class PopStateEvent extends Event {
            constructor(t,o){super(t,o);this.state=(o&&o.state)||null;}
        };
        globalThis.HashChangeEvent = class HashChangeEvent extends Event {
            constructor(t,o){super(t,o);this.oldURL=(o&&o.oldURL)||'';this.newURL=(o&&o.newURL)||'';}
        };
        globalThis.PromiseRejectionEvent = class PromiseRejectionEvent extends Event {
            constructor(t,o){super(t,o);this.promise=(o&&o.promise)||null;this.reason=(o&&o.reason)||undefined;}
        };
        globalThis.StorageEvent = class StorageEvent extends Event {
            constructor(t,o){super(t,o);this.key=(o&&o.key)||null;this.oldValue=(o&&o.oldValue)||null;this.newValue=(o&&o.newValue)||null;this.url=(o&&o.url)||'';this.storageArea=(o&&o.storageArea)||null;}
        };

        // Window dimensions
        window.innerWidth = 1280;
        window.innerHeight = 800;
        window.outerWidth = 1280;
        window.outerHeight = 900;
        window.devicePixelRatio = 1;
        window.scrollX = 0;
        window.scrollY = 0;
        window.pageXOffset = 0;
        window.pageYOffset = 0;
        window.screen = { width: 1280, height: 800, availWidth: 1280, availHeight: 800, colorDepth: 24, pixelDepth: 24, orientation: { type: 'landscape-primary', angle: 0, addEventListener: function(){}, removeEventListener: function(){} } };
        window.visualViewport = { width: 1280, height: 800, offsetLeft: 0, offsetTop: 0, scale: 1, addEventListener: function(){}, removeEventListener: function(){} };

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
            vendor: 'Google Inc.',
            clipboard: (function() {
                var _buf = '';
                return {
                    writeText: function(text) { _buf = String(text); return Promise.resolve(); },
                    readText: function() { return Promise.resolve(_buf); },
                    read: function() { return Promise.resolve([]); },
                    write: function() { return Promise.resolve(); },
                };
            })(),
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
                    // Signal navigation to engine (unless suppressed by engine's own set_url)
                    if (!loc.__suppress_nav && typeof __braille_navigate === 'function') {
                        __braille_navigate(String(v));
                    }
                },
                configurable: true, enumerable: true,
            });
            return loc;
        })();

        // History — pushState/replaceState update URL components without triggering navigation
        globalThis.history = (function() {
            var stateStack = [null];
            var stateIndex = 0;
            function updateUrl(url) {
                if (!url) return;
                var u = String(url);
                // Resolve relative URLs
                if (u.charAt(0) === '/') u = location.origin + u;
                else if (!/^https?:\/\//.test(u)) u = location.origin + location.pathname.replace(/[^\/]*$/, '') + u;
                // Update location components directly (bypass the setter to avoid re-parse side effects)
                var m = u.match(/^(https?:)\/\/([^/:]+)(?::(\d+))?(\/[^?#]*)?(\?[^#]*)?(#.*)?$/);
                if (m) {
                    location._href = u;
                    location.protocol = m[1];
                    location.hostname = m[2];
                    location.port = m[3] || '';
                    location.host = location.port ? location.hostname + ':' + location.port : location.hostname;
                    location.pathname = m[4] || '/';
                    location.search = m[5] || '';
                    location.hash = m[6] || '';
                    location.origin = location.protocol + '//' + location.host;
                }
            }
            return {
                pushState: function(s, t, u) {
                    stateStack.splice(stateIndex + 1);
                    stateStack.push(s);
                    stateIndex = stateStack.length - 1;
                    this.state = s;
                    this.length = stateStack.length;
                    updateUrl(u);
                },
                replaceState: function(s, t, u) {
                    stateStack[stateIndex] = s;
                    this.state = s;
                    updateUrl(u);
                },
                back: function() {
                    if (stateIndex > 0) { stateIndex--; this.state = stateStack[stateIndex]; }
                },
                forward: function() {
                    if (stateIndex < stateStack.length - 1) { stateIndex++; this.state = stateStack[stateIndex]; }
                },
                go: function(n) {
                    var idx = stateIndex + (n || 0);
                    if (idx >= 0 && idx < stateStack.length) { stateIndex = idx; this.state = stateStack[stateIndex]; }
                },
                state: null,
                length: 1,
            };
        })();

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
        globalThis.getComputedStyle = function(el) {
            if (!el || el.__nid === undefined) return new Proxy({}, { get: function(t,p) { return ''; } });
            var nid = el.__nid;
            function toKebab(cc) {
                if (cc === 'cssFloat') return 'float';
                return cc.replace(/[A-Z]/g, function(c) { return '-' + c.toLowerCase(); });
            }
            return new Proxy({
                getPropertyValue: function(prop) { return __n_getComputedStyle(nid, prop); },
                getPropertyPriority: function() { return ''; },
            }, {
                get: function(t, p) {
                    if (p in t) return t[p];
                    if (typeof p !== 'string') return undefined;
                    if (p === 'length') return 0;
                    if (p === 'cssText') return '';
                    return __n_getComputedStyle(nid, toKebab(p));
                }
            });
        };
        globalThis.matchMedia = function(q) {
            var matches = false;
            var m;
            if ((m = q.match(/\(\s*min-width\s*:\s*(\d+)px\s*\)/))) {
                matches = 1280 >= parseInt(m[1]);
            } else if ((m = q.match(/\(\s*max-width\s*:\s*(\d+)px\s*\)/))) {
                matches = 1280 <= parseInt(m[1]);
            } else if ((m = q.match(/\(\s*min-height\s*:\s*(\d+)px\s*\)/))) {
                matches = 800 >= parseInt(m[1]);
            } else if ((m = q.match(/\(\s*max-height\s*:\s*(\d+)px\s*\)/))) {
                matches = 800 <= parseInt(m[1]);
            } else if (/prefers-color-scheme\s*:\s*dark/.test(q)) {
                matches = false;
            } else if (/prefers-color-scheme\s*:\s*light/.test(q)) {
                matches = true;
            } else if (/prefers-reduced-motion\s*:\s*reduce/.test(q)) {
                matches = false;
            }
            return {
                matches: matches, media: q,
                onchange: null,
                addListener: function(cb) { /* deprecated, never fires */ },
                removeListener: function(cb) {},
                addEventListener: function(type, cb) {},
                removeEventListener: function(type, cb) {},
                dispatchEvent: function() { return true; },
            };
        };
        globalThis.requestAnimationFrame = function(cb) { return setTimeout(cb, 16); };
        globalThis.cancelAnimationFrame = function(id) { clearTimeout(id); };
        globalThis.requestIdleCallback = function(cb) { return setTimeout(cb, 0); };
        globalThis.cancelIdleCallback = function(id) { clearTimeout(id); };
        globalThis.getSelection = function() {
            var _ranges = [];
            return {
                get rangeCount() { return _ranges.length; },
                getRangeAt: function(i) { return _ranges[i] || null; },
                addRange: function(r) { _ranges.push(r); },
                removeAllRanges: function() { _ranges = []; },
                removeRange: function(r) { var i = _ranges.indexOf(r); if (i >= 0) _ranges.splice(i, 1); },
                collapse: function(node, offset) { _ranges = []; if (typeof Range !== 'undefined') { var r = new Range(); r.setStart(node, offset || 0); r.collapse(true); _ranges.push(r); } },
                collapseToStart: function() { if (_ranges.length) { _ranges[0].collapse(true); _ranges = [_ranges[0]]; } },
                collapseToEnd: function() { if (_ranges.length) { _ranges[0].collapse(false); _ranges = [_ranges[0]]; } },
                toString: function() { return _ranges.length ? _ranges[0].toString() : ''; },
                isCollapsed: true,
                anchorNode: null, anchorOffset: 0, focusNode: null, focusOffset: 0,
                type: 'None',
            };
        };

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

        // Observer stubs with initial callback firing
        globalThis.ResizeObserver = class {
            constructor(cb) { this._cb = cb; }
            observe(target) {
                var cb = this._cb;
                if (typeof cb === 'function' && target && typeof target.getBoundingClientRect === 'function') {
                    setTimeout(function() {
                        var rect = target.getBoundingClientRect();
                        var w = rect.width, h = rect.height;
                        cb([{
                            target: target,
                            contentRect: rect,
                            borderBoxSize: [{inlineSize: w, blockSize: h}],
                            contentBoxSize: [{inlineSize: w, blockSize: h}],
                            devicePixelContentBoxSize: [{inlineSize: w, blockSize: h}],
                        }], this);
                    }.bind(this), 0);
                }
            }
            unobserve() {}
            disconnect() {}
        };
        globalThis.IntersectionObserver = class {
            constructor(cb, opts) { this._cb = cb; this._opts = opts || {}; }
            observe(target) {
                var cb = this._cb;
                if (typeof cb === 'function' && target && typeof target.getBoundingClientRect === 'function') {
                    setTimeout(function() {
                        var rect = target.getBoundingClientRect();
                        cb([{
                            target: target,
                            isIntersecting: true,
                            intersectionRatio: 1.0,
                            boundingClientRect: rect,
                            intersectionRect: rect,
                            rootBounds: {top:0,left:0,width:1280,height:800,right:1280,bottom:800,x:0,y:0},
                            time: performance.now(),
                        }], this);
                    }.bind(this), 0);
                }
            }
            unobserve() {}
            disconnect() {}
            takeRecords() { return []; }
        };
        // MutationObserver — functional implementation
        (function() {
            var observers = [];
            var pendingDeliver = false;

            function MutationRecord(type, target) {
                this.type = type; this.target = target;
                this.addedNodes = []; this.removedNodes = [];
                this.attributeName = null; this.oldValue = null;
                this.previousSibling = null; this.nextSibling = null;
            }

            function queueRecord(record) {
                for (var i = 0; i < observers.length; i++) {
                    var obs = observers[i];
                    for (var j = 0; j < obs._targets.length; j++) {
                        var entry = obs._targets[j];
                        var target = record.target;
                        // Check if this observer watches this target (or subtree ancestor)
                        var match = false;
                        if (target === entry.target) match = true;
                        else if (entry.options.subtree) {
                            var cur = target;
                            while (cur) { if (cur === entry.target) { match = true; break; } cur = cur.parentNode; }
                        }
                        if (!match) continue;
                        if (record.type === 'attributes' && !entry.options.attributes) continue;
                        if (record.type === 'attributes' && Array.isArray(entry.options.attributeFilter) && entry.options.attributeFilter.indexOf(record.attributeName) < 0) continue;
                        if (record.type === 'childList' && !entry.options.childList) continue;
                        if (record.type === 'characterData' && !entry.options.characterData) continue;
                        obs._records.push(record);
                    }
                }
                if (!pendingDeliver) {
                    pendingDeliver = true;
                    queueMicrotask(function() {
                        pendingDeliver = false;
                        for (var i = 0; i < observers.length; i++) {
                            var obs = observers[i];
                            if (obs._records.length > 0) {
                                var recs = obs._records.splice(0);
                                obs._cb(recs, obs);
                            }
                        }
                    });
                }
            }

            globalThis.MutationObserver = function(cb) {
                this._cb = cb; this._records = []; this._targets = [];
            };
            MutationObserver.prototype.observe = function(target, options) {
                options = options || {};
                if (options.attributeFilter && options.attributes === undefined) options.attributes = true;
                this._targets.push({target: target, options: options});
                if (observers.indexOf(this) < 0) observers.push(this);
            };
            MutationObserver.prototype.disconnect = function() {
                this._targets = [];
                var idx = observers.indexOf(this);
                if (idx >= 0) observers.splice(idx, 1);
            };
            MutationObserver.prototype.takeRecords = function() { return this._records.splice(0); };

            globalThis.__mo_notify = function(type, target, extra) {
                var r = new MutationRecord(type, target);
                if (extra) {
                    if (extra.addedNodes) r.addedNodes = extra.addedNodes;
                    if (extra.removedNodes) r.removedNodes = extra.removedNodes;
                    if (extra.attributeName) r.attributeName = extra.attributeName;
                    if (extra.oldValue !== undefined) r.oldValue = extra.oldValue;
                }
                queueRecord(r);
            };
        })();

        // Performance — real monotonic timer anchored to engine start
        var __perf_start = Date.now();
        globalThis.performance = {
            now: function() { return Date.now() - __perf_start; },
            timeOrigin: Date.now(),
            mark: function(){}, measure: function(){},
            getEntriesByType: function(){return [];}, getEntriesByName: function(){return [];},
            timing: { navigationStart: __perf_start },
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
                this._search = m ? (m[4]||'') : '';
                this.hash = m ? (m[5]||'') : '';
                this.origin = this.protocol + '//' + this.host;
                // searchParams is a live view — mutations sync back to the URL
                var self = this;
                this.searchParams = new URLSearchParams(this._search);
                this.searchParams._url = this;
            }
            get search() { return this._search; }
            set search(v) {
                this._search = v;
                this.searchParams = new URLSearchParams(v);
                this.searchParams._url = this;
                this._rebuildHref();
            }
            _rebuildHref() {
                var s = this.searchParams.toString();
                this._search = s ? '?' + s : '';
                this.href = this.origin + this.pathname + this._search + this.hash;
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
            _sync() { if (this._url) this._url._rebuildHref(); }
            get(n) { var e=this._entries.find(function(e){return e[0]===n;}); return e?e[1]:null; }
            getAll(n) { return this._entries.filter(function(e){return e[0]===n;}).map(function(e){return e[1];}); }
            has(n,v) { return arguments.length > 1 ? this._entries.some(function(e){return e[0]===n && e[1]===v;}) : this._entries.some(function(e){return e[0]===n;}); }
            set(n,v) { var found=false; this._entries=this._entries.filter(function(e){if(e[0]===n){if(!found){e[1]=String(v);found=true;return true;}return false;}return true;}); if(!found) this._entries.push([n,String(v)]); this._sync(); }
            append(n,v) { this._entries.push([n,String(v)]); this._sync(); }
            delete(n,v) { if (arguments.length > 1) { this._entries=this._entries.filter(function(e){return !(e[0]===n && e[1]===String(v));}); } else { this._entries=this._entries.filter(function(e){return e[0]!==n;}); } this._sync(); }
            sort() { this._entries.sort(function(a,b){return a[0]<b[0]?-1:a[0]>b[0]?1:0;}); this._sync(); }
            toString() { return this._entries.map(function(e){return encodeURIComponent(e[0])+'='+encodeURIComponent(e[1]);}).join('&'); }
            forEach(cb) { this._entries.forEach(function(e){cb(e[1],e[0]);}); }
            keys() { return this._entries.map(function(e){return e[0];})[Symbol.iterator](); }
            values() { return this._entries.map(function(e){return e[1];})[Symbol.iterator](); }
            entries() { return this._entries[Symbol.iterator](); }
            get size() { return this._entries.length; }
            [Symbol.iterator]() { return this.entries(); }
        };

        // Encoding — real UTF-8
        globalThis.TextEncoder = class TextEncoder {
            get encoding() { return 'utf-8'; }
            encode(s) {
                s = String(s || '');
                var bytes = [];
                for (var i = 0; i < s.length; i++) {
                    var cp = s.codePointAt(i);
                    if (cp < 0x80) {
                        bytes.push(cp);
                    } else if (cp < 0x800) {
                        bytes.push(0xC0 | (cp >> 6), 0x80 | (cp & 0x3F));
                    } else if (cp < 0x10000) {
                        bytes.push(0xE0 | (cp >> 12), 0x80 | ((cp >> 6) & 0x3F), 0x80 | (cp & 0x3F));
                    } else {
                        bytes.push(0xF0 | (cp >> 18), 0x80 | ((cp >> 12) & 0x3F), 0x80 | ((cp >> 6) & 0x3F), 0x80 | (cp & 0x3F));
                        i++; // skip surrogate pair second half
                    }
                }
                return new Uint8Array(bytes);
            }
            encodeInto(source, destination) {
                var encoded = this.encode(source);
                var written = Math.min(encoded.length, destination.length);
                for (var i = 0; i < written; i++) destination[i] = encoded[i];
                // Count how many source chars were consumed for 'written' bytes
                var read = 0, byteCount = 0;
                for (read = 0; read < source.length && byteCount < written; read++) {
                    var cp = source.codePointAt(read);
                    var size = cp < 0x80 ? 1 : cp < 0x800 ? 2 : cp < 0x10000 ? 3 : 4;
                    if (byteCount + size > written) break;
                    byteCount += size;
                    if (cp >= 0x10000) read++; // skip surrogate pair
                }
                return { read: read, written: byteCount };
            }
        };
        globalThis.TextDecoder = class TextDecoder {
            constructor(label) { this._label = (label || 'utf-8').toLowerCase(); }
            get encoding() { return this._label; }
            decode(buf) {
                if (!buf) return '';
                var bytes = new Uint8Array(buf instanceof ArrayBuffer ? buf : buf.buffer || buf);
                var result = '', i = 0;
                while (i < bytes.length) {
                    var b = bytes[i];
                    if (b < 0x80) { result += String.fromCodePoint(b); i++; }
                    else if ((b & 0xE0) === 0xC0) {
                        var cp = ((b & 0x1F) << 6) | (bytes[i+1] & 0x3F);
                        result += String.fromCodePoint(cp); i += 2;
                    } else if ((b & 0xF0) === 0xE0) {
                        var cp = ((b & 0x0F) << 12) | ((bytes[i+1] & 0x3F) << 6) | (bytes[i+2] & 0x3F);
                        result += String.fromCodePoint(cp); i += 3;
                    } else if ((b & 0xF8) === 0xF0) {
                        var cp = ((b & 0x07) << 18) | ((bytes[i+1] & 0x3F) << 12) | ((bytes[i+2] & 0x3F) << 6) | (bytes[i+3] & 0x3F);
                        result += String.fromCodePoint(cp); i += 4;
                    } else { result += '\uFFFD'; i++; }
                }
                return result;
            }
        };
        // Real base64 btoa/atob
        globalThis.btoa = function(s) {
            var T = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';
            var str = String(s), out = '', i = 0;
            while (i < str.length) {
                var a = str.charCodeAt(i++), b = i < str.length ? str.charCodeAt(i++) : NaN, c = i < str.length ? str.charCodeAt(i++) : NaN;
                var n = (a << 16) | ((isNaN(b) ? 0 : b) << 8) | (isNaN(c) ? 0 : c);
                out += T[(n >> 18) & 63] + T[(n >> 12) & 63] + (isNaN(b) ? '=' : T[(n >> 6) & 63]) + (isNaN(c) ? '=' : T[n & 63]);
            }
            return out;
        };
        globalThis.atob = function(s) {
            var T = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/=';
            var str = String(s).replace(/[\s]/g, ''), out = '', i = 0;
            while (i < str.length) {
                var a = T.indexOf(str.charAt(i++)), b = T.indexOf(str.charAt(i++));
                var c = T.indexOf(str.charAt(i++)), d = T.indexOf(str.charAt(i++));
                var n = (a << 18) | (b << 12) | ((c & 63) << 6) | (d & 63);
                out += String.fromCharCode((n >> 16) & 255);
                if (c !== 64) out += String.fromCharCode((n >> 8) & 255);
                if (d !== 64) out += String.fromCharCode(n & 255);
            }
            return out;
        };

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
        // AbortController / AbortSignal with real event dispatch
        globalThis.AbortSignal = (function() {
            function makeSignal() {
                return { aborted: false, reason: undefined, onabort: null, _listeners: [],
                    addEventListener: function(type, cb) { if (type === 'abort') this._listeners.push(cb); },
                    removeEventListener: function(type, cb) { if (type === 'abort') this._listeners = this._listeners.filter(function(f){return f!==cb;}); },
                    _fire: function() {
                        var ev = {type: 'abort', target: this};
                        if (this.onabort) this.onabort(ev);
                        for (var i = 0; i < this._listeners.length; i++) this._listeners[i](ev);
                    },
                    throwIfAborted: function() { if (this.aborted) throw this.reason || new Error('AbortError'); },
                };
            }
            var AS = {
                abort: function(reason) { var s = makeSignal(); s.aborted = true; s.reason = reason !== undefined ? reason : new Error('AbortError'); return s; },
                timeout: function(ms) { var s = makeSignal(); setTimeout(function() { s.aborted = true; s.reason = new Error('TimeoutError'); s._fire(); }, ms); return s; },
                any: function(signals) { var s = makeSignal(); function onAbort() { if (!s.aborted) { s.aborted = true; s.reason = this.reason; s._fire(); } } for (var i = 0; i < signals.length; i++) { if (signals[i].aborted) { s.aborted = true; s.reason = signals[i].reason; return s; } signals[i].addEventListener('abort', onAbort.bind(signals[i])); } return s; },
            };
            AS._makeSignal = makeSignal;
            return AS;
        })();
        globalThis.AbortController = class AbortController {
            constructor() { this.signal = AbortSignal._makeSignal(); }
            abort(reason) { if (!this.signal.aborted) { this.signal.aborted = true; this.signal.reason = reason !== undefined ? reason : new Error('AbortError'); this.signal._fire(); } }
        };
        // Worker class is registered by worker.rs with real delegation to the host.

        globalThis.XMLHttpRequest = (function() {
            function XMLHttpRequest() {
                this.readyState = 0;
                this.status = 0;
                this.statusText = '';
                this.responseText = '';
                this.response = '';
                this.responseURL = '';
                this.responseType = '';
                this.withCredentials = false;
                this.timeout = 0;
                this.upload = { addEventListener: function(){}, removeEventListener: function(){} };
                this.onreadystatechange = null;
                this.onload = null;
                this.onerror = null;
                this.onprogress = null;
                this.onloadend = null;
                this.onabort = null;
                this.onloadstart = null;
                this.ontimeout = null;
                this._method = 'GET';
                this._url = '';
                this._headers = {};
                this._responseHeaders = {};
                this._listeners = {};
                this._aborted = false;
            }
            XMLHttpRequest.UNSENT = 0;
            XMLHttpRequest.OPENED = 1;
            XMLHttpRequest.HEADERS_RECEIVED = 2;
            XMLHttpRequest.LOADING = 3;
            XMLHttpRequest.DONE = 4;
            XMLHttpRequest.prototype.UNSENT = 0;
            XMLHttpRequest.prototype.OPENED = 1;
            XMLHttpRequest.prototype.HEADERS_RECEIVED = 2;
            XMLHttpRequest.prototype.LOADING = 3;
            XMLHttpRequest.prototype.DONE = 4;

            XMLHttpRequest.prototype.open = function(method, url, async_) {
                this._method = method;
                this._url = url;
                this._headers = {};
                this._responseHeaders = {};
                this._aborted = false;
                this.readyState = 1;
                this.status = 0;
                this.statusText = '';
                this.responseText = '';
                this.response = '';
                this._fireReadyStateChange();
            };
            XMLHttpRequest.prototype.setRequestHeader = function(name, value) {
                this._headers[name] = value;
            };
            XMLHttpRequest.prototype.send = function(body) {
                if (this._aborted) return;
                var self = this;
                var opts = { method: self._method, headers: self._headers };
                if (body !== undefined && body !== null && self._method !== 'GET' && self._method !== 'HEAD') {
                    opts.body = body;
                }
                self.readyState = 1;

                fetch(self._url, opts).then(function(resp) {
                    if (self._aborted) return;
                    self.status = resp.status;
                    self.statusText = resp.statusText || '';
                    self.responseURL = resp.url || self._url;
                    // Store response headers
                    self._responseHeaders = {};
                    if (resp.headers && typeof resp.headers.forEach === 'function') {
                        resp.headers.forEach(function(val, key) {
                            self._responseHeaders[key.toLowerCase()] = val;
                        });
                    }
                    self.readyState = 2;
                    self._fireReadyStateChange();
                    return resp.text();
                }).then(function(text) {
                    if (self._aborted) return;
                    self.responseText = text || '';
                    self.response = self.responseType === 'json' ? JSON.parse(self.responseText) : self.responseText;
                    self.readyState = 4;
                    self._fireReadyStateChange();
                    self._fireEvent('load');
                    self._fireEvent('loadend');
                }).catch(function(err) {
                    if (self._aborted) return;
                    self.readyState = 4;
                    self.status = 0;
                    self._fireReadyStateChange();
                    self._fireEvent('error');
                    self._fireEvent('loadend');
                });
            };
            XMLHttpRequest.prototype.abort = function() {
                this._aborted = true;
                this.readyState = 0;
                this._fireEvent('abort');
            };
            XMLHttpRequest.prototype.getResponseHeader = function(name) {
                return this._responseHeaders[name.toLowerCase()] || null;
            };
            XMLHttpRequest.prototype.getAllResponseHeaders = function() {
                var result = '';
                for (var key in this._responseHeaders) {
                    result += key + ': ' + this._responseHeaders[key] + '\r\n';
                }
                return result;
            };
            XMLHttpRequest.prototype.overrideMimeType = function() {};
            XMLHttpRequest.prototype.addEventListener = function(type, cb) {
                if (!this._listeners[type]) this._listeners[type] = [];
                this._listeners[type].push(cb);
            };
            XMLHttpRequest.prototype.removeEventListener = function(type, cb) {
                if (this._listeners[type]) this._listeners[type] = this._listeners[type].filter(function(f){return f!==cb;});
            };
            XMLHttpRequest.prototype._fireReadyStateChange = function() {
                if (typeof this.onreadystatechange === 'function') {
                    this.onreadystatechange({type: 'readystatechange', target: this});
                }
                this._fireEvent('readystatechange');
            };
            XMLHttpRequest.prototype._fireEvent = function(type) {
                var evt = {type: type, target: this, loaded: this.responseText ? this.responseText.length : 0, total: 0, lengthComputable: false};
                var handler = this['on' + type];
                if (typeof handler === 'function' && type !== 'readystatechange') handler.call(this, evt);
                var cbs = this._listeners[type];
                if (cbs) { for (var i = 0; i < cbs.length; i++) cbs[i].call(this, evt); }
            };
            return XMLHttpRequest;
        })();
        globalThis.DOMParser = class DOMParser {
            parseFromString(str, type) {
                var div = document.createElement('div');
                div.innerHTML = str;
                return {
                    documentElement: div,
                    body: div,
                    head: null,
                    title: '',
                    readyState: 'complete',
                    querySelector: function(sel) { return div.querySelector(sel); },
                    querySelectorAll: function(sel) { return div.querySelectorAll(sel); },
                    getElementById: function(id) {
                        var el = div.querySelector('#' + id);
                        return el || null;
                    },
                    getElementsByTagName: function(tag) { return div.getElementsByTagName(tag); },
                    getElementsByClassName: function(cls) { return div.getElementsByClassName(cls); },
                    createDocumentFragment: function() { return document.createDocumentFragment(); },
                    createElement: function(tag) { return document.createElement(tag); },
                    createTextNode: function(text) { return document.createTextNode(text); },
                };
            }
        };
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
        // Value descriptors on HTML*Element prototypes for React's inputValueTracking.
        // React uses node.constructor.prototype to find native get/set for 'value'
        // and 'checked'. These must exist so React can set up change detection.
        var _valDesc = {
            get: function() {
                if (this.__props && this.__props._value !== undefined) return this.__props._value;
                return (this.getAttribute && this.getAttribute('value')) || '';
            },
            set: function(v) {
                if (!this.__props) this.__props = {};
                this.__props._value = String(v);
                // Also sync to attribute so Rust-side snapshot can read the current value
                if (this.__nid !== undefined) __n_setAttribute(this.__nid, 'value', String(v));
            },
            configurable: true,
        };
        Object.defineProperty(HTMLInputElement.prototype, 'value', _valDesc);
        Object.defineProperty(HTMLTextAreaElement.prototype, 'value', _valDesc);
        Object.defineProperty(HTMLSelectElement.prototype, 'value', _valDesc);
        Object.defineProperty(HTMLInputElement.prototype, 'checked', {
            get: function() {
                if (this.__props && this.__props._checked !== undefined) return this.__props._checked;
                return this.hasAttribute && this.hasAttribute('checked');
            },
            set: function(v) {
                if (!this.__props) this.__props = {};
                this.__props._checked = !!v;
            },
            configurable: true,
        });
        globalThis.DocumentFragment = class DocumentFragment {};
        globalThis.ShadowRoot = class ShadowRoot {};
        globalThis.CSSStyleSheet = class CSSStyleSheet { insertRule(){return 0;} deleteRule(){} get cssRules(){return [];} };
        // ReadableStream (minimal — single-chunk body reader)
        globalThis.ReadableStream = class ReadableStream {
            constructor(src) { this._src = src; this.locked = false; }
            getReader() {
                this.locked = true;
                var data = this._src; var done = false;
                return {
                    read: function() { if (done) return Promise.resolve({done:true,value:undefined}); done = true; return Promise.resolve({done:false,value: typeof data === 'string' ? new TextEncoder().encode(data) : data}); },
                    releaseLock: function() {},
                    cancel: function() { return Promise.resolve(); },
                };
            }
            cancel() { return Promise.resolve(); }
            pipeTo() { return Promise.resolve(); }
            pipeThrough(t) { return t.readable || this; }
            tee() { return [new ReadableStream(this._src), new ReadableStream(this._src)]; }
        };
        globalThis.FormData = class FormData {
            constructor(form) {
                this._entries = [];
                if (form && form.querySelectorAll) {
                    var controls = form.querySelectorAll('input, textarea, select');
                    for (var i = 0; i < controls.length; i++) {
                        var c = controls[i];
                        var name = c.getAttribute('name');
                        if (!name) continue;
                        var tag = c.tagName;
                        if (tag === 'INPUT') {
                            var type = (c.getAttribute('type') || 'text').toLowerCase();
                            if (type === 'checkbox' || type === 'radio') {
                                if (c.checked) this._entries.push([name, c.value || 'on']);
                            } else if (type !== 'file' && type !== 'submit' && type !== 'button' && type !== 'reset' && type !== 'image') {
                                this._entries.push([name, c.value || '']);
                            }
                        } else if (tag === 'TEXTAREA' || tag === 'SELECT') {
                            this._entries.push([name, c.value || '']);
                        }
                    }
                }
            }
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
        // Blob / File / FileReader
        globalThis.Blob = class Blob {
            constructor(parts, options) {
                this._data = '';
                if (parts) for (var i = 0; i < parts.length; i++) {
                    var p = parts[i];
                    if (p instanceof Blob) this._data += p._data;
                    else if (p instanceof ArrayBuffer) this._data += new TextDecoder().decode(p);
                    else if (ArrayBuffer.isView(p)) this._data += new TextDecoder().decode(p);
                    else this._data += String(p);
                }
                this.type = (options && options.type) || '';
                this.size = this._data.length;
            }
            text() { return Promise.resolve(this._data); }
            arrayBuffer() { return Promise.resolve(new TextEncoder().encode(this._data).buffer); }
            slice(start, end, type) {
                var s = this._data.slice(start || 0, end);
                var b = new Blob([s], {type: type || this.type});
                return b;
            }
            stream() { return { getReader: function() { var d = this._d; var done = false; return { read: function() { if (done) return Promise.resolve({done:true}); done=true; return Promise.resolve({value: new TextEncoder().encode(d), done:false}); }, cancel: function() { return Promise.resolve(); } }; }.bind({_d: this._data}) }; }
        };
        globalThis.File = class File extends Blob {
            constructor(parts, name, options) {
                super(parts, options);
                this.name = name;
                this.lastModified = (options && options.lastModified) || Date.now();
            }
        };
        globalThis.FileReader = class FileReader {
            constructor() { this.result = null; this.readyState = 0; this.error = null; this.onload = null; this.onerror = null; this.onloadend = null; }
            _finish(result) {
                var self = this;
                self.readyState = 1;
                setTimeout(function() {
                    self.result = result;
                    self.readyState = 2;
                    if (self.onload) self.onload({target: self});
                    if (self.onloadend) self.onloadend({target: self});
                }, 0);
            }
            readAsText(blob) { this._finish(blob._data); }
            readAsArrayBuffer(blob) { this._finish(new TextEncoder().encode(blob._data).buffer); }
            readAsDataURL(blob) { this._finish('data:' + (blob.type || 'application/octet-stream') + ';base64,' + btoa(blob._data)); }
            abort() { this.readyState = 2; }
        };
        // URL.createObjectURL / revokeObjectURL
        (function() {
            var blobStore = {};
            URL.createObjectURL = function(blob) { var id = 'blob:' + crypto.randomUUID(); blobStore[id] = blob; return id; };
            URL.revokeObjectURL = function(url) { delete blobStore[url]; };
        })();

        globalThis.queueMicrotask = function(cb) { Promise.resolve().then(cb); };
        // Unhandled promise rejection support
        globalThis.__braille_pending_rejections = [];
        globalThis.__braille_drain_rejections = function() {
            var arr = __braille_pending_rejections.splice(0);
            for (var i = 0; i < arr.length; i++) {
                var evt = new PromiseRejectionEvent('unhandledrejection', {
                    cancelable: true, promise: null, reason: arr[i]
                });
                window.dispatchEvent(evt);
                if (typeof window.onunhandledrejection === 'function') {
                    window.onunhandledrejection(evt);
                }
            }
        };
        globalThis.structuredClone = globalThis.structuredClone || function(v) {
            var seen = new Map();
            function clone(val) {
                if (val === null || typeof val !== 'object' && typeof val !== 'function') return val;
                if (typeof val === 'function' || typeof val === 'symbol') throw new DOMException('could not be cloned', 'DataCloneError');
                if (seen.has(val)) return seen.get(val);
                if (val instanceof Date) { var d = new Date(val.getTime()); seen.set(val, d); return d; }
                if (val instanceof RegExp) { var r = new RegExp(val.source, val.flags); seen.set(val, r); return r; }
                if (val instanceof Map) { var m = new Map(); seen.set(val, m); val.forEach(function(v, k) { m.set(clone(k), clone(v)); }); return m; }
                if (val instanceof Set) { var s = new Set(); seen.set(val, s); val.forEach(function(v) { s.add(clone(v)); }); return s; }
                if (val instanceof ArrayBuffer) { var ab = val.slice(0); seen.set(val, ab); return ab; }
                if (ArrayBuffer.isView(val)) { var buf = val.buffer.slice(0); var c = new val.constructor(buf, val.byteOffset, val.length); seen.set(val, c); return c; }
                if (val instanceof Error) { var e = new Error(val.message); e.stack = val.stack; e.name = val.name; seen.set(val, e); return e; }
                if (Array.isArray(val)) { var a = []; seen.set(val, a); for (var i = 0; i < val.length; i++) a[i] = clone(val[i]); return a; }
                var o = {}; seen.set(val, o);
                var keys = Object.keys(val);
                for (var i = 0; i < keys.length; i++) o[keys[i]] = clone(val[keys[i]]);
                return o;
            }
            return clone(v);
        };
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
